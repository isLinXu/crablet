# coding=utf-8
"""数据集爬取流水线：基于 SmartSpider 的图片大规模采集，按每 100 张分桶存储。

架构总览
--------
                    ┌─────────────────────────────────────┐
                    │        DatasetCrawler                │
                    │  keywords × sources × total_count    │
                    └──────────────┬──────────────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                    ▼
     ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
     │  SearchSource    │  │  SiteSource      │  │  URLListSource   │
     │  (SmartSpider)  │  │  (SiteCrawler)   │  │  (直接URL列表)   │
     └────────┬────────┘  └────────┬─────────┘  └────────┬────────┘
              │                    │                      │
              └──────────┬─────────┘                      │
                         ▼                                │
              ┌──────────────────────┐                    │
              │  ImageDownloadQueue  │◄───────────────────┘
              │  (URL + meta)       │
              └──────────┬──────────┘
                         ▼
              ┌──────────────────────┐
              │  DownloadWorker       │
              │  - 下载图片            │
              │  - CLIP 过滤（可选）    │
              │  - 尺寸/方差过滤       │
              │  - 内容去重            │
              └──────────┬───────────┘
                         ▼
              ┌──────────────────────┐
              │  DatasetDirManager   │
              │  batch_0000/ (0-99)  │
              │  batch_0100/ (100-199)│
              │  batch_0200/ (200-299)│
              │  ...                 │
              └──────────────────────┘

输出目录结构
-----------
output_dir/
├── batch_0000/
│   ├── 0000_a1b2c3d4.jpg
│   ├── 0001_e5f6g7h8.png
│   ├── ...
│   └── 0099_z0y9x8w7.webp
├── batch_0100/
│   ├── 0100_m3n4o5p6.jpg
│   ├── ...
├── batch_0200/
│   └── ...
├── metadata.jsonl        # 全量元数据（每行一条）
├── _dataset_report.json  # 采集报告
└── .dataset_progress.json # 断点续传状态
"""
import hashlib
import json
import os
import signal
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from io import BytesIO
from typing import Any, Callable, Optional
from urllib.parse import urlparse

import numpy as np
from loguru import logger
from PIL import Image
from tqdm import tqdm

from .smart_http_client import SmartHttpClient
from .crawl_stats import CrawlStats
from .url_deduplicator import UrlDeduplicator

# torch / clip 延迟导入
try:
    import torch
    _TORCH_AVAILABLE = True
except ImportError:
    _TORCH_AVAILABLE = False
    torch = None

try:
    import clip
    _CLIP_AVAILABLE = True
except ImportError:
    _CLIP_AVAILABLE = False


# ──────────────────────────────────────────────────────────────────────────────
# 常量
# ──────────────────────────────────────────────────────────────────────────────

_BATCH_SIZE = 100          # 每个子目录存放的图片数量
_DEFAULT_DISK_GUARD_MB = 100

# 图片格式 magic bytes
_MAGIC_BYTES = {
    b'\xff\xd8\xff': '.jpg',
    b'\x89PNG\r\n\x1a\n': '.png',
    b'RIFF': '.webp',       # WebP 以 RIFF 开头
    b'GIF8': '.gif',
    b'\x00\x00\x00 ftyp': '.avif',
}


def _detect_ext(peek: bytes) -> str:
    """根据文件头 magic bytes 检测图片格式。"""
    for magic, ext in _MAGIC_BYTES.items():
        if peek[:len(magic)] == magic:
            # WebP 需要进一步确认
            if ext == '.webp' and len(peek) >= 12:
                if peek[8:12] != b'WEBP':
                    continue
            return ext
    # 回退：检查 JPEG
    if peek[:3] == b'\xff\xd8\xff':
        return '.jpg'
    return ''


# ──────────────────────────────────────────────────────────────────────────────
# 数据集目录管理器
# ──────────────────────────────────────────────────────────────────────────────

class DatasetDirManager:
    """管理数据集分桶目录，每 100 张图片一个子目录。

    目录命名规则：batch_0000/, batch_0100/, batch_0200/, ...
    文件命名规则：{全局序号:04d}_{url_hash[:8]}{ext}

    线程安全：所有公共方法通过内部锁保护。
    """

    def __init__(self, output_dir: str, batch_size: int = _BATCH_SIZE):
        self.output_dir = output_dir
        self.batch_size = batch_size
        self._lock = threading.Lock()
        self._saved_count = 0
        os.makedirs(output_dir, exist_ok=True)

    def get_save_path(self, url: str, ext: str) -> str:
        """获取图片保存路径，自动分配到正确的分桶目录。"""
        with self._lock:
            idx = self._saved_count
            self._saved_count += 1

        batch_dir = os.path.join(
            self.output_dir,
            f"batch_{(idx // self.batch_size) * self.batch_size:04d}",
        )
        os.makedirs(batch_dir, exist_ok=True)

        url_hash = hashlib.md5(url.encode()).hexdigest()[:8]
        filename = f"{idx:04d}_{url_hash}{ext}"
        return os.path.join(batch_dir, filename)

    def increment(self) -> int:
        """递增计数器并返回当前值（线程安全）。"""
        with self._lock:
            idx = self._saved_count
            self._saved_count += 1
            return idx

    @property
    def saved_count(self) -> int:
        with self._lock:
            return self._saved_count

    def current_batch_dir(self) -> str:
        """返回当前分桶目录路径。"""
        with self._lock:
            idx = self._saved_count
        batch_name = f"batch_{(idx // self.batch_size) * self.batch_size:04d}"
        return os.path.join(self.output_dir, batch_name)

    def list_batches(self) -> list[str]:
        """列出所有已创建的分桶目录。"""
        batches = []
        for name in sorted(os.listdir(self.output_dir)):
            if name.startswith("batch_") and os.path.isdir(
                os.path.join(self.output_dir, name)
            ):
                batches.append(name)
        return batches

    def count_existing_images(self) -> int:
        """统计已保存的图片数量（扫描磁盘）。"""
        count = 0
        for name in os.listdir(self.output_dir):
            batch_dir = os.path.join(self.output_dir, name)
            if not os.path.isdir(batch_dir) or not name.startswith("batch_"):
                continue
            for fname in os.listdir(batch_dir):
                if fname.lower().endswith((".jpg", ".jpeg", ".png", ".webp", ".gif", ".avif")):
                    count += 1
        return count


# ──────────────────────────────────────────────────────────────────────────────
# 断点续传进度管理器
# ──────────────────────────────────────────────────────────────────────────────

class ProgressManager:
    """管理数据集爬取的断点续传状态。

    状态文件：{output_dir}/.dataset_progress.json
    """

    def __init__(self, output_dir: str):
        self._path = os.path.join(output_dir, ".dataset_progress.json")
        self._lock = threading.Lock()
        self._data: dict = {}

    def load(self) -> dict:
        """加载进度文件。"""
        if os.path.exists(self._path):
            try:
                with open(self._path, "r", encoding="utf-8") as f:
                    self._data = json.load(f)
                logger.info(f"ProgressManager: loaded from {self._path}, "
                           f"saved_count={self._data.get('saved_count', 0)}")
            except Exception as e:
                logger.warning(f"ProgressManager: failed to load {self._path}: {e}")
                self._data = {}
        return self._data

    def save(self, saved_count: int, total_target: int,
             keywords_done: list[str], keywords_remaining: list[str]):
        """保存进度。"""
        from datetime import datetime
        with self._lock:
            self._data = {
                "saved_count": saved_count,
                "total_target": total_target,
                "keywords_done": keywords_done,
                "keywords_remaining": keywords_remaining,
                "last_update": datetime.now().isoformat(),
            }
        try:
            with open(self._path, "w", encoding="utf-8") as f:
                json.dump(self._data, f, ensure_ascii=False, indent=2)
        except Exception as e:
            logger.warning(f"ProgressManager: failed to save: {e}")

    @property
    def saved_count(self) -> int:
        return self._data.get("saved_count", 0)

    @property
    def keywords_done(self) -> list[str]:
        return self._data.get("keywords_done", [])


# ──────────────────────────────────────────────────────────────────────────────
# 元数据写入器
# ──────────────────────────────────────────────────────────────────────────────

class MetadataWriter:
    """线程安全的元数据追加写入器。

    输出格式：JSONL（每行一条 JSON 记录）
    字段：index, url, file_path, batch, keyword, source, sim, width, height, ext
    """

    def __init__(self, output_dir: str):
        self._path = os.path.join(output_dir, "metadata.jsonl")
        self._lock = threading.Lock()
        self._file = open(self._path, "a", encoding="utf-8", buffering=1)

    def write(self, record: dict):
        """写入一条元数据记录。"""
        with self._lock:
            try:
                self._file.write(json.dumps(record, ensure_ascii=False) + "\n")
            except Exception as e:
                logger.warning(f"MetadataWriter: write error: {e}")

    def close(self):
        with self._lock:
            try:
                self._file.close()
            except Exception:
                pass

    def __del__(self):
        self.close()


# ──────────────────────────────────────────────────────────────────────────────
# 数据集爬取器
# ──────────────────────────────────────────────────────────────────────────────

class DatasetCrawler:
    """大规模图片数据集爬取器。

    核心特性
    --------
    1. 分桶存储：每 100 张图片一个子目录（batch_0000/, batch_0100/, ...）
    2. 全局递增编号：文件名包含序号，便于排序和引用
    3. 多源爬取：搜索引擎 + spider_tools 站点 + URL 列表
    4. CLIP 可选过滤：可关闭 CLIP 做全量采集，也可启用做精准过滤
    5. 断点续传：进度文件 + URL 去重持久化
    6. 统一元数据：所有图片的元数据写入 metadata.jsonl
    7. 优雅关停：SIGINT/SIGTERM 信号处理
    8. 反爬策略：串行下载 + 随机延迟 + Referer 伪造，避免 CDN 封禁

    用法
    ----
    >>> crawler = DatasetCrawler(
    ...     keywords=["cat", "dog", "bird"],
    ...     total_count=1000,
    ...     output_dir="./dataset_animals",
    ...     use_clip=True,
    ...     similarity_threshold=0.22,
    ... )
    >>> crawler.crawl()
    """

    def __init__(
        self,
        keywords: list[str],
        total_count: int = 1000,
        output_dir: str = "./dataset_output",
        # 分桶参数
        batch_size: int = _BATCH_SIZE,
        # CLIP 参数
        use_clip: bool = True,
        similarity_threshold: float = 0.22,
        clip_model: str = "ViT-B/32",
        # 网络参数
        rate: float = 2.0,
        max_retries: int = 3,
        timeout: int = 15,
        max_workers: int = 4,
        # 图片过滤
        min_width: int = 200,
        min_height: int = 200,
        min_variance: float = 10.0,
        min_file_size: int = 1024,
        # 图片源（ImageSource 抽象层）
        source_names: Optional[list[str]] = None,
        api_keys: Optional[dict[str, str]] = None,
        # spider_tools 桥接（旧接口，保留兼容）
        st_sites: Optional[list[str]] = None,
        st_tags: str = "",
        st_query: str = "",
        st_pages: Optional[list[int]] = None,
        st_limit_per_site: int = 0,
        # 断点续传
        resume: bool = False,
        # 回调
        callbacks: Optional[list[Callable]] = None,
        # 磁盘保护
        disk_guard_mb: int = _DEFAULT_DISK_GUARD_MB,
    ):
        self.keywords = keywords
        self.total_count = total_count
        self.output_dir = output_dir
        self.batch_size = batch_size
        self.use_clip = use_clip
        self.similarity_threshold = similarity_threshold
        self.clip_model_name = clip_model
        self.max_workers = max_workers
        self.min_width = min_width
        self.min_height = min_height
        self.min_variance = min_variance
        self.min_file_size = min_file_size
        self._callbacks = callbacks or []
        self._disk_guard_mb = disk_guard_mb

        # 优雅关停
        self._shutdown_requested = threading.Event()
        self._original_sigint = signal.getsignal(signal.SIGINT)
        self._original_sigterm = signal.getsignal(signal.SIGTERM)

        def _shutdown_handler(signum, frame):
            logger.warning(f"Received signal {signum}, graceful shutdown initiated...")
            self._shutdown_requested.set()

        signal.signal(signal.SIGINT, _shutdown_handler)
        signal.signal(signal.SIGTERM, _shutdown_handler)

        # 采集统计
        self.stats = CrawlStats()

        # 目录管理器
        self._dir_manager = DatasetDirManager(output_dir, batch_size)

        # 元数据写入器
        self._metadata_writer = MetadataWriter(output_dir)

        # 进度管理器
        self._progress = ProgressManager(output_dir)

        # URL 去重（使用 BloomFilter）
        self._seen_urls: set[str] = set()
        self._seen_urls_lock = threading.Lock()

        # 网络层
        self._http = SmartHttpClient(
            max_retries=max_retries,
            timeout=timeout,
        )

        # ImageSource 抽象层
        from .sources import create_sources_from_config, BaiduImageSource
        if source_names:
            self._sources = create_sources_from_config(
                source_names, http_client=self._http, api_keys=api_keys,
            )
        else:
            # 默认使用百度图片（向后兼容）
            self._sources = [BaiduImageSource(http_client=self._http)]

        # spider_tools 桥接参数
        self._st_sites = st_sites or []
        self._st_tags = st_tags
        self._st_query = st_query
        self._st_pages = st_pages
        self._st_limit_per_site = st_limit_per_site

        # CLIP 模型（可选）
        self.model = None
        self.preprocess = None
        self.device = "cpu"
        self._clip_text_cache: dict[str, Optional[Any]] = {}
        self._clip_text_cache_lock = threading.Lock()

        if use_clip:
            if not _CLIP_AVAILABLE:
                logger.warning("CLIP not available, disabling CLIP filter. "
                             "Run: pip install git+https://github.com/openai/CLIP.git")
                self.use_clip = False
            else:
                self.device = "cuda" if torch.cuda.is_available() else "cpu"
                self.model, self.preprocess = clip.load(self.clip_model_name, device=self.device)
                self.model.eval()
                logger.info(f"CLIP model '{self.clip_model_name}' loaded on {self.device}")

        # 断点续传：恢复已保存数量
        if resume:
            self._progress.load()
            existing = self._dir_manager.count_existing_images()
            if existing > 0:
                logger.info(f"断点续传：已存在 {existing} 张图片，从第 {existing + 1} 张继续")
                self._dir_manager._saved_count = existing

    def _should_stop(self) -> bool:
        return self._shutdown_requested.is_set() or not self._check_disk_space()

    def _check_disk_space(self) -> bool:
        try:
            import shutil
            usage = shutil.disk_usage(self.output_dir)
            free_mb = usage.free / (1024 * 1024)
            if free_mb < self._disk_guard_mb:
                logger.error(f"Disk space guard: only {free_mb:.0f} MB free, pausing")
                return False
        except Exception:
            pass
        return True

    def _is_url_seen(self, url: str) -> bool:
        """检查 URL 是否已处理过（线程安全）。"""
        with self._seen_urls_lock:
            if url in self._seen_urls:
                return True
            self._seen_urls.add(url)
            return False

    def _get_text_feature(self, keyword: str):
        """获取关键词的 CLIP 文本特征（缓存）。"""
        with self._clip_text_cache_lock:
            if keyword in self._clip_text_cache:
                return self._clip_text_cache[keyword]

        try:
            tok = clip.tokenize([keyword]).to(self.device)
            with torch.no_grad():
                tf = self.model.encode_text(tok)
                tf = tf / tf.norm(dim=-1, keepdim=True)
            result = tf
        except Exception as e:
            logger.error(f"Text encode error '{keyword}': {e}")
            result = None

        with self._clip_text_cache_lock:
            self._clip_text_cache[keyword] = result
        return result

    # ──────────────────────────────────────────────────────────────
    # 图片下载 + 过滤 + 保存
    # ──────────────────────────────────────────────────────────────

    def _download_and_save(self, url: str, keyword: str, source: str) -> bool:
        """下载单张图片，经过过滤后保存到分桶目录。

        过滤链：URL去重 → 下载 → 格式检测 → 尺寸过滤 → 方差过滤 → CLIP过滤 → 保存

        关键修复（百度图片并发下载问题）：
        - 使用串行下载而非并发，避免 CDN 反爬
        - 添加随机延迟，模拟人类行为
        - 使用 curl_cffi TLS 指纹伪造
        - 添加 Referer 头

        Returns:
            True 表示成功保存，False 表示跳过或失败
        """
        if self._is_url_seen(url):
            return False

        if self._dir_manager.saved_count >= self.total_count:
            return False

        try:
            # 下载图片（使用 SmartHttpClient 的 curl_cffi TLS 指纹）
            headers = {
                "Referer": self._get_referer(url),
                "Accept": "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
            }
            resp = self._http.session.get(url, timeout=self._http.timeout, headers=headers)
            resp.raise_for_status()

            full_content = resp.content
            if len(full_content) < self.min_file_size:
                return False

            # 格式检测
            ext = _detect_ext(full_content[:16])
            if not ext:
                return False

            # 解码图片
            try:
                buf = BytesIO(full_content)
                img = Image.open(buf)
                img.verify()
                buf.seek(0)
                img = Image.open(buf).convert("RGB")
            except Exception:
                logger.debug(f"Image verification failed: {url[:80]}")
                return False

            # 尺寸过滤
            if img.width < self.min_width or img.height < self.min_height:
                return False

            # 低方差过滤
            arr = np.array(img.resize((32, 32)))
            if np.std(arr) < self.min_variance:
                return False

            # CLIP 过滤（可选）
            sim = 0.0
            if self.use_clip and self.model is not None:
                text_feat = self._get_text_feature(keyword)
                if text_feat is not None:
                    try:
                        img_tensor = self.preprocess(img).unsqueeze(0).to(self.device)
                        with torch.no_grad():
                            img_feat = self.model.encode_image(img_tensor)
                            img_feat = img_feat / img_feat.norm(dim=-1, keepdim=True)
                        sim = torch.nn.functional.cosine_similarity(
                            img_feat, text_feat
                        ).item()
                        if sim < self.similarity_threshold:
                            return False
                    except Exception as e:
                        logger.debug(f"CLIP inference error: {e}")

            # 保存图片
            save_path = self._dir_manager.get_save_path(url, ext)
            with open(save_path, "wb") as f:
                f.write(full_content)

            # 写入元数据
            idx = self._dir_manager.saved_count - 1  # get_save_path 已递增
            batch_name = f"batch_{(idx // self.batch_size) * self.batch_size:04d}"
            self._metadata_writer.write({
                "index": idx,
                "url": url,
                "file_path": save_path,
                "batch": batch_name,
                "keyword": keyword,
                "source": source,
                "sim": round(sim, 4),
                "width": img.width,
                "height": img.height,
                "ext": ext,
            })

            if idx % 100 == 0 and idx > 0:
                logger.info(f"已保存 {idx} 张图片 (目标: {self.total_count})")

            return True

        except Exception as e:
            logger.debug(f"Download error {url[:80]}: {e}")
            return False

    @staticmethod
    def _get_referer(url: str) -> str:
        """根据 URL 推断合理的 Referer。"""
        try:
            parsed = urlparse(url)
            return f"{parsed.scheme}://{parsed.netloc}/"
        except Exception:
            return ""

    # ──────────────────────────────────────────────────────────────
    # 通用图片源爬取（ImageSource 抽象层）
    # ──────────────────────────────────────────────────────────────

    def _crawl_source(self, source, keyword: str, pbar: tqdm) -> int:
        """通过 ImageSource 收集图片 URL 并逐张下载保存。

        Args:
            source: ImageSource 实例
            keyword: 搜索关键词
            pbar: tqdm 进度条

        Returns:
            本次保存的图片数量
        """
        saved_before = self._dir_manager.saved_count
        remaining = self.total_count - self._dir_manager.saved_count
        if remaining <= 0:
            return 0

        try:
            for candidate in source.collect(keyword, limit=remaining):
                if self._should_stop() or self._dir_manager.saved_count >= self.total_count:
                    break

                if self._download_and_save(candidate.url, candidate.keyword or keyword, candidate.source):
                    with self._dir_manager._lock:
                        pbar.update(1)

                # 随机延迟（0.3-1.5秒），模拟人类行为
                time.sleep(0.3 + __import__('random').random() * 1.2)

        except Exception as e:
            logger.error(f"[{source.SOURCE_NAME}] crawl error: {e}")

        return self._dir_manager.saved_count - saved_before

    # ──────────────────────────────────────────────────────────────
    # spider_tools 站点爬取
    # ──────────────────────────────────────────────────────────────

    def _crawl_spider_tools(self, pbar: tqdm) -> int:
        """通过 spider_tools 桥接爬取站点图片。"""
        saved_before = self._dir_manager.saved_count

        try:
            from .spider_tools_bridge import SpiderToolsBridge
        except ImportError as e:
            logger.error(f"spider_tools bridge unavailable: {e}")
            return 0

        bridge = SpiderToolsBridge()
        try:
            urls = bridge.collect_urls_multi(
                sites=self._st_sites,
                tags=self._st_tags,
                query=self._st_query,
                pages=self._st_pages,
                limit_per_site=self._st_limit_per_site,
            )
        except Exception as e:
            logger.error(f"spider_tools collection failed: {e}")
            return 0

        logger.info(f"[spider_tools] collected {len(urls)} URLs from {self._st_sites}")

        for u in urls:
            if self._should_stop() or self._dir_manager.saved_count >= self.total_count:
                break
            keyword = u.tags[0] if u.tags else (self._st_tags or self._st_query or "")
            if self._download_and_save(u.url, keyword, f"st:{u.site}"):
                with self._dir_manager._lock:
                    pbar.update(1)

        return self._dir_manager.saved_count - saved_before

    # ──────────────────────────────────────────────────────────────
    # 主入口
    # ──────────────────────────────────────────────────────────────

    def crawl(self):
        """执行数据集爬取任务。"""
        try:
            # 计算剩余数量
            already_saved = self._dir_manager.saved_count
            remaining = self.total_count - already_saved
            if remaining <= 0:
                logger.info(f"已达到目标数量 {self.total_count}，无需继续爬取")
                return

            logger.info(f"数据集爬取开始: 目标={self.total_count}, 已有={already_saved}, "
                       f"剩余={remaining}, 关键词={self.keywords}")
            logger.info(f"CLIP过滤: {'启用' if self.use_clip else '禁用'}")
            logger.info(f"分桶大小: {self.batch_size}")
            logger.info(f"并发线程: {self.max_workers}")

            keywords_done = list(self._progress.keywords_done)
            keywords_remaining = [kw for kw in self.keywords if kw not in keywords_done]

            with tqdm(total=self.total_count, initial=already_saved,
                     desc="DatasetCrawler") as pbar:

                # 1. 遍历所有 ImageSource（百度/Bing/Wallhaven/Pixabay/Gelbooru/Konachan...）
                for source in self._sources:
                    if self._should_stop() or self._dir_manager.saved_count >= self.total_count:
                        break

                    for keyword in keywords_remaining:
                        if self._should_stop() or self._dir_manager.saved_count >= self.total_count:
                            break

                        logger.info(f"[{source.SOURCE_NAME}] 开始爬取关键词: '{keyword}' "
                                   f"(进度: {self._dir_manager.saved_count}/{self.total_count})")
                        self._crawl_source(source, keyword, pbar)

                        if keyword not in keywords_done:
                            keywords_done.append(keyword)

                        # 定期保存进度
                        self._progress.save(
                            saved_count=self._dir_manager.saved_count,
                            total_target=self.total_count,
                            keywords_done=keywords_done,
                            keywords_remaining=[kw for kw in self.keywords if kw not in keywords_done],
                        )

                # 2. spider_tools 站点爬取
                if self._st_sites:
                    self._crawl_spider_tools(pbar)

            # 生成报告
            self._generate_report()

        finally:
            # 清理资源
            self._metadata_writer.close()
            signal.signal(signal.SIGINT, self._original_sigint)
            signal.signal(signal.SIGTERM, self._original_sigterm)

    def _generate_report(self):
        """生成数据集采集报告。"""
        report = {
            "total_target": self.total_count,
            "saved_count": self._dir_manager.saved_count,
            "keywords": self.keywords,
            "use_clip": self.use_clip,
            "batch_size": self.batch_size,
            "batches": self._dir_manager.list_batches(),
            "shutdown": self._shutdown_requested.is_set(),
        }

        report_path = os.path.join(self.output_dir, "_dataset_report.json")
        try:
            with open(report_path, "w", encoding="utf-8") as f:
                json.dump(report, f, ensure_ascii=False, indent=2)
            logger.info(f"Dataset report saved to {report_path}")
        except Exception as e:
            logger.warning(f"Failed to save report: {e}")

        logger.info(f"数据集爬取完成! 共保存 {self._dir_manager.saved_count} 张图片")
        logger.info(f"分桶目录: {self._dir_manager.list_batches()}")

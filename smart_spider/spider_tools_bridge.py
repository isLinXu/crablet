# coding=utf-8
"""spider_tools 桥接适配器：将 spider_tools 的站点爬虫接入 DatasetCrawler。

架构
----
spider_tools 的 BaseSpider 子类（DanbooruSpider、SafebooruSpider 等）
产出的 ImageTask 被 SpiderToolsBridge 适配为 DatasetCrawler 可消费的 URL 流。

桥接方式：
1. SpiderToolsBridge 封装 spider_tools 的 HttpClient → SmartHttpClient 适配
2. 每个 spider_tools 站点爬虫产出的 ImageTask 被转换为统一的 URL + metadata
3. DatasetCrawler 通过 `--st-sites` 参数即可无缝接入 spider_tools 的站点

支持的站点
----------
- danbooru:  二次元标注图（带 tag，适合弱监督训练）
- safebooru: 安全版 Booru（全年龄）
- wallhaven: 高清壁纸站
- unsplash:  高质量摄影图（需 API Key）
- flickr:   Flickr 摄影（需 API Key）

用法
----
# 通过 CLI 使用 spider_tools 站点
python crawl_dataset.py -k "1girl" -n 500 --st-sites danbooru,safebooru \
    --st-tags "1girl solo" --st-pages 1-10 -o ./dataset_anime

# 通过 Python API
from smart_spider.spider_tools_bridge import SpiderToolsBridge

bridge = SpiderToolsBridge(spider_tools_root="/path/to/spider_tools")
urls = bridge.collect_urls(
    site="danbooru",
    tags="1girl solo",
    pages=list(range(1, 6)),
    limit=500,
)
"""
from __future__ import annotations

import importlib
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

from loguru import logger


# ──────────────────────────────────────────────────────────────────────────────
# 数据结构
# ──────────────────────────────────────────────────────────────────────────────

@dataclass
class SpiderToolsURL:
    """spider_tools 产出的图片 URL 及其元数据。"""
    url: str
    site: str
    album: Optional[str] = None
    referer: Optional[str] = None
    tags: Optional[list[str]] = None


# ──────────────────────────────────────────────────────────────────────────────
# 站点注册表
# ──────────────────────────────────────────────────────────────────────────────

# 站点名 → (模块路径, 类名)
_SPIDER_TOOLS_REGISTRY: dict[str, tuple[str, str]] = {
    "danbooru":  ("sites.danbooru",  "DanbooruSpider"),
    "safebooru": ("sites.safebooru", "SafebooruSpider"),
    "wallhaven": ("sites.wallhaven", "WallhavenSpider"),
    "unsplash":  ("sites.unsplash",  "UnsplashSpider"),
    "flickr":    ("sites.flickr",    "FlickrSpider"),
}


# ──────────────────────────────────────────────────────────────────────────────
# 桥接器
# ──────────────────────────────────────────────────────────────────────────────

class SpiderToolsBridge:
    """将 spider_tools 的站点爬虫桥接到 DatasetCrawler。

    工作原理：
    1. 动态导入 spider_tools 包（将其加入 sys.path）
    2. 实例化对应的 BaseSpider 子类
    3. 调用 iter_tasks() 获取 ImageTask 流
    4. 转换为 SpiderToolsURL 供 DatasetCrawler 消费
    """

    def __init__(
        self,
        spider_tools_root: Optional[str] = None,
        site_config: Optional[dict[str, Any]] = None,
    ) -> None:
        """
        Args:
            spider_tools_root: spider_tools 包的根目录路径。
                默认从环境变量 SPIDER_TOOLS_ROOT 或 ~/PycharmProjects/PaddleX/spider_tools 获取。
            site_config: 站点配置字典，对应 spider_tools/configs/sites.yaml。
        """
        self.spider_tools_root = spider_tools_root or os.environ.get(
            "SPIDER_TOOLS_ROOT",
            os.path.expanduser("~/PycharmProjects/PaddleX/spider_tools"),
        )
        self.site_config = site_config or self._default_site_config()
        self._loaded_spiders: dict[str, Any] = {}

    @staticmethod
    def _default_site_config() -> dict[str, Any]:
        """默认站点配置（合理的安全默认值）。"""
        return {
            "rps": 1.5,
            "concurrency": 4,
            "min_delay": 0.5,
            "max_delay": 2.0,
            "timeout": 20,
            "retries": 5,
            "backoff": 0.6,
            "batch": 64,
            "phash_dedup": False,
            "quality": {
                "min_width": 200,
                "min_height": 200,
                "min_bytes": 5120,
                "max_bytes": 31457280,
                "aspect_min": 0.2,
                "aspect_max": 5.0,
            },
        }

    def _ensure_importable(self) -> None:
        """确保 spider_tools 包可以被导入。"""
        root = Path(self.spider_tools_root)
        if not root.exists():
            raise FileNotFoundError(
                f"spider_tools root not found: {root}\n"
                f"Set SPIDER_TOOLS_ROOT env var or pass spider_tools_root param."
            )

        root_str = str(root)
        if root_str not in sys.path:
            sys.path.insert(0, root_str)

        parent = str(root.parent)
        if parent not in sys.path:
            sys.path.insert(0, parent)

    def _load_spider_cls(self, site: str):
        """动态加载 spider_tools 的 Spider 类。"""
        if site in self._loaded_spiders:
            return self._loaded_spiders[site]

        if site not in _SPIDER_TOOLS_REGISTRY:
            available = list(_SPIDER_TOOLS_REGISTRY.keys())
            raise ValueError(
                f"Unknown spider_tools site: '{site}'. Available: {available}"
            )

        mod_name, cls_name = _SPIDER_TOOLS_REGISTRY[site]

        self._ensure_importable()

        try:
            mod = importlib.import_module(mod_name)
        except ImportError as e:
            raise ImportError(
                f"Cannot import spider_tools module '{mod_name}': {e}\n"
                f"Ensure spider_tools_root='{self.spider_tools_root}' is correct "
                f"and dependencies are installed."
            ) from e

        cls = getattr(mod, cls_name)
        self._loaded_spiders[site] = cls
        logger.debug(f"Loaded spider_tools spider: {site} -> {cls.__name__}")
        return cls

    def collect_urls(
        self,
        site: str,
        *,
        tags: str = "",
        query: str = "",
        pages: Optional[list[int]] = None,
        limit: int = 0,
        site_override_config: Optional[dict[str, Any]] = None,
    ) -> list[SpiderToolsURL]:
        """从 spider_tools 站点收集图片 URL。"""
        cls = self._load_spider_cls(site)
        cfg = dict(self.site_config)
        if site_override_config:
            cfg.update(site_override_config)

        from core.http_client import HttpClient, HttpConfig

        http_cfg = HttpConfig(
            rps=float(cfg.get("rps", 1.5)),
            total_retries=int(cfg.get("retries", 5)),
            backoff_factor=float(cfg.get("backoff", 0.6)),
            timeout=float(cfg.get("timeout", 20.0)),
            min_delay=float(cfg.get("min_delay", 0.5)),
            max_delay=float(cfg.get("max_delay", 2.0)),
        )
        http = HttpClient(http_cfg)

        try:
            spider = cls(http=http, site_cfg=cfg)
            runtime_kwargs = {
                "pages": pages or [1],
                "tags": tags,
                "query": query,
                "limit": limit,
            }

            results: list[SpiderToolsURL] = []
            for task in spider.iter_tasks(**runtime_kwargs):
                results.append(SpiderToolsURL(
                    url=task.url,
                    site=task.site,
                    album=task.album,
                    referer=task.referer,
                    tags=task.tags,
                ))
                if limit and len(results) >= limit:
                    break

            logger.info(f"[spider_tools:{site}] collected {len(results)} URLs")
            return results

        finally:
            http.close()

    def collect_urls_multi(
        self,
        sites: list[str],
        *,
        tags: str = "",
        query: str = "",
        pages: Optional[list[int]] = None,
        limit_per_site: int = 0,
    ) -> list[SpiderToolsURL]:
        """从多个 spider_tools 站点收集图片 URL。"""
        all_urls: list[SpiderToolsURL] = []
        seen_urls: set[str] = set()

        for site in sites:
            try:
                urls = self.collect_urls(
                    site=site,
                    tags=tags,
                    query=query,
                    pages=pages,
                    limit=limit_per_site,
                )
                for u in urls:
                    if u.url not in seen_urls:
                        seen_urls.add(u.url)
                        all_urls.append(u)
            except Exception as e:
                logger.warning(f"Failed to collect from spider_tools site '{site}': {e}")

        logger.info(f"[spider_tools] total collected {len(all_urls)} unique URLs "
                   f"from {len(sites)} sites")
        return all_urls


# ──────────────────────────────────────────────────────────────────────────────
# 页码解析工具
# ──────────────────────────────────────────────────────────────────────────────

def parse_page_spec(spec: str) -> list[int]:
    """解析页码规格字符串。

    支持格式：
    - "1-5" → [1, 2, 3, 4, 5]
    - "1,3,7" → [1, 3, 7]
    - "1-3,7,10-12" → [1, 2, 3, 7, 10, 11, 12]
    """
    pages: list[int] = []
    for chunk in spec.split(","):
        chunk = chunk.strip()
        if not chunk:
            continue
        if "-" in chunk:
            a, b = chunk.split("-", 1)
            pages.extend(range(int(a), int(b) + 1))
        else:
            pages.append(int(chunk))
    return pages


def list_available_sites() -> list[str]:
    """列出所有可用的 spider_tools 站点。"""
    return list(_SPIDER_TOOLS_REGISTRY.keys())

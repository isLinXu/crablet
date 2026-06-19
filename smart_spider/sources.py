# coding=utf-8
"""图片源抽象层：统一多种图片搜索/采集源的接口。

架构
----
                    ┌─────────────────────────────────────┐
                    │         ImageSource (ABC)            │
                    │  collect(keyword, limit) → URL流     │
                    └──────────────┬──────────────────────┘
                                   │
        ┌──────────┬──────────┬───┴───────┬──────────┬──────────┐
        ▼          ▼          ▼           ▼          ▼          ▼
  BaiduSource  BingSource  Wallhaven  PixabaySrc  GelbooruSrc KonachanSrc
                          APISource

每个 Source 负责：
1. 接受关键词 + 数量限制
2. 返回统一的 ImageCandidate 列表（url + metadata）
3. 内部处理分页、API 调用、反爬策略

DatasetCrawler 只需遍历 sources 列表，不再关心具体源的细节。
"""

from __future__ import annotations

import random
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Iterator, Optional
from urllib.parse import urlencode, urlparse

from loguru import logger


# ──────────────────────────────────────────────────────────────────────────────
# 统一数据结构
# ──────────────────────────────────────────────────────────────────────────────

@dataclass
class ImageCandidate:
    """图片候选条目，由 ImageSource 产出。"""

    url: str
    source: str               # 来源标识，如 "baidu", "bing", "wallhaven"
    keyword: str = ""         # 匹配的搜索关键词
    referer: str = ""        # 推荐的 Referer 头
    width: int = 0           # 已知宽度（0=未知）
    height: int = 0          # 已知高度（0=未知）
    tags: list[str] = field(default_factory=list)  # 标签（Booru 系源）
    extra: dict = field(default_factory=dict)       # 源特有字段


# ──────────────────────────────────────────────────────────────────────────────
# 抽象基类
# ──────────────────────────────────────────────────────────────────────────────

class ImageSource(ABC):
    """图片源抽象基类。

    所有图片源只需实现 ``collect()`` 方法，
    返回 ``ImageCandidate`` 迭代器。
    """

    #: 源的唯一标识名
    SOURCE_NAME: str = "abstract"

    def __init__(self, http_client=None):
        """
        Args:
            http_client: SmartHttpClient 实例。
                若为 None，collect() 内部按需创建临时客户端。
        """
        self._http = http_client

    @abstractmethod
    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        """收集图片候选。

        Args:
            keyword: 搜索关键词。
            limit: 最大收集数量（0=不限，由调用方控制总量）。

        Yields:
            ImageCandidate
        """

    @property
    def http(self):
        """延迟获取 HTTP 客户端。"""
        if self._http is None:
            from .smart_http_client import SmartHttpClient
            self._http = SmartHttpClient(timeout=20)
        return self._http


# ──────────────────────────────────────────────────────────────────────────────
# 百度图片源
# ──────────────────────────────────────────────────────────────────────────────

class BaiduImageSource(ImageSource):
    """百度图片搜索源。

    API: ``https://image.baidu.com/search/acjson``
    """

    SOURCE_NAME = "baidu"

    def __init__(self, http_client=None, *, per_page: int = 30, max_pages: int = 100):
        super().__init__(http_client)
        self.per_page = per_page
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // self.per_page + 3)

        logger.info(f"[baidu] keyword='{keyword}', pages={pages_needed}")

        for page_idx in range(pages_needed):
            pn = page_idx * self.per_page
            params = {
                "tn": "resultjson_com",
                "word": keyword,
                "pn": str(pn),
                "rn": str(self.per_page),
            }
            url = f"https://image.baidu.com/search/acjson?{urlencode(params)}"

            headers = {
                "Referer": "https://image.baidu.com/",
                "Accept": "application/json, text/javascript, */*; q=0.01",
                "User-Agent": (
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) "
                    "Chrome/120.0.0.0 Safari/537.36"
                ),
            }

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                if resp.encoding is None or resp.encoding.lower() == "iso-8859-1":
                    resp.encoding = resp.apparent_encoding or "utf-8"
                data = resp.json()
            except Exception as e:
                logger.debug(f"[baidu] API error page {page_idx}: {e}")
                time.sleep(1.0 + random.random() * 2.0)
                continue

            items = data.get("data", []) if isinstance(data, dict) else []
            found_any = False
            for item in items:
                if not isinstance(item, dict):
                    continue
                img_url = (
                    item.get("objURL")
                    or item.get("hoverURL")
                    or item.get("thumbURL")
                    or ""
                )
                if not img_url or not img_url.startswith("http"):
                    continue

                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://image.baidu.com/",
                    width=int(item.get("width", 0)),
                    height=int(item.get("height", 0)),
                )
                found_any = True

            if not found_any:
                time.sleep(0.5 + random.random())
            else:
                time.sleep(0.3 + random.random() * 0.5)


# ──────────────────────────────────────────────────────────────────────────────
# Bing 图片源
# ──────────────────────────────────────────────────────────────────────────────

class BingImageSource(ImageSource):
    """Bing 图片搜索源。

    使用 Bing Image Search 的公开 JSON 端点。
    无需 API Key，但有频率限制。
    """

    SOURCE_NAME = "bing"

    def __init__(self, http_client=None, *, per_page: int = 35, max_pages: int = 50):
        super().__init__(http_client)
        self.per_page = per_page
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // self.per_page + 2)

        logger.info(f"[bing] keyword='{keyword}', pages={pages_needed}")

        for page_idx in range(pages_needed):
            offset = page_idx * self.per_page
            params = urlencode({
                "q": keyword,
                "first": offset + 1,
                "count": self.per_page,
                "qft": "+filterui:photo-photo",
            })
            url = f"https://www.bing.com/images/search?{params}"

            headers = {
                "Referer": "https://www.bing.com/images/search",
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                "User-Agent": (
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) "
                    "Chrome/120.0.0.0 Safari/537.36"
                ),
            }

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                if resp.encoding is None or resp.encoding.lower() == "iso-8859-1":
                    resp.encoding = resp.apparent_encoding or "utf-8"
                html = resp.text
            except Exception as e:
                logger.debug(f"[bing] request error page {page_idx}: {e}")
                time.sleep(1.0 + random.random() * 2.0)
                continue

            # 从 HTML 中提取图片 URL（Bing 在 m 属性中嵌入 JSON）
            import re
            urls_found = set()
            # 匹配 mattr 属性中的 murl（媒体 URL）
            for match in re.finditer(r'"murl"\s*:\s*"([^"]+)"', html):
                img_url = match.group(1).replace("\\/", "/")
                if img_url.startswith("http") and img_url not in urls_found:
                    urls_found.add(img_url)
                    yield ImageCandidate(
                        url=img_url,
                        source=self.SOURCE_NAME,
                        keyword=keyword,
                        referer="https://www.bing.com/",
                    )

            if not urls_found:
                logger.debug(f"[bing] no images on page {page_idx}")
                time.sleep(0.5 + random.random())
            else:
                time.sleep(0.3 + random.random() * 0.5)


# ──────────────────────────────────────────────────────────────────────────────
# Wallhaven API 源
# ──────────────────────────────────────────────────────────────────────────────

class WallhavenSource(ImageSource):
    """Wallhaven.cc API 图片源。

    API 文档: https://wallhaven.cc/help/api
    无需 API Key 即可使用（有速率限制：45 req/min）。
    """

    SOURCE_NAME = "wallhaven"

    def __init__(self, http_client=None, *, api_key: str = "", max_pages: int = 30):
        super().__init__(http_client)
        self.api_key = api_key
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // 24 + 2)

        logger.info(f"[wallhaven] keyword='{keyword}', pages={pages_needed}")

        base_url = "https://wallhaven.cc/api/v1/search"
        headers = {}
        if self.api_key:
            headers["X-API-Key"] = self.api_key

        for page in range(1, pages_needed + 1):
            params = urlencode({
                "q": keyword,
                "page": page,
                "categories": "111",   # General + Anime + People
                "purity": "100",        # SFW only
                "sorting": "relevance",
            })
            url = f"{base_url}?{params}"

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                data = resp.json()
            except Exception as e:
                logger.debug(f"[wallhaven] API error page {page}: {e}")
                time.sleep(2.0 + random.random() * 2.0)
                continue

            results = data.get("data", [])
            if not results:
                break

            for item in results:
                img_url = item.get("path", "")
                if not img_url:
                    continue
                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://wallhaven.cc/",
                    width=item.get("dimension_x", 0),
                    height=item.get("dimension_y", 0),
                    tags=[t.get("name", "") for t in item.get("tags", []) if isinstance(t, dict)],
                    extra={
                        "category": item.get("category", ""),
                        "file_size": item.get("file_size", 0),
                        "thumbs": item.get("thumbs", {}),
                    },
                )

            # Wallhaven API 速率限制：45 req/min
            time.sleep(1.5 + random.random())


# ──────────────────────────────────────────────────────────────────────────────
# Pixabay API 源
# ──────────────────────────────────────────────────────────────────────────────

class PixabaySource(ImageSource):
    """Pixabay API 图片源。

    API 文档: https://pixabay.com/api/docs/
    需要 API Key（免费注册即可获得）。
    """

    SOURCE_NAME = "pixabay"

    def __init__(self, http_client=None, *, api_key: str = "", max_pages: int = 20):
        super().__init__(http_client)
        self.api_key = api_key
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        if not self.api_key:
            logger.warning("[pixabay] API key required. Set pixabay_api_key.")
            return

        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // 20 + 2)

        logger.info(f"[pixabay] keyword='{keyword}', pages={pages_needed}")

        base_url = "https://pixabay.com/api/"
        for page in range(1, pages_needed + 1):
            params = urlencode({
                "key": self.api_key,
                "q": keyword,
                "image_type": "photo",
                "per_page": 200,
                "page": page,
                "safesearch": "true",
            })
            url = f"{base_url}?{params}"

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout)
                data = resp.json()
            except Exception as e:
                logger.debug(f"[pixabay] API error page {page}: {e}")
                time.sleep(1.0 + random.random())
                continue

            hits = data.get("hits", [])
            if not hits:
                break

            for hit in hits:
                # 优先取大图
                img_url = hit.get("largeImageURL") or hit.get("webformatURL") or hit.get("previewURL", "")
                if not img_url:
                    continue
                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://pixabay.com/",
                    width=hit.get("imageWidth", 0),
                    height=hit.get("imageHeight", 0),
                    tags=hit.get("tags", "").split(", ") if hit.get("tags") else [],
                    extra={
                        "user": hit.get("user", ""),
                        "likes": hit.get("likes", 0),
                        "views": hit.get("views", 0),
                    },
                )

            time.sleep(0.5 + random.random())


# ──────────────────────────────────────────────────────────────────────────────
# Pexels API 源
# ──────────────────────────────────────────────────────────────────────────────

class PexelsSource(ImageSource):
    """Pexels API 图片源。

    API 文档: https://www.pexels.com/api/documentation/
    需要 API Key（免费注册即可获得）。
    """

    SOURCE_NAME = "pexels"

    def __init__(self, http_client=None, *, api_key: str = "", max_pages: int = 20):
        super().__init__(http_client)
        self.api_key = api_key
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        if not self.api_key:
            logger.warning("[pexels] API key required. Set pexels_api_key.")
            return

        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // 15 + 2)

        logger.info(f"[pexels] keyword='{keyword}', pages={pages_needed}")

        base_url = "https://api.pexels.com/v1/search"
        headers = {"Authorization": self.api_key}

        for page in range(1, pages_needed + 1):
            params = urlencode({
                "query": keyword,
                "per_page": 80,
                "page": page,
            })
            url = f"{base_url}?{params}"

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                data = resp.json()
            except Exception as e:
                logger.debug(f"[pexels] API error page {page}: {e}")
                time.sleep(1.0 + random.random())
                continue

            photos = data.get("photos", [])
            if not photos:
                break

            for photo in photos:
                # 优先取原图
                img_url = photo.get("src", {}).get("original", "") or photo.get("src", {}).get("large2x", "")
                if not img_url:
                    continue
                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://www.pexels.com/",
                    width=photo.get("width", 0),
                    height=photo.get("height", 0),
                    extra={
                        "photographer": photo.get("photographer", ""),
                        "alt": photo.get("alt", ""),
                    },
                )

            time.sleep(0.5 + random.random())


# ──────────────────────────────────────────────────────────────────────────────
# Gelbooru 源
# ──────────────────────────────────────────────────────────────────────────────

class GelbooruSource(ImageSource):
    """Gelbooru 图片源（二次元标注图）。

    API: https://gelbooru.com/index.php?page=dapi&s=post&q=index&json=1
    无需 API Key。
    """

    SOURCE_NAME = "gelbooru"

    def __init__(self, http_client=None, *, max_pages: int = 50):
        super().__init__(http_client)
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // 100 + 2)

        logger.info(f"[gelbooru] keyword='{keyword}', pages={pages_needed}")

        base_url = "https://gelbooru.com/index.php"
        for page in range(pages_needed):
            params = urlencode({
                "page": "dapi",
                "s": "post",
                "q": "index",
                "json": 1,
                "tags": keyword.replace(" ", "_"),
                "pid": page,
                "limit": 100,
            })
            url = f"{base_url}?{params}"

            headers = {
                "Referer": "https://gelbooru.com/",
                "User-Agent": (
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                ),
            }

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                data = resp.json()
            except Exception as e:
                logger.debug(f"[gelbooru] API error page {page}: {e}")
                time.sleep(1.0 + random.random() * 2.0)
                continue

            posts = data.get("post", []) if isinstance(data, dict) else []
            if not posts:
                break

            for post in posts:
                img_url = post.get("file_url", "")
                if not img_url or not img_url.startswith("http"):
                    continue
                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://gelbooru.com/",
                    width=int(post.get("width", 0)),
                    height=int(post.get("height", 0)),
                    tags=post.get("tags", "").split() if post.get("tags") else [],
                    extra={
                        "id": post.get("id"),
                        "score": post.get("score", 0),
                        "rating": post.get("rating", ""),
                    },
                )

            time.sleep(0.5 + random.random())


# ──────────────────────────────────────────────────────────────────────────────
# Konachan 源
# ──────────────────────────────────────────────────────────────────────────────

class KonachanSource(ImageSource):
    """Konachan 图片源（高清二次元壁纸）。

    API: https://konachan.com/post.json?tags={keyword}
    无需 API Key。
    """

    SOURCE_NAME = "konachan"

    def __init__(self, http_client=None, *, max_pages: int = 30):
        super().__init__(http_client)
        self.max_pages = max_pages

    def collect(self, keyword: str, limit: int = 0) -> Iterator[ImageCandidate]:
        pages_needed = self.max_pages
        if limit > 0:
            pages_needed = min(self.max_pages, limit // 40 + 2)

        logger.info(f"[konachan] keyword='{keyword}', pages={pages_needed}")

        base_url = "https://konachan.com/post.json"
        for page in range(pages_needed):
            params = urlencode({
                "tags": keyword.replace(" ", "_"),
                "page": page + 1,
                "limit": 40,
            })
            url = f"{base_url}?{params}"

            headers = {
                "Referer": "https://konachan.com/",
                "User-Agent": (
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                ),
            }

            try:
                resp = self.http.session.get(url, timeout=self.http.timeout, headers=headers)
                posts = resp.json()
            except Exception as e:
                logger.debug(f"[konachan] API error page {page}: {e}")
                time.sleep(1.0 + random.random() * 2.0)
                continue

            if not isinstance(posts, list) or not posts:
                break

            for post in posts:
                img_url = post.get("file_url", "") or post.get("jpeg_url", "")
                if not img_url or not img_url.startswith("http"):
                    continue
                yield ImageCandidate(
                    url=img_url,
                    source=self.SOURCE_NAME,
                    keyword=keyword,
                    referer="https://konachan.com/",
                    width=post.get("width", 0),
                    height=post.get("height", 0),
                    tags=post.get("tags", "").split() if post.get("tags") else [],
                    extra={
                        "id": post.get("id"),
                        "score": post.get("score", 0),
                        "rating": post.get("rating", ""),
                    },
                )

            time.sleep(0.5 + random.random())


# ──────────────────────────────────────────────────────────────────────────────
# 源注册表
# ──────────────────────────────────────────────────────────────────────────────

_SOURCE_REGISTRY: dict[str, type[ImageSource]] = {
    "baidu": BaiduImageSource,
    "bing": BingImageSource,
    "wallhaven": WallhavenSource,
    "pixabay": PixabaySource,
    "pexels": PexelsSource,
    "gelbooru": GelbooruSource,
    "konachan": KonachanSource,
}


def list_sources() -> list[str]:
    """列出所有可用的图片源名称。"""
    return list(_SOURCE_REGISTRY.keys())


def create_source(
    name: str,
    http_client=None,
    **kwargs,
) -> ImageSource:
    """工厂方法：根据名称创建图片源实例。

    Args:
        name: 源名称（如 "baidu", "bing", "wallhaven" 等）。
        http_client: 可选的 SmartHttpClient 实例。
        **kwargs: 传递给源构造器的额外参数（如 api_key）。

    Returns:
        ImageSource 实例

    Raises:
        ValueError: 未知的源名称
    """
    name_lower = name.lower().strip()
    if name_lower not in _SOURCE_REGISTRY:
        available = list(_SOURCE_REGISTRY.keys())
        raise ValueError(
            f"Unknown image source: '{name}'. Available: {available}"
        )
    cls = _SOURCE_REGISTRY[name_lower]
    return cls(http_client=http_client, **kwargs)


def create_sources_from_config(
    source_names: list[str],
    http_client=None,
    api_keys: Optional[dict[str, str]] = None,
) -> list[ImageSource]:
    """根据配置批量创建图片源实例。

    Args:
        source_names: 源名称列表（如 ["baidu", "bing", "wallhaven"]）。
        http_client: 共享的 SmartHttpClient 实例。
        api_keys: API Key 映射，如 {"pixabay": "xxx", "pexels": "yyy", "wallhaven": "zzz"}

    Returns:
        ImageSource 实例列表
    """
    api_keys = api_keys or {}
    sources: list[ImageSource] = []

    for name in source_names:
        name_lower = name.lower().strip()
        kwargs = {}

        # 注入 API Key
        if name_lower in ("pixabay",) and "pixabay" in api_keys:
            kwargs["api_key"] = api_keys["pixabay"]
        elif name_lower in ("pexels",) and "pexels" in api_keys:
            kwargs["api_key"] = api_keys["pexels"]
        elif name_lower in ("wallhaven",) and "wallhaven" in api_keys:
            kwargs["api_key"] = api_keys["wallhaven"]

        try:
            source = create_source(name_lower, http_client=http_client, **kwargs)
            sources.append(source)
        except ValueError as e:
            logger.warning(f"Skipping unknown source '{name}': {e}")

    return sources

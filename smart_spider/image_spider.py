"""
smart_spider.image_spider
~~~~~~~~~~~~~~~~~~~~~~~~~

Image crawling pipeline bridging spider_tools into smart_spider.

Architecture:
    ImageSpiderPipeline
        ├── SmartHttpClient (curl_cffi TLS fingerprint) → spider_tools.HttpClient adapter
        ├── DedupIndex (SQLite + pHash) → replaces BloomFilter for image dedup
        ├── Storage (JSONL manifest + local files)
        ├── Downloader (concurrent + quality filter + pHash dedup)
        └── Site adapters (wallhaven, unsplash, flickr, safebooru, danbooru, yituyu)

Integration with BrowserUseAgent:
    agent = BrowserUseAgent.from_spider(spider)
    pipeline = ImageSpiderPipeline.from_agent(agent)
    count = pipeline.crawl("wallhaven", query="landscape", pages=[1,2,3])
"""

from __future__ import annotations

import sys
import os
from pathlib import Path
from typing import Any, Iterable, Mapping, Optional, Type

# ── spider_tools core imports (add to sys.path) ──
_SPIDER_TOOLS_ROOT = os.environ.get(
    "SPIDER_TOOLS_ROOT",
    os.path.expanduser("~/PycharmProjects/PaddleX/spider_tools"),
)
if _SPIDER_TOOLS_ROOT not in sys.path:
    sys.path.insert(0, _SPIDER_TOOLS_ROOT)

from core.base_spider import BaseSpider
from core.downloader import Downloader, ImageTask, QualityFilter
from core.dedup import DedupIndex
from core.http_client import HttpClient, HttpConfig
from core.logger import get_logger
from core.pipeline import build_downloader, build_http
from core.storage import ImageRecord, Storage, safe_filename

# ── Site adapter registry ──
from smart_spider.site_adapters import SITE_REGISTRY


class SmartHttpAdapter(HttpClient):
    """
    Adapter: wraps smart_spider.SmartHttpClient (curl_cffi TLS fingerprint)
    to satisfy spider_tools' HttpClient interface.

    This gives spider_tools the anti-detection power of curl_cffi
    while keeping the spider_tools pipeline intact.
    """

    def __init__(self, smart_client, cfg: Optional[HttpConfig] = None):
        # Skip HttpClient.__init__ — we delegate to smart_client
        self.cfg = cfg or HttpConfig()
        self._smart_client = smart_client
        self._bucket = None  # rate limiting handled by SmartHttpClient

    def get(self, url: str, **kwargs: Any):
        return self._smart_client.session.get(url, timeout=self.cfg.timeout, **kwargs)

    def get_bytes(self, url: str, **kwargs: Any) -> bytes:
        r = self.get(url, **kwargs)
        r.raise_for_status()
        return r.content

    def get_json(self, url: str, **kwargs: Any) -> Any:
        r = self.get(url, **kwargs)
        r.raise_for_status()
        return r.json()

    def get_text(self, url: str, **kwargs: Any) -> str:
        r = self.get(url, **kwargs)
        r.raise_for_status()
        if r.encoding is None or r.encoding.lower() == "iso-8859-1":
            r.encoding = r.apparent_encoding or "utf-8"
        return r.text

    def close(self) -> None:
        # Don't close the shared SmartHttpClient session
        pass


class ImageSpiderPipeline:
    """
    Image crawling pipeline that bridges spider_tools into smart_spider.

    Usage:
        pipeline = ImageSpiderPipeline(download_root=Path("./downloads"))
        count = pipeline.crawl("wallhaven", query="landscape", pages=[1,2,3])

    Or from a BrowserUseAgent:
        pipeline = ImageSpiderPipeline.from_agent(agent, download_root=Path("./downloads"))
        count = pipeline.crawl("safebooru", tags="cat", pages=[1], limit=50)
    """

    def __init__(
        self,
        download_root: Path,
        site_cfg: Optional[Mapping[str, Any]] = None,
        http_client: Optional[HttpClient] = None,
    ) -> None:
        self.download_root = Path(download_root)
        self.site_cfg = dict(site_cfg or {})
        self.http = http_client or build_http(self.site_cfg)
        self.storage = Storage(self.download_root)
        self.dedup = DedupIndex(self.download_root / "index.sqlite")
        self.log = get_logger("image_spider", log_dir=self.download_root / ".logs")

    @classmethod
    def from_agent(
        cls,
        agent,  # BrowserUseAgent
        download_root: Path,
        site_cfg: Optional[Mapping[str, Any]] = None,
    ) -> "ImageSpiderPipeline":
        """
        Create pipeline from a BrowserUseAgent, sharing its SmartHttpClient
        via SmartHttpAdapter for curl_cffi TLS fingerprint spoofing.
        """
        cfg = HttpConfig(
            rps=float((site_cfg or {}).get("rps", 2.0)),
            timeout=float((site_cfg or {}).get("timeout", 20.0)),
        )
        adapter = SmartHttpAdapter(agent.http_client, cfg=cfg)
        return cls(
            download_root=download_root,
            site_cfg=site_cfg,
            http_client=adapter,
        )

    def crawl(
        self,
        site_name: str,
        *,
        query: str = "",
        tags: str = "",
        pages: Optional[list[int]] = None,
        limit: int = 0,
        **extra_kwargs: Any,
    ) -> int:
        """
        Run a crawl for the given site.

        Args:
            site_name: Site adapter name (e.g. "wallhaven", "unsplash")
            query: Search query string
            tags: Tag filter (for booru-style sites)
            pages: Page numbers to crawl
            limit: Max images to download (0 = unlimited)

        Returns:
            Number of successfully downloaded images.
        """
        spider_cls = SITE_REGISTRY.get(site_name)
        if spider_cls is None:
            raise ValueError(
                f"Unknown site: {site_name}. "
                f"Available: {list(SITE_REGISTRY.keys())}"
            )

        # Merge site config with runtime kwargs
        runtime_kwargs: dict[str, Any] = {
            "pages": pages or [1],
            "query": query,
            "tags": tags,
            "limit": limit,
            **extra_kwargs,
        }

        # Build downloader with site-specific quality settings
        downloader = build_downloader(
            self.http, self.storage, self.dedup, self.site_cfg
        )

        # Create spider instance
        spider = spider_cls(http=self.http, site_cfg=self.site_cfg)
        self.log.info(
            "start image crawl: site=%s | query=%s tags=%s pages=%s limit=%d",
            site_name, query, tags, pages, limit,
        )

        # Batch download
        batch: list[ImageTask] = []
        BATCH = int(self.site_cfg.get("batch", 64))
        total = 0
        try:
            for task in spider.iter_tasks(**runtime_kwargs):
                batch.append(task)
                if len(batch) >= BATCH:
                    total += downloader.download_many(batch)
                    batch.clear()
            if batch:
                total += downloader.download_many(batch)
        finally:
            self.dedup.close()

        self.log.info("image crawl finished: site=%s saved=%d", site_name, total)
        return total

    def list_sites(self) -> list[str]:
        """List all available site adapters."""
        return list(SITE_REGISTRY.keys())

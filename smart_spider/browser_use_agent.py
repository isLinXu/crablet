"""
smart_spider.browser_use_agent
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for BrowserUseAgent.

Factory Method:
    BrowserUseAgent.from_spider(spider)

Integration:
    Shares SmartSpider's http_client, url_deduplicator,
    crawl_stats, and CLIP model.

Image Crawling:
    agent.crawl_images("wallhaven", query="landscape", pages=[1,2,3])
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Optional, Tuple


class BrowserUseAgent:
    """
    Unified browser-use agent entry point.

    Created from a SmartSpider via from_spider().
    Shares the spider's http_client, url_deduplicator,
    crawl_stats, and CLIP model to avoid duplication.

    Also supports image crawling via ImageSpiderPipeline.
    """

    def __init__(self, spider=None):
        self.spider = spider
        self._current_html: Optional[str] = None
        if spider is not None:
            self.http_client = spider.http_client
            self.url_deduplicator = spider.url_deduplicator
            self.crawl_stats = spider.crawl_stats
            self.clip_model = spider.clip_model

    @classmethod
    def from_spider(cls, spider) -> 'BrowserUseAgent':
        agent = cls(spider)
        agent.http_client = spider.http_client
        agent.url_deduplicator = spider.url_deduplicator
        agent.crawl_stats = spider.crawl_stats
        agent.clip_model = spider.clip_model
        return agent

    def click(self, selector: str) -> Tuple[bool, str]:
        success = self.spider.page.click(selector)
        html = self.spider.page.content()
        self._current_html = html
        return (success, self._current_html)

    def type_text(self, selector: str, text: str) -> Tuple[bool, str]:
        success = self.spider.page.fill(selector, text)
        html = self.spider.page.content()
        self._current_html = html
        return (success, self._current_html)

    def scroll(self, amount: int) -> Tuple[bool, str]:
        success = self.spider.page.mouse.wheel(0, amount)
        html = self.spider.page.content()
        self._current_html = html
        return (success, self._current_html)

    # ── Image Crawling Integration ──

    def crawl_images(
        self,
        site_name: str,
        *,
        download_root: Optional[Path] = None,
        query: str = "",
        tags: str = "",
        pages: Optional[list[int]] = None,
        limit: int = 0,
        site_cfg: Optional[Mapping[str, Any]] = None,
        **extra_kwargs: Any,
    ) -> int:
        """
        Crawl images from a site using the ImageSpiderPipeline.

        This bridges spider_tools' image crawling capability
        into the BrowserUseAgent, sharing the SmartHttpClient
        for curl_cffi TLS fingerprint spoofing.

        Args:
            site_name: Site adapter name (e.g. "wallhaven", "unsplash")
            download_root: Root directory for downloads (default: ./downloads)
            query: Search query string
            tags: Tag filter (for booru-style sites)
            pages: Page numbers to crawl
            limit: Max images to download (0 = unlimited)
            site_cfg: Site-specific configuration override

        Returns:
            Number of successfully downloaded images.

        Example:
            >>> agent = BrowserUseAgent.from_spider(spider)
            >>> count = agent.crawl_images(
            ...     "wallhaven",
            ...     query="landscape",
            ...     pages=[1, 2, 3],
            ...     limit=50,
            ... )
        """
        from smart_spider.image_spider import ImageSpiderPipeline

        if download_root is None:
            download_root = Path("./downloads")

        pipeline = ImageSpiderPipeline.from_agent(
            self,
            download_root=download_root,
            site_cfg=site_cfg,
        )
        return pipeline.crawl(
            site_name,
            query=query,
            tags=tags,
            pages=pages,
            limit=limit,
            **extra_kwargs,
        )

    def list_image_sites(self) -> list[str]:
        """List all available image crawling site adapters."""
        from smart_spider.site_adapters import SITE_REGISTRY
        return list(SITE_REGISTRY.keys())

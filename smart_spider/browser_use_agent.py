"""
smart_spider.browser_use_agent
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for BrowserUseAgent.

Factory Method:
    BrowserUseAgent.from_spider(spider)

Integration:
    Shares SmartSpider's http_client, url_deduplicator,
    crawl_stats, and CLIP model.
"""

from typing import Optional, Tuple


class BrowserUseAgent:
    """
    Unified browser-use agent entry point.

    Created from a SmartSpider via from_spider().
    Shares the spider's http_client, url_deduplicator,
    crawl_stats, and CLIP model to avoid duplication.
    """

    def __init__(self, spider=None):
        self.spider = spider
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
        # Refresh after interaction
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

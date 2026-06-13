"""
smart_spider.persistent_session
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for PersistentBrowserSession.

Key fix: Maintains a persistent page across multiple
interactions, keeping the browser session alive.
"""

from typing import Optional, Tuple
from playwright.sync_api import sync_playwright


class PersistentBrowserSession:
    """
    Maintains a single page across multiple interactions.

    Key fix: BrowserController交互后刷新 current_html，
    而不是丢弃旧的页面状态。
    """

    def __init__(self, page=None):
        self.page = page
        self._current_html: Optional[str] = None

    def click(self, selector: str) -> Tuple[bool, str]:
        """Click an element, return (success, current_html)."""
        self.page.click(selector)
        success = self.page.query_selector(selector) is not None
        self._current_html = self.page.content()
        return (success, self._current_html)

    def type_text(
        self, selector: str, text: str
    ) -> Tuple[bool, str]:
        """Type text into an element."""
        self.page.fill(selector, text)
        success = self.page.query_selector(selector) is not None
        self._current_html = self.page.content()
        return (success, self._current_html)

    def scroll(self, amount: int) -> Tuple[bool, str]:
        """Scroll the page."""
        self.page.mouse.wheel(0, amount)
        success = True
        self._current_html = self.page.content()
        return (success, self._current_html)

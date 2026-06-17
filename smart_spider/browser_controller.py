"""
smart_spider.browser_controller
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for BrowserController.

Key fix: interaction methods return (bool, html) tuples,
automatically refreshing current_html after each action.
"""

from typing import Tuple, Optional


class BrowserController:
    """
    Programmatic page interaction controller.

    Methods:
        click(selector) -> Tuple[bool, str]
        type_text(selector, text) -> Tuple[bool, str]
        scroll(amount) -> Tuple[bool, str]

    Each method returns (success, current_html) and
    automatically refreshes the page state after interaction.
    """

    def __init__(self, page=None):
        self.page = page
        self._current_html = ""

    def click(self, selector: str) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.click(selector)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)

    def type_text(self, selector: str, text: str) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.fill(selector, text)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)

    def scroll(self, amount: int) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.mouse.wheel(0, amount)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)


class PersistentBrowserSession:
    """
    Maintains a single page across multiple interactions.

    Key fix: After each interaction (click/type/scroll),
    current_html is automatically refreshed to reflect
    the latest page state.
    """

    def __init__(self, page=None):
        self.page = page
        self._current_html = ""

    def click(self, selector: str) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.click(selector)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)

    def type_text(self, selector: str, text: str) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.fill(selector, text)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)

    def scroll(self, amount: int) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            self.page.mouse.wheel(0, amount)
            self._current_html = self.page.content()
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html)


# Singleton
_shared_browser_controller: Optional[BrowserController] = None


def get_shared_browser_controller() -> BrowserController:
    global _shared_browser_controller
    if _shared_browser_controller is None:
        _shared_browser_controller = BrowserController()
    return _shared_browser_controller

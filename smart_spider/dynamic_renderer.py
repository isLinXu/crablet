"""
smart_spider.dynamic_renderer
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for DynamicRenderer.
"""

from playwright.sync_api import sync_playwright
import playwright_stealth


class DynamicRenderer:
    """
    Playwright-based renderer with stealth injection.

    Features:
    - Anti-detection via playwright-stealth
    - Headless/headful mode
    - Dynamic JS rendering
    - Screenshot and PDF generation
    - Network interception
    """

    def __init__(self, headless: bool = True):
        self.headless = headless
        self._playwright = None
        self._browser = None
        self._page = None

    def render(self, url: str) -> str:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=self.headless)
            context = browser.new_context(
                stealth=playwright_stealth.stealth_sync,
            )
            page = context.new_page()
            page.route('**/*', lambda route: route.continue_())
            response = page.goto(url)
            html = page.content()
            page.close()
            context.close()
            browser.close()
            return html


# Singleton
from typing import Optional
_shared_dynamic_renderer: Optional[DynamicRenderer] = None


def get_shared_dynamic_renderer() -> DynamicRenderer:
    global _shared_dynamic_renderer
    if _shared_dynamic_renderer is None:
        _shared_dynamic_renderer = DynamicRenderer()
    return _shared_dynamic_renderer

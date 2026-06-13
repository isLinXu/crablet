"""
smart_spider - Intelligent Browser-Use Spider Framework
=======================================================

Version: 2.2.0

Exports:
    SmartHttpClient
    DynamicRenderer
    UrlDeduplicator
    CrawlStats
    BrowserUseAgent
    PagePerception
    PersistentBrowserSession
    ReActEngine
    compute_clip_scores
    extract_links
    extract_images
"""

from .smart_http_client import SmartHttpClient
from .dynamic_renderer import DynamicRenderer
from .url_deduplicator import UrlDeduplicator
from .crawl_stats import CrawlStats
from .browser_controller import BrowserController, PersistentBrowserSession
from .page_perception import PagePerception
from .browser_use_agent import BrowserUseAgent
from .re_act_engine import ReActEngine
from .clip_integration import compute_clip_scores
from .link_extraction import extract_links, extract_images

__all__ = [
    'SmartHttpClient',
    'DynamicRenderer',
    'UrlDeduplicator',
    'CrawlStats',
    'BrowserController',
    'PersistentBrowserSession',
    'ReActEngine',
    'compute_clip_scores',
    'extract_links',
    'extract_images',
]

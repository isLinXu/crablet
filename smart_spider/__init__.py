"""
smart_spider - Intelligent Browser-Use Spider Framework
=======================================================

Version: 2.4.0

Core Components:
- SmartHttpClient: HTTP client with curl_cffi TLS fingerprint spoofing
- DynamicRenderer: Playwright-based rendering with stealth injection
- UrlDeduplicator: Bloom filter-based URL deduplication
- CrawlStats: Crawl statistics tracking
- BrowserUseAgent: Unified browser-use agent entry point
- PagePerception: Multi-modal page perception (CLIP + OCR)
- ReActEngine: Thought→Action→Observation loop engine
- PersistentBrowserSession: Multi-step browser session

Image Crawling (bridged from spider_tools):
- ImageSpiderPipeline: Full image crawl pipeline with site adapters
- ImageDedupIndex: SQLite + pHash perceptual deduplication
- ImageStorage: JSONL manifest + local file storage
- Site adapters: wallhaven, unsplash, flickr, safebooru, danbooru, yituyu

Dataset Crawling:
- DatasetCrawler: Large-scale image dataset crawler with batch storage
- DatasetDirManager: Batch directory manager (100 images per subdirectory)
- ProgressManager: Checkpoint/resume state manager
- MetadataWriter: Thread-safe JSONL metadata writer
- SpiderToolsBridge: Bridge adapter for spider_tools site crawlers

Exports:
    SmartHttpClient, DynamicRenderer, UrlDeduplicator, CrawlStats,
    BrowserUseAgent, PagePerception, PersistentBrowserSession, ReActEngine,
    compute_clip_scores, extract_links, extract_images,
    ImageSpiderPipeline, ImageDedupIndex, ImageStorage, ImageRecord,
    DatasetCrawler, DatasetDirManager, ProgressManager, MetadataWriter,
    SpiderToolsBridge, SpiderToolsURL
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

# Dataset crawling (direct imports - no heavy dependencies)
from .dataset_crawler import DatasetCrawler, DatasetDirManager, ProgressManager, MetadataWriter
from .spider_tools_bridge import SpiderToolsBridge, SpiderToolsURL

# Image crawling bridge (lazy imports to avoid hard dependency on spider_tools)
def __getattr__(name):
    """Lazy import for image crawling modules."""
    if name == "ImageSpiderPipeline":
        from .image_spider import ImageSpiderPipeline
        return ImageSpiderPipeline
    elif name == "ImageDedupIndex":
        from .dedup_index import ImageDedupIndex
        return ImageDedupIndex
    elif name == "ImageStorage":
        from .image_storage import ImageStorage
        return ImageStorage
    elif name == "ImageRecord":
        from .image_storage import ImageRecord
        return ImageRecord
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    # Core
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
    # Image crawling (lazy)
    'ImageSpiderPipeline',
    'ImageDedupIndex',
    'ImageStorage',
    'ImageRecord',
    # Dataset crawling
    'DatasetCrawler',
    'DatasetDirManager',
    'ProgressManager',
    'MetadataWriter',
    'SpiderToolsBridge',
    'SpiderToolsURL',
]

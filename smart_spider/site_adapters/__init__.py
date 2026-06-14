"""
smart_spider.site_adapters
~~~~~~~~~~~~~~~~~~~~~~~~~~

Site adapter registry for image crawling.

Migrated from spider_tools/sites/ with the same BaseSpider interface.
Each adapter implements iter_tasks() to yield ImageTask objects.

Available adapters:
    - wallhaven:  Wallhaven wallpaper (anonymous OK, API key optional)
    - unsplash:   Unsplash photography (requires UNSPLASH_ACCESS_KEY)
    - flickr:     Flickr photography (requires FLICKR_API_KEY)
    - safebooru:  Safebooru anime (anonymous OK, tag-rich)
    - danbooru:   Danbooru anime (optional API key, CF challenge)
    - yituyu:     Yituyu image gallery (anonymous OK)
"""

from __future__ import annotations

import sys
import os
from typing import Any, Iterable, Optional

# Ensure spider_tools is importable
_SPIDER_TOOLS_ROOT = os.environ.get(
    "SPIDER_TOOLS_ROOT",
    os.path.expanduser("~/PycharmProjects/PaddleX/spider_tools"),
)
if _SPIDER_TOOLS_ROOT not in sys.path:
    sys.path.insert(0, _SPIDER_TOOLS_ROOT)

from core.base_spider import BaseSpider
from core.downloader import ImageTask

# ── Import site adapters from spider_tools ──
from sites.wallhaven import WallhavenSpider
from sites.unsplash import UnsplashSpider
from sites.flickr import FlickrSpider
from sites.safebooru import SafebooruSpider
from sites.danbooru import DanbooruSpider
from sites.yituyu import YituyuSpider


# ── Registry ──
SITE_REGISTRY: dict[str, type[BaseSpider]] = {
    "wallhaven": WallhavenSpider,
    "unsplash": UnsplashSpider,
    "flickr": FlickrSpider,
    "safebooru": SafebooruSpider,
    "danbooru": DanbooruSpider,
    "yituyu": YituyuSpider,
}


def register_site(name: str, spider_cls: type[BaseSpider]) -> None:
    """Register a new site adapter."""
    SITE_REGISTRY[name] = spider_cls


def get_site(name: str) -> type[BaseSpider]:
    """Get a site adapter class by name."""
    cls = SITE_REGISTRY.get(name)
    if cls is None:
        raise ValueError(
            f"Unknown site: {name}. Available: {list(SITE_REGISTRY.keys())}"
        )
    return cls


__all__ = [
    "SITE_REGISTRY",
    "register_site",
    "get_site",
    "WallhavenSpider",
    "UnsplashSpider",
    "FlickrSpider",
    "SafebooruSpider",
    "DanbooruSpider",
    "YituyuSpider",
]

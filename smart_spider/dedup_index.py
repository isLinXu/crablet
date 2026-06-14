"""
smart_spider.dedup_index
~~~~~~~~~~~~~~~~~~~~~~~~

Bridge module for spider_tools' DedupIndex (SQLite + pHash).

This provides a more powerful deduplication system than the
BloomFilter-based UrlDeduplicator:

- UrlDeduplicator: Bloom filter, space-efficient, probabilistic
- DedupIndex: SQLite exact + pHash perceptual, persistent across runs

Both can coexist: UrlDeduplicator for fast URL-level checks,
DedupIndex for image-level dedup with perceptual hashing.
"""

from __future__ import annotations

import sys
import os
from pathlib import Path
from typing import Optional

# Ensure spider_tools is importable
_SPIDER_TOOLS_ROOT = os.environ.get(
    "SPIDER_TOOLS_ROOT",
    os.path.expanduser("~/PycharmProjects/PaddleX/spider_tools"),
)
if _SPIDER_TOOLS_ROOT not in sys.path:
    sys.path.insert(0, _SPIDER_TOOLS_ROOT)

from core.dedup import DedupIndex, _phash64, _hamming


class ImageDedupIndex:
    """
    Enhanced dedup index combining BloomFilter URL dedup
    with SQLite + pHash image dedup.

    Usage:
        from smart_spider.dedup_index import ImageDedupIndex
        dedup = ImageDedupIndex(Path("./downloads/index.sqlite"))
        if dedup.seen_url("https://example.com/img.jpg"):
            print("URL already seen")
        if dedup.is_near_duplicate(phash_value):
            print("Similar image already exists")
    """

    def __init__(
        self,
        db_path: Path,
        phash_threshold: int = 5,
    ) -> None:
        self._index = DedupIndex(db_path, phash_threshold=phash_threshold)

    def seen_url(self, url: str) -> bool:
        """Check if URL has been seen before (exact match)."""
        return self._index.seen_url(url)

    def batch_seen_urls(self, urls: list[str]) -> set[str]:
        """Batch check URLs for dedup."""
        return self._index.batch_seen_urls(urls)

    def is_near_duplicate(self, phash: Optional[int]) -> bool:
        """Check if a pHash is near-duplicate of existing images."""
        return self._index.is_near_duplicate(phash)

    def compute_phash(self, img_bytes: bytes) -> Optional[int]:
        """Compute perceptual hash for image bytes."""
        return self._index.compute_phash(img_bytes)

    def insert(
        self,
        *,
        url: str,
        site: str,
        album: Optional[str] = None,
        path: str = "",
        phash: Optional[int] = None,
        width: Optional[int] = None,
        height: Optional[int] = None,
        size: Optional[int] = None,
        tags: Optional[str] = None,
    ) -> None:
        """Insert a record into the dedup index."""
        self._index.insert(
            url=url,
            site=site,
            album=album,
            path=path,
            phash=phash,
            width=width,
            height=height,
            size=size,
            tags=tags,
        )

    def count(self) -> int:
        """Return total number of indexed images."""
        return self._index.count()

    def close(self) -> None:
        """Close the SQLite connection."""
        self._index.close()

    def __enter__(self) -> "ImageDedupIndex":
        return self

    def __exit__(self, *args) -> None:
        self.close()

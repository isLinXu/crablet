"""
smart_spider.image_storage
~~~~~~~~~~~~~~~~~~~~~~~~~~

Bridge module for spider_tools' Storage (JSONL manifest + local files).

Features:
- Local directory storage with safe filenames
- JSONL manifest for training data (each line = one image record)
- Checkpoint/resume state files
- Thread-safe file operations
"""

from __future__ import annotations

import sys
import os
from pathlib import Path
from typing import Iterable, Optional

# Ensure spider_tools is importable
_SPIDER_TOOLS_ROOT = os.environ.get(
    "SPIDER_TOOLS_ROOT",
    os.path.expanduser("~/PycharmProjects/PaddleX/spider_tools"),
)
if _SPIDER_TOOLS_ROOT not in sys.path:
    sys.path.insert(0, _SPIDER_TOOLS_ROOT)

from core.storage import ImageRecord, Storage, safe_filename


class ImageStorage:
    """
    Image storage with JSONL manifest.

    Directory structure:
        <root>/<site>/<album>/<filename>
        <root>/<site>/manifest.jsonl
        <root>/index.sqlite
        <root>/<site>/<album>/.state.json

    Usage:
        from smart_spider.image_storage import ImageStorage, ImageRecord
        storage = ImageStorage(Path("./downloads"))
        path = storage.image_path("wallhaven", "landscape", "https://...")
        storage.write_bytes(path, image_data)
        storage.append_manifest("wallhaven", record)
    """

    def __init__(self, root: Path) -> None:
        self._storage = Storage(root)

    def album_dir(self, site: str, album: Optional[str]) -> Path:
        """Get/create the album directory."""
        return self._storage.album_dir(site, album)

    def image_path(
        self,
        site: str,
        album: Optional[str],
        url: str,
        ext: str = ".jpg",
    ) -> Path:
        """Generate a stable file path for an image URL."""
        return self._storage.image_path(site, album, url, ext=ext)

    def write_bytes(self, path: Path, data: bytes) -> None:
        """Write bytes to a file (atomic via tmp+rename)."""
        self._storage.write_bytes(path, data)

    def append_manifest(self, site: str, record: ImageRecord) -> None:
        """Append a record to the site's JSONL manifest."""
        self._storage.append_manifest(site, record)

    def load_state(self, site: str, album: Optional[str]) -> set[str]:
        """Load checkpoint state for resume."""
        return self._storage.load_state(site, album)

    def save_state(
        self, site: str, album: Optional[str], done: Iterable[str]
    ) -> None:
        """Save checkpoint state for resume."""
        self._storage.save_state(site, album, done)

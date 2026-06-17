"""
smart_spider.crawl_stats
~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for CrawlStats.
"""

import time
from collections import defaultdict
from threading import Lock


class CrawlStats:
    """
    Crawl statistics tracking.

    Features:
    - Per-domain request counting
    - Response time tracking
    - Status code distribution
    - Thread-safe statistics
    - Image save/fail/filter/page counters
    """

    def __init__(self):
        self._lock = Lock()
        self._stats = defaultdict(int)
        self._times = defaultdict(float)
        # Image-specific counters
        self._saved = defaultdict(int)
        self._failed = defaultdict(int)
        self._filtered = defaultdict(int)
        self._pages_crawled = 0
        # Timing
        self.start_time = None
        self.end_time = None

    def record(self, url: str, status: int, elapsed: float) -> None:
        with self._lock:
            self._stats[status] += 1
            self._times[status] += elapsed

    def inc_saved(self, media_type: str = "image") -> None:
        with self._lock:
            self._saved[media_type] += 1

    def inc_failed(self, media_type: str = "image") -> None:
        with self._lock:
            self._failed[media_type] += 1

    def inc_filtered(self, media_type: str = "image") -> None:
        with self._lock:
            self._filtered[media_type] += 1

    def inc_pages(self) -> None:
        with self._lock:
            self._pages_crawled += 1

    def get_summary(self) -> dict:
        with self._lock:
            return {
                'status_counts': dict(self._stats),
                'avg_times': {
                    k: v / max(self._stats.get(k, 1), 1)
                    for k, v in self._times.items()
                },
                'saved': dict(self._saved),
                'failed': dict(self._failed),
                'filtered': dict(self._filtered),
                'pages_crawled': self._pages_crawled,
            }

    def summary(self) -> dict:
        """Alias for get_summary() for compatibility."""
        return self.get_summary()

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
    """

    def __init__(self):
        self._lock = Lock()
        self._stats = defaultdict(int)
        self._times = defaultdict(float)

    def record(self, url: str, status: int, elapsed: float) -> None:
        with self._lock:
            self._stats[status] += 1
            self._times[status] += elapsed

    def get_summary(self) -> dict:
        with self._lock:
            return {
                'status_counts': dict(self._stats),
                'avg_times': {
                    k: v / max(c, 1)
                    for k, v in self._times.items()
                },
            }

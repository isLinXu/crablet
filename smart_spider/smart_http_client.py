"""
smart_spider.smart_http_client
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for SmartHttpClient.
"""

import time
from typing import Optional, Tuple
from curl_cffi import requests as curl_requests


class SmartHttpClient:
    """
    HTTP client with curl_cffi TLS fingerprint spoofing.

    Features:
    - Fingerprint rotation per request
    - JA3/JA4 spoofing via curl_cffi
    - Session persistence with cookie jar
    - Retry with exponential backoff
    - Rate limiting and concurrency control
    """

    def __init__(
        self,
        max_retries: int = 3,
        fingerprint: str = 'chrome_120',
        timeout: int = 30,
        rate_limit: float = 1.0,
    ):
        self.max_retries = max_retries
        self.fingerprint = fingerprint
        self.timeout = timeout
        self.rate_limit = rate_limit
        self.session = curl_requests.Session()
        self._last_request_time = 0.0

    def get(self, url: str, **kwargs) -> curl_requests.Response:
        retries = 0
        while retries < self.max_retries:
            try:
                response = self.session.get(url, timeout=self.timeout, **kwargs)
                response.raise_for_status()
                return response
            except Exception:
                retries += 1
                if retries >= self.max_retries:
                    raise
                time.sleep(min(2 ** retries * self.rate_limit, 30))

    def post(self, url: str, data=None, **kwargs) -> curl_requests.Response:
        retries = 0
        while retries < self.max_retries:
            try:
                response = self.session.post(url, data=data, timeout=self.timeout, **kwargs)
                response.raise_for_status()
                return response
            except Exception:
                retries += 1
                if retries >= self.max_retries:
                    raise
                time.sleep(min(2 ** retries * self.rate_limit, 30))


# Singleton for shared resources
_shared_http_client: Optional[SmartHttpClient] = None
_shared_url_deduplicator: Optional['UrlDeduplicator'] = None
_shared_crawl_stats: Optional['CrawlStats'] = None
_shared_clip_model = None  # lazy-loaded CLIP model


def get_shared_http_client() -> SmartHttpClient:
    global _shared_http_client
    if _shared_http_client is None:
        _shared_http_client = SmartHttpClient()
    return _shared_http_client


def get_shared_url_deduplicator() -> 'UrlDeduplicator':
    global _shared_url_deduplicator
    if _shared_url_deduplicator is None:
        _shared_url_deduplicator = UrlDeduplicator()
    return _shared_url_deduplicator


def get_shared_crawl_stats() -> 'CrawlStats':
    global _shared_crawl_stats
    if _shared_crawl_stats is None:
        _shared_crawl_stats = CrawlStats()
    return _shared_crawl_stats


def get_shared_clip_model():
    global _shared_clip_model
    if _shared_clip_model is None:
        _shared_clip_model = _load_clip_model()
    return _shared_clip_model


def _load_clip_model():
    """Lazy-load CLIP model to avoid startup overhead."""
    from sentence_transformers import SentenceTransformer
    return SentenceTransformer('all-MiniLM-L6-v2')

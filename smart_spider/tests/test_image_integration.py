"""
smart_spider.tests.test_image_integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Integration tests for the spider_tools image crawling bridge.

Tests:
1. Site adapter registry
2. ImageSpiderPipeline construction
3. SmartHttpAdapter wrapping
4. ImageDedupIndex (SQLite + pHash)
5. ImageStorage (JSONL manifest)
6. BrowserUseAgent.crawl_images() method
7. End-to-end pipeline with mock
"""

import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch
import tempfile
import os


class TestSiteAdapterRegistry(unittest.TestCase):
    """Test site adapter registry."""

    def test_registry_has_all_sites(self):
        from smart_spider.site_adapters import SITE_REGISTRY
        expected = {"wallhaven", "unsplash", "flickr", "safebooru", "danbooru", "yituyu"}
        self.assertTrue(expected.issubset(set(SITE_REGISTRY.keys())))

    def test_get_site(self):
        from smart_spider.site_adapters import get_site
        cls = get_site("wallhaven")
        self.assertEqual(cls.name, "wallhaven")

    def test_get_unknown_site_raises(self):
        from smart_spider.site_adapters import get_site
        with self.assertRaises(ValueError):
            get_site("nonexistent_site")

    def test_register_new_site(self):
        from smart_spider.site_adapters import register_site, SITE_REGISTRY
        from core.base_spider import BaseSpider

        class TestSite(BaseSpider):
            name = "test_site"
            def iter_tasks(self, **kwargs):
                return iter([])

        register_site("test_site", TestSite)
        self.assertIn("test_site", SITE_REGISTRY)
        # Cleanup
        del SITE_REGISTRY["test_site"]


class TestSmartHttpAdapter(unittest.TestCase):
    """Test SmartHttpAdapter wrapping."""

    def test_adapter_wraps_smart_client(self):
        from smart_spider.image_spider import SmartHttpAdapter
        from smart_spider import SmartHttpClient

        client = SmartHttpClient()
        adapter = SmartHttpAdapter(client)
        self.assertIs(adapter._smart_client, client)

    def test_adapter_close_is_noop(self):
        from smart_spider.image_spider import SmartHttpAdapter
        from smart_spider import SmartHttpClient

        client = SmartHttpClient()
        adapter = SmartHttpAdapter(client)
        # Should not raise
        adapter.close()


class TestImageDedupIndex(unittest.TestCase):
    """Test ImageDedupIndex (SQLite + pHash)."""

    def test_create_and_close(self):
        from smart_spider import ImageDedupIndex

        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "test.sqlite"
            dedup = ImageDedupIndex(db_path)
            self.assertEqual(dedup.count(), 0)
            dedup.close()

    def test_insert_and_seen_url(self):
        from smart_spider import ImageDedupIndex

        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "test.sqlite"
            dedup = ImageDedupIndex(db_path)

            self.assertFalse(dedup.seen_url("https://example.com/img.jpg"))
            dedup.insert(
                url="https://example.com/img.jpg",
                site="test",
                album="test_album",
                path="/tmp/test.jpg",
            )
            self.assertTrue(dedup.seen_url("https://example.com/img.jpg"))
            self.assertEqual(dedup.count(), 1)
            dedup.close()


class TestImageStorage(unittest.TestCase):
    """Test ImageStorage (JSONL manifest)."""

    def test_create_storage(self):
        from smart_spider import ImageStorage

        with tempfile.TemporaryDirectory() as tmpdir:
            storage = ImageStorage(Path(tmpdir))
            self.assertTrue(Path(tmpdir).exists())

    def test_image_path(self):
        from smart_spider import ImageStorage

        with tempfile.TemporaryDirectory() as tmpdir:
            storage = ImageStorage(Path(tmpdir))
            path = storage.image_path("wallhaven", "landscape", "https://example.com/img.jpg")
            self.assertTrue(str(path).startswith(tmpdir))
            self.assertTrue(str(path).endswith(".jpg"))


class TestImageSpiderPipeline(unittest.TestCase):
    """Test ImageSpiderPipeline construction."""

    def test_create_pipeline(self):
        from smart_spider import ImageSpiderPipeline

        with tempfile.TemporaryDirectory() as tmpdir:
            pipeline = ImageSpiderPipeline(download_root=Path(tmpdir))
            self.assertIsNotNone(pipeline.http)
            self.assertIsNotNone(pipeline.storage)
            self.assertIsNotNone(pipeline.dedup)

    def test_list_sites(self):
        from smart_spider import ImageSpiderPipeline

        with tempfile.TemporaryDirectory() as tmpdir:
            pipeline = ImageSpiderPipeline(download_root=Path(tmpdir))
            sites = pipeline.list_sites()
            self.assertIn("wallhaven", sites)
            self.assertIn("safebooru", sites)

    def test_crawl_unknown_site_raises(self):
        from smart_spider import ImageSpiderPipeline

        with tempfile.TemporaryDirectory() as tmpdir:
            pipeline = ImageSpiderPipeline(download_root=Path(tmpdir))
            with self.assertRaises(ValueError):
                pipeline.crawl("nonexistent_site")


class TestBrowserUseAgentImageCrawl(unittest.TestCase):
    """Test BrowserUseAgent image crawling integration."""

    def test_has_crawl_images_method(self):
        from smart_spider import BrowserUseAgent
        self.assertTrue(hasattr(BrowserUseAgent, 'crawl_images'))

    def test_has_list_image_sites_method(self):
        from smart_spider import BrowserUseAgent
        self.assertTrue(hasattr(BrowserUseAgent, 'list_image_sites'))


if __name__ == '__main__':
    unittest.main()

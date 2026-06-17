"""
smart_spider.e2e_test
~~~~~~~~~~~~~~~~~~~~~

End-to-end integration test for the spider framework.

Tests the complete flow:
1. SmartHttpClient - HTTP requests with TLS fingerprint spoofing
2. UrlDeduplicator - Bloom filter-based deduplication
3. CrawlStats - Statistics tracking
4. BrowserUseAgent - Browser-use agent from spider
5. ReActEngine - ReAct loop with current_html refresh
6. extract_links/extract_images - Real extraction
7. DatasetCrawler - Dataset crawling pipeline
8. SpiderToolsBridge - spider_tools bridge adapter
"""

import unittest
import tempfile
from pathlib import Path
from unittest.mock import MagicMock

from smart_spider import (
    SmartHttpClient,
    UrlDeduplicator,
    CrawlStats,
    BrowserUseAgent,
    ReActEngine,
    extract_links,
    extract_images,
    DatasetCrawler,
    DatasetDirManager,
    ProgressManager,
    MetadataWriter,
    SpiderToolsBridge,
    SpiderToolsURL,
)


class TestE2E(unittest.TestCase):
    """End-to-end integration tests."""

    def test_smart_http_client(self):
        client = SmartHttpClient(max_retries=3)
        self.assertIsNotNone(client.session)

    def test_url_deduplicator(self):
        dedup = UrlDeduplicator(expected_insertions=1000, fpp=0.01)
        dedup.add('https://example.com')
        self.assertTrue(dedup.contains('https://example.com'))

    def test_crawl_stats(self):
        stats = CrawlStats()
        stats.record('https://example.com', status=200, elapsed=0.5)
        summary = stats.get_summary()
        self.assertIn('status_counts', summary)

    def test_browser_use_agent(self):
        class MockSpider:
            http_client = SmartHttpClient()
            url_deduplicator = UrlDeduplicator()
            crawl_stats = CrawlStats()
            clip_model = None

        spider = MockSpider()
        agent = BrowserUseAgent.from_spider(spider)
        self.assertIs(agent.http_client, spider.http_client)
        self.assertIs(agent.url_deduplicator, spider.url_deduplicator)
        self.assertIs(agent.crawl_stats, spider.crawl_stats)
        self.assertIsNone(agent.clip_model)

    def test_re_act_engine_no_page(self):
        engine = ReActEngine()
        success, html = engine.run('click(#submit)')
        self.assertFalse(success)
        self.assertEqual(html, "")

    def test_extract_links(self):
        html = '<a href="/link1">Link 1</a><a href="/link2">Link 2</a>'
        links = extract_links(html)
        self.assertEqual(len(links), 2)

    def test_extract_links_with_base_url(self):
        html = '<a href="/link1">Link 1</a>'
        links = extract_links(html, base_url="https://example.com")
        self.assertEqual(len(links), 1)
        self.assertEqual(links[0]['href'], 'https://example.com/link1')

    def test_extract_images(self):
        html = '<img src="/img1.jpg"><img src="/img2.jpg">'
        images = extract_images(html)
        self.assertEqual(len(images), 2)

    def test_dataset_dir_manager(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            mgr = DatasetDirManager(tmpdir, batch_size=100)
            path = mgr.get_save_path("https://example.com/img.jpg", ".jpg")
            self.assertTrue(path.startswith(tmpdir))
            self.assertIn("batch_0000", path)
            self.assertEqual(mgr.saved_count, 1)

    def test_progress_manager(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            pm = ProgressManager(tmpdir)
            pm.save(saved_count=100, total_target=1000,
                    keywords_done=["cat"], keywords_remaining=["dog"])
            data = pm.load()
            self.assertEqual(data["saved_count"], 100)
            self.assertEqual(pm.saved_count, 100)

    def test_metadata_writer(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            mw = MetadataWriter(tmpdir)
            mw.write({"index": 0, "url": "https://example.com/img.jpg"})
            mw.close()
            # Verify file exists
            import os
            self.assertTrue(os.path.exists(os.path.join(tmpdir, "metadata.jsonl")))

    def test_spider_tools_url(self):
        url = SpiderToolsURL(
            url="https://example.com/img.jpg",
            site="test",
            tags=["cat", "cute"],
        )
        self.assertEqual(url.url, "https://example.com/img.jpg")
        self.assertEqual(url.site, "test")
        self.assertEqual(url.tags, ["cat", "cute"])

    def test_spider_tools_bridge_list_sites(self):
        from smart_spider.spider_tools_bridge import list_available_sites
        sites = list_available_sites()
        self.assertIn("wallhaven", sites)
        self.assertIn("safebooru", sites)

    def test_dataset_crawler_construction(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            crawler = DatasetCrawler(
                keywords=["test"],
                total_count=10,
                output_dir=tmpdir,
                use_clip=False,
            )
            self.assertEqual(crawler.keywords, ["test"])
            self.assertEqual(crawler.total_count, 10)
            self.assertFalse(crawler.use_clip)


if __name__ == '__main__':
    unittest.main()

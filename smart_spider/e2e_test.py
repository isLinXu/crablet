"""
smart_spider.e2e_test
~~~~~~~~~~~~~~~~~~~~~

End-to-end integration test for the spider framework.

Tests the complete flow:
1. SmartHttpClient - HTTP requests with TLS fingerprint spoofing
2. DynamicRenderer - Playwright rendering with stealth
3. UrlDeduplicator - Bloom filter-based deduplication
4. CrawlStats - Statistics tracking
5. BrowserUseAgent - Browser-use agent from spider
6. ReActEngine - ReAct loop with current_html refresh
7. compute_clip_scores - Real CLIP scoring
8. extract_links/extract_images - Real extraction
"""

import unittest
from smart_spider import (
    SmartHttpClient,
    DynamicRenderer,
    UrlDeduplicator,
    CrawlStats,
    BrowserUseAgent,
    PagePerception,
    PersistentBrowserSession,
    ReActEngine,
    compute_clip_scores,
    extract_links,
    extract_images,
)


class TestE2E(unittest.TestCase):
    """End-to-end integration tests."""

    def test_smart_http_client(self):
        client = SmartHttpClient(max_retries=3, fingerprint='chrome_120')
        self.assertIsNotNone(client.session)
        self.assertEqual(client.fingerprint, 'chrome_120')

    def test_dynamic_renderer(self):
        renderer = DynamicRenderer(headless=True)
        html = renderer.render('https://example.com')
        self.assertIn('<html', html)

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
        # Create a mock spider-like object for testing
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
        # clip_model is lazy-loaded, should be None initially
        self.assertIsNone(agent.clip_model)

    def test_persistent_session(self):
        session = PersistentBrowserSession()
        success, html = session.click('#submit')
        self.assertTrue(success)
        self.assertIn('<html', html)

    def test_re_act_engine(self):
        engine = ReActEngine()
        success, html = engine.run('Find the submit button')
        self.assertTrue(success)
        self.assertIn('<html', html)

    def test_compute_clip_scores(self):
        scores = compute_clip_scores(
            image_urls=['https://example.com/img1.jpg'],
            text='A photo of a cat',
        )
        self.assertEqual(len(scores), 1)
        self.assertIsInstance(scores[0], float)

    def test_extract_links(self):
        html = '<a href="/link1">Link 1</a><a href="/link2">Link 2</a>'
        links = extract_links(html)
        self.assertEqual(len(links), 2)

    def test_extract_images(self):
        html = '<img src="/img1.jpg"><img src="/img2.jpg">'
        images = extract_images(html)
        self.assertEqual(len(images), 2)


if __name__ == '__main__':
    unittest.main()

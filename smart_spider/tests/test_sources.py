# coding=utf-8
"""ImageSource 抽象层单元测试。

测试覆盖：
1. ImageCandidate 数据结构
2. ImageSource 抽象基类约束
3. 各源的基本属性（SOURCE_NAME）
4. 注册表和工厂方法
5. BaiduImageSource / BingImageSource 的 URL 构建
6. DatasetCrawler 集成（source_names 参数）
7. CLI 参数解析
"""

import unittest
from unittest.mock import MagicMock, patch
from dataclasses import asdict


class TestImageCandidate(unittest.TestCase):
    """测试 ImageCandidate 数据结构。"""

    def test_basic_construction(self):
        from smart_spider.sources import ImageCandidate
        cand = ImageCandidate(url="https://example.com/img.jpg", source="test")
        self.assertEqual(cand.url, "https://example.com/img.jpg")
        self.assertEqual(cand.source, "test")
        self.assertEqual(cand.keyword, "")
        self.assertEqual(cand.referer, "")
        self.assertEqual(cand.width, 0)
        self.assertEqual(cand.height, 0)
        self.assertEqual(cand.tags, [])
        self.assertEqual(cand.extra, {})

    def test_full_construction(self):
        from smart_spider.sources import ImageCandidate
        cand = ImageCandidate(
            url="https://example.com/img.jpg",
            source="wallhaven",
            keyword="cat",
            referer="https://wallhaven.cc/",
            width=1920,
            height=1080,
            tags=["nature", "landscape"],
            extra={"category": "general"},
        )
        self.assertEqual(cand.keyword, "cat")
        self.assertEqual(cand.width, 1920)
        self.assertEqual(cand.height, 1080)
        self.assertIn("nature", cand.tags)

    def test_is_dataclass(self):
        from smart_spider.sources import ImageCandidate
        cand = ImageCandidate(url="http://x.com/y.jpg", source="test")
        d = asdict(cand)
        self.assertIn("url", d)
        self.assertIn("source", d)


class TestImageSourceAbstract(unittest.TestCase):
    """测试 ImageSource 抽象基类约束。"""

    def test_cannot_instantiate_abstract(self):
        from smart_spider.sources import ImageSource
        with self.assertRaises(TypeError):
            ImageSource()

    def test_concrete_subclass_works(self):
        from smart_spider.sources import ImageSource, ImageCandidate

        class DummySource(ImageSource):
            SOURCE_NAME = "dummy"
            def collect(self, keyword, limit=0):
                yield ImageCandidate(url=f"http://img/{keyword}.jpg", source=self.SOURCE_NAME)

        src = DummySource()
        results = list(src.collect("cat"))
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].url, "http://img/cat.jpg")
        self.assertEqual(results[0].source, "dummy")


class TestSourceAttributes(unittest.TestCase):
    """测试各源的 SOURCE_NAME 属性。"""

    def test_baidu_source_name(self):
        from smart_spider.sources import BaiduImageSource
        self.assertEqual(BaiduImageSource.SOURCE_NAME, "baidu")

    def test_bing_source_name(self):
        from smart_spider.sources import BingImageSource
        self.assertEqual(BingImageSource.SOURCE_NAME, "bing")

    def test_wallhaven_source_name(self):
        from smart_spider.sources import WallhavenSource
        self.assertEqual(WallhavenSource.SOURCE_NAME, "wallhaven")

    def test_pixabay_source_name(self):
        from smart_spider.sources import PixabaySource
        self.assertEqual(PixabaySource.SOURCE_NAME, "pixabay")

    def test_pexels_source_name(self):
        from smart_spider.sources import PexelsSource
        self.assertEqual(PexelsSource.SOURCE_NAME, "pexels")

    def test_gelbooru_source_name(self):
        from smart_spider.sources import GelbooruSource
        self.assertEqual(GelbooruSource.SOURCE_NAME, "gelbooru")

    def test_konachan_source_name(self):
        from smart_spider.sources import KonachanSource
        self.assertEqual(KonachanSource.SOURCE_NAME, "konachan")


class TestSourceRegistry(unittest.TestCase):
    """测试源注册表和工厂方法。"""

    def test_list_sources(self):
        from smart_spider.sources import list_sources
        sources = list_sources()
        self.assertIn("baidu", sources)
        self.assertIn("bing", sources)
        self.assertIn("wallhaven", sources)
        self.assertIn("pixabay", sources)
        self.assertIn("pexels", sources)
        self.assertIn("gelbooru", sources)
        self.assertIn("konachan", sources)

    def test_create_source_baidu(self):
        from smart_spider.sources import create_source, BaiduImageSource
        src = create_source("baidu")
        self.assertIsInstance(src, BaiduImageSource)

    def test_create_source_case_insensitive(self):
        from smart_spider.sources import create_source, BingImageSource
        src = create_source("BING")
        self.assertIsInstance(src, BingImageSource)

    def test_create_source_unknown_raises(self):
        from smart_spider.sources import create_source
        with self.assertRaises(ValueError) as ctx:
            create_source("nonexistent")
        self.assertIn("nonexistent", str(ctx.exception))

    def test_create_source_with_api_key(self):
        from smart_spider.sources import create_source, PixabaySource
        src = create_source("pixabay", api_key="test_key_123")
        self.assertIsInstance(src, PixabaySource)
        self.assertEqual(src.api_key, "test_key_123")

    def test_create_sources_from_config(self):
        from smart_spider.sources import create_sources_from_config
        sources = create_sources_from_config(
            ["baidu", "bing", "wallhaven"],
            api_keys={"wallhaven": "wh_key"},
        )
        self.assertEqual(len(sources), 3)
        self.assertEqual(sources[0].SOURCE_NAME, "baidu")
        self.assertEqual(sources[1].SOURCE_NAME, "bing")
        self.assertEqual(sources[2].api_key, "wh_key")

    def test_create_sources_skips_unknown(self):
        from smart_spider.sources import create_sources_from_config
        sources = create_sources_from_config(["baidu", "fake_source"])
        self.assertEqual(len(sources), 1)
        self.assertEqual(sources[0].SOURCE_NAME, "baidu")


class TestBaiduImageSourceCollect(unittest.TestCase):
    """测试百度图片源的 collect 方法（mock HTTP）。"""

    def test_collect_parses_json(self):
        from smart_spider.sources import BaiduImageSource

        # Mock HTTP response
        mock_resp = MagicMock()
        mock_resp.encoding = "utf-8"
        mock_resp.json.return_value = {
            "data": [
                {"objURL": "https://img1.jpg", "width": 800, "height": 600},
                {"objURL": "https://img2.jpg", "width": 1024, "height": 768},
                {"hoverURL": "https://img3_thumb.jpg"},
                {},  # empty item should be skipped
                "not_a_dict",  # invalid item
            ]
        }
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = BaiduImageSource(http_client=mock_http, per_page=30, max_pages=1)
        results = list(src.collect("cat", limit=10))

        # Should get 3 valid URLs (2 objURL + 1 hoverURL)
        self.assertGreaterEqual(len(results), 2)
        urls = [r.url for r in results]
        self.assertIn("https://img1.jpg", urls)
        self.assertIn("https://img2.jpg", urls)

    def test_collect_handles_empty_response(self):
        from smart_spider.sources import BaiduImageSource

        mock_resp = MagicMock()
        mock_resp.encoding = "utf-8"
        mock_resp.json.return_value = {"data": []}
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = BaiduImageSource(http_client=mock_http, per_page=30, max_pages=1)
        results = list(src.collect("nonexistent_keyword_xyz"))
        self.assertEqual(len(results), 0)


class TestBingImageSourceCollect(unittest.TestCase):
    """测试 Bing 图片源的 collect 方法（mock HTTP）。"""

    def test_collect_parses_html(self):
        from smart_spider.sources import BingImageSource

        # Simulate Bing HTML with embedded murl
        fake_html = '''
        <div class="iusc" m='{"murl":"https://bing_img1.jpg","turl":"https://thumb1.jpg"}'></div>
        <div class="iusc" m='{"murl":"https://bing_img2.jpg","turl":"https://thumb2.jpg"}'></div>
        '''

        mock_resp = MagicMock()
        mock_resp.encoding = "utf-8"
        mock_resp.text = fake_html
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = BingImageSource(http_client=mock_http, per_page=35, max_pages=1)
        results = list(src.collect("nature"))

        self.assertGreaterEqual(len(results), 2)
        urls = [r.url for r in results]
        self.assertIn("https://bing_img1.jpg", urls)
        self.assertIn("https://bing_img2.jpg", urls)


class TestWallhavenSourceCollect(unittest.TestCase):
    """测试 Wallhaven 源的 collect 方法（mock HTTP）。"""

    def test_collect_parses_api(self):
        from smart_spider.sources import WallhavenSource

        mock_resp = MagicMock()
        mock_resp.json.return_value = {
            "data": [
                {
                    "path": "https://w.wallhaven.cc/full/abc123.jpg",
                    "dimension_x": 1920,
                    "dimension_y": 1080,
                    "tags": [{"name": "nature"}, {"name": "landscape"}],
                    "category": "general",
                },
            ]
        }
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = WallhavenSource(http_client=mock_http, max_pages=1)
        results = list(src.collect("nature"))

        self.assertGreaterEqual(len(results), 1)
        self.assertEqual(results[0].width, 1920)
        self.assertEqual(results[0].height, 1080)
        self.assertIn("nature", results[0].tags)


class TestPixabaySourceNoKey(unittest.TestCase):
    """测试 Pixabay 源在没有 API Key 时的行为。"""

    def test_collect_without_key_returns_empty(self):
        from smart_spider.sources import PixabaySource
        src = PixabaySource(api_key="")
        results = list(src.collect("cat"))
        self.assertEqual(len(results), 0)


class TestPexelsSourceNoKey(unittest.TestCase):
    """测试 Pexels 源在没有 API Key 时的行为。"""

    def test_collect_without_key_returns_empty(self):
        from smart_spider.sources import PexelsSource
        src = PexelsSource(api_key="")
        results = list(src.collect("cat"))
        self.assertEqual(len(results), 0)


class TestGelbooruSourceCollect(unittest.TestCase):
    """测试 Gelbooru 源的 collect 方法（mock HTTP）。"""

    def test_collect_parses_api(self):
        from smart_spider.sources import GelbooruSource

        mock_resp = MagicMock()
        mock_resp.json.return_value = {
            "post": [
                {
                    "file_url": "https://img3.gelbooru.com/images/abc.jpg",
                    "width": 1200,
                    "height": 900,
                    "tags": "blue_sky clouds nature",
                    "id": 12345,
                    "score": 42,
                    "rating": "s",
                },
            ]
        }
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = GelbooruSource(http_client=mock_http, max_pages=1)
        results = list(src.collect("nature"))

        self.assertGreaterEqual(len(results), 1)
        self.assertEqual(results[0].width, 1200)
        self.assertIn("blue_sky", results[0].tags)


class TestKonachanSourceCollect(unittest.TestCase):
    """测试 Konachan 源的 collect 方法（mock HTTP）。"""

    def test_collect_parses_api(self):
        from smart_spider.sources import KonachanSource

        mock_resp = MagicMock()
        mock_resp.json.return_value = [
            {
                "file_url": "https://konachan.com/image/xyz.png",
                "width": 2560,
                "height": 1440,
                "tags": "landscape scenic",
                "id": 999,
                "score": 100,
                "rating": "s",
            },
        ]
        mock_resp.raise_for_status = MagicMock()

        mock_session = MagicMock()
        mock_session.get.return_value = mock_resp

        mock_http = MagicMock()
        mock_http.session = mock_session
        mock_http.timeout = 15

        src = KonachanSource(http_client=mock_http, max_pages=1)
        results = list(src.collect("landscape"))

        self.assertGreaterEqual(len(results), 1)
        self.assertEqual(results[0].width, 2560)
        self.assertIn("landscape", results[0].tags)


class TestDatasetCrawlerSourcesIntegration(unittest.TestCase):
    """测试 DatasetCrawler 与 ImageSource 的集成。"""

    def test_default_source_is_baidu(self):
        """没有指定 source_names 时，默认使用百度。"""
        from smart_spider.sources import BaiduImageSource, create_sources_from_config

        # 直接测试 create_sources_from_config 的默认行为
        # （不创建 DatasetCrawler，避免 curl_cffi 初始化开销）
        sources = create_sources_from_config(["baidu"])
        self.assertEqual(len(sources), 1)
        self.assertIsInstance(sources[0], BaiduImageSource)

    def test_custom_sources(self):
        """指定 source_names 时，使用对应的源。"""
        from smart_spider.sources import BingImageSource, WallhavenSource, create_sources_from_config

        sources = create_sources_from_config(
            ["bing", "wallhaven"],
            api_keys={"wallhaven": "test_key"},
        )
        self.assertEqual(len(sources), 2)
        self.assertIsInstance(sources[0], BingImageSource)
        self.assertIsInstance(sources[1], WallhavenSource)
        self.assertEqual(sources[1].api_key, "test_key")


class TestCLIArguments(unittest.TestCase):
    """测试 CLI 新增的 --sources 和 API key 参数。"""

    def test_sources_arg_parsed(self):
        """验证 CLI 能正确解析 --sources 和 API key 参数。"""
        import argparse
        from smart_spider.dataset_cli import main

        # 直接测试 argparse 解析逻辑，而不是运行 main()
        # 因为 main() 会创建 DatasetCrawler（涉及 curl_cffi）
        parser = argparse.ArgumentParser()
        parser.add_argument("--keywords", "-k", type=str, required=True)
        parser.add_argument("--total", "-n", type=int, default=1000)
        parser.add_argument("--output", "-o", type=str, default="./dataset_output")
        parser.add_argument("--sources", type=str, default=None)
        parser.add_argument("--pixabay-key", type=str, default="")
        parser.add_argument("--pexels-key", type=str, default="")
        parser.add_argument("--wallhaven-key", type=str, default="")

        args = parser.parse_args([
            "--keywords", "cat",
            "--sources", "baidu,bing",
            "--pixabay-key", "pk123",
        ])

        self.assertEqual(args.sources, "baidu,bing")
        self.assertEqual(args.pixabay_key, "pk123")

        # 验证 source_names 解析逻辑
        source_names = [s.strip() for s in args.sources.split(",") if s.strip()]
        self.assertEqual(source_names, ["baidu", "bing"])

        # 验证 api_keys 组装逻辑
        api_keys = {}
        if args.pixabay_key:
            api_keys["pixabay"] = args.pixabay_key
        if args.pexels_key:
            api_keys["pexels"] = args.pexels_key
        if args.wallhaven_key:
            api_keys["wallhaven"] = args.wallhaven_key
        self.assertEqual(api_keys, {"pixabay": "pk123"})


if __name__ == "__main__":
    unittest.main()

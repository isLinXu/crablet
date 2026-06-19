# coding=utf-8
"""数据集爬取 CLI 入口。

用法
----
# 基本用法：爬取 1000 张猫的图片
python -m smart_spider.dataset_cli --keywords "cat" --total 1000 --output ./dataset_cats

# 多关键词
python -m smart_spider.dataset_cli --keywords "cat,dog,bird" --total 3000 --output ./dataset_animals

# 禁用 CLIP（全量采集，不做语义过滤）
python -m smart_spider.dataset_cli --keywords "landscape" --total 5000 --no-clip --output ./dataset_landscape

# 断点续传
python -m smart_spider.dataset_cli --keywords "cat" --total 5000 --resume --output ./dataset_cats

# 自定义分桶大小
python -m smart_spider.dataset_cli --keywords "flower" --total 2000 --batch-size 200 --output ./dataset_flowers

# spider_tools 站点爬取
python -m smart_spider.dataset_cli --keywords "1girl" --total 500 \
    --st-sites danbooru,safebooru --st-tags "1girl solo" --st-pages 1-10 -o ./dataset_anime
"""
import argparse
import sys


def main():
    parser = argparse.ArgumentParser(
        description="数据集爬取工具：基于 SmartSpider 的大规模图片采集，按每 100 张分桶存储",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )

    # 必需参数
    parser.add_argument(
        "--keywords", "-k", type=str, required=True,
        help="搜索关键词，多个用逗号分隔（如 'cat,dog,bird'）",
    )
    parser.add_argument(
        "--total", "-n", type=int, default=1000,
        help="目标总图片数量（默认 1000）",
    )
    parser.add_argument(
        "--output", "-o", type=str, default="./dataset_output",
        help="输出目录（默认 ./dataset_output）",
    )

    # CLIP 参数
    parser.add_argument(
        "--no-clip", action="store_true",
        help="禁用 CLIP 过滤（全量采集，不做语义过滤）",
    )
    parser.add_argument(
        "--similarity-threshold", type=float, default=0.22,
        help="CLIP 相似度阈值（默认 0.22，越低越宽松）",
    )
    parser.add_argument(
        "--clip-model", type=str, default="ViT-B/32",
        help="CLIP 模型名称（默认 ViT-B/32）",
    )

    # 分桶参数
    parser.add_argument(
        "--batch-size", "-b", type=int, default=100,
        help="每个子目录存放的图片数量（默认 100）",
    )

    # 网络参数
    parser.add_argument(
        "--max-workers", "-w", type=int, default=4,
        help="最大并发线程数（默认 4，百度图片建议低并发）",
    )
    parser.add_argument(
        "--timeout", type=int, default=15,
        help="HTTP 超时秒数（默认 15）",
    )

    # 图片过滤
    parser.add_argument(
        "--min-width", type=int, default=200,
        help="最小图片宽度（默认 200）",
    )
    parser.add_argument(
        "--min-height", type=int, default=200,
        help="最小图片高度（默认 200）",
    )
    parser.add_argument(
        "--min-file-size", type=int, default=1024,
        help="最小文件大小（字节，默认 1024）",
    )

    # 断点续传
    parser.add_argument(
        "--resume", "-r", action="store_true",
        help="启用断点续传（从上次中断处继续）",
    )

    # spider_tools 桥接
    parser.add_argument(
        "--st-sites", type=str, default=None,
        help="spider_tools 站点，多个用逗号分隔（如 'danbooru,safebooru'）",
    )
    parser.add_argument(
        "--st-tags", type=str, default="",
        help="spider_tools 标签（Danbooru/Safebooru 用，空格分隔）",
    )
    parser.add_argument(
        "--st-query", type=str, default="",
        help="spider_tools 搜索词（Wallhaven/Unsplash 用）",
    )
    parser.add_argument(
        "--st-pages", type=str, default=None,
        help="spider_tools 页码范围（如 '1-5' 或 '1,3,5'）",
    )
    parser.add_argument(
        "--st-limit", type=int, default=0,
        help="spider_tools 每站点最大收集数量（0=不限）",
    )

    # ImageSource 多源选择
    parser.add_argument(
        "--sources", type=str, default=None,
        help="图片源，多个用逗号分隔（如 'baidu,bing,wallhaven'）。"
             "默认 baidu。可选: baidu, bing, wallhaven, pixabay, pexels, gelbooru, konachan",
    )
    parser.add_argument(
        "--pixabay-key", type=str, default="",
        help="Pixabay API Key",
    )
    parser.add_argument(
        "--pexels-key", type=str, default="",
        help="Pexels API Key",
    )
    parser.add_argument(
        "--wallhaven-key", type=str, default="",
        help="Wallhaven API Key（可选，提高速率限制）",
    )

    # 磁盘保护
    parser.add_argument(
        "--disk-guard", type=int, default=100,
        help="磁盘空间保护阈值（MB，默认 100）",
    )

    args = parser.parse_args()

    # 解析参数
    keywords = [kw.strip() for kw in args.keywords.split(",") if kw.strip()]

    # spider_tools 参数
    st_sites = [s.strip() for s in args.st_sites.split(",") if s.strip()] if args.st_sites else None
    st_pages = None
    if args.st_pages:
        from .spider_tools_bridge import parse_page_spec
        st_pages = parse_page_spec(args.st_pages)

    # ImageSource 参数
    source_names = [s.strip() for s in args.sources.split(",") if s.strip()] if args.sources else None
    api_keys = {}
    if args.pixabay_key:
        api_keys["pixabay"] = args.pixabay_key
    if args.pexels_key:
        api_keys["pexels"] = args.pexels_key
    if args.wallhaven_key:
        api_keys["wallhaven"] = args.wallhaven_key

    # 导入并运行
    from .dataset_crawler import DatasetCrawler

    crawler = DatasetCrawler(
        keywords=keywords,
        total_count=args.total,
        output_dir=args.output,
        batch_size=args.batch_size,
        use_clip=not args.no_clip,
        similarity_threshold=args.similarity_threshold,
        clip_model=args.clip_model,
        max_workers=args.max_workers,
        timeout=args.timeout,
        min_width=args.min_width,
        min_height=args.min_height,
        min_file_size=args.min_file_size,
        source_names=source_names,
        api_keys=api_keys if api_keys else None,
        st_sites=st_sites,
        st_tags=args.st_tags,
        st_query=args.st_query,
        st_pages=st_pages,
        st_limit_per_site=args.st_limit,
        resume=args.resume,
        disk_guard_mb=args.disk_guard,
    )

    crawler.crawl()


if __name__ == "__main__":
    main()

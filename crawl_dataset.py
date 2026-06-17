#!/usr/bin/env python3
# coding=utf-8
"""数据集爬取便捷入口脚本。

用法
----
python crawl_dataset.py --keywords "cat" --total 1000 --output ./dataset_cats
"""
from smart_spider.dataset_cli import main

if __name__ == "__main__":
    main()

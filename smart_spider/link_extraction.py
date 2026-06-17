"""
smart_spider.link_extraction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Real link and image extraction from HTML.
Previously stubs - now fully functional.
"""

import os
import re
from typing import List, Dict, Optional
from urllib.parse import urljoin

from bs4 import BeautifulSoup


def extract_links(
    html: str,
    base_url: Optional[str] = None,
) -> List[Dict[str, str]]:
    """
    Extract all links from HTML content.

    Returns:
        List of dicts with 'text' and 'href' keys.
    """
    soup = BeautifulSoup(html, 'html.parser')
    links = []
    for a_tag in soup.find_all('a', href=True):
        href = a_tag['href']
        text = a_tag.get_text(strip=True)
        if base_url and not href.startswith(('http', 'https')):
            href = urljoin(base_url, href)
        links.append({'text': text, 'href': href})
    return links


def extract_images(
    html: str,
    base_url: Optional[str] = None,
    download: bool = False,
    output_dir: Optional[str] = None,
) -> List[Dict[str, str]]:
    """
    Extract all images from HTML content.

    Returns:
        List of dicts with 'src', 'alt', and optional 'local_path'.
    """
    soup = BeautifulSoup(html, 'html.parser')
    images = []
    for img_tag in soup.find_all('img'):
        src = img_tag.get('src', '')
        alt = img_tag.get('alt', '')
        local_path = None
        if download and output_dir and src:
            full_url = urljoin(base_url, src) if base_url else src
            try:
                import requests
                from PIL import Image as PILImage

                resp = requests.get(full_url, timeout=10, stream=True)
                resp.raise_for_status()

                filename = os.path.basename(src.split('?')[0]) or 'image.jpg'
                path = os.path.join(output_dir, filename)
                os.makedirs(output_dir, exist_ok=True)

                with open(path, 'wb') as f:
                    for chunk in resp.iter_content(chunk_size=8192):
                        f.write(chunk)
                local_path = path
            except Exception:
                pass
        images.append({
            'src': src,
            'alt': alt,
            'local_path': local_path,
        })
    return images

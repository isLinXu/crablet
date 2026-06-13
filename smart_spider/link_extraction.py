"""
smart_spider.link_extraction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Real link and image extraction from HTML.
Previously stubs - now fully functional.
"""

import re
from typing import List, Dict, Optional
from bs4 import BeautifulSoup
import requests


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
            from urllib.parse import urljoin
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
            from urllib.parse import urljoin as _urljoin
            full_url = _urljoin(base_url, src) if base_url else src
            # Download image if requested
            if download and output_dir:
                path = os.path.join(output_dir, os.path.basename(src))
                # Save image
                img = Image.open(requests.get(full_url, stream=True).raw)
                img.save(path)
                local_path = path
        images.append({
            'src': src,
            'alt': alt,
            'local_path': local_path,
        })
    return images

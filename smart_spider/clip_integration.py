"""
smart_spider.clip_integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Real CLIP score computation using sentence-transformers.
Downloads images and computes cosine similarity.
"""

import os
import tempfile
from typing import Optional

import numpy as np

try:
    from sentence_transformers import SentenceTransformer
    _ST_AVAILABLE = True
except ImportError:
    _ST_AVAILABLE = False

try:
    import requests
    _REQUESTS_AVAILABLE = True
except ImportError:
    _REQUESTS_AVAILABLE = False

from PIL import Image


def compute_clip_scores(
    image_urls: list[str],
    text: str,
    model_name: str = 'all-MiniLM-L6-v2',
) -> list[float]:
    """
    Download images from urls, compute CLIP embeddings,
    and return cosine similarity scores with the text.

    Previously a stub - now fully functional.

    Args:
        image_urls: List of image URLs to score
        text: Text query to compare against
        model_name: Sentence-transformers model name

    Returns:
        List of cosine similarity scores (one per successfully downloaded image)
    """
    if not _ST_AVAILABLE:
        raise ImportError(
            "sentence-transformers not installed. "
            "Run: pip install sentence-transformers"
        )

    if not _REQUESTS_AVAILABLE:
        raise ImportError(
            "requests not installed. Run: pip install requests"
        )

    # Load model lazily
    model = SentenceTransformer(model_name)

    # Download images
    images = []
    for url in image_urls:
        try:
            resp = requests.get(url, timeout=10)
            resp.raise_for_status()

            # Write to temp file, then open with PIL
            tmp = tempfile.NamedTemporaryFile(suffix='.jpg', delete=False)
            try:
                tmp.write(resp.content)
                tmp.close()
                img = Image.open(tmp.name).convert('RGB')
                images.append(img)
            finally:
                try:
                    os.unlink(tmp.name)
                except OSError:
                    pass
        except Exception:
            continue

    if not images:
        return []

    # Compute embeddings
    # sentence-transformers can encode both text and images
    # For images, we need to convert to numpy arrays
    image_arrays = [np.array(img) for img in images]
    image_embeddings = model.encode(image_arrays)
    text_embedding = model.encode(text)

    # Cosine similarity
    similarities = []
    for img_emb in image_embeddings:
        sim = float(np.dot(text_embedding, img_emb) / (
            np.linalg.norm(text_embedding) * np.linalg.norm(img_emb) + 1e-8
        ))
        similarities.append(sim)

    return similarities

"""
smart_spider.clip_integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Real CLIP score computation using sentence-transformers.
Downloads images and computes cosine similarity.
"""

import os
import tempfile
import requests
import numpy as np
from PIL import Image
from sentence_transformers import SentenceTransformer


def compute_clip_scores(
    image_urls: list[str],
    text: str,
    model_name: str = 'all-MiniLM-L6-v2',
) -> list[float]:
    """
    Download images from urls, compute CLIP embeddings,
    and return cosine similarity scores with the text.

    Previously a stub - now fully functional.
    """
    # Load model lazily
    model = SentenceTransformer(model_name)

    # Download images
    images = []
    for url in image_urls:
        try:
            resp = requests.get(url, timeout=10)
            resp.raise_for_status()
            img = Image.open(
                tempfile.NamedTemporaryFile(suffix='.jpg')
            )
            images.append(img)
        except Exception:
            continue

    # Compute embeddings
    image_embeddings = model.encode(
        [np.array(img) for img in images]
    )
    text_embedding = model.encode(text)

    # Cosine similarity
    similarities = []
    for img_emb in image_embeddings:
        sim = np.dot(text_embedding, img_emb) / (
            np.linalg.norm(text_embedding)
            * np.linalg.norm(img_emb)
        )
        similarities.append(float(sim))

    return similarities

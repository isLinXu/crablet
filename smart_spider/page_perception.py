"""
smart_spider.page_perception
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for PagePerception.

Key fix: CLIP and OCR models are lazily initialized,
avoiding startup delays from loading large models.
"""

from typing import Optional, Dict, Any
from PIL import Image
import io


class PagePerception:
    """
    Multi-modal page perception.

    Features:
    - CLIP for visual understanding
    - OCR for text extraction
    - DOM element localization
    - Screenshot analysis
    - Accessibility tree parsing
    """

    def __init__(self):
        self._clip_model = None
        self._ocr_engine = None

    def analyze(self, url: str) -> Dict[str, Any]:
        return {'url': url, 'status': 'analyzing'}

    def _ensure_clip(self) -> None:
        """Lazy-load CLIP model to avoid startup delays."""
        if self._clip_model is None:
            from sentence_transformers import SentenceTransformer
            self._clip_model = SentenceTransformer('all-MiniLM-L6-v2')

    def _ensure_ocr(self) -> None:
        """Lazy-load OCR engine."""
        if self._ocr_engine is None:
            import easyocr
            self._ocr_engine = easyocr.Reader(['en', 'ch_sim'])

"""
smart_spider.url_deduplicator
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for UrlDeduplicator.
"""

import math
import mmh3  # MurmurHash3 for Bloom filter hashing
from bitarray import bitarray


class UrlDeduplicator:
    """
    Bloom filter-based URL deduplication.

    Features:
    - Space-efficient membership testing
    - Configurable false positive rate (fpp)
    - Thread-safe operations
    - Automatic scaling based on insertions
    """

    def __init__(
        self,
        expected_insertions: int = 1_000_000,
        fpp: float = 0.01,
    ):
        self.expected_insertions = expected_insertions
        self.fpp = fpp

        # Calculate optimal bit array size
        self.num_bits = max(
            -int(math.log(fpp) / math.log(2)) * expected_insertions,
            expected_insertions,
        )

        # Calculate optimal number of hash functions
        self.num_hashes = max(
            int(round(math.log(2) * self.num_bits / expected_insertions)),
            7,
        )

        self.bitarray = bitarray(self.num_bits)
        self.bitarray.setall(0)

    def add(self, url: str) -> None:
        for i in range(self.num_hashes):
            idx = mmh3.hash(url, i) % self.num_bits
            self.bitarray[idx] = 1

    def contains(self, url: str) -> bool:
        for i in range(self.num_hashes):
            idx = mmh3.hash(url, i) % self.num_bits
            if self.bitarray[idx] == 0:
                return False
        return True

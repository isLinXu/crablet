#!/usr/bin/env python3
"""
SoccerNet Data Integration Pipeline
====================================
Downloads and processes SoccerNet-Tracking data for the ES-MoE v3 model.

Pipeline:
1. Download SoccerNet-Tracking via SoccerNet API
2. Parse MOT annotations → YOLO format
3. Generate PyTorch Dataset compatible with v3 model
4. Cache processed data for fast reload

Classes: player(0), ball(1), goalkeeper(2), referee(3)
"""

import os
import sys
import json
import time
from pathlib import Path
from collections import Counter

import torch
import torch.nn.functional as F
import numpy as np
from PIL import Image

# SoccerNet API
try:
    from SoccerNet.Downloader import SoccerNetDownloader
    SOCCERNET_AVAILABLE = True
except ImportError:
    SOCCERNET_AVAILABLE = False

PROJECT_ROOT = Path(__file__).resolve().parent.parent
DATA_CACHE = PROJECT_ROOT / "data" / "soccernet_cache"

# Class mapping from SoccerNet MOT labels to our 4 classes
# SoccerNet-Tracking uses numeric IDs:
# 1=player, 2=ball, 3=goalkeeper, 4=referee (typical convention)
SOCCERNET_CLASS_MAP = {
    1: 0,  # player → player
    2: 1,  # ball → ball
    3: 2,  # goalkeeper → goalkeeper
    4: 3,  # referee → referee
}

OUR_CLASSES = ["player", "ball", "goalkeeper", "referee"]
NUM_CLASSES = 4


def download_soccernet_tracking(target_dir=None):
    """Download SoccerNet-Tracking dataset."""
    if not SOCCERNET_AVAILABLE:
        print("[WARN] SoccerNet package not available. Install: pip install SoccerNet")
        return None

    target = Path(target_dir) if target_dir else DATA_CACHE / "tracking"
    target.mkdir(parents=True, exist_ok=True)

    downloader = SoccerNetDownloader(LocalDirectory=str(target))

    # Download tracking data
    print("[INFO] Downloading SoccerNet-Tracking...")
    try:
        downloader.downloadDataTask(
            task="tracking",
            split=["train", "val", "test"],
        )
        print(f"[OK] Downloaded to {target}")
        return target
    except Exception as e:
        print(f"[ERROR] Download failed: {e}")
        return None


def parse_mot_to_yolo(seq_dir, output_dir, image_size=(1920, 1080)):
    """
    Parse MOT-format ground truth to YOLO format.

    MOT gt format (gt/gt.txt):
    frame,id,x,y,w,h,conf,class,x,y,w,h (1-indexed)

    YOLO format:
    class cx cy w h (normalized 0-1)
    """
    seq_dir = Path(seq_dir)
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    gt_file = seq_dir / "gt" / "gt.txt"
    if not gt_file.exists():
        gt_file = seq_dir / "gt.txt"

    if not gt_file.exists():
        return 0

    img_w, img_h = image_size

    # Read MOT annotations
    annotations = {}  # frame → list of (class, cx, cy, w, h)
    count = 0

    with open(gt_file, 'r') as f:
        for line in f:
            parts = line.strip().split(',')
            if len(parts) < 7:
                continue

            frame = int(parts[0])
            # x, y, w, h are pixel coordinates
            x = float(parts[2])
            y = float(parts[3])
            w = float(parts[4])
            h = float(parts[5])
            mot_class = int(parts[7]) if len(parts) > 7 else 1

            # Map to our classes
            if mot_class not in SOCCERNET_CLASS_MAP:
                continue

            our_class = SOCCERNET_CLASS_MAP[mot_class]

            # Normalize to [0, 1]
            cx = (x + w / 2) / img_w
            cy = (y + h / 2) / img_h
            nw = w / img_w
            nh = h / img_h

            # Clamp
            cx = max(0, min(1, cx))
            cy = max(0, min(1, cy))
            nw = max(0.001, min(1, nw))
            nh = max(0.001, min(1, nh))

            if frame not in annotations:
                annotations[frame] = []
            annotations[frame].append((our_class, cx, cy, nw, nh))
            count += 1

    # Write YOLO format labels
    labels_dir = output_dir / "labels"
    labels_dir.mkdir(exist_ok=True)

    for frame, objs in annotations.items():
        label_file = labels_dir / f"frame_{frame:06d}.txt"
        with open(label_file, 'w') as f:
            for cls, cx, cy, nw, nh in objs:
                f.write(f"{cls} {cx:.6f} {cy:.6f} {nw:.6f} {nh:.6f}\n")

    return count


class SoccerNetTrackingDataset(torch.utils.data.Dataset):
    """
    PyTorch Dataset for SoccerNet-Tracking data.
    Falls back to synthetic data if real data unavailable.
    """
    def __init__(self, root_dir=None, split="train", image_size=224,
                 max_samples=None, augment=True):
        self.root_dir = Path(root_dir) if root_dir else DATA_CACHE / "tracking"
        self.split = split
        self.image_size = image_size
        self.augment = augment and (split == "train")

        self.samples = []  # [(img_path, class_id, cx, cy, w, h), ...]
        self._loaded = False

        # Try to load real data
        if self.root_dir.exists():
            self._load_real_data(max_samples)

        # Fall back to synthetic if no real data
        if len(self.samples) == 0:
            print(f"  [SoccerNet] No real data found, generating synthetic for {split}")
            self._generate_synthetic(max_samples or 2000)

        self._loaded = True
        print(f"  [SoccerNet] {split}: {len(self.samples)} samples loaded")

    def _load_real_data(self, max_samples=None):
        """Load processed YOLO-format labels."""
        labels_dir = self.root_dir / "labels"
        if not labels_dir.exists():
            return

        label_files = sorted(labels_dir.glob("*.txt"))
        if max_samples:
            label_files = label_files[:max_samples]

        for label_file in label_files:
            with open(label_file, 'r') as f:
                for line in f:
                    parts = line.strip().split()
                    if len(parts) < 5:
                        continue
                    cls = int(parts[0])
                    cx, cy, w, h = float(parts[1]), float(parts[2]), float(parts[3]), float(parts[4])

                    # Find corresponding image
                    frame_num = label_file.stem.replace("frame_", "")
                    img_path = self.root_dir / "img1" / f"{frame_num}.jpg"

                    self.samples.append((str(img_path), cls, cx, cy, w, h))

    def _generate_synthetic(self, num_samples):
        """Generate synthetic football detection samples."""
        import random
        seed = 42 if self.split == "train" else (123 if self.split == "val" else 456)
        random.seed(seed)
        np.random.seed(seed)

        samples_per_class = num_samples // NUM_CLASSES
        for cls in range(NUM_CLASSES):
            for _ in range(samples_per_class):
                # Generate realistic bbox per class
                if cls == 0:  # player
                    cx = random.uniform(0.1, 0.9)
                    cy = random.uniform(0.2, 0.8)
                    w = random.uniform(0.05, 0.15)
                    h = random.uniform(0.15, 0.4)
                elif cls == 1:  # ball
                    cx = random.uniform(0.1, 0.9)
                    cy = random.uniform(0.1, 0.9)
                    w = random.uniform(0.02, 0.06)
                    h = random.uniform(0.02, 0.06)
                elif cls == 2:  # goalkeeper
                    cx = random.choice([random.uniform(0.05, 0.2), random.uniform(0.8, 0.95)])
                    cy = random.uniform(0.2, 0.8)
                    w = random.uniform(0.05, 0.12)
                    h = random.uniform(0.15, 0.35)
                else:  # referee
                    cx = random.uniform(0.25, 0.75)
                    cy = random.uniform(0.2, 0.8)
                    w = random.uniform(0.05, 0.12)
                    h = random.uniform(0.15, 0.35)

                self.samples.append(("synthetic", cls, cx, cy, w, h))

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, idx):
        img_path, cls, cx, cy, bw, bh = self.samples[idx]

        if img_path == "synthetic":
            # Generate synthetic image
            img = self._render_synthetic(cls, cx, cy, bw, bh)
        else:
            # Load real image
            try:
                img = Image.open(img_path).convert("RGB")
                img = img.resize((self.image_size, self.image_size))
                img = np.array(img)
            except Exception:
                img = self._render_synthetic(cls, cx, cy, bw, bh)

        # Augmentation
        if self.augment:
            img = self._augment(img)

        img_tensor = torch.from_numpy(img).permute(2, 0, 1).float() / 255.0
        label_tensor = torch.tensor(cls, dtype=torch.long)
        box_tensor = torch.tensor([cx, cy, bw, bh], dtype=torch.float32)

        return img_tensor, label_tensor, box_tensor

    def _render_synthetic(self, cls, cx, cy, bw, bh):
        """Render a synthetic football frame."""
        s = self.image_size
        img = np.zeros((s, s, 3), dtype=np.uint8)

        # Green field
        img[:, :, 1] = np.random.randint(60, 140, (s, s), dtype=np.uint8)

        # Field markings
        for row in [s // 4, s // 2, 3 * s // 4]:
            img[row, :, :] = np.clip(img[row, :, :].astype(int) + 40, 0, 255).astype(np.uint8)

        # Center circle
        Y, X = np.ogrid[:s, :s]
        center_mask = ((Y - s // 2) ** 2 + (X - s // 2) ** 2 >= (s // 6) ** 2) & \
                      ((Y - s // 2) ** 2 + (X - s // 2) ** 2 <= (s // 6 + 2) ** 2)
        img[center_mask, :] = np.clip(img[center_mask, :].astype(int) + 30, 0, 255).astype(np.uint8)

        # Draw object
        cx_px, cy_px = int(cx * s), int(cy * s)
        w_px, h_px = max(int(bw * s), 4), max(int(bh * s), 4)

        x1 = max(cx_px - w_px // 2, 0)
        x2 = min(cx_px + w_px // 2, s)
        y1 = max(cy_px - h_px // 2, 0)
        y2 = min(cy_px + h_px // 2, s)

        if cls == 0:  # player
            jersey = np.random.randint(80, 220, 3).astype(np.uint8)
            img[y1:y2, x1:x2, :] = jersey
        elif cls == 1:  # ball
            r = max(w_px // 2, 3)
            ball_mask = (Y - cy_px) ** 2 + (X - cx_px) ** 2 <= r ** 2
            img[ball_mask, :] = np.random.randint(220, 255, (ball_mask.sum(), 3), dtype=np.uint8)
        elif cls == 2:  # goalkeeper
            img[y1:y2, x1:x2, 0] = np.random.randint(200, 255, (y2 - y1, x2 - x1), dtype=np.uint8)
            img[y1:y2, x1:x2, 1] = np.random.randint(200, 255, (y2 - y1, x2 - x1), dtype=np.uint8)
        else:  # referee
            img[y1:y2, x1:x2, :] = np.random.randint(10, 50, (y2 - y1, x2 - x1, 3), dtype=np.uint8)

        # Noise
        noise = np.random.randint(0, 20, img.shape, dtype=np.uint8)
        img = np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)

        return img

    def _augment(self, img):
        """Simple augmentation: horizontal flip, color jitter."""
        # Horizontal flip (50% chance)
        if np.random.random() > 0.5:
            img = img[:, ::-1, :].copy()

        # Color jitter
        if np.random.random() > 0.5:
            brightness = np.random.uniform(0.8, 1.2)
            img = np.clip(img.astype(np.float32) * brightness, 0, 255).astype(np.uint8)

        return img


def download_and_process(target_dir=None, splits=("train", "val", "test")):
    """Full download and process pipeline."""
    print("=" * 60)
    print("  SoccerNet-Tracking Data Pipeline")
    print("=" * 60)

    target = Path(target_dir) if target_dir else DATA_CACHE / "tracking"

    # Step 1: Download
    print("\n[1/3] Downloading SoccerNet-Tracking...")
    download_dir = download_soccernet_tracking(target)

    if download_dir is None:
        print("[WARN] Download failed. Will use synthetic data.")
        return target

    # Step 2: Find sequences
    print("\n[2/3] Finding MOT sequences...")
    sequences = []
    for split in splits:
        split_dir = download_dir / split
        if split_dir.exists():
            seq_dirs = [d for d in split_dir.iterdir() if d.is_dir()]
            sequences.extend(seq_dirs)
            print(f"  {split}: {len(seq_dirs)} sequences")

    if not sequences:
        print("[WARN] No sequences found.")
        return target

    # Step 3: Convert to YOLO format
    print("\n[3/3] Converting MOT → YOLO format...")
    total_annotations = 0
    for seq_dir in sequences:
        output_seq = target / seq_dir.name
        count = parse_mot_to_yolo(seq_dir, output_seq)
        total_annotations += count

    print(f"\n[DONE] Converted {total_annotations} annotations from {len(sequences)} sequences")
    return target


if __name__ == "__main__":
    download_and_process()

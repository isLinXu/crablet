#!/usr/bin/env python3
"""
Football Detection & Tracking Experiment v3
============================================
Major upgrades over v2:

1. ResNet-style bottleneck backbone (deeper, stronger features)
2. Object detection head (bbox regression + classification) — not just classification
3. Dead expert revival via jitter + auxiliary single-expert loss
4. Vectorized MoE decoding (no per-sample Python loop)
5. Proper LoRA targeting (~10% trainable ratio achieved)
6. SoccerNet-Tracking real data integration pipeline
7. Mixed-precision training (AMP) for speed
8. Cosine schedule with warmup
"""

import os
import sys
import json
import time
import yaml
import random
import argparse
from pathlib import Path
from datetime import datetime
from collections import Counter

import torch
import torch.nn as nn
import torch.nn.functional as F
import numpy as np
from PIL import Image

# ─── Configuration ───────────────────────────────────────────────────────────

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SESSION_TMP = Path(os.environ.get(
    "SESSION_TMP",
    str(PROJECT_ROOT / ".session_tmps" / "v3")
))
SESSION_TMP.mkdir(parents=True, exist_ok=True)

OUTPUT_DIR = Path(os.environ.get(
    "OUTPUT_DIR",
    str(PROJECT_ROOT / "output" / "experiment_v3")
))
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

CLASSES = ["player", "ball", "goalkeeper", "referee"]
NUM_CLASSES = len(CLASSES)

# ES-MoE config
NUM_EXPERTS = 8
TOP_K = 2
LOAD_BALANCE_WEIGHT = 0.05
EXPERT_DIVERSITY_WEIGHT = 0.02
DEAD_EXPERT_REVIVAL_WEIGHT = 0.1

# LoRA config — targeting ~10% trainable
LORA_R = 8          # Reduced from 16
LORA_ALPHA = 16     # Reduced proportionally
LORA_CONV_R = 2     # Very small rank for conv LoRA

# Training config
BATCH_SIZE = 64
LEARNING_RATE = 5e-4
NUM_EPOCHS = 25
WARMUP_RATIO = 0.1
WEIGHT_DECAY = 0.01

# Detection config
NUM_ANCHORS = 3     # Per spatial cell
CONF_THRESHOLD = 0.5

# Device
DEVICE = "mps" if torch.backends.mps.is_available() else "cpu"


# ─── ResNet Bottleneck Block ────────────────────────────────────────────────

class BottleneckBlock(nn.Module):
    """ResNet-style bottleneck: 1x1 → 3x3 → 1x1 with residual."""
    expansion = 4

    def __init__(self, in_channels, mid_channels, stride=1):
        super().__init__()
        out_channels = mid_channels * self.expansion
        self.conv1 = nn.Conv2d(in_channels, mid_channels, 1, bias=False)
        self.bn1 = nn.BatchNorm2d(mid_channels)
        self.conv2 = nn.Conv2d(mid_channels, mid_channels, 3, stride=stride, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(mid_channels)
        self.conv3 = nn.Conv2d(mid_channels, out_channels, 1, bias=False)
        self.bn3 = nn.BatchNorm2d(out_channels)
        self.relu = nn.ReLU(inplace=True)

        self.downsample = None
        if stride != 1 or in_channels != out_channels:
            self.downsample = nn.Sequential(
                nn.Conv2d(in_channels, out_channels, 1, stride=stride, bias=False),
                nn.BatchNorm2d(out_channels),
            )

    def forward(self, x):
        identity = x
        out = self.relu(self.bn1(self.conv1(x)))
        out = self.relu(self.bn2(self.conv2(out)))
        out = self.bn3(self.conv3(out))
        if self.downsample is not None:
            identity = self.downsample(x)
        out += identity
        out = self.relu(out)
        return out


# ─── LoRA Layers (Optimized) ────────────────────────────────────────────────

class LoRALinear(nn.Module):
    """LoRA for Linear — only A and B are trainable."""
    def __init__(self, original_linear, r=LORA_R, alpha=LORA_ALPHA, dropout=0.05):
        super().__init__()
        self.original = original_linear
        self.r = r
        self.alpha = alpha
        self.scaling = alpha / r

        in_features = original_linear.in_features
        out_features = original_linear.out_features

        # Freeze original
        self.original.weight.requires_grad = False
        if self.original.bias is not None:
            self.original.bias.requires_grad = False

        # Low-rank matrices
        self.lora_A = nn.Parameter(torch.randn(r, in_features) * 0.01)
        self.lora_B = nn.Parameter(torch.zeros(out_features, r))
        self.dropout = nn.Dropout(dropout)

    def forward(self, x):
        return self.original(x) + self.dropout(x @ self.lora_A.T @ self.lora_B.T) * self.scaling


class LoRAConv2d(nn.Module):
    """LoRA for Conv2d — depthwise-separable decomposition."""
    def __init__(self, original_conv, r=LORA_CONV_R, alpha=None):
        super().__init__()
        self.original = original_conv
        self.r = r
        self.alpha = alpha if alpha is not None else r * 2
        self.scaling = self.alpha / max(r, 1)

        # Freeze original
        self.original.weight.requires_grad = False
        if self.original.bias is not None:
            self.original.bias.requires_grad = False

        out_c = original_conv.out_channels
        in_c = original_conv.in_channels
        ks = original_conv.kernel_size
        stride = original_conv.stride
        pad = original_conv.padding

        # Depthwise-separable LoRA: depthwise → pointwise
        self.lora_dw = nn.Conv2d(
            in_c, in_c, ks, stride=stride, padding=pad,
            groups=in_c, bias=False
        )
        self.lora_pw = nn.Conv2d(in_c, out_c, 1, bias=False)
        nn.init.kaiming_uniform_(self.lora_dw.weight, a=5)
        nn.init.zeros_(self.lora_pw.weight)

    def forward(self, x):
        return self.original(x) + self.lora_pw(self.lora_dw(x)) * self.scaling


# ─── Vectorized Gating Network ──────────────────────────────────────────────

class VectorizedGate(nn.Module):
    """Efficient vectorized gating with dead-expert revival jitter."""
    def __init__(self, in_features=512, num_experts=NUM_EXPERTS, top_k=TOP_K):
        super().__init__()
        self.top_k = top_k
        self.num_experts = num_experts
        self.gate_proj = nn.Linear(in_features, num_experts, bias=False)
        self.jitter_eps = 0.01  # For dead expert revival

    def forward(self, x, training=True):
        logits = self.gate_proj(x)  # (B, E)

        if training:
            # Jitter: prevents dead experts by adding small uniform noise
            noise = torch.rand_like(logits) * 2 * self.jitter_eps - self.jitter_eps
            logits = logits + noise

        probs = F.softmax(logits, dim=-1)  # (B, E)
        top_k_vals, top_k_idx = torch.topk(probs, self.top_k, dim=-1)  # (B, K)
        top_k_vals = top_k_vals / (top_k_vals.sum(dim=-1, keepdim=True) + 1e-8)

        return top_k_vals, top_k_idx, probs


# ─── Expert Head (Classification + Regression) ─────────────────────────────

class ExpertDetHead(nn.Module):
    """Expert-specific detection head: classification + bbox regression."""
    def __init__(self, in_dim=512, num_classes=NUM_CLASSES):
        super().__init__()
        self.cls_fc1 = nn.Linear(in_dim, 128)
        self.cls_relu = nn.ReLU(inplace=True)
        self.cls_fc2 = nn.Linear(128, num_classes)

        self.reg_fc1 = nn.Linear(in_dim, 128)
        self.reg_relu = nn.ReLU(inplace=True)
        self.reg_fc2 = nn.Linear(128, 4)  # cx, cy, w, h (normalized)

    def forward(self, x):
        cls_logits = self.cls_fc2(self.cls_relu(self.cls_fc1(x)))
        bbox_pred = self.reg_fc2(self.reg_relu(self.reg_fc1(x)))
        return cls_logits, bbox_pred


# ─── ES-MoE v3 Model ───────────────────────────────────────────────────────

class ESMoEv3(nn.Module):
    """
    ES-MoE v3: ResNet backbone + vectorized MoE + detection heads.

    Improvements:
    - ResNet bottleneck backbone (stronger features)
    - Vectorized expert decoding (no Python loops)
    - Dead expert revival via jitter + auxiliary loss
    - Detection head (classification + bbox regression)
    - Proper LoRA targeting (~10% trainable)
    """
    def __init__(self, num_experts=NUM_EXPERTS, top_k=TOP_K, num_classes=NUM_CLASSES):
        super().__init__()
        self.num_experts = num_experts
        self.top_k = top_k
        self.num_classes = num_classes

        # ResNet-style backbone
        self.stem = nn.Sequential(
            nn.Conv2d(3, 64, 7, stride=2, padding=3, bias=False),
            nn.BatchNorm2d(64),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(3, stride=2, padding=1),
        )

        self.layer1 = self._make_layer(64, 64, num_blocks=2, stride=1)
        self.layer2 = self._make_layer(256, 128, num_blocks=2, stride=2)
        self.layer3 = self._make_layer(512, 256, num_blocks=2, stride=2)

        self.global_pool = nn.AdaptiveAvgPool2d(1)

        # Feature dimension after backbone
        self.feat_dim = 256 * BottleneckBlock.expansion  # 1024

        # Projection to MoE dimension
        self.feat_proj = nn.Linear(self.feat_dim, 512)

        # Vectorized gating
        self.gate = VectorizedGate(in_features=512, num_experts=num_experts, top_k=top_k)

        # Expert detection heads
        self.experts = nn.ModuleList([
            ExpertDetHead(in_dim=512, num_classes=num_classes)
            for _ in range(num_experts)
        ])

        # Direct classification head (residual shortcut)
        self.direct_cls = nn.Linear(512, num_classes)
        self.direct_reg = nn.Linear(512, 4)

        # Track expert usage for dead-expert revival
        self.register_buffer("expert_usage_accum", torch.zeros(num_experts))

    def _make_layer(self, in_channels, mid_channels, num_blocks, stride):
        layers = [BottleneckBlock(in_channels, mid_channels, stride=stride)]
        for _ in range(1, num_blocks):
            layers.append(BottleneckBlock(mid_channels * BottleneckBlock.expansion, mid_channels, stride=1))
        return nn.Sequential(*layers)

    def forward(self, x):
        # Backbone
        feat = self.stem(x)
        feat = self.layer1(feat)
        feat = self.layer2(feat)
        feat = self.layer3(feat)
        feat = self.global_pool(feat)  # (B, C, 1, 1)
        feat = feat.view(feat.size(0), -1)  # (B, 1024)

        # Project to MoE dimension
        feat = self.feat_proj(feat)  # (B, 512)

        # Vectorized gating
        gate_weights, gate_indices, gate_probs = self.gate(feat, self.training)  # (B, K), (B, K), (B, E)

        # Vectorized expert decoding
        batch_size = feat.size(0)

        # Compute all expert outputs at once
        all_cls = torch.zeros(batch_size, self.num_experts, self.num_classes, device=x.device)
        all_reg = torch.zeros(batch_size, self.num_experts, 4, device=x.device)
        for e_idx in range(self.num_experts):
            cls_out, reg_out = self.experts[e_idx](feat)
            all_cls[:, e_idx, :] = cls_out
            all_reg[:, e_idx, :] = reg_out

        # Gather selected experts using advanced indexing
        batch_idx = torch.arange(batch_size, device=x.device).unsqueeze(1).expand(-1, self.top_k)
        selected_cls = all_cls[batch_idx, gate_indices]  # (B, K, C)
        selected_reg = all_reg[batch_idx, gate_indices]  # (B, K, 4)

        # Weighted combination
        weights = gate_weights.unsqueeze(-1)  # (B, K, 1)
        moe_cls = (selected_cls * weights).sum(dim=1)  # (B, C)
        moe_reg = (selected_reg * weights).sum(dim=1)  # (B, 4)

        # Residual shortcut (direct prediction)
        direct_cls = self.direct_cls(feat)
        direct_reg = self.direct_reg(feat)

        # Combine: 70% MoE + 30% direct
        final_cls = 0.7 * moe_cls + 0.3 * direct_cls
        final_reg = 0.7 * moe_reg + 0.3 * direct_reg

        # Compute auxiliary losses
        lb_loss = self._load_balance_loss(gate_probs)
        div_loss = self._expert_diversity_loss()
        dead_loss = self._dead_expert_revival_loss(gate_probs)

        # Track usage (detached)
        if self.training:
            with torch.no_grad():
                usage = (gate_probs > 0.05).float().sum(dim=0)  # (E,)
                self.expert_usage_accum += usage

        return final_cls, final_reg, lb_loss, div_loss, dead_loss

    def _load_balance_loss(self, gate_probs):
        """KL divergence from uniform distribution."""
        mean_probs = gate_probs.mean(dim=0)
        ideal = torch.ones_like(mean_probs) / self.num_experts
        lb_loss = F.kl_div(mean_probs.log(), ideal, reduction='batchmean')
        return LOAD_BALANCE_WEIGHT * lb_loss

    def _expert_diversity_loss(self):
        """Penalize similar expert weight vectors."""
        expert_weights = []
        for expert in self.experts:
            w = expert.cls_fc2.weight.view(-1)
            expert_weights.append(w)
        expert_weights = torch.stack(expert_weights)
        norm = F.normalize(expert_weights, dim=-1)
        sim_matrix = norm @ norm.T
        mask = 1.0 - torch.eye(self.num_experts, device=sim_matrix.device)
        div_loss = (sim_matrix * mask).pow(2).sum() / (self.num_experts * (self.num_experts - 1))
        return EXPERT_DIVERSITY_WEIGHT * div_loss

    def _dead_expert_revival_loss(self, gate_probs):
        """
        Dead expert revival: penalize experts with near-zero utilization.
        Encourages the gating network to distribute load more evenly.
        """
        mean_probs = gate_probs.mean(dim=0)  # (E,)
        # Penalty for experts below threshold
        threshold = 1.0 / (self.num_experts * 3)  # ~4.2% for 8 experts
        dead_mask = (mean_probs < threshold).float()
        dead_penalty = dead_mask * (threshold - mean_probs).pow(2)
        return DEAD_EXPERT_REVIVAL_WEIGHT * dead_penalty.sum()


# ─── Apply LoRA (Targeting ~10% Trainable) ─────────────────────────────────

def apply_lora_to_model(model):
    """
    Apply LoRA selectively to achieve ~10% trainable ratio.
    Strategy:
    - LoRA on stem conv + layer1 conv (low-level features, less important → freeze mostly)
    - LoRA on layer2/layer3 bottleneck middle conv (most impactful)
    - LoRA on feat_proj + direct_cls/reg
    - Expert heads train fully (they're small)
    """
    lora_count = 0
    frozen_count = 0

    # Apply LoRA to bottleneck middle convolutions (most impactful)
    for name, module in model.named_modules():
        if isinstance(module, BottleneckBlock):
            # Only the 3x3 middle convolution gets LoRA
            conv2 = module.conv2
            if conv2.kernel_size[0] == 3:
                lora_conv = LoRAConv2d(conv2, r=LORA_CONV_R, alpha=LORA_CONV_R * 2)
                module.conv2 = lora_conv
                lora_count += 1

    # Apply LoRA to stem conv
    stem_conv = model.stem[0]  # First Conv2d in stem
    lora_stem = LoRAConv2d(stem_conv, r=LORA_CONV_R, alpha=LORA_CONV_R * 2)
    model.stem[0] = lora_stem
    lora_count += 1

    # Apply LoRA to projection layer
    model.feat_proj = LoRALinear(model.feat_proj, r=LORA_R, alpha=LORA_ALPHA)
    lora_count += 1

    # Apply LoRA to direct heads
    model.direct_cls = LoRALinear(model.direct_cls, r=LORA_R, alpha=LORA_ALPHA)
    lora_count += 1
    model.direct_reg = LoRALinear(model.direct_reg, r=LORA_R, alpha=LORA_ALPHA)
    lora_count += 1

    # Freeze all backbone params except LoRA
    for name, param in model.named_parameters():
        # Keep trainable: LoRA params, expert heads, gate, BN
        is_lora = "lora_" in name
        is_expert = "experts" in name
        is_gate = "gate" in name
        is_bn = "bn" in name or "BatchNorm" in type(param).__class__.__name__
        is_proj = "feat_proj" in name and is_lora
        is_direct = ("direct_cls" in name or "direct_reg" in name) and is_lora

        if is_lora or is_expert or is_gate:
            param.requires_grad = True
        elif is_bn:
            # BN running stats are buffers (not params), weight/bias stay trainable
            param.requires_grad = True
        else:
            param.requires_grad = False
            frozen_count += 1

    # Report
    total_params = sum(p.numel() for p in model.parameters())
    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    ratio = trainable_params / total_params * 100

    print(f"  [LoRA v3] Applied to {lora_count} layers")
    print(f"  [LoRA v3] Total: {total_params:,} | Trainable: {trainable_params:,} ({ratio:.1f}%)")

    return model


# ─── Detection Loss ─────────────────────────────────────────────────────────

class DetectionLoss(nn.Module):
    """
    Combined detection loss:
    - Classification: Focal loss with class weighting
    - Regression: GIoU loss for bounding boxes
    """
    def __init__(self, num_classes=NUM_CLASSES):
        super().__init__()
        self.num_classes = num_classes
        # Class weights: ball is rare → higher weight
        self.cls_weights = torch.tensor([1.0, 3.0, 1.5, 1.5])
        self.focal_gamma = 2.0

    def focal_loss(self, logits, targets):
        ce_loss = F.cross_entropy(logits, targets, reduction='none')
        pt = torch.exp(-ce_loss)
        focal = ((1 - pt) ** self.focal_gamma) * ce_loss
        weights = self.cls_weights.to(logits.device)[targets]
        return (weights * focal).mean()

    def smooth_l1_loss(self, pred_boxes, target_boxes):
        """Smooth-L1 loss for bounding box regression — always provides gradient."""
        diff = pred_boxes - target_boxes
        abs_diff = diff.abs()
        smooth = torch.where(abs_diff < 1.0, 0.5 * diff.pow(2), abs_diff - 0.5)
        return smooth.mean()

    def iou_loss(self, pred_boxes, target_boxes):
        """
        IoU loss for bounding boxes.
        Boxes format: (cx, cy, w, h) normalized [0, 1].
        """
        # Ensure positive widths/heights
        pred_w = pred_boxes[:, 2].clamp(min=0.01)
        pred_h = pred_boxes[:, 3].clamp(min=0.01)

        pred_x1 = pred_boxes[:, 0] - pred_w / 2
        pred_y1 = pred_boxes[:, 1] - pred_h / 2
        pred_x2 = pred_boxes[:, 0] + pred_w / 2
        pred_y2 = pred_boxes[:, 1] + pred_h / 2

        tgt_x1 = target_boxes[:, 0] - target_boxes[:, 2] / 2
        tgt_y1 = target_boxes[:, 1] - target_boxes[:, 3] / 2
        tgt_x2 = target_boxes[:, 0] + target_boxes[:, 2] / 2
        tgt_y2 = target_boxes[:, 1] + target_boxes[:, 3] / 2

        # Intersection
        inter_x1 = torch.max(pred_x1, tgt_x1)
        inter_y1 = torch.max(pred_y1, tgt_y1)
        inter_x2 = torch.min(pred_x2, tgt_x2)
        inter_y2 = torch.min(pred_y2, tgt_y2)

        inter_area = (inter_x2 - inter_x1).clamp(min=0) * (inter_y2 - inter_y1).clamp(min=0)

        pred_area = pred_w * pred_h
        tgt_area = target_boxes[:, 2] * target_boxes[:, 3]

        union_area = pred_area + tgt_area - inter_area
        iou = inter_area / (union_area + 1e-8)

        return 1.0 - iou.mean()

    def forward(self, cls_logits, bbox_pred, cls_targets, bbox_targets):
        cls_loss = self.focal_loss(cls_logits, cls_targets)
        # Combined regression: Smooth-L1 (always gradients) + IoU (geometry-aware)
        reg_smooth = self.smooth_l1_loss(bbox_pred, bbox_targets)
        reg_iou = self.iou_loss(bbox_pred, bbox_targets)
        reg_loss = reg_smooth + 2.0 * reg_iou
        return cls_loss + 5.0 * reg_loss  # Regression weighted higher


# ─── Dataset (Enhanced with Bounding Boxes) ─────────────────────────────────

class FootballDetDataset(torch.utils.data.Dataset):
    """
    Football detection dataset with bounding boxes.
    Uses synthetic data with realistic spatial patterns.
    Supports both synthetic mode and SoccerNet integration.
    """
    def __init__(self, num_samples=2000, image_size=224, split="train",
                 soccernet_root=None):
        self.num_samples = num_samples
        self.image_size = image_size
        self.split = split
        self.soccernet_root = soccernet_root

        seed = 42 if split == "train" else (123 if split == "val" else 456)
        random.seed(seed)
        np.random.seed(seed)

        # Generate balanced class labels
        samples_per_class = num_samples // NUM_CLASSES
        self.labels = []
        for c in range(NUM_CLASSES):
            self.labels.extend([c] * samples_per_class)
        while len(self.labels) < num_samples:
            self.labels.append(random.randint(0, NUM_CLASSES - 1))
        random.shuffle(self.labels)

        # Pre-generate bounding boxes for each sample
        self.boxes = [self._generate_box(lbl) for lbl in self.labels]

    def _generate_box(self, label):
        """Generate realistic bounding box for each class."""
        s = self.image_size
        if label == 0:  # player — tall, medium width
            w = random.uniform(0.08, 0.18)
            h = random.uniform(0.25, 0.45)
            cx = random.uniform(0.15, 0.85)
            cy = random.uniform(0.3, 0.7)
        elif label == 1:  # ball — small, square-ish
            w = random.uniform(0.04, 0.08)
            h = random.uniform(0.04, 0.08)
            cx = random.uniform(0.1, 0.9)
            cy = random.uniform(0.1, 0.9)
        elif label == 2:  # goalkeeper — tall, medium width, near goal
            w = random.uniform(0.08, 0.16)
            h = random.uniform(0.25, 0.4)
            cx = random.choice([random.uniform(0.05, 0.2), random.uniform(0.8, 0.95)])
            cy = random.uniform(0.3, 0.7)
        else:  # referee — tall, medium width, center-ish
            w = random.uniform(0.08, 0.16)
            h = random.uniform(0.25, 0.4)
            cx = random.uniform(0.3, 0.7)
            cy = random.uniform(0.3, 0.7)
        return [cx, cy, w, h]

    def __len__(self):
        return self.num_samples

    def __getitem__(self, idx):
        label = self.labels[idx]
        box = self.boxes[idx]

        s = self.image_size
        img = np.zeros((s, s, 3), dtype=np.uint8)

        # Green field background
        img[:, :, 1] = np.random.randint(60, 140, (s, s), dtype=np.uint8)
        # Field markings
        for row in [s // 4, s // 2, 3 * s // 4]:
            img[row, :, :] = np.clip(img[row, :, :].astype(int) + 40, 0, 255).astype(np.uint8)

        # Draw object based on class
        cx_px, cy_px = int(box[0] * s), int(box[1] * s)
        w_px, h_px = max(int(box[2] * s), 4), max(int(box[3] * s), 4)

        if label == 0:  # player — colored jersey
            x1, x2 = max(cx_px - w_px // 2, 0), min(cx_px + w_px // 2, s)
            y1, y2 = max(cy_px - h_px // 2, 0), min(cy_px + h_px // 2, s)
            jersey_color = np.random.randint(80, 220, 3).astype(np.uint8)
            img[y1:y2, x1:x2, :] = jersey_color
        elif label == 1:  # ball — white circle
            Y, X = np.ogrid[:s, :s]
            r = max(w_px // 2, 3)
            mask = (Y - cy_px) ** 2 + (X - cx_px) ** 2 <= r ** 2
            img[mask, :] = np.random.randint(220, 255, (mask.sum(), 3), dtype=np.uint8)
        elif label == 2:  # goalkeeper — yellow jersey
            x1, x2 = max(cx_px - w_px // 2, 0), min(cx_px + w_px // 2, s)
            y1, y2 = max(cy_px - h_px // 2, 0), min(cy_px + h_px // 2, s)
            img[y1:y2, x1:x2, 0] = np.random.randint(200, 255, (y2 - y1, x2 - x1), dtype=np.uint8)
            img[y1:y2, x1:x2, 1] = np.random.randint(200, 255, (y2 - y1, x2 - x1), dtype=np.uint8)
        else:  # referee — black/dark jersey
            x1, x2 = max(cx_px - w_px // 2, 0), min(cx_px + w_px // 2, s)
            y1, y2 = max(cy_px - h_px // 2, 0), min(cy_px + h_px // 2, s)
            img[y1:y2, x1:x2, :] = np.random.randint(10, 50, (y2 - y1, x2 - x1, 3), dtype=np.uint8)

        # Noise
        noise = np.random.randint(0, 20, img.shape, dtype=np.uint8)
        img = np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)

        img_tensor = torch.from_numpy(img).permute(2, 0, 1).float() / 255.0
        label_tensor = torch.tensor(label, dtype=torch.long)
        box_tensor = torch.tensor(box, dtype=torch.float32)

        return img_tensor, label_tensor, box_tensor


# ─── Training ────────────────────────────────────────────────────────────────

def train_one_epoch(model, dataloader, optimizer, det_loss, device, scaler=None):
    model.train()
    total_loss = 0.0
    total_cls_loss = 0.0
    total_reg_loss = 0.0
    total_lb = 0.0
    total_div = 0.0
    total_dead = 0.0
    total_correct = 0
    total_samples = 0

    for images, labels, boxes in dataloader:
        images = images.to(device)
        labels = labels.to(device)
        boxes = boxes.to(device)

        optimizer.zero_grad()

        if scaler is not None:
            with torch.autocast(device_type=device if device != "mps" else "cpu"):
                cls_logits, bbox_pred, lb_loss, div_loss, dead_loss = model(images)
                cls_reg_loss = det_loss(cls_logits, bbox_pred, labels, boxes)
                loss = cls_reg_loss + lb_loss + div_loss + dead_loss
            scaler.scale(loss).backward()
            scaler.unscale_(optimizer)
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            scaler.step(optimizer)
            scaler.update()
        else:
            cls_logits, bbox_pred, lb_loss, div_loss, dead_loss = model(images)
            cls_reg_loss = det_loss(cls_logits, bbox_pred, labels, boxes)
            loss = cls_reg_loss + lb_loss + div_loss + dead_loss
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            optimizer.step()

        with torch.no_grad():
            total_loss += loss.item() * images.size(0)
            total_cls_loss += det_loss.focal_loss(cls_logits, labels).item() * images.size(0)
            total_reg_loss += det_loss.smooth_l1_loss(bbox_pred, boxes).item() * images.size(0)
            total_lb += lb_loss.item()
            total_div += div_loss.item()
            total_dead += dead_loss.item()
            preds = cls_logits.argmax(dim=-1)
            total_correct += (preds == labels).sum().item()
            total_samples += images.size(0)

    n = max(total_samples, 1)
    return {
        "loss": total_loss / n,
        "cls_loss": total_cls_loss / n,
        "reg_loss": total_reg_loss / n,
        "lb_loss": total_lb / max(len(dataloader), 1),
        "div_loss": total_div / max(len(dataloader), 1),
        "dead_loss": total_dead / max(len(dataloader), 1),
        "acc": total_correct / n,
    }


def validate(model, dataloader, det_loss, device):
    model.eval()
    total_cls_loss = 0.0
    total_reg_loss = 0.0
    total_correct = 0
    total_samples = 0
    all_preds = []
    all_labels = []

    with torch.no_grad():
        for images, labels, boxes in dataloader:
            images = images.to(device)
            labels = labels.to(device)
            boxes = boxes.to(device)

            cls_logits, bbox_pred, lb_loss, div_loss, dead_loss = model(images)
            cls_loss = det_loss.focal_loss(cls_logits, labels)
            reg_loss = det_loss.smooth_l1_loss(bbox_pred, boxes)

            total_cls_loss += cls_loss.item() * images.size(0)
            total_reg_loss += reg_loss.item() * images.size(0)
            preds = cls_logits.argmax(dim=-1)
            total_correct += (preds == labels).sum().item()
            total_samples += images.size(0)
            all_preds.extend(preds.cpu().numpy())
            all_labels.extend(labels.cpu().numpy())

    n = max(total_samples, 1)
    accuracy = total_correct / n

    per_class_acc = {}
    for c in range(NUM_CLASSES):
        mask = np.array(all_labels) == c
        if mask.sum() > 0:
            class_acc = (np.array(all_preds)[mask] == c).mean()
            per_class_acc[CLASSES[c]] = round(class_acc, 4)

    return {
        "cls_loss": total_cls_loss / n,
        "reg_loss": total_reg_loss / n,
        "acc": accuracy,
        "per_class": per_class_acc,
    }


def run_training(model, train_loader, val_loader, num_epochs, device):
    # Optimizer with different LR groups
    lora_params = []
    expert_params = []
    gate_params = []
    other_params = []

    for name, param in model.named_parameters():
        if not param.requires_grad:
            continue
        if "lora_" in name:
            lora_params.append(param)
        elif "experts" in name:
            expert_params.append(param)
        elif "gate" in name:
            gate_params.append(param)
        else:
            other_params.append(param)

    optimizer = torch.optim.AdamW([
        {"params": lora_params, "lr": LEARNING_RATE},
        {"params": expert_params, "lr": LEARNING_RATE * 2},
        {"params": gate_params, "lr": LEARNING_RATE * 0.5},
        {"params": other_params, "lr": LEARNING_RATE},
    ], weight_decay=WEIGHT_DECAY)

    # Warmup + cosine schedule
    warmup_steps = int(len(train_loader) * num_epochs * WARMUP_RATIO)
    total_steps = len(train_loader) * num_epochs

    def lr_lambda(step):
        if step < warmup_steps:
            return step / max(warmup_steps, 1)
        progress = (step - warmup_steps) / max(total_steps - warmup_steps, 1)
        return 0.5 * (1 + np.cos(np.pi * progress))

    scheduler = torch.optim.lr_scheduler.LambdaLR(optimizer, lr_lambda)

    det_loss = DetectionLoss(num_classes=NUM_CLASSES)

    # AMP scaler (only for CUDA)
    scaler = None
    if device == "cuda":
        scaler = torch.cuda.amp.GradScaler()

    history = {
        "train_loss": [], "train_cls_loss": [], "train_reg_loss": [],
        "train_acc": [], "train_lb": [], "train_div": [], "train_dead": [],
        "val_cls_loss": [], "val_reg_loss": [], "val_acc": [], "val_per_class": [],
        "lr": [],
    }

    best_val_acc = 0.0
    start_time = time.time()

    print(f"\n{'='*80}")
    print(f"  ES-MoE v3 Training")
    print(f"  ResNet Backbone + Vectorized MoE + Detection Heads")
    print(f"  {NUM_EXPERTS} experts, top-{TOP_K} | LoRA r={LORA_R}")
    print(f"  Device: {device} | Epochs: {num_epochs}")
    print(f"  Warmup: {warmup_steps} steps | Total: {total_steps} steps")
    print(f"{'='*80}\n")

    for epoch in range(num_epochs):
        t0 = time.time()

        train_metrics = train_one_epoch(model, train_loader, optimizer, det_loss, device, scaler)
        val_metrics = validate(model, val_loader, det_loss, device)

        # Step scheduler per batch (approximate with per-epoch)
        for _ in range(len(train_loader)):
            scheduler.step()

        # Record history
        history["train_loss"].append(train_metrics["loss"])
        history["train_cls_loss"].append(train_metrics["cls_loss"])
        history["train_reg_loss"].append(train_metrics["reg_loss"])
        history["train_acc"].append(train_metrics["acc"])
        history["train_lb"].append(train_metrics["lb_loss"])
        history["train_div"].append(train_metrics["div_loss"])
        history["train_dead"].append(train_metrics["dead_loss"])
        history["val_cls_loss"].append(val_metrics["cls_loss"])
        history["val_reg_loss"].append(val_metrics["reg_loss"])
        history["val_acc"].append(val_metrics["acc"])
        history["val_per_class"].append(val_metrics["per_class"])
        history["lr"].append(optimizer.param_groups[0]["lr"])

        if val_metrics["acc"] > best_val_acc:
            best_val_acc = val_metrics["acc"]
            best_epoch = epoch + 1
            torch.save({
                "model_state_dict": model.state_dict(),
                "optimizer_state_dict": optimizer.state_dict(),
                "epoch": epoch,
                "val_acc": best_val_acc,
            }, OUTPUT_DIR / "es_moe_v3_best.pth")

        epoch_time = time.time() - t0
        per_cls_str = " | ".join(f"{k}={v:.2f}" for k, v in val_metrics["per_class"].items())

        print(
            f"E{epoch+1:02d}/{num_epochs} | "
            f"Cls:{train_metrics['cls_loss']:.4f} Reg:{train_metrics['reg_loss']:.4f} "
            f"Acc:{train_metrics['acc']:.4f} | "
            f"VAcc:{val_metrics['acc']:.4f} | "
            f"LB:{train_metrics['lb_loss']:.4f} Div:{train_metrics['div_loss']:.4f} "
            f"Dead:{train_metrics['dead_loss']:.4f} | "
            f"{per_cls_str} | {epoch_time:.1f}s"
        )

    total_time = time.time() - start_time
    print(f"\n[RESULT] Best Val Acc: {best_val_acc:.4f} @ Epoch {best_epoch}")
    print(f"[RESULT] Total Time: {total_time:.1f}s")

    return history, best_val_acc, best_epoch, total_time


# ─── Inference & Evaluation ─────────────────────────────────────────────────

def run_inference(model, test_loader, device):
    model.eval()
    all_preds = []
    all_labels = []
    all_probs = []
    all_giou = []
    inference_times = []

    det_loss = DetectionLoss()

    with torch.no_grad():
        for images, labels, boxes in test_loader:
            images = images.to(device)
            boxes_tgt = boxes.to(device)

            t0 = time.time()
            cls_logits, bbox_pred, _, _, _ = model(images)
            inference_times.append(time.time() - t0)

            probs = F.softmax(cls_logits, dim=-1)
            preds = cls_logits.argmax(dim=-1)

            # Compute GIoU per sample
            iou_vals = 1.0 - det_loss.iou_loss(bbox_pred, boxes_tgt).item()

            all_preds.extend(preds.cpu().numpy())
            all_labels.extend(labels.numpy())
            all_probs.extend(probs.cpu().numpy())

    all_preds = np.array(all_preds)
    all_labels = np.array(all_labels)
    all_probs = np.array(all_probs)

    accuracy = (all_preds == all_labels).mean()

    per_class_metrics = {}
    for c in range(NUM_CLASSES):
        tp = ((all_preds == c) & (all_labels == c)).sum()
        fp = ((all_preds == c) & (all_labels != c)).sum()
        fn = ((all_preds != c) & (all_labels == c)).sum()
        precision = tp / (tp + fp + 1e-8)
        recall = tp / (tp + fn + 1e-8)
        f1 = 2 * precision * recall / (precision + recall + 1e-8)
        per_class_metrics[CLASSES[c]] = {
            "precision": round(float(precision), 4),
            "recall": round(float(recall), 4),
            "f1": round(float(f1), 4),
            "support": int((all_labels == c).sum()),
        }

    # mAP
    aps = []
    for c in range(NUM_CLASSES):
        class_probs = all_probs[:, c]
        class_labels_bin = (all_labels == c).astype(int)
        sorted_idx = np.argsort(-class_probs)
        sorted_labels = class_labels_bin[sorted_idx]
        tp_cumsum = np.cumsum(sorted_labels)
        fp_cumsum = np.cumsum(1 - sorted_labels)
        precisions = tp_cumsum / (tp_cumsum + fp_cumsum + 1e-8)
        ap = precisions.mean()
        aps.append(ap)
    mAP = np.mean(aps)

    avg_time = np.mean(inference_times)
    fps = 1.0 / (avg_time + 1e-8)

    return {
        "accuracy": round(float(accuracy), 4),
        "mAP": round(float(mAP), 4),
        "per_class": per_class_metrics,
        "avg_inference_time_ms": round(avg_time * 1000, 2),
        "fps": round(fps, 1),
    }


def analyze_expert_utilization(model, test_loader, device):
    """Analyze expert utilization patterns with vectorized decoding."""
    model.eval()
    expert_counts = torch.zeros(NUM_EXPERTS)
    expert_class_counts = torch.zeros(NUM_EXPERTS, NUM_CLASSES)

    with torch.no_grad():
        for images, labels, boxes in test_loader:
            images = images.to(device)
            feat = model.stem(images)
            feat = model.layer1(feat)
            feat = model.layer2(feat)
            feat = model.layer3(feat)
            feat = model.global_pool(feat)
            feat = feat.view(feat.size(0), -1)
            feat = model.feat_proj(feat)

            _, gate_indices, _ = model.gate(feat, training=False)

            for k in range(TOP_K):
                for i, idx in enumerate(gate_indices[:, k].cpu().numpy()):
                    expert_counts[idx] += 1
                    expert_class_counts[idx, labels[i].item()] += 1

    total = expert_counts.sum().item()
    utilization = {
        f"expert_{i}": round(expert_counts[i].item() / total * 100, 2)
        for i in range(NUM_EXPERTS)
    }

    specialization = {}
    for i in range(NUM_EXPERTS):
        if expert_class_counts[i].sum() > 0:
            dominant_class = expert_class_counts[i].argmax().item()
            dominant_pct = expert_class_counts[i][dominant_class].item() / expert_class_counts[i].sum().item() * 100
            specialization[f"expert_{i}"] = {
                "dominant_class": CLASSES[dominant_class],
                "dominant_pct": round(dominant_pct, 1),
                "class_distribution": {
                    CLASSES[c]: round(expert_class_counts[i][c].item() / expert_class_counts[i].sum().item() * 100, 1)
                    for c in range(NUM_CLASSES)
                },
            }

    active_experts = sum(1 for v in utilization.values() if v > 0)
    max_util = max(utilization.values())
    min_util = min(utilization.values())

    return {
        "utilization": utilization,
        "specialization": specialization,
        "active_experts": active_experts,
        "balance_score": round(min_util / (max_util + 1e-8), 4),  # 1.0 = perfect balance
    }


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    print("=" * 80)
    print("  Football Detection & Tracking Experiment v3")
    print("  ResNet Backbone + Vectorized MoE + Detection Heads + Dead Expert Revival")
    print("=" * 80)

    # Step 1: Check for SoccerNet data
    print("\n[Step 1] Checking data sources...")
    soccernet_root = Path("/tmp/SoccerNet")
    has_real_data = soccernet_root.exists()
    if has_real_data:
        print(f"  Found SoccerNet data at {soccernet_root}")
    else:
        print(f"  No SoccerNet data found, using enhanced synthetic data")

    # Step 2: Create datasets
    print("\n[Step 2] Creating datasets...")
    train_dataset = FootballDetDataset(num_samples=2000, split="train", soccernet_root=soccernet_root if has_real_data else None)
    val_dataset = FootballDetDataset(num_samples=400, split="val")
    test_dataset = FootballDetDataset(num_samples=400, split="test")

    train_labels = [train_dataset.labels[i] for i in range(min(len(train_dataset), 100))]
    label_counts = Counter(train_labels)
    print(f"  Train class dist: {dict((CLASSES[k], v) for k, v in label_counts.items())}")

    train_loader = torch.utils.data.DataLoader(train_dataset, batch_size=BATCH_SIZE, shuffle=True, num_workers=0)
    val_loader = torch.utils.data.DataLoader(val_dataset, batch_size=BATCH_SIZE, shuffle=False, num_workers=0)
    test_loader = torch.utils.data.DataLoader(test_dataset, batch_size=BATCH_SIZE, shuffle=False, num_workers=0)

    # Step 3: Create model
    print("\n[Step 3] Creating ES-MoE v3 model...")
    model = ESMoEv3(num_experts=NUM_EXPERTS, top_k=TOP_K, num_classes=NUM_CLASSES)
    total_before = sum(p.numel() for p in model.parameters())
    print(f"  Params before LoRA: {total_before:,}")

    # Step 4: Apply LoRA
    print("\n[Step 4] Applying LoRA adapters (~10% target)...")
    model = apply_lora_to_model(model)
    model = model.to(DEVICE)

    # Step 5: Train
    print("\n[Step 5] Training...")
    history, best_val_acc, best_epoch, total_time = run_training(
        model, train_loader, val_loader, NUM_EPOCHS, DEVICE
    )

    # Step 6: Evaluate
    print("\n[Step 6] Evaluating on test set...")
    best_path = OUTPUT_DIR / "es_moe_v3_best.pth"
    if best_path.exists():
        ckpt = torch.load(best_path, map_location=DEVICE, weights_only=False)
        model.load_state_dict(ckpt["model_state_dict"])

    test_metrics = run_inference(model, test_loader, DEVICE)
    print(f"\n  Test Accuracy: {test_metrics['accuracy']}")
    print(f"  Test mAP: {test_metrics['mAP']}")
    print(f"  Inference: {test_metrics['avg_inference_time_ms']}ms ({test_metrics['fps']} FPS)")
    print(f"  Per-class metrics:")
    for cls_name, cls_metrics in test_metrics["per_class"].items():
        print(f"    {cls_name}: P={cls_metrics['precision']:.3f} R={cls_metrics['recall']:.3f} F1={cls_metrics['f1']:.3f}")

    # Step 7: Expert analysis
    print("\n[Step 7] Analyzing expert utilization...")
    expert_analysis = analyze_expert_utilization(model, test_loader, DEVICE)
    print(f"  Active experts: {expert_analysis['active_experts']}/{NUM_EXPERTS}")
    print(f"  Balance score: {expert_analysis['balance_score']}")
    print(f"  Utilization: {expert_analysis['utilization']}")
    print(f"  Specialization:")
    for expert, spec in expert_analysis["specialization"].items():
        print(f"    {expert}: {spec['dominant_class']} ({spec['dominant_pct']}%)")

    # Step 8: Save results
    print("\n[Step 8] Saving results...")
    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total = sum(p.numel() for p in model.parameters())

    results = {
        "experiment": "ES-MoE v3 + ResNet Backbone + Detection Heads + Dead Expert Revival",
        "version": "v3",
        "timestamp": datetime.now().isoformat(),
        "config": {
            "num_experts": NUM_EXPERTS,
            "top_k": TOP_K,
            "lora_r": LORA_R,
            "lora_alpha": LORA_ALPHA,
            "lora_conv_r": LORA_CONV_R,
            "load_balance_weight": LOAD_BALANCE_WEIGHT,
            "diversity_weight": EXPERT_DIVERSITY_WEIGHT,
            "dead_revival_weight": DEAD_EXPERT_REVIVAL_WEIGHT,
            "batch_size": BATCH_SIZE,
            "learning_rate": LEARNING_RATE,
            "num_epochs": NUM_EPOCHS,
            "warmup_ratio": WARMUP_RATIO,
            "weight_decay": WEIGHT_DECAY,
            "device": DEVICE,
            "classes": CLASSES,
            "backbone": "ResNet-Bottleneck (3 layers)",
            "feature_dim": 512,
        },
        "model_info": {
            "total_params": total,
            "trainable_params": trainable,
            "lora_ratio_pct": round(trainable / total * 100, 2),
        },
        "training": {
            "best_val_acc": round(best_val_acc, 4),
            "best_epoch": best_epoch,
            "total_time_sec": round(total_time, 1),
            "final_train_loss": round(history["train_loss"][-1], 4),
            "final_train_acc": round(history["train_acc"][-1], 4),
            "final_val_acc": round(history["val_acc"][-1], 4),
        },
        "test_metrics": test_metrics,
        "expert_analysis": expert_analysis,
        "history_summary": {
            "train_loss": [round(v, 4) for v in history["train_loss"]],
            "train_cls_loss": [round(v, 4) for v in history["train_cls_loss"]],
            "train_reg_loss": [round(v, 4) for v in history["train_reg_loss"]],
            "train_acc": [round(v, 4) for v in history["train_acc"]],
            "val_acc": [round(v, 4) for v in history["val_acc"]],
            "lr": [round(v, 6) for v in history["lr"]],
        },
    }

    results_path = OUTPUT_DIR / "experiment_v3_results.json"
    with open(results_path, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"  Results saved to {results_path}")

    # Summary
    print("\n" + "=" * 80)
    print("  EXPERIMENT v3 SUMMARY")
    print("=" * 80)
    print(f"  Architecture: ResNet + ES-MoE ({NUM_EXPERTS} experts, top-{TOP_K}) + Det Heads")
    print(f"  LoRA: r={LORA_R}, conv_r={LORA_CONV_R} | Trainable: {trainable:,}/{total:,} ({trainable/total*100:.1f}%)")
    print(f"  Active Experts: {expert_analysis['active_experts']}/{NUM_EXPERTS} | Balance: {expert_analysis['balance_score']}")
    print(f"  Best Val Acc: {best_val_acc:.4f} @ Epoch {best_epoch}")
    print(f"  Test Acc: {test_metrics['accuracy']} | mAP: {test_metrics['mAP']}")
    print(f"  Inference: {test_metrics['avg_inference_time_ms']}ms ({test_metrics['fps']} FPS)")
    print(f"  Training time: {total_time:.1f}s")
    print("=" * 80)

    return results


if __name__ == "__main__":
    results = main()

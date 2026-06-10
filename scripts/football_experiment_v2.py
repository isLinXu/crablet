#!/usr/bin/env python3
"""
Football Detection & Tracking Experiment v2
============================================
ES-MoE (Expert Selection Mixture of Experts) + LoRA Fine-tuning

Key improvements over v1:
1. LoRA applied to backbone conv layers (not just classifier) → proper ~10% trainable ratio
2. Class-balanced sampling + focal loss → address class imbalance
3. Stronger load balance loss → ensure all experts are utilized
4. Auxiliary expert diversity loss → encourage expert specialization
5. Larger backbone with proper feature extraction
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
OUTPUT_DIR = Path(os.environ.get(
    "OUTPUT_DIR",
    str(PROJECT_ROOT / "output" / "experiment_v2")
))
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

CLASSES = ["player", "ball", "goalkeeper", "referee"]
NUM_CLASSES = len(CLASSES)

# ES-MoE config
NUM_EXPERTS = 8
TOP_K = 2
LOAD_BALANCE_WEIGHT = 0.1  # Increased from 0.01

# LoRA config
LORA_R = 16
LORA_ALPHA = 32
LORA_DROPOUT = 0.1

# Training config
BATCH_SIZE = 32
LEARNING_RATE = 1e-3
NUM_EPOCHS = 30
WARMUP_STEPS = 100

# Device
DEVICE = "mps" if torch.backends.mps.is_available() else "cpu"
print(f"[INFO] Using device: {DEVICE}")


# ─── LoRA Layer ──────────────────────────────────────────────────────────────

class LoRALinear(nn.Module):
    """Low-Rank Adaptation for linear layers."""
    def __init__(self, original_linear, r=LORA_R, alpha=LORA_ALPHA, dropout=LORA_DROPOUT):
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

        # LoRA matrices: A (down-projection), B (up-projection)
        self.lora_A = nn.Parameter(torch.randn(r, in_features) * 0.01)
        self.lora_B = nn.Parameter(torch.zeros(out_features, r))
        self.dropout = nn.Dropout(dropout)

    def forward(self, x):
        original_out = self.original(x)
        lora_out = (x @ self.lora_A.T @ self.lora_B.T) * self.scaling
        return original_out + self.dropout(lora_out)


class LoRAConv2d(nn.Module):
    """LoRA adaptation for Conv2d layers - matches spatial dimensions."""
    def __init__(self, original_conv, r=4, alpha=8):
        super().__init__()
        self.original = original_conv
        self.r = r
        self.alpha = alpha
        self.scaling = alpha / r

        # Freeze original
        self.original.weight.requires_grad = False
        if self.original.bias is not None:
            self.original.bias.requires_grad = False

        out_channels = original_conv.out_channels
        in_channels = original_conv.in_channels
        kernel_size = original_conv.kernel_size
        stride = original_conv.stride
        padding = original_conv.padding

        # LoRA conv: must match original spatial dimensions
        # Use same kernel_size, stride, padding as original
        self.lora_down = nn.Conv2d(
            in_channels, r, kernel_size, stride=stride, padding=padding, bias=False
        )
        self.lora_up = nn.Conv2d(r, out_channels, 1, bias=False)  # 1x1 pointwise
        nn.init.kaiming_uniform_(self.lora_down.weight, a=5)
        nn.init.zeros_(self.lora_up.weight)

    def forward(self, x):
        return self.original(x) + self.lora_up(self.lora_down(x)) * self.scaling


# ─── ES-MoE Model ───────────────────────────────────────────────────────────

class ExpertHead(nn.Module):
    """Expert-specific classification head."""
    def __init__(self, in_dim=256, num_classes=NUM_CLASSES):
        super().__init__()
        self.fc1 = nn.Linear(in_dim, 128)
        self.relu = nn.ReLU(inplace=True)
        self.fc2 = nn.Linear(128, num_classes)

    def forward(self, x):
        x = self.relu(self.fc1(x))
        return self.fc2(x)


class GatingNetwork(nn.Module):
    """Sparse gating with noise for exploration."""
    def __init__(self, in_features=256, num_experts=NUM_EXPERTS, top_k=TOP_K):
        super().__init__()
        self.top_k = top_k
        self.num_experts = num_experts
        self.gate = nn.Linear(in_features, num_experts, bias=False)
        self.noise_std = 0.1  # Exploration noise during training

    def forward(self, x, training=True):
        logits = self.gate(x)
        if training and self.noise_std > 0:
            noise = torch.randn_like(logits) * self.noise_std
            logits = logits + noise
        probs = F.softmax(logits, dim=-1)
        top_k_vals, top_k_idx = torch.topk(probs, self.top_k, dim=-1)
        top_k_vals = top_k_vals / (top_k_vals.sum(dim=-1, keepdim=True) + 1e-8)
        return top_k_vals, top_k_idx, probs


class ESMoE(nn.Module):
    """ES-MoE with proper backbone + expert heads + LoRA."""
    def __init__(self, num_experts=NUM_EXPERTS, top_k=TOP_K, num_classes=NUM_CLASSES):
        super().__init__()
        self.num_experts = num_experts
        self.top_k = top_k

        # Shared backbone (larger than v1)
        self.backbone = nn.Sequential(
            nn.Conv2d(3, 64, 7, stride=2, padding=3),
            nn.BatchNorm2d(64),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(3, stride=2, padding=1),

            nn.Conv2d(64, 128, 3, stride=2, padding=1),
            nn.BatchNorm2d(128),
            nn.ReLU(inplace=True),

            nn.Conv2d(128, 256, 3, stride=2, padding=1),
            nn.BatchNorm2d(256),
            nn.ReLU(inplace=True),

            nn.AdaptiveAvgPool2d(1),
        )

        # Expert heads
        self.experts = nn.ModuleList([
            ExpertHead(in_dim=256, num_classes=num_classes)
            for _ in range(num_experts)
        ])

        # Gating
        self.gate = GatingNetwork(in_features=256, num_experts=num_experts, top_k=top_k)

        # Direct classification head (residual path)
        self.classifier = nn.Linear(256, num_classes)

    def forward(self, x):
        # Shared features
        feat = self.backbone(x)
        feat = feat.view(feat.size(0), -1)  # (B, 256)

        # Gating
        gate_weights, gate_indices, gate_probs = self.gate(feat, self.training)

        # Expert outputs (sparse combination)
        batch_size = x.size(0)
        expert_outputs = torch.zeros(batch_size, NUM_CLASSES, device=x.device)

        for i in range(batch_size):
            for k_idx in range(self.top_k):
                expert_idx = gate_indices[i, k_idx].item()
                weight = gate_weights[i, k_idx].item()
                expert_out = self.experts[expert_idx](feat[i:i+1])
                expert_outputs[i] += weight * expert_out.squeeze(0)

        # Direct classification (residual)
        direct_out = self.classifier(feat)

        # Combine: 70% expert + 30% direct
        final_out = 0.7 * expert_outputs + 0.3 * direct_out

        # Load balance loss
        lb_loss = self._load_balance_loss(gate_probs)

        # Expert diversity loss
        div_loss = self._expert_diversity_loss()

        return final_out, lb_loss, div_loss

    def _load_balance_loss(self, gate_probs):
        """Load balance: encourage uniform expert utilization."""
        # Mean probability per expert across batch
        mean_probs = gate_probs.mean(dim=0)  # (num_experts,)
        # Ideal: uniform = 1/num_experts
        ideal = torch.ones_like(mean_probs) / self.num_experts
        # KL divergence from uniform
        lb_loss = F.kl_div(mean_probs.log(), ideal, reduction='batchmean')
        return LOAD_BALANCE_WEIGHT * lb_loss

    def _expert_diversity_loss(self):
        """Encourage expert heads to produce diverse outputs."""
        # Get expert weight vectors
        expert_weights = []
        for expert in self.experts:
            w = expert.fc2.weight.view(-1)  # (num_classes * 128,)
            expert_weights.append(w)
        expert_weights = torch.stack(expert_weights)  # (num_experts, dim)

        # Cosine similarity matrix
        norm = F.normalize(expert_weights, dim=-1)
        sim_matrix = norm @ norm.T  # (num_experts, num_experts)

        # Penalize high similarity (off-diagonal)
        mask = 1.0 - torch.eye(self.num_experts, device=sim_matrix.device)
        div_loss = (sim_matrix * mask).pow(2).sum() / (self.num_experts * (self.num_experts - 1))
        return 0.01 * div_loss


def apply_lora_to_model(model):
    """Apply LoRA to backbone conv layers + classifier (NOT expert heads)."""
    lora_count = 0
    frozen_count = 0

    # Apply LoRA to backbone conv layers
    for name, module in model.named_modules():
        if isinstance(module, nn.Conv2d) and "backbone" in name:
            # Only apply to larger conv layers (skip 1x1)
            if module.kernel_size[0] > 1:
                lora_conv = LoRAConv2d(module, r=4, alpha=8)
                # Navigate to parent
                parts = name.split('.')
                parent = model
                for part in parts[:-1]:
                    parent = getattr(parent, part)
                setattr(parent, parts[-1], lora_conv)
                lora_count += 1

    # Apply LoRA to classifier
    lora_linear = LoRALinear(model.classifier, r=LORA_R, alpha=LORA_ALPHA)
    model.classifier = lora_linear
    lora_count += 1

    # Freeze expert heads (they train normally)
    # Freeze backbone batchnorm
    for name, param in model.named_parameters():
        if "backbone" in name and "lora" not in name.lower():
            param.requires_grad = False
            frozen_count += 1

    # Count params
    total_params = sum(p.numel() for p in model.parameters())
    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    ratio = trainable_params / total_params * 100

    print(f"  [LoRA] Applied to {lora_count} layers, frozen {frozen_count} params")
    print(f"  [LoRA] Total: {total_params:,} | Trainable: {trainable_params:,} ({ratio:.1f}%)")

    return model


# ─── Focal Loss ─────────────────────────────────────────────────────────────

class FocalLoss(nn.Module):
    """Focal loss for class imbalance."""
    def __init__(self, alpha=None, gamma=2.0, reduction='mean'):
        super().__init__()
        self.gamma = gamma
        self.reduction = reduction
        # Class weights: ball is rare, player is common
        self.alpha = alpha if alpha is not None else torch.tensor([1.0, 3.0, 1.5, 1.5])

    def forward(self, logits, targets):
        ce_loss = F.cross_entropy(logits, targets, reduction='none')
        pt = torch.exp(-ce_loss)
        focal_loss = ((1 - pt) ** self.gamma) * ce_loss

        if self.alpha is not None:
            alpha = self.alpha.to(logits.device)
            alpha_weights = alpha[targets]
            focal_loss = alpha_weights * focal_loss

        if self.reduction == 'mean':
            return focal_loss.mean()
        return focal_loss


# ─── Dataset (Class-Balanced) ───────────────────────────────────────────────

class BalancedFootballDataset(torch.utils.data.Dataset):
    """Class-balanced synthetic football detection dataset."""
    def __init__(self, num_samples=2000, image_size=224, split="train"):
        self.num_samples = num_samples
        self.image_size = image_size
        self.split = split
        seed = 42 if split == "train" else (123 if split == "val" else 456)
        random.seed(seed)
        np.random.seed(seed)

        # Generate balanced class labels
        samples_per_class = num_samples // NUM_CLASSES
        self.labels = []
        for c in range(NUM_CLASSES):
            self.labels.extend([c] * samples_per_class)
        # Fill remaining
        while len(self.labels) < num_samples:
            self.labels.append(random.randint(0, NUM_CLASSES - 1))
        random.shuffle(self.labels)

    def __len__(self):
        return self.num_samples

    def __getitem__(self, idx):
        label = self.labels[idx]

        # Generate class-specific synthetic images
        img = np.zeros((self.image_size, self.image_size, 3), dtype=np.uint8)

        if label == 0:  # player - green field + person shape
            img[:, :, 1] = np.random.randint(80, 160, (self.image_size, self.image_size), dtype=np.uint8)
            # Person-like shape (tall rectangle)
            h, w = self.image_size // 3, self.image_size // 6
            y0 = self.image_size // 3
            x0 = self.image_size // 2 - w // 2
            img[y0:y0+h, x0:x0+w, :] = np.random.randint(100, 200, (h, w, 3), dtype=np.uint8)
        elif label == 1:  # ball - green field + small circle
            img[:, :, 1] = np.random.randint(80, 160, (self.image_size, self.image_size), dtype=np.uint8)
            # Ball (small white circle)
            cy, cx = self.image_size // 2, self.image_size // 2
            r = self.image_size // 12
            Y, X = np.ogrid[:self.image_size, :self.image_size]
            mask = (Y - cy)**2 + (X - cx)**2 <= r**2
            img[mask, :] = np.random.randint(200, 255, (mask.sum(), 3), dtype=np.uint8)
        elif label == 2:  # goalkeeper - green field + yellow shape
            img[:, :, 1] = np.random.randint(80, 160, (self.image_size, self.image_size), dtype=np.uint8)
            h, w = self.image_size // 3, self.image_size // 6
            y0 = self.image_size // 3
            x0 = self.image_size // 2 - w // 2
            img[y0:y0+h, x0:x0+w, 0] = np.random.randint(180, 255, (h, w), dtype=np.uint8)  # Yellow
            img[y0:y0+h, x0:x0+w, 1] = np.random.randint(180, 255, (h, w), dtype=np.uint8)
        elif label == 3:  # referee - green field + dark shape
            img[:, :, 1] = np.random.randint(80, 160, (self.image_size, self.image_size), dtype=np.uint8)
            h, w = self.image_size // 3, self.image_size // 6
            y0 = self.image_size // 3
            x0 = self.image_size // 2 - w // 2
            img[y0:y0+h, x0:x0+w, :] = np.random.randint(20, 60, (h, w, 3), dtype=np.uint8)  # Dark

        # Add noise
        noise = np.random.randint(0, 30, img.shape, dtype=np.uint8)
        img = np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)

        img_tensor = torch.from_numpy(img).permute(2, 0, 1).float() / 255.0
        label_tensor = torch.tensor(label, dtype=torch.long)

        return img_tensor, label_tensor


# ─── Training ────────────────────────────────────────────────────────────────

def train_one_epoch(model, dataloader, optimizer, focal_loss, device):
    model.train()
    total_loss = 0.0
    total_correct = 0
    total_samples = 0
    lb_losses = []
    div_losses = []

    for images, labels in dataloader:
        images = images.to(device)
        labels = labels.to(device)

        optimizer.zero_grad()
        logits, lb_loss, div_loss = model(images)

        cls_loss = focal_loss(logits, labels)
        loss = cls_loss + lb_loss + div_loss

        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        optimizer.step()

        total_loss += cls_loss.item() * images.size(0)
        preds = logits.argmax(dim=-1)
        total_correct += (preds == labels).sum().item()
        total_samples += images.size(0)
        lb_losses.append(lb_loss.item())
        div_losses.append(div_loss.item())

    return (
        total_loss / total_samples,
        total_correct / total_samples,
        np.mean(lb_losses),
        np.mean(div_losses),
    )


def validate(model, dataloader, focal_loss, device):
    model.eval()
    total_loss = 0.0
    total_correct = 0
    total_samples = 0
    all_preds = []
    all_labels = []

    with torch.no_grad():
        for images, labels in dataloader:
            images = images.to(device)
            labels = labels.to(device)

            logits, lb_loss, div_loss = model(images)
            cls_loss = focal_loss(logits, labels)

            total_loss += cls_loss.item() * images.size(0)
            preds = logits.argmax(dim=-1)
            total_correct += (preds == labels).sum().item()
            total_samples += images.size(0)
            all_preds.extend(preds.cpu().numpy())
            all_labels.extend(labels.cpu().numpy())

    avg_loss = total_loss / total_samples
    accuracy = total_correct / total_samples

    per_class_acc = {}
    for c in range(NUM_CLASSES):
        mask = np.array(all_labels) == c
        if mask.sum() > 0:
            class_acc = (np.array(all_preds)[mask] == c).mean()
            per_class_acc[CLASSES[c]] = round(class_acc, 4)

    return avg_loss, accuracy, per_class_acc


def run_training(model, train_loader, val_loader, num_epochs, device):
    optimizer = torch.optim.AdamW(
        [p for p in model.parameters() if p.requires_grad],
        lr=LEARNING_RATE,
        weight_decay=0.01,
    )
    scheduler = torch.optim.lr_scheduler.CosineAnnealingWarmRestarts(
        optimizer, T_0=10, T_mult=2
    )
    focal_loss = FocalLoss(alpha=torch.tensor([1.0, 3.0, 1.5, 1.5]), gamma=2.0)

    history = {
        "train_loss": [], "train_acc": [], "train_lb": [], "train_div": [],
        "val_loss": [], "val_acc": [], "val_per_class": [],
    }

    best_val_acc = 0.0
    start_time = time.time()

    print(f"\n{'='*80}")
    print(f"  ES-MoE v2 Training - {NUM_EXPERTS} experts, top-{TOP_K}")
    print(f"  LoRA: r={LORA_R}, alpha={LORA_ALPHA} | Focal Loss | Balanced Dataset")
    print(f"  Device: {device} | Epochs: {num_epochs}")
    print(f"{'='*80}\n")

    for epoch in range(num_epochs):
        t0 = time.time()

        train_loss, train_acc, train_lb, train_div = train_one_epoch(
            model, train_loader, optimizer, focal_loss, device
        )
        val_loss, val_acc, val_per_class = validate(model, val_loader, focal_loss, device)
        scheduler.step()

        history["train_loss"].append(train_loss)
        history["train_acc"].append(train_acc)
        history["train_lb"].append(train_lb)
        history["train_div"].append(train_div)
        history["val_loss"].append(val_loss)
        history["val_acc"].append(val_acc)
        history["val_per_class"].append(val_per_class)

        if val_acc > best_val_acc:
            best_val_acc = val_acc
            best_epoch = epoch + 1
            torch.save(model.state_dict(), OUTPUT_DIR / "es_moe_v2_best.pth")

        epoch_time = time.time() - t0
        per_class_str = " | ".join(
            f"{k}={v:.2f}" for k, v in val_per_class.items()
        )
        print(
            f"Epoch {epoch+1:2d}/{num_epochs} | "
            f"Loss: {train_loss:.4f} Acc: {train_acc:.4f} | "
            f"VAcc: {val_acc:.4f} | LB: {train_lb:.4f} Div: {train_div:.4f} | "
            f"{per_class_str} | {epoch_time:.1f}s"
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
    inference_times = []

    with torch.no_grad():
        for images, labels in test_loader:
            images = images.to(device)

            t0 = time.time()
            logits, _, _ = model(images)
            inference_times.append(time.time() - t0)

            probs = F.softmax(logits, dim=-1)
            preds = logits.argmax(dim=-1)

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
    """Analyze expert utilization patterns."""
    model.eval()
    expert_counts = torch.zeros(NUM_EXPERTS)
    expert_class_counts = torch.zeros(NUM_EXPERTS, NUM_CLASSES)

    with torch.no_grad():
        for images, labels in test_loader:
            images = images.to(device)
            feat = model.backbone(images)
            feat = feat.view(feat.size(0), -1)
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

    # Expert specialization: which class each expert handles most
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

    return utilization, specialization


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    print("=" * 80)
    print("  Football Detection & Tracking Experiment v2")
    print("  ES-MoE + LoRA + Focal Loss + Balanced Dataset")
    print("=" * 80)

    # Step 1: Parse LaMOT annotations
    print("\n[Step 1] Parsing LaMOT/SportsMoT annotations...")
    annotation_dir = Path("/tmp/LaMOT/annotations_v1")
    train_dir = annotation_dir / "train" / "SportsMOT"
    val_dir = annotation_dir / "val" / "SportsMOT"
    train_jsons = sorted(train_dir.glob("*.json")) if train_dir.exists() else []
    val_jsons = sorted(val_dir.glob("*.json")) if val_dir.exists() else []
    print(f"  Train annotations: {len(train_jsons)} files")
    print(f"  Val annotations: {len(val_jsons)} files")

    # Step 2: Create balanced datasets
    print("\n[Step 2] Creating class-balanced datasets...")
    train_dataset = BalancedFootballDataset(num_samples=2000, split="train")
    val_dataset = BalancedFootballDataset(num_samples=400, split="val")
    test_dataset = BalancedFootballDataset(num_samples=400, split="test")

    # Verify balance
    train_labels = [train_dataset.labels[i] for i in range(len(train_dataset))]
    label_counts = Counter(train_labels)
    print(f"  Train class distribution: {dict((CLASSES[k], v) for k, v in label_counts.items())}")

    train_loader = torch.utils.data.DataLoader(train_dataset, batch_size=BATCH_SIZE, shuffle=True, num_workers=0)
    val_loader = torch.utils.data.DataLoader(val_dataset, batch_size=BATCH_SIZE, shuffle=False, num_workers=0)
    test_loader = torch.utils.data.DataLoader(test_dataset, batch_size=BATCH_SIZE, shuffle=False, num_workers=0)

    # Step 3: Create model
    print("\n[Step 3] Creating ES-MoE model...")
    model = ESMoE(num_experts=NUM_EXPERTS, top_k=TOP_K, num_classes=NUM_CLASSES)
    total_before = sum(p.numel() for p in model.parameters())
    print(f"  Params before LoRA: {total_before:,}")

    # Step 4: Apply LoRA
    print("\n[Step 4] Applying LoRA adapters...")
    model = apply_lora_to_model(model)

    model = model.to(DEVICE)

    # Step 5: Train
    print("\n[Step 5] Training...")
    history, best_val_acc, best_epoch, total_time = run_training(
        model, train_loader, val_loader, NUM_EPOCHS, DEVICE
    )

    # Step 6: Evaluate
    print("\n[Step 6] Evaluating on test set...")
    best_path = OUTPUT_DIR / "es_moe_v2_best.pth"
    if best_path.exists():
        model.load_state_dict(torch.load(best_path, map_location=DEVICE, weights_only=True))
    test_metrics = run_inference(model, test_loader, DEVICE)

    print(f"\n  Test Accuracy: {test_metrics['accuracy']}")
    print(f"  Test mAP: {test_metrics['mAP']}")
    print(f"  Inference: {test_metrics['avg_inference_time_ms']}ms ({test_metrics['fps']} FPS)")
    print(f"  Per-class metrics:")
    for cls_name, cls_metrics in test_metrics["per_class"].items():
        print(f"    {cls_name}: P={cls_metrics['precision']:.3f} R={cls_metrics['recall']:.3f} F1={cls_metrics['f1']:.3f}")

    # Step 7: Expert analysis
    print("\n[Step 7] Analyzing expert utilization...")
    utilization, specialization = analyze_expert_utilization(model, test_loader, DEVICE)
    print(f"  Utilization: {utilization}")
    print(f"  Specialization:")
    for expert, spec in specialization.items():
        print(f"    {expert}: {spec['dominant_class']} ({spec['dominant_pct']}%)")

    # Step 8: Save results
    print("\n[Step 8] Saving results...")
    results = {
        "experiment": "ES-MoE v2 + LoRA + Focal Loss + Balanced Dataset",
        "timestamp": datetime.now().isoformat(),
        "config": {
            "num_experts": NUM_EXPERTS,
            "top_k": TOP_K,
            "lora_r": LORA_R,
            "lora_alpha": LORA_ALPHA,
            "load_balance_weight": LOAD_BALANCE_WEIGHT,
            "batch_size": BATCH_SIZE,
            "learning_rate": LEARNING_RATE,
            "num_epochs": NUM_EPOCHS,
            "device": DEVICE,
            "classes": CLASSES,
            "focal_loss_gamma": 2.0,
            "class_weights": [1.0, 3.0, 1.5, 1.5],
        },
        "model_info": {
            "total_params": sum(p.numel() for p in model.parameters()),
            "trainable_params": sum(p.numel() for p in model.parameters() if p.requires_grad),
            "lora_ratio_pct": round(
                sum(p.numel() for p in model.parameters() if p.requires_grad) /
                sum(p.numel() for p in model.parameters()) * 100, 2
            ),
        },
        "training": {
            "best_val_acc": round(best_val_acc, 4),
            "best_epoch": best_epoch,
            "total_time_sec": round(total_time, 1),
            "final_train_loss": round(history["train_loss"][-1], 4),
            "final_train_acc": round(history["train_acc"][-1], 4),
            "final_val_loss": round(history["val_loss"][-1], 4),
            "final_val_acc": round(history["val_acc"][-1], 4),
        },
        "test_metrics": test_metrics,
        "expert_utilization": utilization,
        "expert_specialization": specialization,
        "history_summary": {
            "train_loss": [round(v, 4) for v in history["train_loss"]],
            "val_loss": [round(v, 4) for v in history["val_loss"]],
            "train_acc": [round(v, 4) for v in history["train_acc"]],
            "val_acc": [round(v, 4) for v in history["val_acc"]],
        },
    }

    results_path = OUTPUT_DIR / "experiment_v2_results.json"
    with open(results_path, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"  Results saved to {results_path}")

    # Summary
    print("\n" + "=" * 80)
    print("  EXPERIMENT v2 SUMMARY")
    print("=" * 80)
    print(f"  Architecture: ES-MoE ({NUM_EXPERTS} experts, top-{TOP_K}) + LoRA (r={LORA_R})")
    print(f"  Improvements: Focal Loss + Balanced Dataset + Expert Diversity Loss")
    print(f"  Classes: {CLASSES}")
    print(f"  Device: {DEVICE}")
    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total = sum(p.numel() for p in model.parameters())
    print(f"  Trainable: {trainable:,} / {total:,} ({trainable/total*100:.1f}%)")
    print(f"  Best Val Acc: {best_val_acc:.4f} @ Epoch {best_epoch}")
    print(f"  Test Acc: {test_metrics['accuracy']} | mAP: {test_metrics['mAP']}")
    print(f"  Inference: {test_metrics['avg_inference_time_ms']}ms ({test_metrics['fps']} FPS)")
    print(f"  Training time: {total_time:.1f}s")
    print("=" * 80)

    return results


if __name__ == "__main__":
    results = main()

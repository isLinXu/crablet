#!/usr/bin/env python3
"""
Delivery Intent Gate — 6维交付意图门控

维度：
1. 格式合法性 (Format Legality): 产出格式是否符合规范？
2. 结构完整性 (Structural Completeness): 必需字段是否齐全？
3. 语义一致性 (Semantic Consistency): 内容是否自洽？
4. 安全合规性 (Safety Compliance): 是否满足安全要求？
5. 性能基线 (Performance Baseline): 是否满足性能基线？
6. 质量评判 (Quality Judgment): 5维启发式评分是否达标？

使用方式:
    # 命令行门控
    python delivery_intent_gate.py --input <交付物目录>

    # 作为模块使用
    from delivery_intent_gate import DeliveryIntentGate
    gate = DeliveryIntentGate()
    result = gate.check(deliverable_dir)
"""

import argparse
import json
import os
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import List, Dict, Optional, Tuple
from enum import Enum

# 添加当前目录到 sys.path 以便导入 quality_judge
sys.path.insert(0, os.path.dirname(__file__))

from quality_judge import RUBRIC


# ======================== 门控结果 ========================

class GateStatus(str, Enum):
    PASS = "PASS"
    WARN = "WARN"
    FAIL = "FAIL"


@dataclass
class GateResult:
    """单个门控维度的结果"""
    dimension: str
    status: GateStatus
    score: float
    details: str
    suggestions: List[str] = field(default_factory=list)


@dataclass
class DeliveryGateReport:
    """完整的门控报告"""
    deliverable_path: str
    dimensions: List[GateResult]
    overall_status: GateStatus
    overall_score: float
    timestamp: str = ""
    
    def to_dict(self) -> dict:
        return {
            "deliverable_path": self.deliverable_path,
            "overall_status": self.overall_status.value,
            "overall_score": self.overall_score,
            "timestamp": self.timestamp,
            "dimensions": [
                {
                    "dimension": d.dimension,
                    "status": d.status.value,
                    "score": d.score,
                    "details": d.details,
                    "suggestions": d.suggestions,
                }
                for d in self.dimensions
            ],
        }


# ======================== 门控阈值 ========================

GATE_THRESHOLDS = {
    "format_legality": {"pass": 0.9, "warn": 0.7},
    "structural_completeness": {"pass": 0.85, "warn": 0.6},
    "semantic_consistency": {"pass": 0.8, "warn": 0.6},
    "safety_compliance": {"pass": 0.95, "warn": 0.8},
    "performance_baseline": {"pass": 0.7, "warn": 0.5},
    "quality_judgment": {"pass": 3.5, "warn": 3.0},  # 5分制
}


# ======================== 交付意图门控 ========================

class DeliveryIntentGate:
    """6维交付意图门控"""
    
    def __init__(self, thresholds: dict = None, quality_judge=None):
        self.thresholds = thresholds or GATE_THRESHOLDS
        self.quality_judge = quality_judge
    
    def check(self, deliverable_dir: str) -> DeliveryGateReport:
        """执行6维门控检查"""
        deliverable_path = Path(deliverable_dir)
        
        if not deliverable_path.exists():
            return DeliveryGateReport(
                deliverable_path=deliverable_dir,
                dimensions=[],
                overall_status=GateStatus.FAIL,
                overall_score=0.0,
                timestamp=self._now(),
            )
        
        # 执行6维检查
        dimensions = [
            self._check_format_legality(deliverable_path),
            self._check_structural_completeness(deliverable_path),
            self._check_semantic_consistency(deliverable_path),
            self._check_safety_compliance(deliverable_path),
            self._check_performance_baseline(deliverable_path),
            self._check_quality_judgment(deliverable_path),
        ]
        
        # 计算总体状态
        overall_status = self._compute_overall_status(dimensions)
        overall_score = self._compute_overall_score(dimensions)
        
        return DeliveryGateReport(
            deliverable_path=deliverable_dir,
            dimensions=dimensions,
            overall_status=overall_status,
            overall_score=overall_score,
            timestamp=self._now(),
        )
    
    def _check_format_legality(self, path: Path) -> GateResult:
        """维度1: 格式合法性"""
        score = 1.0
        details = []
        suggestions = []
        
        # 检查必需文件是否存在
        required_files = ["manifest.json", "config.json"]
        for req_file in required_files:
            if not (path / req_file).exists():
                score -= 0.3
                details.append(f"缺少必需文件: {req_file}")
                suggestions.append(f"添加 {req_file} 文件")
        
        # 检查 JSON 格式是否合法
        json_files = list(path.glob("*.json"))
        for jf in json_files:
            try:
                with open(jf, "r", encoding="utf-8") as f:
                    json.load(f)
            except json.JSONDecodeError as e:
                score -= 0.2
                details.append(f"JSON 格式错误: {jf.name} - {e}")
                suggestions.append(f"修复 {jf.name} 的 JSON 格式")
        
        status = self._score_to_status(score, "format_legality")
        return GateResult(
            dimension="format_legality",
            status=status,
            score=score,
            details="; ".join(details) if details else "格式合法",
            suggestions=suggestions,
        )
    
    def _check_structural_completeness(self, path: Path) -> GateResult:
        """维度2: 结构完整性"""
        score = 1.0
        details = []
        suggestions = []
        
        # 检查目录结构
        expected_dirs = ["src", "tests", "docs"]
        for d in expected_dirs:
            if not (path / d).is_dir():
                score -= 0.15
                details.append(f"缺少目录: {d}")
                suggestions.append(f"创建 {d}/ 目录")
        
        # 检查 README
        if not (path / "README.md").exists() and not (path / "README").exists():
            score -= 0.1
            details.append("缺少 README.md")
            suggestions.append("添加 README.md 文件")
        
        status = self._score_to_status(score, "structural_completeness")
        return GateResult(
            dimension="structural_completeness",
            status=status,
            score=score,
            details="; ".join(details) if details else "结构完整",
            suggestions=suggestions,
        )
    
    def _check_semantic_consistency(self, path: Path) -> GateResult:
        """维度3: 语义一致性"""
        score = 1.0
        details = []
        suggestions = []
        
        # 检查 manifest.json 和 config.json 的一致性
        manifest_path = path / "manifest.json"
        config_path = path / "config.json"
        
        if manifest_path.exists() and config_path.exists():
            try:
                with open(manifest_path, "r") as f:
                    manifest = json.load(f)
                with open(config_path, "r") as f:
                    config = json.load(f)
                
                # 检查版本一致性
                m_version = manifest.get("version", "")
                c_version = config.get("version", "")
                if m_version and c_version and m_version != c_version:
                    score -= 0.3
                    details.append(f"版本不一致: manifest={m_version}, config={c_version}")
                    suggestions.append("统一 manifest.json 和 config.json 的版本号")
                
                # 检查名称一致性
                m_name = manifest.get("name", "")
                c_name = config.get("name", "")
                if m_name and c_name and m_name != c_name:
                    score -= 0.2
                    details.append(f"名称不一致: manifest={m_name}, config={c_name}")
                    suggestions.append("统一 manifest.json 和 config.json 的名称")
            except (json.JSONDecodeError, KeyError) as e:
                score -= 0.2
                details.append(f"解析错误: {e}")
        
        status = self._score_to_status(score, "semantic_consistency")
        return GateResult(
            dimension="semantic_consistency",
            status=status,
            score=score,
            details="; ".join(details) if details else "语义一致",
            suggestions=suggestions,
        )
    
    def _check_safety_compliance(self, path: Path) -> GateResult:
        """维度4: 安全合规性"""
        score = 1.0
        details = []
        suggestions = []
        
        # 检查敏感信息泄露
        sensitive_patterns = [
            "password", "secret", "api_key", "token",
            "密码", "密钥", "令牌",
        ]
        
        for file_path in path.rglob("*"):
            if file_path.is_file() and file_path.suffix in (".py", ".json", ".yaml", ".yml", ".toml", ".md"):
                try:
                    with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                        content = f.read().lower()
                    
                    for pattern in sensitive_patterns:
                        if pattern in content:
                            # 检查是否有硬编码的值
                            import re
                            if re.search(rf'{pattern}\s*[:=]\s*["\'][^"\']+["\']', content, re.IGNORECASE):
                                score -= 0.15
                                details.append(f"潜在敏感信息泄露: {file_path.name} ({pattern})")
                                suggestions.append(f"移除 {file_path.name} 中的硬编码 {pattern}")
                except (UnicodeDecodeError, PermissionError):
                    pass
        
        status = self._score_to_status(score, "safety_compliance")
        return GateResult(
            dimension="safety_compliance",
            status=status,
            score=score,
            details="; ".join(details) if details else "安全合规",
            suggestions=suggestions,
        )
    
    def _check_performance_baseline(self, path: Path) -> GateResult:
        """维度5: 性能基线"""
        score = 1.0
        details = []
        suggestions = []
        
        # 检查文件大小
        max_file_size = 100 * 1024  # 100KB
        for file_path in path.rglob("*"):
            if file_path.is_file():
                file_size = file_path.stat().st_size
                if file_size > max_file_size:
                    score -= 0.1
                    details.append(f"文件过大: {file_path.name} ({file_size / 1024:.1f}KB)")
                    suggestions.append(f"拆分 {file_path.name} 为更小的模块")
        
        # 检查文件数量
        all_files = list(path.rglob("*"))
        if len(all_files) > 200:
            score -= 0.1
            details.append(f"文件数量过多: {len(all_files)}")
            suggestions.append("减少不必要的文件")
        
        status = self._score_to_status(score, "performance_baseline")
        return GateResult(
            dimension="performance_baseline",
            status=status,
            score=score,
            details="; ".join(details) if details else "性能达标",
            suggestions=suggestions,
        )
    
    def _check_quality_judgment(self, path: Path) -> GateResult:
        """维度6: 质量评判（5维启发式评分）"""
        if self.quality_judge is None:
            # 如果没有提供质量评判器，使用默认启发式评分
            from quality_judge import HeuristicScorer
            scorer = HeuristicScorer()
            
            # 收集所有文件内容
            all_data = {}
            for file_path in path.rglob("*"):
                if file_path.is_file() and file_path.suffix in (".py", ".json", ".yaml", ".yml", ".md"):
                    try:
                        with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                            all_data[file_path.name] = f.read()
                    except (UnicodeDecodeError, PermissionError):
                        pass
            
            scores = scorer.score(all_data)
        else:
            scores = self.quality_judge.score({"path": str(path)})
        
        # 计算加权平均分
        total_weight = 0.0
        weighted_sum = 0.0
        for dimension, score_data in scores.items():
            weight = RUBRIC.get(dimension, {}).get("weight", 0.2)
            raw_score = score_data.get("raw", 3.0)
            weighted_sum += raw_score * weight
            total_weight += weight
        
        avg_score = weighted_sum / total_weight if total_weight > 0 else 3.0
        
        # 生成详细信息
        details = []
        for dimension, score_data in scores.items():
            raw = score_data.get("raw", 0)
            details.append(f"{dimension}: {raw:.2f}")
        
        status = self._score_to_status(avg_score, "quality_judgment")
        return GateResult(
            dimension="quality_judgment",
            status=status,
            score=avg_score,
            details="; ".join(details),
            suggestions=self._generate_quality_suggestions(scores),
        )
    
    def _generate_quality_suggestions(self, scores: dict) -> List[str]:
        """根据质量评分生成改进建议"""
        suggestions = []
        for dimension, score_data in scores.items():
            raw = score_data.get("raw", 3.0)
            if raw < 3.0:
                suggestions.append(f"提升 {dimension} 评分（当前: {raw:.2f}）")
        return suggestions
    
    def _score_to_status(self, score: float, dimension: str) -> GateStatus:
        """将分数转换为门控状态"""
        thresholds = self.thresholds.get(dimension, {"pass": 0.8, "warn": 0.6})
        if score >= thresholds["pass"]:
            return GateStatus.PASS
        elif score >= thresholds["warn"]:
            return GateStatus.WARN
        else:
            return GateStatus.FAIL
    
    def _compute_overall_status(self, dimensions: List[GateResult]) -> GateStatus:
        """计算总体门控状态"""
        if any(d.status == GateStatus.FAIL for d in dimensions):
            return GateStatus.FAIL
        elif any(d.status == GateStatus.WARN for d in dimensions):
            return GateStatus.WARN
        else:
            return GateStatus.PASS
    
    def _compute_overall_score(self, dimensions: List[GateResult]) -> float:
        """计算总体分数"""
        if not dimensions:
            return 0.0
        return sum(d.score for d in dimensions) / len(dimensions)
    
    def _now(self) -> str:
        """获取当前时间戳"""
        from datetime import datetime
        return datetime.now().isoformat()


# ======================== CLI 入口 ========================

def main():
    parser = argparse.ArgumentParser(description="Delivery Intent Gate - 6维交付意图门控")
    parser.add_argument("--input", type=str, required=True,
                        help="交付物目录路径")
    parser.add_argument("--thresholds", type=str, default=None,
                        help="门控阈值 JSON 文件路径（可选）")
    parser.add_argument("--output", type=str, default=None,
                        help="输出报告路径（可选）")
    
    args = parser.parse_args()
    
    # 加载阈值
    thresholds = GATE_THRESHOLDS
    if args.thresholds:
        with open(args.thresholds, "r") as f:
            thresholds = json.load(f)
    
    # 创建门控
    gate = DeliveryIntentGate(thresholds=thresholds)
    
    # 执行检查
    report = gate.check(args.input)
    
    # 输出报告
    report_dict = report.to_dict()
    print(json.dumps(report_dict, indent=2, ensure_ascii=False))
    
    # 保存报告
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(report_dict, f, indent=2, ensure_ascii=False)
        print(f"\n[INFO] 报告已保存到: {args.output}")
    
    # 返回退出码
    if report.overall_status == GateStatus.FAIL:
        sys.exit(1)
    elif report.overall_status == GateStatus.WARN:
        sys.exit(2)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()

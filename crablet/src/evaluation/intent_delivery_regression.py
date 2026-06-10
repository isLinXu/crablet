#!/usr/bin/env python3
"""
Intent Delivery Regression — 质量回归基线管理

管理质量评分的回归基线，支持：
- 建立基线：对存量交付物进行评分，记录基线
- 回归检测：对比当前评分与基线，检测回归
- 报告生成：生成回归报告

使用方式:
    # 建立基线
    python intent_delivery_regression.py --baseline --input <交付物目录>

    # 回归检测
    python intent_delivery_regression.py --check --input <交付物目录>

    # 作为模块使用
    from intent_delivery_regression import IntentDeliveryRegression
    regression = IntentDeliveryRegression()
    result = regression.check(deliverable_dir)
"""

import argparse
import json
import os
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import List, Dict, Optional, Tuple
from datetime import datetime


# ======================== 回归基线数据结构 ========================

@dataclass
class BaselineEntry:
    """基线条目"""
    name: str
    overall_score: float
    dimension_scores: Dict[str, float]
    timestamp: str = ""
    metadata: Dict = field(default_factory=dict)


@dataclass
class RegressionResult:
    """回归检测结果"""
    name: str
    current_score: float
    baseline_score: float
    delta: float
    is_regression: bool
    dimension_deltas: Dict[str, float] = field(default_factory=dict)
    details: str = ""


@dataclass
class RegressionReport:
    """回归报告"""
    total_entries: int
    regressions: int
    improvements: int
    stable: int
    results: List[RegressionResult] = field(default_factory=list)
    overall_status: str = "STABLE"
    timestamp: str = ""
    
    def to_dict(self) -> dict:
        return {
            "total_entries": self.total_entries,
            "regressions": self.regressions,
            "improvements": self.improvements,
            "stable": self.stable,
            "overall_status": self.overall_status,
            "timestamp": self.timestamp,
            "results": [
                {
                    "name": r.name,
                    "current_score": r.current_score,
                    "baseline_score": r.baseline_score,
                    "delta": r.delta,
                    "is_regression": r.is_regression,
                    "dimension_deltas": r.dimension_deltas,
                    "details": r.details,
                }
                for r in self.results
            ],
        }


# ======================== 回归基线管理器 ========================

class IntentDeliveryRegression:
    """质量回归基线管理器"""
    
    # 回归阈值
    REGRESSION_THRESHOLD = -0.3  # 下降超过0.3视为回归
    IMPROVEMENT_THRESHOLD = 0.3  # 上升超过0.3视为改进
    
    def __init__(self, baseline_path: str = None):
        self.baseline_path = baseline_path or os.path.join(
            os.path.dirname(__file__), "references", "quality_baseline.json"
        )
        self.baselines: Dict[str, BaselineEntry] = {}
        self._load_baselines()
    
    def _load_baselines(self):
        """加载基线数据"""
        baseline_file = Path(self.baseline_path)
        if baseline_file.exists():
            with open(baseline_file, "r", encoding="utf-8") as f:
                data = json.load(f)
            
            for entry in data.get("baselines", []):
                self.baselines[entry["name"]] = BaselineEntry(
                    name=entry["name"],
                    overall_score=entry["overall_score"],
                    dimension_scores=entry.get("dimension_scores", {}),
                    timestamp=entry.get("timestamp", ""),
                    metadata=entry.get("metadata", {}),
                )
    
    def _save_baselines(self):
        """保存基线数据"""
        baseline_file = Path(self.baseline_path)
        baseline_file.parent.mkdir(parents=True, exist_ok=True)
        
        data = {
            "version": "1.0.0",
            "description": "Quality regression baseline",
            "updated_at": datetime.now().isoformat(),
            "baselines": [
                {
                    "name": entry.name,
                    "overall_score": entry.overall_score,
                    "dimension_scores": entry.dimension_scores,
                    "timestamp": entry.timestamp,
                    "metadata": entry.metadata,
                }
                for entry in self.baselines.values()
            ],
        }
        
        with open(baseline_file, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
    
    def establish_baseline(self, name: str, scores: dict, metadata: dict = None):
        """建立基线
        
        Args:
            name: 基线名称
            scores: 评分结果（来自 QualityJudge.evaluate()）
            metadata: 附加元数据
        """
        # 计算各维度分数
        dimension_scores = {}
        heuristic = scores.get("heuristic_scores", {})
        for dim, score_data in heuristic.items():
            dimension_scores[dim] = score_data.get("raw", 3.0)
        
        entry = BaselineEntry(
            name=name,
            overall_score=scores.get("overall_score", 3.0),
            dimension_scores=dimension_scores,
            timestamp=datetime.now().isoformat(),
            metadata=metadata or {},
        )
        
        self.baselines[name] = entry
        self._save_baselines()
    
    def check_regression(self, name: str, current_scores: dict) -> RegressionResult:
        """检测回归
        
        Args:
            name: 基线名称
            current_scores: 当前评分结果
        
        Returns:
            回归检测结果
        """
        if name not in self.baselines:
            return RegressionResult(
                name=name,
                current_score=current_scores.get("overall_score", 3.0),
                baseline_score=0.0,
                delta=0.0,
                is_regression=False,
                details="无基线数据",
            )
        
        baseline = self.baselines[name]
        current_overall = current_scores.get("overall_score", 3.0)
        delta = current_overall - baseline.overall_score
        
        # 计算各维度差异
        dimension_deltas = {}
        heuristic = current_scores.get("heuristic_scores", {})
        for dim, score_data in heuristic.items():
            current_dim = score_data.get("raw", 3.0)
            baseline_dim = baseline.dimension_scores.get(dim, 3.0)
            dimension_deltas[dim] = current_dim - baseline_dim
        
        is_regression = delta < self.REGRESSION_THRESHOLD
        
        return RegressionResult(
            name=name,
            current_score=current_overall,
            baseline_score=baseline.overall_score,
            delta=delta,
            is_regression=is_regression,
            dimension_deltas=dimension_deltas,
            details=f"Δ={delta:+.2f} ({'回归' if is_regression else '稳定/改进'})",
        )
    
    def batch_check(self, current_scores: Dict[str, dict]) -> RegressionReport:
        """批量检测回归
        
        Args:
            current_scores: 名称到当前评分的映射
        
        Returns:
            回归报告
        """
        results = []
        regressions = 0
        improvements = 0
        stable = 0
        
        for name, scores in current_scores.items():
            result = self.check_regression(name, scores)
            results.append(result)
            
            if result.is_regression:
                regressions += 1
            elif result.delta > self.IMPROVEMENT_THRESHOLD:
                improvements += 1
            else:
                stable += 1
        
        # 计算总体状态
        if regressions > 0:
            overall_status = "REGRESSION_DETECTED"
        elif improvements > stable:
            overall_status = "IMPROVING"
        else:
            overall_status = "STABLE"
        
        return RegressionReport(
            total_entries=len(results),
            regressions=regressions,
            improvements=improvements,
            stable=stable,
            results=results,
            overall_status=overall_status,
            timestamp=datetime.now().isoformat(),
        )
    
    def list_baselines(self) -> List[Dict]:
        """列出所有基线"""
        return [
            {
                "name": entry.name,
                "overall_score": entry.overall_score,
                "dimension_scores": entry.dimension_scores,
                "timestamp": entry.timestamp,
            }
            for entry in self.baselines.values()
        ]


# ======================== CLI 入口 ========================

def main():
    parser = argparse.ArgumentParser(description="Intent Delivery Regression - 质量回归基线管理")
    parser.add_argument("--baseline", action="store_true", default=False,
                        help="建立基线")
    parser.add_argument("--check", action="store_true", default=False,
                        help="检测回归")
    parser.add_argument("--list", action="store_true", default=False,
                        help="列出基线")
    parser.add_argument("--input", type=str, required=True,
                        help="交付物目录路径")
    parser.add_argument("--name", type=str, default=None,
                        help="基线名称（默认使用目录名）")
    parser.add_argument("--baseline-path", type=str, default=None,
                        help="基线文件路径（可选）")
    parser.add_argument("--output", type=str, default=None,
                        help="输出报告路径（可选）")
    
    args = parser.parse_args()
    
    # 创建回归管理器
    regression = IntentDeliveryRegression(baseline_path=args.baseline_path)
    
    # 获取基线名称
    name = args.name or Path(args.input).name
    
    if args.list:
        # 列出基线
        baselines = regression.list_baselines()
        print(json.dumps(baselines, indent=2, ensure_ascii=False))
        return
    
    # 执行质量评估
    from quality_judge import QualityJudge
    judge = QualityJudge()
    
    if Path(args.input).is_dir():
        scores = judge.evaluate_directory(args.input)
    else:
        with open(args.input, "r", encoding="utf-8") as f:
            content = f.read()
        scores = judge.evaluate({"content": content, "filename": Path(args.input).name})
    
    if args.baseline:
        # 建立基线
        regression.establish_baseline(name, scores)
        print(f"[INFO] 基线已建立: {name} (分数: {scores.get('overall_score', 0):.2f})")
    
    if args.check:
        # 检测回归
        result = regression.check_regression(name, scores)
        print(json.dumps({
            "name": result.name,
            "current_score": result.current_score,
            "baseline_score": result.baseline_score,
            "delta": result.delta,
            "is_regression": result.is_regression,
            "details": result.details,
        }, indent=2, ensure_ascii=False))
        
        if result.is_regression:
            sys.exit(1)


if __name__ == "__main__":
    main()

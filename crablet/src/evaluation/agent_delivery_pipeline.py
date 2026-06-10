#!/usr/bin/env python3
"""
Agent Delivery Pipeline — 5步交付流水线

步骤：
1. 规划 (Plan): 解析用户意图，生成交付计划
2. 构建 (Build): 根据计划构建交付物
3. 校验 (Validate): 使用门禁校验交付物
4. 修复 (Fix): 自动修复校验发现的问题
5. 质量评判 (Judge): 使用5维启发式评分器评估质量

使用方式:
    # 完整流水线
    python agent_delivery_pipeline.py --input <交付物目录>

    # 仅质量评判
    python agent_delivery_pipeline.py --input <交付物目录> --stage judge

    # 作为模块使用
    from agent_delivery_pipeline import AgentDeliveryPipeline
    pipeline = AgentDeliveryPipeline()
    result = pipeline.run(deliverable_dir)
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


# ======================== 流水线状态 ========================

class PipelineStage(str, Enum):
    PLAN = "plan"
    BUILD = "build"
    VALIDATE = "validate"
    FIX = "fix"
    JUDGE = "judge"


class PipelineStatus(str, Enum):
    SUCCESS = "SUCCESS"
    PARTIAL = "PARTIAL"
    FAILED = "FAILED"


@dataclass
class StageResult:
    """单个阶段的结果"""
    stage: PipelineStage
    status: PipelineStatus
    score: float
    details: str
    artifacts: List[str] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)


@dataclass
class PipelineReport:
    """完整的流水线报告"""
    deliverable_path: str
    stages: List[StageResult]
    overall_status: PipelineStatus
    overall_score: float
    quality_score: float = 0.0
    gate_report: Optional[Dict] = None
    
    def to_dict(self) -> dict:
        return {
            "deliverable_path": self.deliverable_path,
            "overall_status": self.overall_status.value,
            "overall_score": self.overall_score,
            "quality_score": self.quality_score,
            "stages": [
                {
                    "stage": s.stage.value,
                    "status": s.status.value,
                    "score": s.score,
                    "details": s.details,
                    "artifacts": s.artifacts,
                    "errors": s.errors,
                }
                for s in self.stages
            ],
            "gate_report": self.gate_report,
        }


# ======================== 交付流水线 ========================

class AgentDeliveryPipeline:
    """5步交付流水线"""
    
    def __init__(self, config: dict = None):
        self.config = config or {}
        self.quality_judge = None
        self.gate = None
    
    def run(self, deliverable_dir: str, stages: List[PipelineStage] = None) -> PipelineReport:
        """运行交付流水线"""
        deliverable_path = Path(deliverable_dir)
        
        if not deliverable_path.exists():
            return PipelineReport(
                deliverable_path=deliverable_dir,
                stages=[],
                overall_status=PipelineStatus.FAILED,
                overall_score=0.0,
            )
        
        # 默认执行所有阶段
        if stages is None:
            stages = list(PipelineStage)
        
        # 执行各阶段
        stage_results = []
        for stage in stages:
            result = self._execute_stage(stage, deliverable_path)
            stage_results.append(result)
            
            # 如果某个阶段失败，后续阶段可能无法执行
            if result.status == PipelineStatus.FAILED and stage in (
                PipelineStage.PLAN, PipelineStage.BUILD
            ):
                break
        
        # 计算总体状态
        overall_status = self._compute_overall_status(stage_results)
        overall_score = self._compute_overall_score(stage_results)
        
        # 获取质量评分
        quality_score = 0.0
        for result in stage_results:
            if result.stage == PipelineStage.JUDGE:
                quality_score = result.score
        
        # 获取门控报告
        gate_report = None
        for result in stage_results:
            if result.stage == PipelineStage.VALIDATE:
                gate_report = {"score": result.score, "details": result.details}
        
        return PipelineReport(
            deliverable_path=deliverable_dir,
            stages=stage_results,
            overall_status=overall_status,
            overall_score=overall_score,
            quality_score=quality_score,
            gate_report=gate_report,
        )
    
    def _execute_stage(self, stage: PipelineStage, path: Path) -> StageResult:
        """执行单个阶段"""
        if stage == PipelineStage.PLAN:
            return self._stage_plan(path)
        elif stage == PipelineStage.BUILD:
            return self._stage_build(path)
        elif stage == PipelineStage.VALIDATE:
            return self._stage_validate(path)
        elif stage == PipelineStage.FIX:
            return self._stage_fix(path)
        elif stage == PipelineStage.JUDGE:
            return self._stage_judge(path)
        else:
            return StageResult(
                stage=stage,
                status=PipelineStatus.FAILED,
                score=0.0,
                details=f"未知阶段: {stage}",
            )
    
    def _stage_plan(self, path: Path) -> StageResult:
        """步骤1: 规划"""
        # 解析交付物目录结构
        plan = {
            "files": [],
            "directories": [],
            "total_size": 0,
        }
        
        for item in path.rglob("*"):
            if item.is_file():
                plan["files"].append(str(item.relative_to(path)))
                plan["total_size"] += item.stat().st_size
            elif item.is_dir():
                plan["directories"].append(str(item.relative_to(path)))
        
        return StageResult(
            stage=PipelineStage.PLAN,
            status=PipelineStatus.SUCCESS,
            score=1.0,
            details=f"规划完成: {len(plan['files'])} 文件, {len(plan['directories'])} 目录",
            artifacts=list(plan["files"][:10]),  # 只保留前10个
        )
    
    def _stage_build(self, path: Path) -> StageResult:
        """步骤2: 构建"""
        # 检查构建产物是否存在
        build_artifacts = []
        for pattern in ["*.py", "*.json", "*.yaml", "*.yml", "*.md"]:
            build_artifacts.extend(path.rglob(pattern))
        
        if build_artifacts:
            return StageResult(
                stage=PipelineStage.BUILD,
                status=PipelineStatus.SUCCESS,
                score=1.0,
                details=f"构建完成: {len(build_artifacts)} 产物",
                artifacts=[str(a.relative_to(path)) for a in build_artifacts[:10]],
            )
        else:
            return StageResult(
                stage=PipelineStage.BUILD,
                status=PipelineStatus.FAILED,
                score=0.0,
                details="构建失败: 无产物",
            )
    
    def _stage_validate(self, path: Path) -> StageResult:
        """步骤3: 校验"""
        # 使用交付意图门控进行校验
        from delivery_intent_gate import DeliveryIntentGate
        
        gate = DeliveryIntentGate()
        report = gate.check(str(path))
        
        status = PipelineStatus.SUCCESS if report.overall_status.value == "PASS" else \
                 PipelineStatus.PARTIAL if report.overall_status.value == "WARN" else \
                 PipelineStatus.FAILED
        
        return StageResult(
            stage=PipelineStage.VALIDATE,
            status=status,
            score=report.overall_score,
            details=f"校验结果: {report.overall_status.value} (分数: {report.overall_score:.2f})",
            errors=[f"{d.dimension}: {d.details}" for d in report.dimensions if d.status.value == "FAIL"],
        )
    
    def _stage_fix(self, path: Path) -> StageResult:
        """步骤4: 修复"""
        # 自动修复常见问题
        fixes_applied = []
        
        # 修复1: 确保 manifest.json 存在
        manifest_path = path / "manifest.json"
        if not manifest_path.exists():
            default_manifest = {
                "name": path.name,
                "version": "0.1.0",
                "description": f"Auto-generated manifest for {path.name}",
            }
            with open(manifest_path, "w", encoding="utf-8") as f:
                json.dump(default_manifest, f, indent=2, ensure_ascii=False)
            fixes_applied.append("创建默认 manifest.json")
        
        # 修复2: 确保 README.md 存在
        readme_path = path / "README.md"
        if not readme_path.exists():
            with open(readme_path, "w", encoding="utf-8") as f:
                f.write(f"# {path.name}\n\nAuto-generated README.\n")
            fixes_applied.append("创建默认 README.md")
        
        return StageResult(
            stage=PipelineStage.FIX,
            status=PipelineStatus.SUCCESS,
            score=1.0,
            details=f"修复完成: {len(fixes_applied)} 项",
            artifacts=fixes_applied,
        )
    
    def _stage_judge(self, path: Path) -> StageResult:
        """步骤5: 质量评判"""
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
        
        # 执行5维评分
        scores = scorer.score(all_data)
        
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
        
        status = PipelineStatus.SUCCESS if avg_score >= 3.5 else \
                 PipelineStatus.PARTIAL if avg_score >= 3.0 else \
                 PipelineStatus.FAILED
        
        return StageResult(
            stage=PipelineStage.JUDGE,
            status=status,
            score=avg_score,
            details=f"质量评判: {avg_score:.2f}/5.0 ({'; '.join(details)})",
        )
    
    def _compute_overall_status(self, results: List[StageResult]) -> PipelineStatus:
        """计算总体状态"""
        if any(r.status == PipelineStatus.FAILED for r in results):
            return PipelineStatus.FAILED
        elif any(r.status == PipelineStatus.PARTIAL for r in results):
            return PipelineStatus.PARTIAL
        else:
            return PipelineStatus.SUCCESS
    
    def _compute_overall_score(self, results: List[StageResult]) -> float:
        """计算总体分数"""
        if not results:
            return 0.0
        return sum(r.score for r in results) / len(results)


# ======================== CLI 入口 ========================

def main():
    parser = argparse.ArgumentParser(description="Agent Delivery Pipeline - 5步交付流水线")
    parser.add_argument("--input", type=str, required=True,
                        help="交付物目录路径")
    parser.add_argument("--stage", type=str, default=None,
                        choices=["plan", "build", "validate", "fix", "judge"],
                        help="仅执行指定阶段")
    parser.add_argument("--output", type=str, default=None,
                        help="输出报告路径（可选）")
    
    args = parser.parse_args()
    
    # 创建流水线
    pipeline = AgentDeliveryPipeline()
    
    # 确定执行阶段
    if args.stage:
        stages = [PipelineStage(args.stage)]
    else:
        stages = None  # 执行所有阶段
    
    # 运行流水线
    report = pipeline.run(args.input, stages=stages)
    
    # 输出报告
    report_dict = report.to_dict()
    print(json.dumps(report_dict, indent=2, ensure_ascii=False))
    
    # 保存报告
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(report_dict, f, indent=2, ensure_ascii=False)
        print(f"\n[INFO] 报告已保存到: {args.output}")
    
    # 返回退出码
    if report.overall_status == PipelineStatus.FAILED:
        sys.exit(1)
    elif report.overall_status == PipelineStatus.PARTIAL:
        sys.exit(2)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Quality Judge — LLM-as-Judge 5-Dimensional Quality Evaluator

评估维度：
1. 意图保真度 (Intent Fidelity): 产出是否符合用户原始意图？
2. 拓扑合理性 (Topology Rationality): 依赖关系是否合理？
3. 节点必要性 (Node Necessity): 是否存在冗余节点？
4. 过度简化 (Over-Simplification): 是否丢失了关键信息？
5. 过度复杂 (Over-Complexity): 是否引入了不必要的复杂性？

使用方式:
    # 命令行评估（启发式）
    python quality_judge.py --input <交付物目录>

    # LLM-as-Judge 评估（需要 LLM API key）
    QUALITY_JUDGE_LLM=your-api-key python quality_judge.py --input <交付物目录> --llm-judge

    # 黄金对照集评估
    python quality_judge.py --input <交付物目录> --golden <黄金对照集路径>

    # 作为模块使用
    from quality_judge import QualityJudge
    judge = QualityJudge()
    result = judge.evaluate(deliverable_dir)
"""

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import List, Dict, Optional, Tuple, Any
from datetime import datetime
import hashlib


# ======================== 评分准则 ========================

RUBRIC = {
    "intent_fidelity": {
        "weight": 0.25,
        "description": "产出是否符合用户原始意图",
        "min_score": 1,
        "max_score": 5,
        "score_descriptions": {
            1: "完全偏离意图",
            2: "部分符合意图，但有明显偏差",
            3: "基本符合意图，有小瑕疵",
            4: "高度符合意图，仅微量偏差",
            5: "完美符合意图"
        }
    },
    "topology_rationality": {
        "weight": 0.25,
        "description": "依赖关系是否合理",
        "min_score": 1,
        "max_score": 5,
        "score_descriptions": {
            1: "依赖关系完全不合理",
            2: "依赖关系部分合理",
            3: "依赖关系基本合理",
            4: "依赖关系高度合理",
            5: "依赖关系完美"
        }
    },
    "node_necessity": {
        "weight": 0.20,
        "description": "是否包含冗余节点",
        "min_score": 1,
        "max_score": 5,
        "score_descriptions": {
            1: "大量冗余节点",
            2: "有一些冗余节点",
            3: "仅有少量冗余节点",
            4: "几乎没有冗余节点",
            5: "零冗余节点"
        }
    },
    "over_simplification": {
        "weight": 0.15,
        "description": "是否丢失了关键信息",
        "min_score": 1,
        "max_score": 5,
        "score_descriptions": {
            1: "丢失了大量关键信息",
            2: "丢失了部分关键信息",
            3: "丢失了少量关键信息",
            4: "几乎没有丢失信息",
            5: "完全保留了关键信息"
        }
    },
    "over_complexity": {
        "weight": 0.15,
        "description": "是否引入了不必要的复杂性",
        "min_score": 1,
        "max_score": 5,
        "score_descriptions": {
            1: "引入了大量不必要的复杂性",
            2: "引入了部分不必要的复杂性",
            3: "引入了少量不必要的复杂性",
            4: "几乎没有引入不必要的复杂性",
            5: "完全简洁，无不必要的复杂性"
        }
    }
}


# ======================== 黄金对照 Schema ========================

GOLDEN_REFERENCE_SCHEMA = {
    "type": "object",
    "required": ["intent", "topology", "nodes", "complexity"],
    "properties": {
        "intent": {
            "type": "integer",
            "minimum": RUBRIC["intent_fidelity"]["min_score"],
            "maximum": RUBRIC["intent_fidelity"]["max_score"],
            "description": RUBRIC["intent_fidelity"]["description"]
        },
        "topology": {
            "type": "integer",
            "minimum": RUBRIC["topology_rationality"]["min_score"],
            "maximum": RUBRIC["topology_rationality"]["max_score"],
            "description": RUBRIC["topology_rationality"]["description"]
        },
        "nodes": {
            "type": "integer",
            "minimum": RUBRIC["node_necessity"]["min_score"],
            "maximum": RUBRIC["node_necessity"]["max_score"],
            "description": RUBRIC["node_necessity"]["description"]
        },
        "complexity": {
            "type": "integer",
            "minimum": RUBRIC["over_simplification"]["min_score"],
            "maximum": RUBRIC["over_simplification"]["max_score"],
            "description": RUBRIC["over_simplification"]["description"]
        }
    }
}


# ======================== 启发式评分器 ========================

class HeuristicScorer:
    """5维启发式评分器"""
    
    def __init__(self, rubric: dict = None, golden_schema: dict = None):
        self.rubric = rubric or RUBRIC
        self.golden_schema = golden_schema or GOLDEN_REFERENCE_SCHEMA
    
    def score(self, data: dict) -> dict:
        """对输入数据进行5维评分"""
        scores = {}
        for dimension, config in self.rubric.items():
            weight = config.get("weight", 0.2)
            min_score = config.get("min_score", 1)
            max_score = config.get("max_score", 5)
            
            # 计算原始分数
            raw_score = self._compute_raw_score(data, dimension)
            weighted_score = raw_score * weight
            
            # 确保分数在范围内
            clamped_score = max(min_score, min(max_score, raw_score))
            
            scores[dimension] = {
                "raw": raw_score,
                "weighted": weighted_score,
                "clamped": clamped_score,
            }
        
        return scores
    
    def _compute_raw_score(self, data: dict, dimension: str) -> float:
        """计算原始分数（1-5）"""
        if dimension == "intent_fidelity":
            return self._score_intent_fidelity(data)
        elif dimension == "topology_rationality":
            return self._score_topology_rationality(data)
        elif dimension == "node_necessity":
            return self._score_node_necessity(data)
        elif dimension == "over_simplification":
            return self._score_over_simplification(data)
        elif dimension == "over_complexity":
            return self._score_over_complexity(data)
        else:
            return 3.0  # 默认中等分数
    
    def _score_intent_fidelity(self, data: dict) -> float:
        """评分：意图保真度"""
        if "intent" in data:
            intent = data["intent"]
            if isinstance(intent, (int, float)):
                return float(intent)
        text = json.dumps(data, default=str, ensure_ascii=False).lower()
        intent_keywords = ["创建", "生成", "写", "制作", "设计", "build", "create", "generate", "make"]
        for kw in intent_keywords:
            if kw in text:
                return 4.0
        return 2.0
    
    def _score_topology_rationality(self, data: dict) -> float:
        """评分：拓扑合理性"""
        if "dependencies" in data:
            deps = data["dependencies"]
            if isinstance(deps, list):
                if self._is_dag(deps):
                    return 4.0
                else:
                    return 2.0
        return 3.0
    
    def _is_dag(self, deps: list) -> bool:
        """检查依赖是否形成有向无环图"""
        try:
            visited = set()
            stack = []
            
            def dfs(node):
                if id(node) in visited:
                    return
                visited.add(id(node))
                stack.append(node)
                for neighbor in node.get("dependencies", []):
                    dfs(neighbor)
            
            for dep in deps:
                dfs(dep)
            
            return True
        except:
            return False
    
    def _score_node_necessity(self, data: dict) -> float:
        """评分：节点必要性"""
        if "nodes" in data:
            nodes = data["nodes"]
            if isinstance(nodes, list):
                unique_ids = set()
                for node in nodes:
                    node_id = node.get("id", "") if isinstance(node, dict) else str(node)
                    if node_id:
                        if node_id in unique_ids:
                            return 1.0
                        unique_ids.add(node_id)
                return 5.0 if len(unique_ids) == len(nodes) else 2.0
        return 3.0
    
    def _score_over_simplification(self, data: dict) -> float:
        """评分：过度简化"""
        text = json.dumps(data, default=str).lower()
        key_info_keywords = ["version", "api", "config", "secret", "key", "token", "password"]
        key_info_present = any(kw in text for kw in key_info_keywords)
        return 4.0 if key_info_present else 2.0
    
    def _score_over_complexity(self, data: dict) -> float:
        """评分：过度复杂"""
        text = json.dumps(data, default=str)
        unnecessary_complexity_patterns = [
            r"wrapper.*wrapper",
            r"factory.*factory",
            r"proxy.*proxy",
        ]
        for pattern in unnecessary_complexity_patterns:
            if re.search(pattern, text, re.IGNORECASE):
                return 1.0
        return 4.0


# ======================== LLM-as-Judge 评分器 ========================

class LLMJudge:
    """LLM-as-Judge 评分器
    
    使用 LLM 对交付物进行语义级别的质量评估。
    支持多种 LLM 后端（OpenAI、Anthropic 等）。
    """
    
    def __init__(self, api_key: str = None, model: str = None, base_url: str = None):
        self.api_key = api_key or os.environ.get("QUALITY_JUDGE_LLM", "")
        self.model = model or os.environ.get("QUALITY_JUDGE_MODEL", "gpt-4o-mini")
        self.base_url = base_url or os.environ.get("QUALITY_JUDGE_BASE_URL", None)
        self.rubric = RUBRIC
    
    def evaluate(self, sample_output: str, reference_output: str = None,
                 intent: str = None) -> dict:
        """使用 LLM 进行5维评分
        
        Args:
            sample_output: 待评估的产出内容
            reference_output: 参考产出（可选）
            intent: 用户原始意图（可选）
        
        Returns:
            包含5维评分和总体评分的字典
        """
        if not self.api_key:
            # 无 API key 时回退到启发式评分
            scorer = HeuristicScorer()
            return scorer.score({"content": sample_output, "intent": intent})
        
        try:
            return self._llm_evaluate(sample_output, reference_output, intent)
        except Exception as e:
            # LLM 调用失败时回退到启发式评分
            print(f"[WARN] LLM 评估失败，回退到启发式评分: {e}", file=sys.stderr)
            scorer = HeuristicScorer()
            return scorer.score({"content": sample_output, "intent": intent})
    
    def _llm_evaluate(self, sample_output: str, reference_output: str = None,
                       intent: str = None) -> dict:
        """调用 LLM 进行评估"""
        # 构建 prompt
        rubric_text = self._format_rubric()
        
        prompt = f"""你是一个专业的质量评判器。请根据以下评分准则，对给定的产出进行5维评分。

## 评分准则

{rubric_text}

## 待评估产出

{sample_output}
"""
        if reference_output:
            prompt += f"\n## 参考产出\n\n{reference_output}\n"
        if intent:
            prompt += f"\n## 用户原始意图\n\n{intent}\n"
        
        prompt += """

## 评分要求

请严格按照评分准则，对每个维度给出1-5分的评分，并简要说明理由。
输出格式必须为 JSON：

```json
{
  "intent_fidelity": {"score": X, "reason": "..."},
  "topology_rationality": {"score": X, "reason": "..."},
  "node_necessity": {"score": X, "reason": "..."},
  "over_simplification": {"score": X, "reason": "..."},
  "over_complexity": {"score": X, "reason": "..."}
}
```
"""
        # 调用 LLM API
        response = self._call_llm(prompt)
        
        # 解析 LLM 响应
        return self._parse_llm_response(response)
    
    def _call_llm(self, prompt: str) -> str:
        """调用 LLM API"""
        try:
            import openai
            
            client = openai.OpenAI(
                api_key=self.api_key,
                base_url=self.base_url,
            )
            
            response = client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": "你是一个专业的质量评判器，严格按照评分准则进行评估。"},
                    {"role": "user", "content": prompt},
                ],
                temperature=0.1,  # 低温度确保评分一致性
                max_tokens=1000,
            )
            
            return response.choices[0].message.content
        except ImportError:
            raise RuntimeError("需要安装 openai 包: pip install openai")
        except Exception as e:
            raise RuntimeError(f"LLM API 调用失败: {e}")
    
    def _parse_llm_response(self, response: str) -> dict:
        """解析 LLM 响应为结构化评分"""
        # 尝试从响应中提取 JSON
        json_match = re.search(r'```json\s*(.*?)\s*```', response, re.DOTALL)
        if json_match:
            try:
                parsed = json.loads(json_match.group(1))
            except json.JSONDecodeError:
                parsed = {}
        else:
            try:
                parsed = json.loads(response)
            except json.JSONDecodeError:
                parsed = {}
        
        # 转换为标准格式
        scores = {}
        for dimension in RUBRIC:
            if dimension in parsed:
                dim_data = parsed[dimension]
                if isinstance(dim_data, dict):
                    raw_score = float(dim_data.get("score", 3.0))
                    reason = dim_data.get("reason", "")
                else:
                    raw_score = float(dim_data)
                    reason = ""
                
                weight = RUBRIC[dimension].get("weight", 0.2)
                min_score = RUBRIC[dimension].get("min_score", 1)
                max_score = RUBRIC[dimension].get("max_score", 5)
                
                scores[dimension] = {
                    "raw": max(min_score, min(max_score, raw_score)),
                    "weighted": raw_score * weight,
                    "clamped": max(min_score, min(max_score, raw_score)),
                    "reason": reason,
                }
            else:
                # 缺失维度，使用默认分数
                scores[dimension] = {
                    "raw": 3.0,
                    "weighted": 3.0 * RUBRIC[dimension].get("weight", 0.2),
                    "clamped": 3.0,
                    "reason": "LLM 未提供此维度评分",
                }
        
        return scores
    
    def _format_rubric(self) -> str:
        """格式化评分准则为文本"""
        lines = []
        for dim, config in RUBRIC.items():
            lines.append(f"### {dim} (权重: {config['weight']})")
            lines.append(f"描述: {config['description']}")
            lines.append("评分标准:")
            for score, desc in config["score_descriptions"].items():
                lines.append(f"  {score}分: {desc}")
            lines.append("")
        return "\n".join(lines)


# ======================== 黄金对照集匹配器 ========================

class GoldenReferenceMatcher:
    """黄金对照集匹配器
    
    将产出与黄金对照集进行匹配，计算相似度评分。
    """
    
    def __init__(self, golden_path: str = None):
        self.golden_cases = []
        if golden_path:
            self.load_golden(golden_path)
    
    def load_golden(self, path: str):
        """加载黄金对照集"""
        golden_file = Path(path)
        if golden_file.exists():
            with open(golden_file, "r", encoding="utf-8") as f:
                data = json.load(f)
            
            if "examples" in data:
                self.golden_cases = data["examples"]
            elif isinstance(data, list):
                self.golden_cases = data
            else:
                self.golden_cases = [data]
    
    def match(self, sample: dict) -> dict:
        """将样本与黄金对照集进行匹配
        
        Args:
            sample: 待匹配的样本数据
        
        Returns:
            包含最佳匹配和相似度的字典
        """
        if not self.golden_cases:
            return {"best_match": None, "similarity": 0.0, "all_matches": []}
        
        matches = []
        for case in self.golden_cases:
            similarity = self._compute_similarity(sample, case)
            matches.append({
                "case_id": case.get("id", "unknown"),
                "family": case.get("family", "unknown"),
                "similarity": similarity,
                "expected_range": case.get("expected_score_range", {}),
            })
        
        # 按相似度排序
        matches.sort(key=lambda x: x["similarity"], reverse=True)
        
        best = matches[0] if matches else None
        
        return {
            "best_match": best,
            "similarity": best["similarity"] if best else 0.0,
            "all_matches": matches,
        }
    
    def _compute_similarity(self, sample: dict, case: dict) -> float:
        """计算样本与黄金案例的相似度"""
        # 基于多维度的相似度计算
        dimensions = ["intent", "topology", "nodes", "complexity"]
        total_diff = 0.0
        count = 0
        
        for dim in dimensions:
            sample_val = sample.get(dim, 3)
            case_val = case.get(dim, 3)
            
            if isinstance(sample_val, (int, float)) and isinstance(case_val, (int, float)):
                diff = abs(sample_val - case_val) / 5.0  # 归一化到 0-1
                total_diff += diff
                count += 1
        
        if count == 0:
            return 0.0
        
        # 相似度 = 1 - 平均差异
        similarity = 1.0 - (total_diff / count)
        return max(0.0, min(1.0, similarity))


# ======================== 质量评判器（统一入口） ========================

class QualityJudge:
    """质量评判器 — 统一入口
    
    整合启发式评分、LLM-as-Judge 和黄金对照集匹配。
    """
    
    def __init__(self, llm_api_key: str = None, golden_path: str = None,
                 rubric: dict = None):
        self.heuristic_scorer = HeuristicScorer(rubric=rubric)
        self.llm_judge = LLMJudge(api_key=llm_api_key) if llm_api_key else None
        self.golden_matcher = GoldenReferenceMatcher(golden_path) if golden_path else None
        self.rubric = rubric or RUBRIC
    
    def evaluate(self, data: dict, intent: str = None,
                 reference: str = None, use_llm: bool = False) -> dict:
        """执行质量评估
        
        Args:
            data: 待评估的数据
            intent: 用户原始意图
            reference: 参考产出
            use_llm: 是否使用 LLM-as-Judge
        
        Returns:
            包含评分、匹配结果和总体评估的字典
        """
        # 1. 启发式评分
        heuristic_scores = self.heuristic_scorer.score(data)
        
        # 2. LLM-as-Judge 评分（可选）
        llm_scores = None
        if use_llm and self.llm_judge:
            sample_text = json.dumps(data, default=str, ensure_ascii=False)
            llm_scores = self.llm_judge.evaluate(sample_text, reference, intent)
        
        # 3. 黄金对照集匹配（可选）
        golden_match = None
        if self.golden_matcher:
            golden_match = self.golden_matcher.match(data)
        
        # 4. 计算加权平均分
        scores = llm_scores if (use_llm and llm_scores) else heuristic_scores
        avg_score = self._compute_weighted_average(scores)
        
        # 5. 生成评估报告
        report = {
            "overall_score": avg_score,
            "heuristic_scores": heuristic_scores,
            "llm_scores": llm_scores,
            "golden_match": golden_match,
            "verdict": self._compute_verdict(avg_score),
            "timestamp": datetime.now().isoformat(),
        }
        
        return report
    
    def evaluate_directory(self, directory: str, use_llm: bool = False) -> dict:
        """评估整个目录
        
        Args:
            directory: 交付物目录路径
            use_llm: 是否使用 LLM-as-Judge
        
        Returns:
            包含所有文件评分的字典
        """
        dir_path = Path(directory)
        if not dir_path.exists():
            return {"error": f"目录不存在: {directory}"}
        
        # 收集所有文件内容
        all_data = {}
        for file_path in dir_path.rglob("*"):
            if file_path.is_file() and file_path.suffix in (".py", ".json", ".yaml", ".yml", ".md", ".toml"):
                try:
                    with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                        all_data[file_path.name] = f.read()
                except (UnicodeDecodeError, PermissionError):
                    pass
        
        # 对每个文件进行评估
        results = {}
        for filename, content in all_data.items():
            results[filename] = self.evaluate(
                {"content": content, "filename": filename},
                use_llm=use_llm,
            )
        
        # 计算目录级别的总体评分
        if results:
            avg_scores = []
            for filename, report in results.items():
                avg_scores.append(report["overall_score"])
            directory_avg = sum(avg_scores) / len(avg_scores)
        else:
            directory_avg = 0.0
        
        return {
            "directory": directory,
            "overall_score": directory_avg,
            "file_count": len(results),
            "verdict": self._compute_verdict(directory_avg),
            "files": results,
            "timestamp": datetime.now().isoformat(),
        }
    
    def _compute_weighted_average(self, scores: dict) -> float:
        """计算加权平均分"""
        total_weight = 0.0
        weighted_sum = 0.0
        
        for dimension, score_data in scores.items():
            if dimension not in self.rubric:
                continue
            weight = self.rubric[dimension].get("weight", 0.2)
            raw_score = score_data.get("raw", 3.0)
            weighted_sum += raw_score * weight
            total_weight += weight
        
        return weighted_sum / total_weight if total_weight > 0 else 3.0
    
    def _compute_verdict(self, score: float) -> str:
        """根据分数计算判定结果"""
        if score >= 4.0:
            return "EXCELLENT"
        elif score >= 3.5:
            return "GOOD"
        elif score >= 3.0:
            return "ACCEPTABLE"
        elif score >= 2.5:
            return "NEEDS_IMPROVEMENT"
        else:
            return "POOR"


# ======================== CLI 入口 ========================

def main():
    parser = argparse.ArgumentParser(description="Quality Judge - 5维质量评判器")
    parser.add_argument("--input", type=str, required=True,
                        help="交付物目录路径")
    parser.add_argument("--rubric", type=str, default=None,
                        help="评分准则 JSON 文件路径（可选）")
    parser.add_argument("--golden", type=str, default=None,
                        help="黄金对照集 JSON 文件路径（可选）")
    parser.add_argument("--llm-judge", action="store_true", default=False,
                        help="启用 LLM-as-Judge 评估（需要 API key）")
    parser.add_argument("--output", type=str, default=None,
                        help="输出报告路径（可选）")
    
    args = parser.parse_args()
    
    # 创建质量评判器
    judge = QualityJudge(
        llm_api_key=os.environ.get("QUALITY_JUDGE_LLM") if args.llm_judge else None,
        golden_path=args.golden,
    )
    
    # 执行评估
    if Path(args.input).is_dir():
        result = judge.evaluate_directory(args.input, use_llm=args.llm_judge)
    else:
        # 单文件评估
        with open(args.input, "r", encoding="utf-8") as f:
            content = f.read()
        result = judge.evaluate(
            {"content": content, "filename": Path(args.input).name},
            use_llm=args.llm_judge,
        )
    
    # 输出结果
    output = json.dumps(result, indent=2, ensure_ascii=False, default=str)
    print(output)
    
    # 保存报告
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(output)
        print(f"\n[INFO] 报告已保存到: {args.output}")


if __name__ == "__main__":
    main()

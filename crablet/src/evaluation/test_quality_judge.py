#!/usr/bin/env python3
"""
Tests for quality_judge.py

测试覆盖：
1. 5维启发式评分器 (HeuristicScorer)
2. LLM-as-Judge 评分器 (LLMJudge)
3. 黄金对照集匹配器 (GoldenReferenceMatcher)
4. 质量评判器统一入口 (QualityJudge)
5. 边界情况处理
6. 性能测试
"""

import json
import os
import sys
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

# 添加当前目录到 sys.path 以便导入 quality_judge
sys.path.insert(0, os.path.dirname(__file__))

from quality_judge import (
    HeuristicScorer, LLMJudge, GoldenReferenceMatcher, QualityJudge,
    RUBRIC, GOLDEN_REFERENCE_SCHEMA,
)


# ======================== Pytest Fixtures ========================

@pytest.fixture
def scorer():
    return HeuristicScorer()


@pytest.fixture
def golden_matcher(tmp_path):
    """创建带黄金对照集的匹配器"""
    golden_file = tmp_path / "golden.json"
    golden_data = {
        "examples": [
            {"id": "golden_001", "intent": 5, "topology": 5, "nodes": 5, "complexity": 5, "family": "approval"},
            {"id": "golden_002", "intent": 3, "topology": 3, "nodes": 3, "complexity": 3, "family": "notification"},
            {"id": "golden_003", "intent": 1, "topology": 1, "nodes": 1, "complexity": 1, "family": "broken"},
        ]
    }
    golden_file.write_text(json.dumps(golden_data))
    return GoldenReferenceMatcher(str(golden_file))


@pytest.fixture
def quality_judge():
    return QualityJudge()


@pytest.fixture
def sample_dir(tmp_path):
    """创建示例交付物目录"""
    # 创建目录结构
    (tmp_path / "src").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "docs").mkdir()
    
    # 创建文件
    (tmp_path / "manifest.json").write_text(json.dumps({
        "name": "test-deliverable",
        "version": "0.1.0",
    }))
    (tmp_path / "config.json").write_text(json.dumps({
        "name": "test-deliverable",
        "version": "0.1.0",
        "api_key": "test-key",
    }))
    (tmp_path / "README.md").write_text("# Test Deliverable\n\nThis is a test.")
    (tmp_path / "src" / "main.py").write_text("def main():\n    print('hello')\n")
    (tmp_path / "tests" / "test_main.py").write_text("def test_main():\n    assert True\n")
    
    return tmp_path


# ======================== HeuristicScorer 测试 ========================

class TestHeuristicScorer:
    """测试启发式评分器"""
    
    def test_score_returns_all_dimensions(self, scorer):
        """测试评分返回所有维度"""
        scores = scorer.score({"content": "test"})
        assert set(scores.keys()) == set(RUBRIC.keys())
    
    def test_score_intent_fidelity_with_explicit_intent(self, scorer):
        """测试明确意图的评分"""
        scores = scorer.score({"intent": 4})
        assert scores["intent_fidelity"]["raw"] == 4.0
    
    def test_score_intent_fidelity_with_keywords(self, scorer):
        """测试关键词匹配的意图评分"""
        scores = scorer.score({"content": "创建一个新的工作流"})
        assert scores["intent_fidelity"]["raw"] == 4.0
    
    def test_score_intent_fidelity_no_keywords(self, scorer):
        """测试无关键词的意图评分"""
        scores = scorer.score({"content": "random text"})
        assert scores["intent_fidelity"]["raw"] == 2.0
    
    def test_score_topology_with_dag(self, scorer):
        """测试 DAG 拓扑评分"""
        scores = scorer.score({"dependencies": [{"id": "A"}, {"id": "B"}]})
        assert scores["topology_rationality"]["raw"] == 4.0
    
    def test_score_topology_no_deps(self, scorer):
        """测试无依赖的拓扑评分"""
        scores = scorer.score({"content": "test"})
        assert scores["topology_rationality"]["raw"] == 3.0
    
    def test_score_node_necessity_no_duplicates(self, scorer):
        """测试无重复节点的评分"""
        scores = scorer.score({"nodes": [{"id": "A"}, {"id": "B"}, {"id": "C"}]})
        assert scores["node_necessity"]["raw"] == 5.0
    
    def test_score_node_necessity_with_duplicates(self, scorer):
        """测试有重复节点的评分"""
        scores = scorer.score({"nodes": [{"id": "A"}, {"id": "A"}]})
        assert scores["node_necessity"]["raw"] == 1.0
    
    def test_score_over_simplification_with_key_info(self, scorer):
        """测试保留关键信息的评分"""
        scores = scorer.score({"content": "version: 1.0, api: /v1"})
        assert scores["over_simplification"]["raw"] == 4.0
    
    def test_score_over_simplification_no_key_info(self, scorer):
        """测试丢失关键信息的评分"""
        scores = scorer.score({"content": "simple text"})
        assert scores["over_simplification"]["raw"] == 2.0
    
    def test_score_over_complexity_clean(self, scorer):
        """测试简洁代码的评分"""
        scores = scorer.score({"content": "clean code"})
        assert scores["over_complexity"]["raw"] == 4.0
    
    def test_score_over_complexity_nested(self, scorer):
        """测试过度复杂的评分"""
        scores = scorer.score({"content": "wrapper wrapper factory"})
        assert scores["over_complexity"]["raw"] == 1.0
    
    def test_score_clamped(self, scorer):
        """测试分数在范围内"""
        scores = scorer.score({"content": "test"})
        for dim, score_data in scores.items():
            assert score_data["clamped"] >= 1
            assert score_data["clamped"] <= 5


# ======================== LLMJudge 测试 ========================

class TestLLMJudge:
    """测试 LLM-as-Judge 评分器"""
    
    def test_evaluate_without_api_key_falls_back(self):
        """测试无 API key 时回退到启发式评分"""
        judge = LLMJudge(api_key=None)
        result = judge.evaluate("test content")
        assert "intent_fidelity" in result
        assert "topology_rationality" in result
    
    def test_evaluate_with_invalid_api_key_falls_back(self):
        """测试无效 API key 时回退到启发式评分"""
        judge = LLMJudge(api_key="invalid-key")
        result = judge.evaluate("test content")
        # 应该回退到启发式评分
        assert "intent_fidelity" in result
    
    def test_parse_llm_response_valid_json(self):
        """测试解析有效的 LLM JSON 响应"""
        judge = LLMJudge()
        response = '''```json
{
  "intent_fidelity": {"score": 4, "reason": "高度符合意图"},
  "topology_rationality": {"score": 3, "reason": "基本合理"},
  "node_necessity": {"score": 5, "reason": "无冗余节点"},
  "over_simplification": {"score": 4, "reason": "保留了关键信息"},
  "over_complexity": {"score": 5, "reason": "简洁"}
}
```'''
        result = judge._parse_llm_response(response)
        assert result["intent_fidelity"]["raw"] == 4.0
        assert result["topology_rationality"]["raw"] == 3.0
        assert result["node_necessity"]["raw"] == 5.0
    
    def test_parse_llm_response_invalid_json(self):
        """测试解析无效的 LLM 响应"""
        judge = LLMJudge()
        result = judge._parse_llm_response("This is not JSON")
        # 应该使用默认分数
        for dim in RUBRIC:
            assert dim in result
            assert result[dim]["raw"] == 3.0
    
    def test_format_rubric(self):
        """测试评分准则格式化"""
        judge = LLMJudge()
        rubric_text = judge._format_rubric()
        assert "intent_fidelity" in rubric_text
        assert "topology_rationality" in rubric_text
        assert "权重" in rubric_text


# ======================== GoldenReferenceMatcher 测试 ========================

class TestGoldenReferenceMatcher:
    """测试黄金对照集匹配器"""
    
    def test_match_perfect(self, golden_matcher):
        """测试完美匹配"""
        sample = {"intent": 5, "topology": 5, "nodes": 5, "complexity": 5}
        result = golden_matcher.match(sample)
        assert result["best_match"]["case_id"] == "golden_001"
        assert result["similarity"] >= 0.9
    
    def test_match_partial(self, golden_matcher):
        """测试部分匹配"""
        sample = {"intent": 3, "topology": 3, "nodes": 3, "complexity": 3}
        result = golden_matcher.match(sample)
        assert result["best_match"]["case_id"] == "golden_002"
        assert result["similarity"] >= 0.5
    
    def test_match_poor(self, golden_matcher):
        """测试差匹配"""
        sample = {"intent": 1, "topology": 1, "nodes": 1, "complexity": 1}
        result = golden_matcher.match(sample)
        assert result["best_match"]["case_id"] == "golden_003"
        assert result["similarity"] >= 0.9
    
    def test_match_no_golden(self):
        """测试无黄金对照集"""
        matcher = GoldenReferenceMatcher()
        result = matcher.match({"intent": 3})
        assert result["best_match"] is None
        assert result["similarity"] == 0.0
    
    def test_match_returns_all_matches(self, golden_matcher):
        """测试返回所有匹配"""
        sample = {"intent": 3, "topology": 3, "nodes": 3, "complexity": 3}
        result = golden_matcher.match(sample)
        assert len(result["all_matches"]) == 3
    
    def test_match_sorted_by_similarity(self, golden_matcher):
        """测试匹配结果按相似度排序"""
        sample = {"intent": 3, "topology": 3, "nodes": 3, "complexity": 3}
        result = golden_matcher.match(sample)
        similarities = [m["similarity"] for m in result["all_matches"]]
        assert similarities == sorted(similarities, reverse=True)


# ======================== QualityJudge 测试 ========================

class TestQualityJudge:
    """测试质量评判器统一入口"""
    
    def test_evaluate_basic(self, quality_judge):
        """测试基本评估"""
        result = quality_judge.evaluate({"content": "test"})
        assert "overall_score" in result
        assert "heuristic_scores" in result
        assert "verdict" in result
    
    def test_evaluate_with_intent(self, quality_judge):
        """测试带意图的评估"""
        result = quality_judge.evaluate({"content": "创建工作流"}, intent="创建审批流程")
        assert result["overall_score"] >= 1.0
    
    def test_evaluate_directory(self, quality_judge, sample_dir):
        """测试目录评估"""
        result = quality_judge.evaluate_directory(str(sample_dir))
        assert "overall_score" in result
        assert "file_count" in result
        assert result["file_count"] > 0
    
    def test_evaluate_directory_nonexistent(self, quality_judge):
        """测试不存在的目录"""
        result = quality_judge.evaluate_directory("/nonexistent/path")
        assert "error" in result
    
    def test_verdict_excellent(self, quality_judge):
        """测试优秀判定"""
        verdict = quality_judge._compute_verdict(4.5)
        assert verdict == "EXCELLENT"
    
    def test_verdict_good(self, quality_judge):
        """测试良好判定"""
        verdict = quality_judge._compute_verdict(3.7)
        assert verdict == "GOOD"
    
    def test_verdict_acceptable(self, quality_judge):
        """测试可接受判定"""
        verdict = quality_judge._compute_verdict(3.2)
        assert verdict == "ACCEPTABLE"
    
    def test_verdict_needs_improvement(self, quality_judge):
        """测试需改进判定"""
        verdict = quality_judge._compute_verdict(2.7)
        assert verdict == "NEEDS_IMPROVEMENT"
    
    def test_verdict_poor(self, quality_judge):
        """测试差判定"""
        verdict = quality_judge._compute_verdict(1.5)
        assert verdict == "POOR"
    
    def test_evaluate_with_golden(self, tmp_path):
        """测试带黄金对照集的评估"""
        golden_file = tmp_path / "golden.json"
        golden_data = {
            "examples": [
                {"id": "test_001", "intent": 4, "topology": 4, "nodes": 4, "complexity": 4, "family": "test"},
            ]
        }
        golden_file.write_text(json.dumps(golden_data))
        
        judge = QualityJudge(golden_path=str(golden_file))
        result = judge.evaluate({"intent": 4, "topology": 4, "nodes": 4, "complexity": 4})
        
        assert result["golden_match"] is not None
        assert result["golden_match"]["best_match"]["case_id"] == "test_001"


# ======================== 边界情况测试 ========================

class TestEdgeCases:
    """测试边界情况"""
    
    def test_empty_data(self, scorer):
        """测试空数据"""
        scores = scorer.score({})
        assert all(dim in scores for dim in RUBRIC)
    
    def test_very_large_data(self, scorer):
        """测试大数据"""
        large_data = {"content": "x" * 100000}
        scores = scorer.score(large_data)
        assert all(dim in scores for dim in RUBRIC)
    
    def test_unicode_data(self, scorer):
        """测试 Unicode 数据"""
        scores = scorer.score({"content": "你好世界 🌍"})
        assert all(dim in scores for dim in RUBRIC)
    
    def test_special_characters(self, scorer):
        """测试特殊字符"""
        scores = scorer.score({"content": "!@#$%^&*()"})
        assert all(dim in scores for dim in RUBRIC)
    
    def test_nested_data(self, scorer):
        """测试嵌套数据"""
        scores = scorer.score({
            "level1": {
                "level2": {
                    "level3": "deep value"
                }
            }
        })
        assert all(dim in scores for dim in RUBRIC)


# ======================== 性能测试 ========================

class TestPerformance:
    """性能测试"""
    
    def test_heuristic_scorer_speed(self, scorer):
        """测试启发式评分器速度"""
        import time
        
        start = time.time()
        for _ in range(100):
            scorer.score({"content": "test content for performance"})
        duration = time.time() - start
        
        # 100次评分应在1秒内完成
        assert duration < 1.0
    
    def test_golden_matcher_speed(self, golden_matcher):
        """测试黄金对照集匹配速度"""
        import time
        
        start = time.time()
        for _ in range(100):
            golden_matcher.match({"intent": 3, "topology": 3, "nodes": 3, "complexity": 3})
        duration = time.time() - start
        
        # 100次匹配应在1秒内完成
        assert duration < 1.0


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

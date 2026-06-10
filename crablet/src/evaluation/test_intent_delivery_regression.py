#!/usr/bin/env python3
"""
Tests for intent_delivery_regression.py

测试覆盖：
1. 基线建立与加载
2. 回归检测
3. 批量检测
4. 报告生成
"""

import json
import os
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, os.path.dirname(__file__))

from intent_delivery_regression import IntentDeliveryRegression, RegressionReport


# ======================== Pytest Fixtures ========================

@pytest.fixture
def baseline_dir(tmp_path):
    """创建基线目录"""
    baseline_file = tmp_path / "quality_baseline.json"
    baseline_data = {
        "version": "1.0.0",
        "baselines": [
            {
                "name": "test_flow",
                "overall_score": 3.5,
                "dimension_scores": {
                    "intent_fidelity": 4.0,
                    "topology_rationality": 3.5,
                    "node_necessity": 3.0,
                    "over_simplification": 3.5,
                    "over_complexity": 3.5,
                },
                "timestamp": "2026-06-09T00:00:00",
                "metadata": {"family": "test"},
            }
        ]
    }
    baseline_file.write_text(json.dumps(baseline_data))
    return str(baseline_file)


@pytest.fixture
def regression(baseline_dir):
    """创建回归管理器"""
    return IntentDeliveryRegression(baseline_path=baseline_dir)


# ======================== 基线管理测试 ========================

class TestBaselineManagement:
    """测试基线管理"""
    
    def test_load_existing_baseline(self, baseline_dir):
        """测试加载已有基线"""
        reg = IntentDeliveryRegression(baseline_path=baseline_dir)
        baselines = reg.list_baselines()
        assert len(baselines) == 1
        assert baselines[0]["name"] == "test_flow"
        assert baselines[0]["overall_score"] == 3.5
    
    def test_establish_new_baseline(self, tmp_path):
        """测试建立新基线"""
        baseline_file = str(tmp_path / "new_baseline.json")
        reg = IntentDeliveryRegression(baseline_path=baseline_file)
        
        scores = {
            "overall_score": 4.0,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 4.0},
                "topology_rationality": {"raw": 4.0},
                "node_necessity": {"raw": 4.0},
                "over_simplification": {"raw": 4.0},
                "over_complexity": {"raw": 4.0},
            },
        }
        
        reg.establish_baseline("new_flow", scores)
        
        # 重新加载
        reg2 = IntentDeliveryRegression(baseline_path=baseline_file)
        baselines = reg2.list_baselines()
        assert len(baselines) == 1
        assert baselines[0]["name"] == "new_flow"
        assert baselines[0]["overall_score"] == 4.0
    
    def test_update_existing_baseline(self, regression, baseline_dir):
        """测试更新已有基线"""
        new_scores = {
            "overall_score": 4.2,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 4.5},
                "topology_rationality": {"raw": 4.0},
                "node_necessity": {"raw": 4.0},
                "over_simplification": {"raw": 4.0},
                "over_complexity": {"raw": 4.0},
            },
        }
        
        regression.establish_baseline("test_flow", new_scores)
        
        # 重新加载
        reg2 = IntentDeliveryRegression(baseline_path=baseline_dir)
        baselines = reg2.list_baselines()
        assert baselines[0]["overall_score"] == 4.2


# ======================== 回归检测测试 ========================

class TestRegressionDetection:
    """测试回归检测"""
    
    def test_no_regression(self, regression):
        """测试无回归"""
        current_scores = {
            "overall_score": 3.6,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 4.0},
                "topology_rationality": {"raw": 3.5},
                "node_necessity": {"raw": 3.0},
                "over_simplification": {"raw": 3.5},
                "over_complexity": {"raw": 3.5},
            },
        }
        
        result = regression.check_regression("test_flow", current_scores)
        assert not result.is_regression
        assert result.delta >= 0
    
    def test_regression_detected(self, regression):
        """测试检测到回归"""
        current_scores = {
            "overall_score": 2.8,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 3.0},
                "topology_rationality": {"raw": 2.5},
                "node_necessity": {"raw": 2.5},
                "over_simplification": {"raw": 3.0},
                "over_complexity": {"raw": 3.0},
            },
        }
        
        result = regression.check_regression("test_flow", current_scores)
        assert result.is_regression
        assert result.delta < -0.3
    
    def test_improvement_detected(self, regression):
        """测试检测到改进"""
        current_scores = {
            "overall_score": 4.2,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 4.5},
                "topology_rationality": {"raw": 4.0},
                "node_necessity": {"raw": 4.0},
                "over_simplification": {"raw": 4.0},
                "over_complexity": {"raw": 4.0},
            },
        }
        
        result = regression.check_regression("test_flow", current_scores)
        assert not result.is_regression
        assert result.delta > 0.3
    
    def test_no_baseline(self, regression):
        """测试无基线"""
        current_scores = {
            "overall_score": 3.5,
            "heuristic_scores": {},
        }
        
        result = regression.check_regression("nonexistent_flow", current_scores)
        assert not result.is_regression
        assert result.details == "无基线数据"
    
    def test_dimension_deltas(self, regression):
        """测试维度差异"""
        current_scores = {
            "overall_score": 3.2,
            "heuristic_scores": {
                "intent_fidelity": {"raw": 3.5},
                "topology_rationality": {"raw": 3.0},
                "node_necessity": {"raw": 2.5},
                "over_simplification": {"raw": 3.5},
                "over_complexity": {"raw": 3.5},
            },
        }
        
        result = regression.check_regression("test_flow", current_scores)
        assert "intent_fidelity" in result.dimension_deltas
        assert result.dimension_deltas["intent_fidelity"] == -0.5


# ======================== 批量检测测试 ========================

class TestBatchCheck:
    """测试批量检测"""
    
    def test_batch_check_mixed(self, regression):
        """测试混合批量检测"""
        current_scores = {
            "test_flow": {
                "overall_score": 3.6,
                "heuristic_scores": {
                    "intent_fidelity": {"raw": 4.0},
                    "topology_rationality": {"raw": 3.5},
                    "node_necessity": {"raw": 3.0},
                    "over_simplification": {"raw": 3.5},
                    "over_complexity": {"raw": 3.5},
                },
            },
            "new_flow": {
                "overall_score": 4.0,
                "heuristic_scores": {
                    "intent_fidelity": {"raw": 4.0},
                    "topology_rationality": {"raw": 4.0},
                    "node_necessity": {"raw": 4.0},
                    "over_simplification": {"raw": 4.0},
                    "over_complexity": {"raw": 4.0},
                },
            },
        }
        
        report = regression.batch_check(current_scores)
        assert report.total_entries == 2
        assert isinstance(report, RegressionReport)
    
    def test_batch_check_all_stable(self, regression):
        """测试全部稳定"""
        current_scores = {
            "test_flow": {
                "overall_score": 3.5,
                "heuristic_scores": {
                    "intent_fidelity": {"raw": 4.0},
                    "topology_rationality": {"raw": 3.5},
                    "node_necessity": {"raw": 3.0},
                    "over_simplification": {"raw": 3.5},
                    "over_complexity": {"raw": 3.5},
                },
            },
        }
        
        report = regression.batch_check(current_scores)
        assert report.stable >= 1
    
    def test_batch_check_with_regression(self, regression):
        """测试有回归的批量检测"""
        current_scores = {
            "test_flow": {
                "overall_score": 2.5,
                "heuristic_scores": {
                    "intent_fidelity": {"raw": 2.5},
                    "topology_rationality": {"raw": 2.5},
                    "node_necessity": {"raw": 2.5},
                    "over_simplification": {"raw": 2.5},
                    "over_complexity": {"raw": 2.5},
                },
            },
        }
        
        report = regression.batch_check(current_scores)
        assert report.regressions >= 1
        assert report.overall_status == "REGRESSION_DETECTED"


# ======================== 报告生成测试 ========================

class TestReportGeneration:
    """测试报告生成"""
    
    def test_report_to_dict(self, regression):
        """测试报告序列化"""
        current_scores = {
            "test_flow": {
                "overall_score": 3.5,
                "heuristic_scores": {
                    "intent_fidelity": {"raw": 4.0},
                    "topology_rationality": {"raw": 3.5},
                    "node_necessity": {"raw": 3.0},
                    "over_simplification": {"raw": 3.5},
                    "over_complexity": {"raw": 3.5},
                },
            },
        }
        
        report = regression.batch_check(current_scores)
        report_dict = report.to_dict()
        
        assert "total_entries" in report_dict
        assert "results" in report_dict
        assert len(report_dict["results"]) == 1


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

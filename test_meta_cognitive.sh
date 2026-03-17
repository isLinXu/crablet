#!/bin/bash
set -e

echo "=========================================="
echo "Crablet Meta-Cognitive System Test Script"
echo "=========================================="
echo ""

# 切换到项目目录
cd /Users/gatilin/PycharmProjects/crablet-latest-v260313/crablet

echo "1. Checking compilation..."
cargo check --lib 2>&1 | grep -E "(error|warning:.*unused)" || echo "✓ Compilation check passed"

echo ""
echo "2. Running unit tests for meta_controller..."
cargo test --lib meta_controller 2>&1 | tail -20

echo ""
echo "3. Running integration tests..."
cargo test --test integration_meta_cognitive_test 2>&1 | tail -20

echo ""
echo "4. Running simple meta tests..."
cargo test --test meta_simple_test 2>&1 | tail -20

echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "✓ All tests completed"

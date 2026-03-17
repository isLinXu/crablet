#!/bin/bash
# Crablet Meta-Cognitive System - Final Verification Script

set -e

echo "========================================================================"
echo "  Crablet 元认知系统 - 最终验证脚本"
echo "========================================================================"
echo ""

# 切换到项目目录
cd /Users/gatilin/PycharmProjects/crablet-latest-v260313/crablet

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 统计函数
success_count=0
fail_count=0

print_success() {
    echo -e "${GREEN}✓${NC} $1"
    ((success_count++))
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
    ((fail_count++))
}

print_section() {
    echo ""
    echo "========================================================================"
    echo "  $1"
    echo "========================================================================"
    echo ""
}

# 1. 磁盘空间检查
print_section "1. 磁盘空间检查"

available_space=$(df -h . | awk 'NR==2 {print $4}' | sed 's/G//')
if (( $(echo "$available_space < 5.0" | bc -l) )); then
    print_error "磁盘空间不足: ${available_space}GB (需要至少 5GB)"
    echo "请运行: cargo clean"
    exit 1
else
    print_success "磁盘空间充足: ${available_space}GB"
fi

# 2. 编译检查
print_section "2. 编译检查"

echo "运行 cargo check --lib..."
if cargo check --lib 2>&1 | tee /tmp/cargo_check.log; then
    print_success "编译检查通过"

    # 检查是否有警告
    warning_count=$(grep -c "warning:" /tmp/cargo_check.log || true)
    if [ $warning_count -eq 0 ]; then
        print_success "无编译警告"
    else
        print_warning "发现 $warning_count 个编译警告"
    fi
else
    print_error "编译检查失败"
    echo "查看错误: cat /tmp/cargo_check.log"
fi

# 3. 单元测试
print_section "3. 元认知单元测试"

echo "运行 meta_controller 单元测试..."
if cargo test --lib meta_controller 2>&1 | tee /tmp/unit_test.log; then
    test_count=$(grep -o "test result: ok" /tmp/unit_test.log | wc -l)
    print_success "单元测试通过 ($test_count 个测试套件)"
else
    print_error "单元测试失败"
    echo "查看错误: cat /tmp/unit_test.log"
fi

# 4. 集成测试
print_section "4. 集成测试"

echo "运行 integration_meta_cognitive_test..."
if cargo test --test integration_meta_cognitive_test 2>&1 | tee /tmp/integration_test.log; then
    print_success "集成测试通过"
else
    print_error "集成测试失败"
    echo "查看错误: cat /tmp/integration_test.log"
fi

echo "运行 meta_simple_test..."
if cargo test --test meta_simple_test 2>&1 | tee /tmp/simple_test.log; then
    print_success "简单测试通过"
else
    print_error "简单测试失败"
    echo "查看错误: cat /tmp/simple_test.log"
fi

# 5. 代码质量检查
print_section "5. 代码质量检查"

echo "运行 Clippy..."
if cargo clippy --lib 2>&1 | tee /tmp/clippy.log; then
    print_success "Clippy 检查通过"
else
    print_warning "Clippy 发现一些问题"
fi

echo "运行 fmt 检查..."
if cargo fmt --all -- --check 2>&1; then
    print_success "代码格式正确"
else
    print_warning "代码格式需要调整"
    echo "运行: cargo fmt --all"
fi

# 6. 性能测试 (可选)
print_section "6. 性能测试 (可选)"

echo "运行性能测试..."
if cargo test --lib -- --nocapture performance 2>&1 | tee /tmp/performance_test.log; then
    print_success "性能测试完成"
else
    print_warning "性能测试未执行或失败"
fi

# 7. 文档生成
print_section "7. 文档生成"

echo "生成文档..."
if cargo doc --no-deps --lib 2>&1 | tee /tmp/doc_gen.log; then
    print_success "文档生成成功"
    echo "文档位置: target/doc/crablet/index.html"
else
    print_warning "文档生成失败"
fi

# 8. Release 构建
print_section "8. Release 构建"

echo "构建 release 版本..."
if cargo build --release 2>&1 | tee /tmp/release_build.log; then
    print_success "Release 构建成功"
    echo "二进制位置: target/release/crablet"
else
    print_error "Release 构建失败"
    echo "查看错误: cat /tmp/release_build.log"
fi

# 9. 最终统计
print_section "验证总结"

echo ""
echo "成功项目: $success_count"
echo "失败项目: $fail_count"
echo ""

if [ $fail_count -eq 0 ]; then
    print_success "✅ 所有验证通过!"
    echo ""
    echo "元认知系统已准备就绪,可以部署使用。"
else
    print_error "❌ 部分验证失败,请检查上述错误"
    echo ""
    echo "建议修复以下问题后重新运行此脚本。"
fi

echo ""
echo "日志文件位置:"
echo "  - 编译检查: /tmp/cargo_check.log"
echo "  - 单元测试: /tmp/unit_test.log"
echo "  - 集成测试: /tmp/integration_test.log"
echo "  - 简单测试: /tmp/simple_test.log"
echo "  - Clippy: /tmp/clippy.log"
echo "  - Release 构建: /tmp/release_build.log"
echo ""
echo "========================================================================"

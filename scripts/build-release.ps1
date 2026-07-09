#Requires -Version 5.1
# ============================================================
# build-release.ps1 — [已弃用] 请使用 scripts/pack.sh
#
# 本脚本已被 scripts/pack.sh（v3 统一打包脚本）取代。
# Windows runner 自带 Git Bash，可直接运行 pack.sh。
#
# 迁移指南：
#   .\scripts\build-release.ps1              →  bash scripts/pack.sh --ci
#   .\scripts\build-release.ps1 -BuildTauri  →  bash scripts/pack.sh --ci
#
# 多平台交叉编译 sidecar（旧脚本的核心功能）可通过环境变量实现：
#   rustup target add <target>
#   cargo build --release -p crablet --target <target> --no-default-features --features web
#   Copy-Item target\<target>\release\crablet.exe desktop\binaries\crablet-<target>.exe
# ============================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path

Write-Host "⚠️  scripts/build-release.ps1 已弃用，转发到 scripts/pack.sh ..." -ForegroundColor Yellow
Write-Host "   建议今后直接使用: bash scripts/pack.sh [选项]" -ForegroundColor Yellow
Write-Host ""

# 转发到统一打包脚本（CI 模式，无交互）
if (Get-Command bash -ErrorAction SilentlyContinue) {
    & bash "$ProjectRoot/scripts/pack.sh" --ci
    exit $LASTEXITCODE
} else {
    Write-Host "❌ bash 不可用，请在 Git Bash 中运行: bash scripts/pack.sh" -ForegroundColor Red
    exit 1
}

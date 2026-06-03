<#
build-desktop.ps1 — Crablet 桌面应用 Windows 打包脚本

用法（在 Windows PowerShell 中运行）：
    .\build-desktop.ps1            # 构建 crablet 后端 + Tauri NSIS 安装包 (.exe)
    .\build-desktop.ps1 -SkipBackend   # 跳过后端编译（已有 target\release\crablet.exe）

前置条件：
    - Rust 工具链（rustup，含 x86_64-pc-windows-msvc target）
    - Visual Studio Build Tools（MSVC + Windows SDK）
    - cargo-tauri:   cargo install tauri-cli --version "^2"
    - WebView2 Runtime（Win11 自带；Win10 需安装，NSIS 安装包也会自动引导）

产物：
    target\release\bundle\nsis\Crablet_0.1.0_x64-setup.exe   (安装包)
    target\release\crablet-desktop.exe                       (主程序，可直接运行)
#>

param(
    [switch]$SkipBackend
)

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = Split-Path -Parent $ScriptDir
$DesktopDir  = Join-Path $ProjectRoot "desktop"
$TargetDir   = Join-Path $ProjectRoot "target\release"
$SidecarName = "crablet-x86_64-pc-windows-msvc.exe"

Write-Host "Crablet Windows 桌面打包" -ForegroundColor Cyan
Write-Host "   项目根: $ProjectRoot"
Write-Host ""

# --- Step 1: 编译 crablet 后端 (release) ---
if (-not $SkipBackend) {
    Write-Host "[1/4] 编译 crablet 后端 (release)..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    # crt-static: 静态链接 CRT，避免目标机缺少 VC++ 运行库
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
    cargo build --release -p crablet
    Pop-Location
} else {
    Write-Host "[1/4] 跳过后端编译 (-SkipBackend)" -ForegroundColor Yellow
}

if (-not (Test-Path (Join-Path $TargetDir "crablet.exe"))) {
    throw "找不到 $TargetDir\crablet.exe，请先编译后端。"
}

# --- Step 2: 复制 sidecar 二进制（Tauri v2 要求带 target triple 后缀）---
Write-Host "[2/4] 复制 sidecar 二进制..." -ForegroundColor Yellow
$BinariesDir = Join-Path $DesktopDir "binaries"
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
Copy-Item (Join-Path $TargetDir "crablet.exe") (Join-Path $BinariesDir $SidecarName) -Force
Write-Host "   已复制到: $BinariesDir\$SidecarName"

# --- Step 3: 构建 Tauri NSIS 安装包 ---
Write-Host "[3/4] 构建 Tauri NSIS 安装包..." -ForegroundColor Yellow
Push-Location $DesktopDir
cargo tauri build --bundles nsis
Pop-Location

# --- Step 4: 修复 sidecar 路径 ---
Write-Host "[4/4] 修复 NSIS 包内 sidecar 路径..." -ForegroundColor Yellow
$NsisDir = Join-Path $TargetDir "bundle\nsis"
if (Test-Path $NsisDir) {
    Get-ChildItem $NsisDir -Recurse -Filter "crablet.exe" -ErrorAction SilentlyContinue | ForEach-Object {
        $targetPath = Join-Path $_.DirectoryName $SidecarName
        if (-not (Test-Path $targetPath)) {
            Copy-Item $_.FullName $targetPath -Force
            Write-Host "   复制: $targetPath"
        }
    }
}

Write-Host ""
Write-Host "打包完成！" -ForegroundColor Green
Get-ChildItem $NsisDir -Filter "*.exe" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host ("   安装包: {0}  ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB))
}
Write-Host "   主程序: $TargetDir\crablet-desktop.exe"

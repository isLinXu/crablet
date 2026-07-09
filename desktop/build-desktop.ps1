<#
build-desktop.ps1 — [已弃用] Windows 打包统一入口

本脚本已简化为对 scripts/pack.sh 的等价 PowerShell 实现封装。
Windows 上没有原生 bash，因此本脚本保留独立实现，但逻辑已与 scripts/pack.sh 保持一致：
  前端构建 → sidecar 编译 (crt-static) → Tauri NSIS 打包 → sidecar 路径修复

用法（在 Windows PowerShell 中运行）：
    .\build-desktop.ps1                # 完整打包：前端 + 后端 + NSIS
    .\build-desktop.ps1 -SkipBackend    # 跳过后端编译（已有 target\release\crablet.exe）
    .\build-desktop.ps1 -SkipFrontend   # 跳过前端构建（已有 frontend\dist）

前置条件：
    - Rust 工具链（rustup，含 x86_64-pc-windows-msvc target）
    - Visual Studio Build Tools（MSVC + Windows SDK）
    - cargo-tauri:   cargo install tauri-cli --version "^2"
    - Node.js 20+ / npm（用于前端构建）
    - WebView2 Runtime（Win11 自带；Win10 需安装，NSIS 安装包也会自动引导）

产物：
    target\release\bundle\nsis\Crablet_<version>_x64-setup.exe   (安装包)
    target\release\crablet-desktop.exe                           (主程序，可直接运行)
#>

param(
    [switch]$SkipBackend,
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"

$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = Split-Path -Parent $ScriptDir
$DesktopDir  = Join-Path $ProjectRoot "desktop"
$FrontendDir = Join-Path $ProjectRoot "frontend"
$TargetDir   = Join-Path $ProjectRoot "target\release"
$UiDir       = Join-Path $DesktopDir "ui"
$SidecarName = "crablet-x86_64-pc-windows-msvc.exe"

# 版本号：单一真相源 crablet/Cargo.toml
$CargoToml = Join-Path $ProjectRoot "crablet\Cargo.toml"
$VersionLine = Select-String -Path $CargoToml -Pattern '^version' | Select-Object -First 1
$Version = if ($VersionLine) { ($VersionLine.Line -split '"')[1] } else { "0.1.0" }

Write-Host "🦀 Crablet Windows 桌面打包 (统一脚本对齐 scripts/pack.sh)" -ForegroundColor Cyan
Write-Host "   项目根: $ProjectRoot"
Write-Host "   版本:   $Version"
Write-Host ""

# --- Step 0: 前端构建 ---
if (-not $SkipFrontend) {
    Write-Host "[0/5] 前端 SPA 构建..." -ForegroundColor Yellow
    $distDir = Join-Path $FrontendDir "dist"
    if (Test-Path (Join-Path $distDir "index.html")) {
        Write-Host "   ✅ 前端产物已存在 ($distDir)"
    } else {
        if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
            throw "npm 未安装，无法构建前端。请安装 Node.js 或使用 -SkipFrontend 跳过。"
        }
        Push-Location $FrontendDir
        npm install
        npm run build
        Pop-Location
        Write-Host "   ✅ 前端构建完成"
    }
    Write-Host "   同步前端产物到 desktop\ui\ ..."
    if (Test-Path $UiDir) { Remove-Item -Recurse -Force $UiDir }
    Copy-Item -Recurse $distDir $UiDir
    Write-Host "   ✅ UI 同步完成"
} else {
    Write-Host "[0/5] 跳过前端构建 (-SkipFrontend)" -ForegroundColor Yellow
}

# --- Step 1: 编译 crablet 后端 (release, web feature only) ---
if (-not $SkipBackend) {
    Write-Host "[1/5] 编译 crablet 后端 (release, --features web)..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    # crt-static: 静态链接 CRT，避免目标机缺少 VC++ 运行库
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
    cargo build --release -p crablet --no-default-features --features web
    Pop-Location
} else {
    Write-Host "[1/5] 跳过后端编译 (-SkipBackend)" -ForegroundColor Yellow
}

if (-not (Test-Path (Join-Path $TargetDir "crablet.exe"))) {
    throw "找不到 $TargetDir\crablet.exe，请先编译后端。"
}

# --- Step 2: 复制 sidecar 二进制（Tauri v2 要求带 target triple 后缀）---
Write-Host "[2/5] 复制 sidecar 二进制..." -ForegroundColor Yellow
$BinariesDir = Join-Path $DesktopDir "binaries"
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
Copy-Item (Join-Path $TargetDir "crablet.exe") (Join-Path $BinariesDir $SidecarName) -Force
Write-Host "   已复制到: $BinariesDir\$SidecarName"

# --- Step 3: 构建 Tauri NSIS 安装包 ---
Write-Host "[3/5] 构建 Tauri NSIS 安装包..." -ForegroundColor Yellow
Push-Location $DesktopDir
if (Test-Path "package.json") {
    npm install --silent 2>$null
}
cargo tauri build --bundles nsis
Pop-Location

# --- Step 4: 修复 sidecar 路径 ---
Write-Host "[4/5] 修复 NSIS 包内 sidecar 路径..." -ForegroundColor Yellow
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

# --- Step 5: 代码签名（可选，需要 SIGNTOOL_CERT + SIGNTOOL_PASSWORD）---
Write-Host "[5/5] 代码签名（可选）..." -ForegroundColor Yellow
if ($env:SIGNTOOL_CERT -and (Get-Command signtool -ErrorAction SilentlyContinue)) {
    $exeFiles = Get-ChildItem $NsisDir -Filter "*.exe" -ErrorAction SilentlyContinue
    foreach ($exe in $exeFiles) {
        signtool sign /f $env:SIGNTOOL_CERT /p $env:SIGNTOOL_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $exe.FullName
        Write-Host "   ✅ 已签名: $($exe.Name)"
    }
} else {
    Write-Host "   ⚠️  未设置 SIGNTOOL_CERT 环境变量，跳过签名（SmartScreen 会提示未知发布者）"
}

Write-Host ""
Write-Host "打包完成！" -ForegroundColor Green
Get-ChildItem $NsisDir -Filter "*.exe" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host ("   安装包: {0}  ({1:N1} MB)" -f $_.FullName, ($_.Length / 1MB))
}
Write-Host "   主程序: $TargetDir\crablet-desktop.exe"

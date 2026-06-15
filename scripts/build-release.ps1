#Requires -Version 5.1
# ============================================================
# build-release.ps1 — Crablet 桌面端统一分发打包脚本 (Windows)
# 编译 sidecar → 复制到 binaries/ → 构建 Tauri
# ============================================================
param(
    [switch]$BuildSidecar = $true,
    [switch]$BuildTauri = $true,
    [string[]]$SkipSidecar = @()
)

$ErrorActionPreference = "Stop"

# 项目目录
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = (Resolve-Path "$ScriptDir\..").Path
$DesktopDir = "$ProjectRoot\desktop"
$BinariesDir = "$DesktopDir\binaries"
$CrateDir = "$ProjectRoot\crablet"

# 版本
$VersionLine = (Get-Content "$CrateDir\Cargo.toml" | Select-String -Pattern '^version\s*=').Line
$Version = [regex]::Match($VersionLine, '"([^"]+)"').Groups[1].Value

Write-Host "🦀 Crablet 桌面端打包脚本 v${Version}" -ForegroundColor Green
Write-Host "项目根目录: $ProjectRoot"
Write-Host ""

# 支持的 sidecar 目标平台
$SidecarTargets = @(
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu"
)

$SidecarName = "crablet"

# --------------------------------------------------
# 1) 编译 sidecar
# --------------------------------------------------
if ($BuildSidecar) {
    Write-Host "📦 Step 1: 编译 sidecar" -ForegroundColor Green
    New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null

    foreach ($target in $SidecarTargets) {
        if ($SkipSidecar -contains $target) {
            Write-Host "  ⏭️  跳过 $target" -ForegroundColor Yellow
            continue
        }

        # 检查工具链是否安装
        $installed = rustup target list --installed 2>$null | Select-String $target
        if (-not $installed) {
            Write-Host "  ⏭️  工具链未安装，跳过 $target (运行: rustup target add $target)" -ForegroundColor Yellow
            continue
        }

        Write-Host "  🔨 编译 $target ..." -ForegroundColor Green
        Push-Location $CrateDir

        try {
            cargo build --release --target $target 2>$null
            if ($target -like "*windows*") {
                Copy-Item "target\$target\release\${SidecarName}.exe" "$BinariesDir\${SidecarName}-${target}.exe" -Force -ErrorAction SilentlyContinue
            } else {
                Copy-Item "target\$target\release\${SidecarName}" "$BinariesDir\${SidecarName}-${target}" -Force -ErrorAction SilentlyContinue
            }
            Write-Host "  ✅ $target 完成" -ForegroundColor Green
        } catch {
            Write-Host "  ❌ $target 编译失败: $_" -ForegroundColor Red
        }

        Pop-Location
    }

    # 本地无后缀副本
    $hostTarget = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
    $hostExe = "$BinariesDir\${SidecarName}-${hostTarget}.exe"
    if (Test-Path $hostExe) {
        Copy-Item $hostExe "$BinariesDir\${SidecarName}.exe" -Force -ErrorAction SilentlyContinue
        Write-Host "  ✅ 创建通用 sidecar 副本 (无后缀)" -ForegroundColor Green
    }

    Write-Host ""
}

# --------------------------------------------------
# 2) 构建 Tauri
# --------------------------------------------------
if ($BuildTauri) {
    Write-Host "🚀 Step 2: 构建 Tauri 桌面端" -ForegroundColor Green
    Push-Location $DesktopDir

    if (Test-Path "package.json") {
        Write-Host "  📦 安装前端依赖 ..." -ForegroundColor Green
        npm install --silent 2>$null
    }

    Write-Host "  🔨 执行 tauri build ..." -ForegroundColor Green
    try {
        npm run tauri build 2>$null
    } catch {
        try {
            cargo tauri build 2>$null
        } catch {
            Write-Host "  ❌ Tauri 构建失败，请检查环境：" -ForegroundColor Red
            Write-Host "     npm install -g @tauri-apps/cli"
            Write-Host "     cargo install tauri-cli"
            exit 1
        }
    }

    Write-Host "  ✅ Tauri 构建完成" -ForegroundColor Green
    Write-Host ""

    Pop-Location
}

# --------------------------------------------------
# 3) 输出分发物
# --------------------------------------------------
Write-Host "📂 分发物清单" -ForegroundColor Green
$bundleDir = "$DesktopDir\src-tauri\target\release\bundle"
if (Test-Path $bundleDir) {
    Get-ChildItem -Path $bundleDir -Recurse -File | ForEach-Object {
        $size = [math]::Round($_.Length / 1MB, 2)
        Write-Host "  • $($_.Name)" -ForegroundColor Green -NoNewline
        Write-Host " (${size} MB)"
    }
}

Write-Host ""
Write-Host "🎉 全部完成！" -ForegroundColor Green

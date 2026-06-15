#Requires -Version 5.1
# ============================================================
# generate-icons.ps1 — 从源 PNG 生成 Tauri 桌面端全套图标
# 使用 .NET System.Drawing / Windows API (无需外部依赖)
# ============================================================
param(
    [string]$SourcePng = "$env:USERPROFILE\Downloads\微信图片_20260601105735_9720_209.png",
    [string]$OutDir = "$PSScriptRoot\icons"
)

if (-not (Test-Path $SourcePng)) {
    Write-Error "❌ 源 PNG 不存在: $SourcePng"
    Write-Host "用法: .\generate-icons.ps1 -SourcePng <路径> -OutDir <目录>"
    exit 1
}

# 确保 .NET Drawing 可用
Add-Type -AssemblyName System.Drawing -ErrorAction SilentlyContinue

$null = New-Item -ItemType Directory -Force -Path $OutDir

Write-Host "🎨 图标源: $SourcePng"
Write-Host "📁 输出目录: $OutDir"

function Resize-Image {
    param(
        [string]$InputPath,
        [string]$OutputPath,
        [int]$Width,
        [int]$Height
    )
    try {
        $src = [System.Drawing.Image]::FromFile($InputPath)
        $dst = New-Object System.Drawing.Bitmap($Width, $Height)
        $g = [System.Drawing.Graphics]::FromImage($dst)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($src, 0, 0, $Width, $Height)
        $dst.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose()
        $dst.Dispose()
        $src.Dispose()
        Write-Host "  ✅ $OutputPath (${Width}x${Height})"
    } catch {
        Write-Warning "  ⚠️ 生成失败: $OutputPath — $_"
    }
}

# 1) macOS iconset
$macSizes = @(16, 32, 64, 128, 256, 512, 1024)
$iconsetDir = Join-Path $OutDir "icon.iconset"
$null = New-Item -ItemType Directory -Force -Path $iconsetDir

foreach ($size in $macSizes) {
    Resize-Image -InputPath $SourcePng -OutputPath "$iconsetDir\icon_${size}x${size}.png" -Width $size -Height $size
}

# 2) Windows .ico (16,32,48,64,128,256)
$icoSizes = @(16, 32, 48, 64, 128, 256)
$icoTmp = Join-Path $OutDir ".ico_tmp"
$null = New-Item -ItemType Directory -Force -Path $icoTmp

foreach ($size in $icoSizes) {
    Resize-Image -InputPath $SourcePng -OutputPath "$icoTmp\icon_${size}.png" -Width $size -Height $size
}

# 使用 .NET 生成多尺寸 ICO
function ConvertTo-Icon {
    param(
        [string[]]$PngPaths,
        [string]$OutputPath
    )
    try {
        $fs = [System.IO.File]::OpenWrite($OutputPath)
        $writer = New-Object System.IO.BinaryWriter($fs)

        $count = $PngPaths.Count
        $headerSize = 6 + $count * 16
        $dataOffset = $headerSize

        # ICO 文件头
        $writer.Write([UInt16]0)      # 保留
        $writer.Write([UInt16]1)       # 类型: 图标
        $writer.Write([UInt16]$count)  # 图像数量

        $dataOffsets = @()
        $dataSizes = @()
        $images = @()

        foreach ($pngPath in $PngPaths) {
            $img = [System.Drawing.Image]::FromFile($pngPath)
            $ms = New-Object System.IO.MemoryStream
            $img.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
            $bytes = $ms.ToArray()
            $images += , $bytes
            $dataOffsets += $dataOffset
            $dataSizes += $bytes.Length
            $dataOffset += $bytes.Length
        }

        for ($i = 0; $i -lt $count; $i++) {
            $img = [System.Drawing.Image]::FromFile($PngPaths[$i])
            $w = $img.Width
            $h = $img.Height
            $img.Dispose()

            $writer.Write([byte](if ($w -ge 256) { 0 } else { $w }))
            $writer.Write([byte](if ($h -ge 256) { 0 } else { $h }))
            $writer.Write([byte]0)       # 颜色数
            $writer.Write([byte]0)       # 保留
            $writer.Write([UInt16]1)     # 颜色平面
            $writer.Write([UInt16]32)    # 位深
            $writer.Write([UInt32]$dataSizes[$i])
            $writer.Write([UInt32]$dataOffsets[$i])
        }

        foreach ($bytes in $images) {
            $writer.Write($bytes)
        }

        $writer.Close()
        $fs.Close()
        Write-Host "  ✅ icon.ico (全尺寸: $icoSizes)"
    } catch {
        Write-Warning "  ⚠️ icon.ico 生成失败: $_"
    }
}

$icoPngPaths = $icoSizes | ForEach-Object { Join-Path $icoTmp "icon_$_.png" }
ConvertTo-Icon -PngPaths $icoPngPaths -OutputPath (Join-Path $OutDir "icon.ico")
Remove-Item -Recurse -Force $icoTmp -ErrorAction SilentlyContinue

# 3) Tauri bundle PNGs
$tauriSizes = @(32, 64, 128)
foreach ($size in $tauriSizes) {
    Resize-Image -InputPath $SourcePng -OutputPath "$OutDir\${size}x${size}.png" -Width $size -Height $size
}
Resize-Image -InputPath $SourcePng -OutputPath "$OutDir\128x128@2x.png" -Width 256 -Height 256
Resize-Image -InputPath $SourcePng -OutputPath "$OutDir\icon.png" -Width 1024 -Height 1024

# 4) Windows Store PNGs
$storeSizes = @(30, 44, 71, 89, 107, 142, 150, 284, 310)
foreach ($size in $storeSizes) {
    Resize-Image -InputPath $SourcePng -OutputPath "$OutDir\Square${size}x${size}Logo.png" -Width $size -Height $size
}
Resize-Image -InputPath $SourcePng -OutputPath "$OutDir\StoreLogo.png" -Width 50 -Height 50

# 5) Android mipmap
$androidDirs = @(
    @{dir="mipmap-mdpi"; size=48},
    @{dir="mipmap-hdpi"; size=72},
    @{dir="mipmap-xhdpi"; size=96},
    @{dir="mipmap-xxhdpi"; size=144},
    @{dir="mipmap-xxxhdpi"; size=192}
)
foreach ($item in $androidDirs) {
    $dir = Join-Path $OutDir "android\$($item.dir)"
    $null = New-Item -ItemType Directory -Force -Path $dir
    Resize-Image -InputPath $SourcePng -OutputPath "$dir\ic_launcher.png" -Width $item.size -Height $item.size
}

Write-Host ""
Write-Host "🎉 全部图标生成完成！输出: $OutDir"

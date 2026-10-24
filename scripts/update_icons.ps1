Add-Type -AssemblyName System.Drawing

$srcPath = "d:\!EPIC LAUNCHER\public\mystic logo.png"
if (-not (Test-Path $srcPath)) {
    Write-Error "Source image not found at $srcPath"
    exit 1
}

$img = [System.Drawing.Image]::FromFile($srcPath)

function Resize-And-Save($image, [int]$w, [int]$h, [string]$outPath) {
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($image, 0, 0, $w, $h)
    $g.Dispose()
    $bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "Created $outPath (${w}x${h})"
}

Resize-And-Save $img 32 32 "d:\!EPIC LAUNCHER\src-tauri\icons\32x32.png"
Resize-And-Save $img 128 128 "d:\!EPIC LAUNCHER\src-tauri\icons\128x128.png"
Resize-And-Save $img 256 256 "d:\!EPIC LAUNCHER\src-tauri\icons\128x128@2x.png"

# Create ICO
$icoBmp = New-Object System.Drawing.Bitmap 256, 256
$icoG = [System.Drawing.Graphics]::FromImage($icoBmp)
$icoG.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$icoG.DrawImage($img, 0, 0, 256, 256)
$icoG.Dispose()

$hIcon = $icoBmp.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($hIcon)
$fs = New-Object System.IO.FileStream("d:\!EPIC LAUNCHER\src-tauri\icons\icon.ico", [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$icon.Dispose()
$icoBmp.Dispose()
$img.Dispose()

Write-Host "All desktop icons updated with Mystic Logo successfully!"

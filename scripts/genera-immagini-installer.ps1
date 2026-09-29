# Immagini dell'installer NSIS di Tabby: fascia laterale (164x314) e intestazione (150x57), BMP 24 bit.
param([string]$Root = (Join-Path $PSScriptRoot '..'))
Add-Type -AssemblyName System.Drawing

$icon = [System.Drawing.Image]::FromFile((Join-Path $Root 'app-icon.png'))
$outDir = Join-Path $Root 'src-tauri\nsis'

function Save24($bmp, $path) {
    $copy = New-Object System.Drawing.Bitmap $bmp.Width, $bmp.Height, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($copy)
    $g.DrawImage($bmp, 0, 0, $bmp.Width, $bmp.Height)
    $g.Dispose()
    $copy.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $copy.Dispose()
}

# Fascia laterale: sfondo scuro, gattino al centro, nome sotto.
$side = New-Object System.Drawing.Bitmap 164, 314
$g = [System.Drawing.Graphics]::FromImage($side)
$g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'; $g.TextRenderingHint = 'AntiAliasGridFit'
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.Point 0, 0), (New-Object System.Drawing.Point 0, 314), ([System.Drawing.ColorTranslator]::FromHtml('#343a4a')), ([System.Drawing.ColorTranslator]::FromHtml('#1c1f28'))
$g.FillRectangle($grad, 0, 0, 164, 314)
$g.DrawImage($icon, 22, 70, 120, 120)
$font = New-Object System.Drawing.Font 'Segoe UI Semibold', 20
$fmt = New-Object System.Drawing.StringFormat; $fmt.Alignment = 'Center'
$g.DrawString('Tabby', $font, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 0, 205, 164, 40), $fmt)
$small = New-Object System.Drawing.Font 'Segoe UI', 8.5
$g.DrawString("le tue note,`nsempre a bordo", $small, (New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#b8bdcc'))), (New-Object System.Drawing.RectangleF 0, 242, 164, 40), $fmt)
$g.Dispose()
Save24 $side (Join-Path $outDir 'sidebar.bmp')

# Intestazione: fondo bianco con il gattino a destra.
$head = New-Object System.Drawing.Bitmap 150, 57
$g = [System.Drawing.Graphics]::FromImage($head)
$g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'
$g.Clear([System.Drawing.Color]::White)
$g.DrawImage($icon, 97, 5, 47, 47)
$g.Dispose()
Save24 $head (Join-Path $outDir 'header.bmp')

$icon.Dispose()
"ok"

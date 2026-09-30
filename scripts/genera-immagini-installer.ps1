# Converte le immagini di assets\ nei BMP 24 bit dell'installer NSIS:
# fascia-installer.png -> sidebar.bmp (164x314), intestazione.png -> header.bmp (150x57).
param([string]$Root = (Join-Path $PSScriptRoot '..'))
Add-Type -AssemblyName System.Drawing

$outDir = Join-Path $Root 'src-tauri\nsis'

function ToBmp24($name, $target) {
    $src = [System.Drawing.Image]::FromFile((Resolve-Path (Join-Path $Root "assets\$name")))
    $copy = New-Object System.Drawing.Bitmap $src.Width, $src.Height, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($copy)
    $g.Clear([System.Drawing.Color]::White)
    $g.DrawImage($src, 0, 0, $src.Width, $src.Height)
    $g.Dispose()
    $copy.Save((Join-Path $outDir $target), [System.Drawing.Imaging.ImageFormat]::Bmp)
    $copy.Dispose(); $src.Dispose()
}

ToBmp24 'fascia-installer.png' 'sidebar.bmp'
ToBmp24 'intestazione.png' 'header.bmp'
"ok"

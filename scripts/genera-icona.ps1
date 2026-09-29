# Disegna l'icona di Tabby (app-icon.png, 1024 px): un gatto tigrato che fa capolino
# davanti alle linguette colorate del deck. Poi: npx tauri icon app-icon.png
Add-Type -AssemblyName System.Drawing
$size = 1024
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear([System.Drawing.Color]::Transparent)

function RoundRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}
function Color($hex, $alpha = 255) {
    $c = [System.Drawing.ColorTranslator]::FromHtml($hex)
    return [System.Drawing.Color]::FromArgb($alpha, $c.R, $c.G, $c.B)
}
function Brush($hex, $alpha = 255) { New-Object System.Drawing.SolidBrush (Color $hex $alpha) }
function Pt([float]$x, [float]$y) { New-Object System.Drawing.PointF $x, $y }
function RoundPen($hex, [float]$width, $alpha = 255) {
    $pen = New-Object System.Drawing.Pen (Color $hex $alpha), $width
    $pen.StartCap = 'Round'; $pen.EndCap = 'Round'; $pen.LineJoin = 'Round'
    return $pen
}

# Sfondo: quadrato arrotondato scuro con leggero gradiente.
$bg = RoundRect 40 40 944 944 210
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush (Pt 0 40), (Pt 0 984), (Color '#343a4a'), (Color '#1c1f28')
$g.FillPath($grad, $bg)

# Linguette del deck sul bordo destro, sovrapposte "a scandole".
foreach ($t in @(@('#4f9cf5', 250), @('#2fcf8b', 420), @('#f5c542', 590))) {
    $g.FillPath((Brush '#000000' 70), (RoundRect 704 ($t[1] + 10) 190 190 44))
    $g.FillPath((Brush $t[0]), (RoundRect 696 $t[1] 190 190 44))
}

# --- Il gatto ---
$fur = '#f2a14e'; $furDark = '#c8692a'; $inner = '#f7b6b0'; $ink = '#2b2233'

# Orecchie (con l'interno rosa).
$g.FillPolygon((Brush $fur), [System.Drawing.PointF[]]@((Pt 190 470), (Pt 250 200), (Pt 420 360)))
$g.FillPolygon((Brush $fur), [System.Drawing.PointF[]]@((Pt 710 470), (Pt 650 200), (Pt 480 360)))
$g.FillPolygon((Brush $inner), [System.Drawing.PointF[]]@((Pt 240 420), (Pt 268 268), (Pt 370 368)))
$g.FillPolygon((Brush $inner), [System.Drawing.PointF[]]@((Pt 660 420), (Pt 632 268), (Pt 530 368)))

# Ombra e testa.
$g.FillEllipse((Brush '#000000' 80), 168, 330, 580, 520)
$g.FillEllipse((Brush $fur), 160, 310, 580, 520)
# Muso chiaro.
$g.FillEllipse((Brush '#fbe3c4'), 300, 600, 300, 210)

# Strisce tigrate sulla fronte e sulle guance.
$stripe = RoundPen $furDark 30
$g.DrawLine($stripe, 450, 340, 450, 440)
$g.DrawLine($stripe, 390, 350, 405, 430)
$g.DrawLine($stripe, 510, 350, 495, 430)
$g.DrawLine($stripe, 175, 560, 245, 575)
$g.DrawLine($stripe, 180, 630, 245, 625)
$g.DrawLine($stripe, 725, 560, 655, 575)
$g.DrawLine($stripe, 720, 630, 655, 625)

# Occhi con riflesso.
$g.FillEllipse((Brush $ink), 300, 500, 80, 100)
$g.FillEllipse((Brush $ink), 520, 500, 80, 100)
$g.FillEllipse((Brush '#ffffff'), 324, 516, 28, 28)
$g.FillEllipse((Brush '#ffffff'), 544, 516, 28, 28)

# Naso e bocca.
$g.FillPolygon((Brush '#e8747c'), [System.Drawing.PointF[]]@((Pt 422 640), (Pt 478 640), (Pt 450 672)))
$mouth = RoundPen $ink 12
$g.DrawArc($mouth, 400, 650, 50, 50, 0, 150)
$g.DrawArc($mouth, 450, 650, 50, 50, 30, 150)

# Baffi.
$whisker = RoundPen '#ffffff' 9 220
$g.DrawLine($whisker, 330, 690, 150, 660)
$g.DrawLine($whisker, 330, 715, 155, 730)
$g.DrawLine($whisker, 570, 690, 750, 660)
$g.DrawLine($whisker, 570, 715, 745, 730)

$out = Join-Path $PSScriptRoot '..\app-icon.png'
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
$out

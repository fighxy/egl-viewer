# Writes the preview's hand cursors (D-148): grab.cur (open hand, over the image) and
# grabbing.cur (closed hand, while the button is held). Drawn here, so no third-party artwork.
#
#   powershell -ExecutionPolicy Bypass -File crates/gpui-desktop/assets/cursors/make-cursors.ps1
#   ... -Preview <folder>   also writes enlarged PNGs to look at
#
# Vector shapes drawn anti-aliased into a 32x32 canvas (the hand itself is about 20 px tall):
# white fill, black outline, the black line between fingers. 32-bit cursors with alpha.

param([string]$Preview = "")

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$size = 32

function New-Capsule([float]$x, [float]$y, [float]$w, [float]$h) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $r = [math]::Min($w, $h)
    $p.AddArc($x, $y, $r, $r, 180, 180)
    $p.AddLine(($x + $w), ($y + $r / 2), ($x + $w), ($y + $h - $r / 2))
    $p.AddArc($x, ($y + $h - $r), $r, $r, 0, 180)
    $p.CloseFigure()
    $p
}

function New-RoundRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $r * 2
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc(($x + $w - $d), $y, $d, $d, 270, 90)
    $p.AddArc(($x + $w - $d), ($y + $h - $d), $d, $d, 0, 90)
    $p.AddArc($x, ($y + $h - $d), $d, $d, 90, 90)
    $p.CloseFigure()
    $p
}

function New-Thumb([float]$angle) {
    # A capsule rotated about its base, which sits at the palm's lower left.
    $p = New-Capsule 0 0 3.4 8
    $m = New-Object System.Drawing.Drawing2D.Matrix
    $m.Translate(11.2, 21.5)
    $m.Rotate($angle)
    $m.Translate(-1.7, -8)
    $p.Transform($m)
    $p
}

# Finger capsules (x, top, height); the palm covers their lower ends.
$openFingers = @(@(10.2, 6.6, 10), @(13.4, 5.0, 11), @(16.6, 5.8, 10), @(19.8, 8.4, 8))
$closedFingers = @(@(10.2, 11.0, 5), @(13.4, 10.4, 5.6), @(16.6, 10.8, 5.2), @(19.8, 12.0, 4.6))

function Draw-Hand($g, $fingers, [float]$thumbAngle) {
    $black = New-Object System.Drawing.Pen ([System.Drawing.Color]::Black), 1.3
    $black.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $white = [System.Drawing.Brushes]::White
    $palm = New-RoundRect 9.6 13.0 13.6 12.0 3.6
    $thumb = New-Thumb $thumbAngle
    # Outline of everything, then the white parts: fingers keep the lines between them, the
    # palm drawn last (without its outline) hides their lower ends.
    foreach ($part in @($thumb, $palm)) { $g.DrawPath($black, $part) }
    foreach ($f in $fingers) { $g.DrawPath($black, (New-Capsule $f[0] $f[1] 3.2 $f[2])) }
    $g.FillPath($white, $thumb)
    foreach ($f in $fingers) {
        $finger = New-Capsule $f[0] $f[1] 3.2 $f[2]
        $g.FillPath($white, $finger)
        $g.DrawPath($black, $finger)
    }
    $g.FillPath($white, (New-RoundRect 10.25 15.2 12.3 9.15 3.0))
    $g.DrawPath($black, $thumb)
    $g.FillPath($white, (New-RoundRect 10.8 16.5 11.2 7.9 2.6))
}

function Write-Cursor($fingers, [float]$thumbAngle, [string]$path, [int]$hotX, [int]$hotY) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    Draw-Hand $g $fingers $thumbAngle
    $g.Dispose()

    $xor = New-Object byte[] ($size * $size * 4)     # BGRA, bottom-up
    $and = New-Object byte[] ($size * 4)             # 1 bpp, bottom-up; 1 = transparent
    for ($y = 0; $y -lt $size; $y++) {
        $dibRow = $size - 1 - $y
        for ($x = 0; $x -lt $size; $x++) {
            $c = $bmp.GetPixel($x, $y)
            $i = ($dibRow * $size + $x) * 4
            $xor[$i] = $c.B; $xor[$i + 1] = $c.G; $xor[$i + 2] = $c.R; $xor[$i + 3] = $c.A
            if ($c.A -eq 0) {
                $k = $dibRow * 4 + [math]::Floor($x / 8)
                $and[$k] = $and[$k] -bor (0x80 -shr ($x % 8))
            }
        }
    }

    $image = New-Object System.IO.MemoryStream
    $w = New-Object System.IO.BinaryWriter $image
    # BITMAPINFOHEADER; the height counts the XOR image and the AND mask.
    $w.Write([uint32]40); $w.Write([int32]$size); $w.Write([int32]($size * 2))
    $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]0)
    $w.Write([uint32]($xor.Length + $and.Length)); $w.Write([int32]0); $w.Write([int32]0)
    $w.Write([uint32]0); $w.Write([uint32]0)
    $w.Write($xor); $w.Write($and); $w.Flush()
    $bytes = $image.ToArray()

    $file = New-Object System.IO.MemoryStream
    $f = New-Object System.IO.BinaryWriter $file
    $f.Write([uint16]0); $f.Write([uint16]2); $f.Write([uint16]1)      # ICONDIR: cursor, 1 image
    $f.Write([byte]$size); $f.Write([byte]$size); $f.Write([byte]0); $f.Write([byte]0)
    $f.Write([uint16]$hotX); $f.Write([uint16]$hotY)
    $f.Write([uint32]$bytes.Length); $f.Write([uint32]22)
    $f.Write($bytes); $f.Flush()
    [System.IO.File]::WriteAllBytes($path, $file.ToArray())
    "wrote $path"

    if ($Preview -ne "") {
        $scale = 8
        $big = New-Object System.Drawing.Bitmap ($size * $scale), ($size * $scale)
        $bg = [System.Drawing.Graphics]::FromImage($big)
        $bg.Clear([System.Drawing.Color]::FromArgb(255, 120, 120, 120))
        $bg.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
        $bg.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
        $bg.DrawImage($bmp, 0, 0, $size * $scale, $size * $scale)
        $red = New-Object System.Drawing.Pen ([System.Drawing.Color]::Red), 2
        $bg.DrawEllipse($red, $hotX * $scale - 6, $hotY * $scale - 6, 12, 12)
        $name = [System.IO.Path]::GetFileNameWithoutExtension($path)
        $big.Save((Join-Path $Preview "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        # Actual size on the image, next to the enlargement.
        $actual = New-Object System.Drawing.Bitmap 96, 48
        $ag = [System.Drawing.Graphics]::FromImage($actual)
        $ag.Clear([System.Drawing.Color]::FromArgb(255, 60, 60, 60))
        $ag.DrawImage($bmp, 8, 8, $size, $size)
        $actual.Save((Join-Path $Preview "$name-1x.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    }
}

$here = $PSScriptRoot
Write-Cursor $openFingers -48 (Join-Path $here "grab.cur") 16 17
Write-Cursor $closedFingers -28 (Join-Path $here "grabbing.cur") 16 19

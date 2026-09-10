# Generate test images into test_images\ (ASCII only, ConstrainedLanguage-safe)
# PowerShell equivalent of iv-core examples\gen_test_images.rs
$ErrorActionPreference = 'Stop'
$dir = "e:\LcL\ImageView\test_images"
New-Item -ItemType Directory -Force -Path $dir | Out-Null

function LE16([byte[]]$b, [int]$o, [long]$v) {
    $b[$o] = [byte]($v -band 255); $b[$o+1] = [byte](($v -shr 8) -band 255)
}
function LE32([byte[]]$b, [int]$o, [long]$v) {
    $b[$o] = [byte]($v -band 255)
    $b[$o+1] = [byte](($v -shr 8) -band 255)
    $b[$o+2] = [byte](($v -shr 16) -band 255)
    $b[$o+3] = [byte](($v -shr 24) -band 255)
}
function BE16([byte[]]$b, [int]$o, [long]$v) {
    $b[$o] = [byte](($v -shr 8) -band 255); $b[$o+1] = [byte]($v -band 255)
}
function BE32([byte[]]$b, [int]$o, [long]$v) {
    $b[$o] = [byte](($v -shr 24) -band 255)
    $b[$o+1] = [byte](($v -shr 16) -band 255)
    $b[$o+2] = [byte](($v -shr 8) -band 255)
    $b[$o+3] = [byte]($v -band 255)
}
function SaveBytes([string]$path, [byte[]]$b) {
    Set-Content -Path $path -Value $b -Encoding Byte
    Write-Host ("saved " + $path + " (" + $b.Length + " bytes)")
}

# ---------- 1. PNG / JPG: copy system wallpapers (valid standard files) ----------
Copy-Item "C:\Windows\Web\Wallpaper\MSI\MSI.png" "$dir\gradient.png" -Force
Copy-Item "C:\Windows\Web\Wallpaper\MSI\MSI_MAG.jpg" "$dir\gradient.jpg" -Force
Write-Host "PNG/JPG done"

# ---------- 2. TGA 256x256 uncompressed 32-bit top-down ----------
$tga = New-Object byte[] (18 + 256*256*4)
$tga[2] = 2                      # image type: uncompressed true-color
LE16 $tga 12 256                 # width
LE16 $tga 14 256                 # height
$tga[16] = 32                    # bits per pixel
$tga[17] = 40                    # descriptor: top-down + 8 attr bits (0x28)
$i = 18
for ($y = 0; $y -lt 256; $y++) {
    for ($x = 0; $x -lt 256; $x++) {
        $tga[$i]   = [byte](($x + $y) % 256)   # B
        $tga[$i+1] = [byte]($y % 256)          # G
        $tga[$i+2] = [byte]($x % 256)          # R
        $tga[$i+3] = [byte]($x * 255 / 256)    # A
        $i += 4
    }
}
SaveBytes "$dir\gradient.tga" $tga
Write-Host "TGA done"

# ---------- helpers for DDS ----------
function Rgb565([int]$r, [int]$g, [int]$b) {
    return ((($r -shr 3) -shl 11) -bor (($g -shr 2) -shl 5) -bor ($b -shr 3))
}
function DdsHeaderFourcc([byte[]]$d, [string]$fourcc, [int]$w, [int]$h, [int]$mips, [int]$linSize) {
    $d[0] = 0x44; $d[1] = 0x44; $d[2] = 0x53; $d[3] = 0x20   # "DDS "
    LE32 $d 4 124
    LE32 $d 8 (0x4 -bor 0x2 -bor 0x1000 -bor 0x80000 -bor 0x20000)
    LE32 $d 12 $h
    LE32 $d 16 $w
    LE32 $d 20 $linSize
    LE32 $d 24 0
    LE32 $d 28 $mips
    LE32 $d 76 32
    LE32 $d 80 0x4                       # pixel format flags: FOURCC
    $d[84] = [byte]$fourcc[0]; $d[85] = [byte]$fourcc[1]; $d[86] = [byte]$fourcc[2]; $d[87] = [byte]$fourcc[3]
    LE32 $d 108 0x1000                   # caps: texture
}

# ---------- 3. BC1 DDS: 32x32, 3 mips, 4-color quadrant grid ----------
$bc1Size = 128 + (64 + 16 + 4) * 8
$bc1 = New-Object byte[] $bc1Size
DdsHeaderFourcc $bc1 "DXT1" 32 32 3 ((64 + 16 + 4) * 8)
$pal = @(@(255,0,0), @(0,255,0), @(0,0,255), @(255,255,255))
$mipW = @(32, 16, 8)
$mipH = @(32, 16, 8)
$o = 128
for ($m = 0; $m -lt 3; $m++) {
    $bxs = ($mipW[$m] + 3) -shr 2
    $bys = ($mipH[$m] + 3) -shr 2
    for ($by = 0; $by -lt $bys; $by++) {
        for ($bx = 0; $bx -lt $bxs; $bx++) {
            $pi = (($bx -shr 1) + ($by -shr 1) * 2) % 4
            $col = $pal[$pi]
            $c0 = Rgb565 $col[0] $col[1] $col[2]
            LE16 $bc1 $o $c0
            LE16 $bc1 ($o + 2) $c0
            # indices stay zero -> all pixels use c0
            $o += 8
        }
    }
}
SaveBytes "$dir\bc1_mips.dds" $bc1
Write-Host "BC1 mips done"

# ---------- 4. BC3 DDS: 32x32 pure blue + horizontal alpha ramp ----------
$bc3 = New-Object byte[] (128 + 64 * 16)
DdsHeaderFourcc $bc3 "DXT5" 32 32 1 1024
$idxTab = @(7, 4, 2, 0)   # column alpha index (a0=255, a1=0, 8-index mode)
$o = 128
for ($blk = 0; $blk -lt 64; $blk++) {
    $bc3[$o] = 255; $bc3[$o+1] = 0    # a0 > a1 -> 8-index mode
    $bits = [long]0
    for ($py = 0; $py -lt 4; $py++) {
        for ($px = 0; $px -lt 4; $px++) {
            $bits = $bits -bor ([long]$idxTab[$px] -shl (($py * 4 + $px) * 3))
        }
    }
    # 16 x 3-bit indices = 48 bits = 6 bytes (low 6 bytes of the u64)
    for ($k = 0; $k -lt 6; $k++) { $bc3[$o + 2 + $k] = [byte](($bits -shr (8 * $k)) -band 255) }
    $c0 = Rgb565 0 0 255
    LE16 $bc3 ($o + 8) $c0
    LE16 $bc3 ($o + 10) $c0
    # bytes 12..15: color indices stay zero
    $o += 16
}
SaveBytes "$dir\bc3_alpha.dds" $bc3
Write-Host "BC3 done"

# ---------- 5. BC5 DDS (ATI2): R horizontal ramp / G vertical ramp ----------
$bc5 = New-Object byte[] (128 + 64 * 16)
DdsHeaderFourcc $bc5 "ATI2" 32 32 1 1024
$o = 128
for ($blk = 0; $blk -lt 64; $blk++) {
    # red block (BC4-style): horizontal ramp
    $bc5[$o] = 255; $bc5[$o+1] = 0    # r0 > r1 -> 8-index mode
    $bits = [long]0
    for ($py = 0; $py -lt 4; $py++) {
        for ($px = 0; $px -lt 4; $px++) {
            $bits = $bits -bor ([long]$idxTab[$px] -shl (($py * 4 + $px) * 3))
        }
    }
    for ($k = 0; $k -lt 6; $k++) { $bc5[$o + 2 + $k] = [byte](($bits -shr (8 * $k)) -band 255) }
    # green block: vertical ramp
    $bc5[$o+8] = 255; $bc5[$o+9] = 0
    $bits = [long]0
    for ($py = 0; $py -lt 4; $py++) {
        for ($px = 0; $px -lt 4; $px++) {
            $bits = $bits -bor ([long]$idxTab[$py] -shl (($py * 4 + $px) * 3))
        }
    }
    for ($k = 0; $k -lt 6; $k++) { $bc5[$o + 10 + $k] = [byte](($bits -shr (8 * $k)) -band 255) }
    $o += 16
}
SaveBytes "$dir\bc5_normal.dds" $bc5
Write-Host "BC5 done"

# ---------- 6. Uncompressed RGBA8 DDS: 64x64 diagonal ramp + alpha checker ----------
$raw = New-Object byte[] (128 + 64 * 64 * 4)
$raw[0] = 0x44; $raw[1] = 0x44; $raw[2] = 0x53; $raw[3] = 0x20
LE32 $raw 4 124
LE32 $raw 8 (0x4 -bor 0x2 -bor 0x1000 -bor 0x8 -bor 0x20000)
LE32 $raw 12 64        # height
LE32 $raw 16 64        # width
LE32 $raw 20 256       # pitch
LE32 $raw 28 1         # mip count
LE32 $raw 76 32
LE32 $raw 80 (0x1 -bor 0x40)          # ALPHAPIXELS | RGB
LE32 $raw 88 32                        # bpp
LE32 $raw 92 0x00FF0000                # R mask
LE32 $raw 96 0x0000FF00                # G mask
LE32 $raw 100 0x000000FF               # B mask
LE32 $raw 104 0xFF000000               # A mask
LE32 $raw 108 0x1000
$o = 128
for ($y = 0; $y -lt 64; $y++) {
    for ($x = 0; $x -lt 64; $x++) {
        $raw[$o] = 128                    # B
        $raw[$o+1] = [byte]($y * 4)       # G
        $raw[$o+2] = [byte]($x * 4)       # R
        if ((($x -shr 3) + ($y -shr 3)) % 2 -eq 0) { $raw[$o+3] = 255 } else { $raw[$o+3] = 128 }
        $o += 4
    }
}
SaveBytes "$dir\rgba8888.dds" $raw
Write-Host "Uncompressed DDS done"

# ---------- 7. PSD: 64x64 RAW composite (R/G ramp, B const, A checker) ----------
$psd = New-Object byte[] (40 + 4 * 64 * 64)
$psd[0] = 0x38; $psd[1] = 0x42; $psd[2] = 0x50; $psd[3] = 0x53   # "8BPS"
BE16 $psd 4 1        # version
BE16 $psd 12 4       # channels: R G B A
BE32 $psd 14 64      # height
BE32 $psd 18 64      # width
BE16 $psd 22 8       # depth
BE16 $psd 24 3       # mode: RGB
BE32 $psd 26 0       # color mode data length
BE32 $psd 30 0       # image resources length
BE32 $psd 34 0       # layer & mask length
BE16 $psd 38 0       # compression: RAW
$o = 40
for ($plane = 0; $plane -lt 4; $plane++) {
    for ($y = 0; $y -lt 64; $y++) {
        for ($x = 0; $x -lt 64; $x++) {
            if ($plane -eq 0) { $v = $x * 4 }
            elseif ($plane -eq 1) { $v = $y * 4 }
            elseif ($plane -eq 2) { $v = 200 }
            else {
                if ((($x -shr 3) + ($y -shr 3)) % 2 -eq 0) { $v = 255 } else { $v = 96 }
            }
            $psd[$o] = [byte]$v
            $o++
        }
    }
}
SaveBytes "$dir\checker.psd" $psd
Write-Host "PSD done"

Write-Host "All test images generated."
Get-ChildItem $dir | Select-Object Name, Length | Format-Table -AutoSize

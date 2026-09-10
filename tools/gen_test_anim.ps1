# gen_test_anim.ps1 - generate test_images/anim_test.gif (12 frames, 120ms each, loop)
# Pure PowerShell + GIF89a/LZW, ASCII only (ConstrainedLanguage safe).
$ErrorActionPreference = 'Stop'

$outDir = Join-Path $PSScriptRoot '..\test_images'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$path = Join-Path $outDir 'anim_test.gif'

$W = 160; $H = 120; $N = 12

# ---- GIF LZW encoder (min code size 8) ----
function Encode-GifLzw([byte[]]$idx) {
    $CLEAR = 256; $EOI = 257
    $cap = $idx.Count * 2 + 16
    $codes = New-Object int[] $cap
    $sizes = New-Object byte[] $cap
    $nc = 0
    $codeSize = 9
    $nextCode = 258
    $dict = @{}
    $codes[$nc] = $CLEAR; $sizes[$nc] = 9; $nc++
    $prefix = [int]$idx[0]
    for ($p = 1; $p -lt $idx.Count; $p++) {
        $k = [int]$idx[$p]
        $key = "$prefix,$k"
        if ($dict.ContainsKey($key)) { $prefix = $dict[$key]; continue }
        $codes[$nc] = $prefix; $sizes[$nc] = [byte]$codeSize; $nc++
        $dict[$key] = $nextCode
        # giflib 规则：分配码 N 后若 N+1 > (1<<codeSize) 则升位宽，即 N >= (1<<codeSize) 时升
        if ($nextCode -ge (1 -shl $codeSize)) { $codeSize++ }
        $nextCode++
        if ($nextCode -ge 4096) {
            $codes[$nc] = $CLEAR; $sizes[$nc] = [byte]$codeSize; $nc++
            $dict.Clear(); $codeSize = 9; $nextCode = 258
        }
        $prefix = $k
    }
    $codes[$nc] = $prefix; $sizes[$nc] = [byte]$codeSize; $nc++
    $codes[$nc] = $EOI; $sizes[$nc] = [byte]$codeSize; $nc++
    # pack LSB-first
    $out = New-Object byte[] ($nc * 2 + 8)
    $no = 0
    $bitBuf = 0; $bitCnt = 0
    for ($c = 0; $c -lt $nc; $c++) {
        $bitBuf = $bitBuf -bor ($codes[$c] -shl $bitCnt)
        $bitCnt += $sizes[$c]
        while ($bitCnt -ge 8) {
            $out[$no] = [byte]($bitBuf -band 0xFF); $no++
            $bitBuf = $bitBuf -shr 8; $bitCnt -= 8
        }
    }
    if ($bitCnt -gt 0) { $out[$no] = [byte]($bitBuf -band 0xFF); $no++ }
    return ,$out[0..($no - 1)]
}

# ---- file buffer ----
$f = New-Object byte[] 800000
$nf = 0
function Wv([int]$v) { $script:f[$script:nf] = [byte]($v -band 0xFF); $script:nf++ }
function W16([int]$v) { Wv $v; Wv ($v -shr 8) }
function Ws([string]$s) { foreach ($ch in $s.ToCharArray()) { Wv ([byte]$ch) } }

# header
Ws 'GIF89a'
W16 $W; W16 $H
Wv 0xF7; Wv 0; Wv 0

# global color table (256 entries x 3 bytes)
$pal = New-Object byte[] 768
$pal[0] = 24; $pal[1] = 26; $pal[2] = 32                       # 0: background
for ($i = 0; $i -lt 12; $i++) {                                # 1..12: box color per frame
    $o = 3 + $i * 3
    $pal[$o] = [byte](64 + $i * 15)
    $pal[$o + 1] = [byte](255 - $i * 15)
    $pal[$o + 2] = [byte](110 + ($i % 3) * 45)
}
$pal[39] = 76; $pal[40] = 141; $pal[41] = 255                  # 13: progress dot on
$pal[42] = 58; $pal[43] = 60; $pal[44] = 68                    # 14: progress dot off
foreach ($b in $pal) { Wv $b }

# NETSCAPE looping extension (loop forever)
Wv 0x21; Wv 0xFF; Wv 0x0B
Ws 'NETSCAPE2.0'
Wv 3; Wv 1; W16 0; Wv 0

# frame Y offsets (fake sine, avoids [Math] calls)
$yoff = @(40, 55, 63, 62, 53, 43, 38, 44, 56, 64, 61, 49)

for ($i = 0; $i -lt $N; $i++) {
    # frame pixel indices
    $idx = New-Object byte[] ($W * $H)
    $bx = 8 + $i * 11
    $by = $yoff[$i]
    $ci = $i + 1
    for ($y = 0; $y -lt 26; $y++) {
        for ($x = 0; $x -lt 26; $x++) {
            $px = $bx + $x; $py = $by + $y
            if ($px -ge 0 -and $px -lt $W -and $py -ge 0 -and $py -lt $H) {
                $idx[$py * $W + $px] = [byte]$ci
            }
        }
    }
    for ($k = 0; $k -lt $N; $k++) {
        $c = 14; if ($k -le $i) { $c = 13 }
        $x0 = 12 + $k * 11
        for ($y = 10; $y -lt 15; $y++) {
            for ($x = $x0; $x -lt $x0 + 5; $x++) {
                $idx[$y * $W + $x] = [byte]$c
            }
        }
    }
    $lzw = Encode-GifLzw $idx

    # Graphic Control Extension: disposal=1 (keep), delay=12 (120ms), no transparency
    Wv 0x21; Wv 0xF9; Wv 4; Wv 4; W16 12; Wv 0; Wv 0
    # Image Descriptor: full canvas
    Wv 0x2C; W16 0; W16 0; W16 $W; W16 $H; Wv 0
    Wv 8
    # data sub-blocks (max 255 bytes each)
    $off = 0
    while ($off -lt $lzw.Count) {
        $len = $lzw.Count - $off
        if ($len -gt 255) { $len = 255 }
        Wv $len
        for ($j = 0; $j -lt $len; $j++) { Wv $lzw[$off + $j] }
        $off += $len
    }
    Wv 0
}

Wv 0x3B

$final = $f[0..($nf - 1)]
Set-Content -Path $path -Encoding Byte -Value $final
Write-Host "written: $path ($($final.Count) bytes, $N frames x 120ms)"

# register_thumbnail.ps1 - register iv_shell.dll as Explorer thumbnail provider (HKCU, no admin)
# Covers: .dds .tga .psd .webp .qoi .hdr .ppm .pgm .pbm (formats without a system handler)
$ErrorActionPreference = 'Stop'

$clsid = '{7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B}'
$dll = Join-Path $PSScriptRoot 'iv_shell.dll'
if (-not (Test-Path -LiteralPath $dll)) { $dll = Join-Path $PSScriptRoot '..\target\release\iv_shell.dll' }
if (-not (Test-Path $dll)) { throw "iv_shell.dll not found: $dll (build with: cargo build -p iv-shell --release)" }
$dll = (Resolve-Path $dll).Path

# CLSID registration
$base = "HKCU:\Software\Classes\CLSID\$clsid"
New-Item -Path "$base\InprocServer32" -Force | Out-Null
Set-ItemProperty -Path $base -Name '(Default)' -Value 'LcL ImageViewer Thumbnail Provider'
Set-ItemProperty -Path "$base\InprocServer32" -Name '(Default)' -Value $dll
Set-ItemProperty -Path "$base\InprocServer32" -Name 'ThreadingModel' -Value 'Apartment'

# per-extension handler: shellex\{E357FCCD-...} is the thumbnail-provider slot
foreach ($ext in @('.dds', '.tga', '.psd', '.webp', '.qoi', '.hdr', '.ppm', '.pgm', '.pbm')) {
    $key = "HKCU:\Software\Classes\$ext\shellex\{E357FCCD-A995-4576-B01F-234630154E96}"
    New-Item -Path $key -Force | Out-Null
    Set-ItemProperty -Path $key -Name '(Default)' -Value $clsid
    Write-Host "registered: $ext"
}

# refresh shell thumbnail/icon cache
ie4uinit.exe -show 2>$null
Write-Host 'Done. Open a folder with .dds/.tga files in Explorer (medium/large icons) to verify.'
Write-Host 'If thumbnails still show old icons, clear cache: ie4uinit.exe -ClearIconCache'

# unregister_thumbnail.ps1 - remove the thumbnail provider registration (HKCU)
$ErrorActionPreference = 'Stop'

$clsid = '{7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B}'

foreach ($ext in @('.dds', '.tga', '.psd', '.qoi', '.hdr', '.ppm', '.pgm', '.pbm')) {
    $key = "HKCU:\Software\Classes\$ext\shellex\{E357FCCD-A995-4576-B01F-234630154E96}"
    if (Test-Path $key) {
        Remove-Item -Path $key -Recurse -Force
        Write-Host "removed: $ext"
    }
}
$base = "HKCU:\Software\Classes\CLSID\$clsid"
if (Test-Path $base) {
    Remove-Item -Path $base -Recurse -Force
    Write-Host 'removed CLSID registration'
}

ie4uinit.exe -show 2>$null
Write-Host 'Done.'

param([string]$FFmpegDirectory)
$ErrorActionPreference = 'Stop'
$componentRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../src-tauri/bin'))
New-Item -ItemType Directory -Force -Path $componentRoot | Out-Null
$version = '2026.08.19'
$expected = '66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a'
$target = Join-Path $componentRoot 'yt-dlp.exe'
if (!(Test-Path -LiteralPath $target) -or (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    $staged = Join-Path $componentRoot 'yt-dlp.exe.download'
    Invoke-WebRequest -Uri "https://github.com/yt-dlp/yt-dlp/releases/download/$version/yt-dlp.exe" -OutFile $staged
    if ((Get-FileHash -LiteralPath $staged -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'yt-dlp checksum mismatch; no executable installed.' }
    Move-Item -LiteralPath $staged -Destination $target -Force
}
if (!$FFmpegDirectory) { $FFmpegDirectory = Split-Path (Get-Command ffmpeg.exe -ErrorAction Stop).Source }
foreach ($component in @('ffmpeg.exe', 'ffprobe.exe')) {
    $source = Join-Path $FFmpegDirectory $component
    if (!(Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing $source" }
    if ([IO.Path]::GetFullPath($source) -ne (Join-Path $componentRoot $component)) { Copy-Item -LiteralPath $source -Destination (Join-Path $componentRoot $component) -Force }
}
Write-Output 'Bilibili binaries are ready. Retain the matching FFmpeg licenses/build notices in src-tauri/bin.'

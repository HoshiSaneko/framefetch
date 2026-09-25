Bundled download components
==========================

Keep this entire directory next to FrameFetch.exe in portable distributions.

1. yt-dlp 2026.08.19
   Upstream: https://github.com/yt-dlp/yt-dlp
   Binary: https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe
   SHA-256: 66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a
   License: yt-dlp-LICENSE.txt; see upstream for bundled dependency notices.

2. FFmpeg / FFprobe 8.1.2 full build
   Build provider: https://www.gyan.dev/ffmpeg/builds/
   Upstream/source: https://ffmpeg.org/download.html
   License and configuration: FFmpeg-LICENSE.txt, FFmpeg-README.txt.

Executables are ignored by Git and must be supplied before packaging.

macOS
-----
Run bash scripts/prepare-macos.sh from the repository root.
yt-dlp: upstream universal macOS executable, verified against release SHA2-256SUMS.
FFmpeg/FFprobe: OSXExperts static builds (Apple Silicon 9.0 / Intel 8.0).
Provider/source/build details: https://www.osxexperts.net/
The existing FFmpeg-README.txt describes the Windows build only.
macOS resources are embedded in FrameFetch.app/Contents/Resources/bin.
Retain upstream license notices and matching source/build information when distributing.

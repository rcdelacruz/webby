# WebKit for Windows with media: working notes

Goal: webby on Windows must stay on real WebKit and play streaming audio
and video (YouTube, music, Dolby Digital Plus 5.1 tracks).

## What is wrong today

webby's Windows installer bundles the WebKit build that the Playwright
project publishes. Inspecting it and WebKit's source shows:

- Media goes through Media Foundation (`MediaPlayerPrivateMediaFoundation`),
  which hands a URL to Windows and nothing more.
- `Source/cmake/OptionsWin.cmake` turns off `ENABLE_MEDIA_SOURCE`,
  `ENABLE_WEB_AUDIO` and encrypted media, because the Windows port has no
  implementation behind them. No runtime setting changes that.
- Result: no Media Source Extensions, no Web Audio, no DRM. YouTube and
  most music and video sites cannot play. `webby:media` in the address
  bar shows what a build reports.

Things already tried in the Windows backend that did not help media:
turning GPU compositing off (needed for correct rendering and clicks),
running DPI-unaware, a Safari user agent, and a `mediaCapabilities` shim.

## Plan: bring WebKit's GStreamer media layer to the Windows port

WebKit's GTK and WPE ports get streaming, Web Audio and every codec from
about 47,000 lines under `Source/WebCore/platform/**/gstreamer`. Findings
that make porting it to Windows realistic:

- It was done before: the Windows port ran GStreamer around 2014
  (webkit.org bug 135766) before the option was dropped.
- The build is modular: other ports enable it by including
  `Source/cmake/GStreamerDefinitions.cmake`, `GStreamerDependencies.cmake`,
  `GStreamerChecks.cmake` and `Source/WebCore/platform/GStreamer.cmake`.
- Dependence on the GLib main loop is concentrated in one place,
  `connectSimpleBusMessageCallback` in `GStreamerCommon.cpp`
  (`gst_bus_add_signal_watch_full`). The Windows port runs a Win32 message
  loop, so the GLib default context has to be pumped from it.
- Linux-only paths (dmabuf, PipeWire, desktop portal) are optional or
  belong to features that can stay off.
- The Windows port already takes its dependencies from vcpkg
  (`vcpkg.json` at the WebKit root). vcpkg has a `gstreamer` port (1.28)
  with `plugins-base`, `plugins-good`, `plugins-bad`, `libav`, `dash`,
  `hls`; `libav` brings software decoders including E-AC-3 (Dolby Digital
  Plus), so playback would not depend on Windows codec packs.

Expected work, in order:

1. Build the stock Windows port from source and run webby against it.
2. Add `gstreamer` (with glib) to the Windows vcpkg features; enable
   `USE_GSTREAMER`, `USE_GLIB` without the GLib event loop, and build WTF's
   `wtf/glib` helpers on Windows.
3. Replace the Media Foundation player with `MediaPlayerPrivateGStreamer`,
   software video path first (`USE_GSTREAMER_GL` off), then make it compile
   and link.
4. Pump the GLib main context from the Windows run loop.
5. Turn on `ENABLE_MEDIA_SOURCE` and `ENABLE_WEB_AUDIO`; test YouTube,
   plain audio and video files, then E-AC-3.
6. Ship the GStreamer runtime and plugins in the installer.

Alternative considered: Windows has its own Media Source Extensions engine
(`IMFMediaSourceExtension`, `IMFSourceBuffer`). It fits WebKit poorly,
because WebKit's `SourceBufferPrivate` expects the platform to parse
appended bytes into samples and keeps the buffering logic itself.

## Building WebKit's Windows port

On Windows (native), from WebKit's documentation:

- Visual Studio with "Desktop development with C++", plus CMake, Ninja,
  Perl, Python 3.11, Ruby, gperf and LLVM (clang-cl).
- In a shell with `vcvars64.bat` loaded and `CC=clang-cl`, `CXX=clang-cl`,
  `VCPKG_ROOT` pointing at a vcpkg checkout:
  `perl Tools/Scripts/build-webkit --release`
- Output: `WebKitBuild\Release\bin64` (`WebKit2.dll`, the helper process
  executables and their DLLs). Copy that next to `webby.exe`, or run webby
  with those files beside it. Set `WEBBY_GPU=1` only to test GPU
  compositing.

On Linux (cross-compile), supported upstream:

- `clang-20`, `clang-tools-20`, `lld-20`, `llvm-20`, ninja, cmake, ruby,
  gperf, then `python3 Tools/Scripts/check-win-cross-build-deps`, which
  installs the Windows SDK (via xwin), the clang runtime library and vcpkg
  under `WebKitLibraries/windows`. Its bundled xwin 0.7.0 fails on the
  current SDK; xwin 0.10 (`cargo install xwin`) works with the same
  arguments.
- `perl Tools/Scripts/build-webkit --win --release`

`.github/workflows/webkit-windows.yml` on this branch does the native
build in CI and applies `webkit/patches/*.patch` first.

WebKit revision used so far: `5fb92adf43dcc854668e7e4879ff0064bd567c3d`.

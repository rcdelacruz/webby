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

## Local build log (Windows 11, Ryzen 5 5600X, 32 GB)

Step 1 of the plan is done: the stock Windows port at the revision above
builds here in about an hour, and webby runs against it.

What the build needed beyond the list above:

- Tools that worked: Visual Studio 2022 Build Tools 17.14 (MSVC 14.44,
  Windows SDK 10.0.26100), LLVM 22.1.8, CMake 4.4, Ninja 1.13, Strawberry
  Perl 5.42, Ruby 3.3, Python 3.11, gperf 3.1.
- **Symbolic links.** WebKit's CMake forwards headers with symbolic links
  (`WEBKIT_SYMLINK_FILES`, and Skia's header directories). An ordinary
  Windows account may not create them, and the build stops with "A
  required privilege is not held by the client". Turn on Developer Mode
  (Settings, System, For developers) or build as administrator. Copies and
  hard links are not a substitute: clang-cl then sees a header reached by
  two paths as two files and reports redefinitions. Directory junctions do
  work, which is what patch 0001 uses for Skia.
- **vcpkg triplet.** WebKit's `x64-windows-webkit` triplet sets `CC` and
  `CXX` to `cl.exe`. Ports built with autotools (libiconv, gettext, libffi,
  all needed by GLib and GStreamer) then fail in libtool. Patch 0002
  removes the override.
- `build-webkit` runs `vcpkg install` on every configure. Two vcpkg runs
  wait on each other's lock, so do not prebuild dependencies in another
  directory while WebKit configures.
- To run webby against the build, copy `WebKitBuild\Release\bin` (without
  the `.pdb` files and test programs) into a directory with `webby.exe`.

How webby's own test page behaves on the stock build: `https://amp.rcdc.me/web`
loads, and playback fails with "This browser cannot play music here"
because `AudioContext` is missing. The page plays music through an
`<audio>` element connected to a Web Audio graph, and picks video by
`canPlayType` with HLS as a fallback. So Web Audio is as much a
requirement as Media Source Extensions.

## GStreamer port: decisions so far

- `USE(GLIB)` stays off on Windows. It guards about 150 places outside
  the GTK directories (IPC, `SharedBuffer`, the network cache, `URL`), and
  it would also select the GLib run loop (`PlatformUse.h` tests it before
  `OS(WINDOWS)`). Only the `wtf/glib` smart pointer headers the GStreamer
  code includes get their guard widened to `USE(GLIB) || USE(GSTREAMER)`.
- Bus messages. `connectSimpleBusMessageCallback` adds a bus signal watch,
  which is a GLib main loop source that nothing would dispatch here. On
  builds without the GLib event loop it now connects to the bus's
  `sync-message` signal, hands each message to the `RunLoop` of the thread
  that asked for the watch, and emits it there with
  `gst_bus_async_signal_func`. Existing `message::…` handlers are unchanged.
- `fnmatch` (two files) maps to `g_pattern_match_simple` on Windows;
  `/proc/self/cmdline` maps to `g_win32_get_command_line`.
- Video starts on the software path (`USE_GSTREAMER_GL` off): the fallback
  video sink plus `ImageGStreamerSkia`. Composited video needs
  `USE_COORDINATED_GRAPHICS`, which the Windows port does not have.
- The HRTF impulse responses that GTK embeds as a GResource are copied to
  `WebKit.resources\audio` and read by a new `AudioBusWin.cpp`.
- Dependencies come from a new `gstreamer` feature in WebKit's
  `vcpkg.json` (GLib, GStreamer 1.28 with base, good, bad, libav, dash,
  hls, ogg, vorbis, opus, vpx, mpg123, and a host `pkgconf`, since WebKit
  finds GStreamer through pkg-config).

## Status: media plays (local build, 2026-11-25)

With `webkit/patches` applied, webby on Windows plays, verified by pressing
play and looking:

- YouTube (Media Source Extensions), picture and sound.
- `https://amp.rcdc.me/web`: films through hls.js and music through its
  Web Audio graph.
- A local test page: AAC, E-AC-3 5.1 (Dolby Digital Plus), H.264 in MP4,
  Web Audio, Media Source, each with the picture drawn; 1080p30 shows 181
  of 181 frames.
- Fullscreen from a page (webby takes its window borderless; Esc leaves).

What it took beyond the plan above:

- **Bus messages** needed no GLib loop pumping after all: the bus's
  `sync-message` signal hands each message to WebKit's `RunLoop`.
- **No picture at first.** On this port the page is drawn through the GPU
  process, and `RemoteGraphicsContextProxy::drawVideoFrame()` was empty
  outside Cocoa. Patch 0004.
- **Ten frames a second at first.** vcpkg's GStreamer has no ORC, so
  `videoconvert` to BGRx is plain C. The sink now converts on the GPU
  (`d3d11upload ! d3d11convert ! d3d11download`) before WebKit sees the
  frame.
- `/var/tmp` as the download directory made every plain file fail with
  "Could not create temp file"; pages saw `play()` reject.
- vcpkg host tools must use the WebKit triplet (`VCPKG_HOST_TRIPLET`), or
  GStreamer's host GLib finds no zlib.
- vcpkg builds GStreamer with debug logging compiled out. `GST_DEBUG` does
  nothing; use `gst-launch-1.0`/`gst-inspect-1.0` from
  `vcpkg_installed\x64-windows-webkit\tools\gstreamer` to test pipelines.

webby's part (in `crates/app/src/platform/windows.rs`): it sets
`GST_PLUGIN_SYSTEM_PATH_1_0`, `GST_PLUGIN_SCANNER_1_0` and
`GST_REGISTRY_1_0` when a `gstreamer-1.0` folder of plugins sits next to
`webby.exe`, enables the fullscreen preference and implements the
fullscreen client.

Runtime layout that works: everything from `WebKitBuild\Release\bin`, every
DLL from `vcpkg_installed\x64-windows-webkit\bin`, the plugins from
`vcpkg_installed\x64-windows-webkit\plugins\gstreamer` in a `gstreamer-1.0`
folder, and `gst-plugin-scanner.exe` from `tools\gstreamer`, all next to
`webby.exe`. About 240 MB.

Still to do:

- Scrolling: with its own WebKit build webby now runs aware of the
  display scale with GPU compositing on (`WEBBY_DPI=unaware` and
  `WEBBY_GPU=0` turn either off). Tested on a 150% display: drawing,
  clicks (32 of 32), wheel scrolling, fullscreen and all media checks are
  correct. The two must go together: compositing in a scale-unaware
  process draws the page at two thirds size in the lower left. Whether
  scrolling now feels smooth still needs a human to judge. The downloaded
  WebKit build keeps the old settings.
- Video is composited now (see Smoothness below), but each frame is still downloaded from the GPU,
  shared with the GPU process as a bitmap and drawn. A TextureMapper layer
  fed with the D3D11 texture would avoid the round trip.
- The installer and `webkit-windows.yml` do not collect the GStreamer
  runtime yet (step 6). A trimmed plugin set would cut the size.
- No FLAC or ORC in the vcpkg feature set chosen; FLAC plays through
  libav.
- Encrypted media (DRM) is off, as before.
- Not tested: HEVC and 4K, long sessions, seeking under load, more than
  one player at a time.

## Smoothness (2026-11-25, measured on screen)

Counting frames that reach WebKit's video sink says nothing about what the
display shows. These numbers come from sampling a patch of the playing
video on screen 120 times a second (`C:\wk-tools\play.ps1 -SampleX …` on the
build machine), for a 1080p clip on a 2560x1440 display at 150%:

| Clip | Window, at first | Window, now | Fullscreen, at first | Fullscreen, now |
| --- | --- | --- | --- | --- |
| 24 fps | not measured | 24.0 | not measured | 24.0 |
| 30 fps | 14.8 | 29.8 | 11.1 | 30.0 |
| 60 fps | not measured | 50 | not measured | 48 |

What was wrong, in the order found:

1. `paint()` runs once per page tile, and each call converted the frame and
   sent all 8 MB of it to the GPU process again. Fixed by keeping the image
   of the current sample.
2. Page tiles are rasterized on the CPU (`DrawingAreaWC::createImageBuffer`
   asks for `RenderingMode::Unaccelerated`), so painting a video costs time
   in proportion to its area on screen. Fixed by patch 0006: the video gets
   a compositor layer of its own (`WCPlatformLayerImage`), and a frame is
   one shared bitmap and one texture upload whatever its size on screen.

Scrolling: the port was built with `ENABLE_SMOOTH_SCROLLING` off, so a wheel
notch moved the page 100 pixels in one step. Patch 0008 turns it on and
webby sets the `ScrollAnimatorEnabled` preference: the same input now gives
steps of about 19 pixels at 60 a second.

Left: 60 fps video reaches about 50 on screen; each frame still crosses
from the web process to the GPU process as a copied bitmap. Sharing the
Direct3D texture would remove the copy. The full patch series has been
built piece by piece but not yet in one clean build from scratch (0008
changes a build option, which recompiles everything).

## Smoothness, second pass (2026-11-25 evening)

Everything below was measured on screen (a patch of the display sampled 120
times a second) on real pages: the amp app and Wikipedia, on a 2560x1440
display at 150% and 120Hz. Earlier "smooth" figures in this file came from
test pages and were wrong for real sites; see the first cause below.

| Check | Before | After |
| --- | --- | --- |
| Wheel scrolling, Wikipedia | 25-33 ms per screen update | 16.6 ms |
| Wheel scrolling, amp app (scrolls an inner panel) | about 80 ms | 16.7 ms |
| Film in amp (1080p H.264, 23.976 fps), window and fullscreen | full rate, uneven | 23.8/s, typical gap 41.7 ms |
| 30 fps clip | typical gap 40.8 ms | 33.3 ms |
| 60 fps 1080p clip | 47-50/s | 54-58/s |
| Thin lines across images at 150% scaling | present | gone |

Causes, each a patch:

- 0009: the Windows run loop rounded waits up to the 15.6 ms system tick, so
  a 16 ms timer fired every 31 ms on any page without a fast script timer.
  (My scroll test page had one, which is why it measured 60 a second.)
- 0008 only takes effect in a fresh build directory: an existing one keeps
  `ENABLE_SMOOTH_SCROLLING=OFF` in CMakeCache.txt. Check `cmakeconfig.h`.
- 0010: tile rects were truncated at fractional scale (the lines); the GPU
  process blocked on vsync at every update; video frame memory was
  allocated per frame.
- 0011: rendering updates are paced by DWM's presents (DwmFlush on a
  thread) instead of a timer, and pages are told the display's real refresh
  rate. webby also sets `PreferPageRenderingUpdatesNear60FPSEnabled` to
  false so pages update at that rate (`WEBBY_FULL_RATE=0` undoes it).
- 0012: on this port every scroll step re-laid-out sticky and fixed boxes
  and repainted their containers; sticky boxes had no layer; scrollable
  boxes repainted all they showed. All three now just move layers.
- 0013: the pipeline ran on the audio device clock, which steps 10 ms at a
  time on Windows; GStreamer's clock waits used the coarse system tick;
  decode, conversion and the sink shared one thread.

Not fixed:

- 60 fps video above 1080p: a 1440p60 VP9 clip (GPU decoded) reaches about
  47/s. Each frame is still read back from the GPU, copied to shared memory
  and uploaded again. The fix is to hand the decoder's Direct3D texture to
  the compositor.
- Scrolling on a page whose scripts keep the main thread busy (YouTube
  while a video plays: about 33 ms per update, stalls to 0.3 s). This port
  scrolls on the main thread; it needs a scrolling thread.
- A film frame arrives 15-25 ms late one to three times in six seconds:
  frames go through the page's main thread on the way to the screen.
- A new row of tiles is painted on the CPU when it scrolls into view, a
  30-50 ms hitch now and then on long scrolls.

Tools on the build machine (`C:\wk-tools`): `scrolltest.ps1`, `sample.ps1`,
`filmrun.ps1`, `cadrun.ps1`, `amprun.ps1` measure on screen; `with-probes.diff`
and `WebbyTrace.h.txt` are timing probes for the update loop
(`strip-probes.py` removes them); `gstbench.py` times the GStreamer path
without WebKit.

## Handoff (2026-11-25, end of day)

Patches 0014-0016 came after the section above:

- 0014: the refresh signal waits on the display's vertical blank
  (`IDXGIOutput::WaitForVBlank`), DwmFlush only as fallback.
- 0015: video decoded by Direct3D 11 stays on the GPU. The player copies each
  frame, GPU to GPU, into a shared texture and the compositor opens it by
  share handle through ANGLE. `WEBKIT_GST_D3D11_DOWNLOAD_FRAMES=1` restores
  the read-back path.
- 0016: the player sends each frame from its streaming thread straight to the
  GPU process (`RemoteWCLayerTreeHost::PresentSharedTexture`), not through the
  page's main thread.

Measured on screen, in a window, with 0001-0016: 1080p60 59.8/s, 4K60 VP9
59.1/s, 24 fps gaps 41.7 ms typical and 45.6 ms worst, web process CPU for
4K60 about 5%. The media check page passes 9/9.

Open, in order of importance:

1. Flicker when scrolling the lists of the amp app (Movies, Music,
   Collections), reported by the user on the build with 0001-0016. Not
   diagnosed. One capture during a fast scroll of Albums showed every cover as
   a blank placeholder while the text was drawn. Suspects: 0012 (composited
   overflow scrolling, sticky layers) and 0010 (tile rects). To bisect, build
   without 0012's `setAsyncOverflowScrollingEnabled` line first.
2. The app's Video > Movies page stayed on skeleton placeholders twice in a
   test window. Cause unknown.
3. Scrolling a script-heavy page (YouTube during playback) is jerky: this port
   scrolls on the main thread.
4. The app's own fullscreen button was not re-checked after 0015 and 0016.
5. Fullscreen 60 fps has no trustworthy measurement: a GDI screen capture does
   not see every frame of a fullscreen window.

State of the build machine: WebKit tree `C:\wk`, branch `webby-gstreamer`,
17 commits on 5fb92adf43 (these patches). Build with
`C:\wk-tools\ninja-gst.cmd`, collect with `C:\wk-tools\assemble-gst.sh <dir>`.
`C:\wk-tools\gst` has the build of 0001-0016; `C:\wk-tools\gst-prev` an older
one without the scrolling work (no flicker, slow scrolling).
`C:\wk-tools\gst-dev` is a mixed copy: its `WebKit2.dll` is the build of
0001-0017 (same hash as `C:\wk-build-gst\Release\bin`), but its three WebKit
helper executables and its `webby.exe` are from 21:59, an hour older. The
measurements below were taken with that mix.

## Flicker in the amp lists: found and fixed (2026-11-25, late; patch 0017)

What it was. This is open item 1 above. Whether item 2 (Movies stuck on
placeholders) had the same cause is not known. In the amp
lists the mouse rests on a card while the page scrolls under it. Hovering a
card starts a short animation on its play button, and while that runs WebKit
makes that card, and every card after it in the list, a layer of its own
(about 40 at a time). On this port those layers then came out blank except
for the ones that would be on screen with the list scrolled to the top. The
page itself had already been repainted without the cards, so whole stretches
of the list went empty for 100 to 300 ms, several times in one scroll.
It depends on where the mouse is, so some scrolls showed it and some did not.

Why. A composited layer inside a scrollable box does not hang under the
scroller's own layers. It hangs under a clip layer and a "scrolling proxy"
layer that repeats the scroller's offset. Patch 0012 turned composited
overflow scrolling on, and two things about proxies were missing here:

- Ports with a scrolling thread let it move the proxies. Here nothing moved
  them between geometry updates. The main thread now does, in
  `RenderLayerBacking::updateScrollOffset`.
- `GraphicsLayerWC::computeVisibleAndCoverageRect` left out the bounds origin
  of a parent that does not clip, which is what a proxy is. Every layer below
  a proxy took the unscrolled viewport for its visible part, and on this port
  a layer only has tiles where it is visible. This was the part that made
  them blank.

How it was found, for next time: a contact sheet of the screen during a
scroll (one small capture every 60 ms or so) showed which frames were blank;
logging each layer's visible rect and tile paints from `recursiveCommitChanges`
showed card layers with a visible rect of 0x0. Suspects that were ruled out:
tile rect rounding (0010), tile bitmaps arriving empty, the flush before an
update timing out, view transitions.

Measured on screen with 0001-0017, amp Albums and Movies lists, wheel scrolls
of 15 and 30 notches down and up:

| Check | 0001-0016 | 0001-0017 |
| --- | --- | --- |
| Captures in which part of the list is missing, of about 38 per scroll | several (counted by eye on the contact sheets, roughly 6 to 10) | 0 |
| Wheel scrolling, typical gap between screen updates, amp and Wikipedia | 16.7 ms | 16.7 ms |
| Movies page shows its posters after loading (8 loads) | stuck on placeholders twice (from the earlier handoff, not re-measured) | 8 of 8; the failure was never reproduced, so this does not show it is fixed |
| The app's fullscreen button during a film: enter, picture, Esc | not checked | works, 23.7 frames/s in fullscreen and in the window |
| Media check page | 9/9 | 9/9 |

The first rows were measured on a 3840x2160 display at 225% and the
fullscreen row on 2560x1440 at 150%.

The 17 patch files, applied to 5fb92adf43, give the same tree as `C:\wk`
(compared by tree hash). The build in `C:\wk-build-gst` is up to date with
that tree (ninja reports no work) and has `ENABLE_SMOOTH_SCROLLING 1` in
`cmakeconfig.h`, so a separate build from scratch was not done. One was
started in `C:\wk-build-clean` and stopped early; that directory can be
deleted.

Still open:

1. The cards still become layers and stop being layers as the mouse passes
   over them. Nothing goes blank now, but each switch repaints the visible
   part of the list on the CPU: a gap of 50 to 80 ms now and then on a long
   scroll. Fewer switches would need WebKit to keep cards after an animating
   one in the list's own layer.
2. Scrolling a script-heavy page (YouTube during playback) is jerky: this
   port scrolls on the main thread. Needs a scrolling thread; a large change.
3. Fullscreen 60 fps still has no trustworthy measurement.
4. `webkit-windows.yml` now collects the GStreamer runtime with WebKit, but
   that step has never run: the CI build is too costly, so the workflow only
   starts by hand and WebKit is built on the build machine. The installer
   built by `ci.yml` still bundles the downloaded WebKit. Switching the installer over, and trimming
   the plugin set, are the remaining parts of step 6.

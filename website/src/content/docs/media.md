---
title: Media on Windows
description: What plays in webby on Windows, how to check a build, and the switches for testing.
order: 4
---

On macOS and Linux, webby plays whatever the system's WebKit plays. On
Windows there is no system WebKit, and WebKit's Windows port is built
without Media Source Extensions and Web Audio. From 0.1.1, webby ships
its own build of that port with WebKit's GStreamer media backend added.

## What plays

Checked by hand on Windows 11:

- YouTube, which needs Media Source Extensions
- HLS streams played through hls.js
- Web Audio
- AAC, Dolby Digital Plus 5.1 (E-AC-3) and H.264 in MP4
- Fullscreen from a page's own button; Esc leaves

Not yet:

- Encrypted (DRM) video
- Smooth scrolling on a page whose scripts keep it busy, such as YouTube
  while a video plays

## Checking a build

Type `webby:media` in the address bar. The page lists what this build
tells web pages: the user agent, whether `MediaSource`, `AudioContext`
and `MediaKeys` exist, and what `canPlayType`, `isTypeSupported` and
`mediaCapabilities.decodingInfo` answer for common formats. It works on
every platform and is the first thing to attach to a media bug report.

## Switches for testing

These environment variables exist to narrow down a rendering problem on
Windows. You should not need them in normal use.

| Variable | Values | What it does |
| --- | --- | --- |
| `WEBBY_GPU` | `1`, `0` | Turns GPU compositing on or off. |
| `WEBBY_DPI` | `aware`, `unaware` | Whether webby follows each monitor's scale itself or lets Windows magnify the window. |
| `WEBBY_FULL_RATE` | `1`, `0` | `0` holds page updates near 60 a second instead of the display's refresh rate. |

GPU compositing and scale awareness go together: turn one off and leave
the other on, and pages draw at the wrong size.

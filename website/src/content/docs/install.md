---
title: Install
description: Install webby on Windows, macOS or Linux, and get past the first-launch warning.
order: 1
---

Get the installer for your system from the [download page](/download/).

| System | File | Install |
| --- | --- | --- |
| Windows 10 or 11, 64-bit | `webby-<version>-windows-x64-setup.exe` | Run it. It installs for your user only, with no administrator prompt. WebKit is included. |
| macOS 11 or later | `webby-<version>-macos.dmg` | Open it and drag webby to Applications. |
| Debian or Ubuntu, 64-bit | `webby_<version>_amd64.deb` | `sudo apt install ./webby_<version>_amd64.deb` |

## The first-launch warning

webby is an alpha and its installers are not code signed yet, so Windows
and macOS warn you the first time.

**Windows.** SmartScreen shows "Windows protected your PC". Choose
*More info*, then *Run anyway*.

**macOS.** The first launch shows "Apple could not verify webby is free of
malware". Click *Done*, open *System Settings → Privacy & Security*,
scroll to *Security* and click *Open Anyway*. Or run this in Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/webby.app
```

## Which platforms are tested

| Platform | Engine | State |
| --- | --- | --- |
| Windows | WebKit's own Windows port (`WebKit2.dll`); no WebView2, no Chromium | Tested by hand on Windows 11 |
| macOS | System WebKit (`WKWebView`), the engine Safari uses | Builds and packages in CI; not yet tested by hand |
| Linux | WebKitGTK 6.0 (GTK 4) | Tested end to end |

If something misbehaves, please
[open an issue](https://github.com/rcdelacruz/webby/issues).

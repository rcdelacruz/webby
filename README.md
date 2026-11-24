# webby

A small web browser written in Rust that renders with **WebKit on every
platform**, including Windows, where Safari has not been available since 2012.

| Platform | Engine | State |
| --- | --- | --- |
| Linux | WebKitGTK 6.0 (GTK 4) | Builds, runs, exercised end to end |
| macOS | System WebKit (`WKWebView`), the engine Safari uses | Compiles; not yet run on a Mac |
| Windows | WebKit's own Windows port (`WebKit2.dll`), no WebView2 or Chromium | Compiles; not yet run on Windows |

"Compiles" means the code type-checks for that target. Those two backends
were written on Linux and need a first run on real hardware, so expect to
fix things there.

## What it does

- Tabs: open, close, switch, middle-click to close; `window.open` and
  `target=_blank` open as a tab next to the current one
- Address bar that accepts URLs, bare hosts (`example.com`,
  `localhost:3000`) and search terms (DuckDuckGo)
- Back, forward, reload, stop, load progress, HTTPS indicator
- Start page listing the sites you visit most
- History and session restore: tabs reopen where you left off
- Light and dark appearance following the system

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| New tab | Ctrl+T | ⌘T |
| Close tab | Ctrl+W | ⌘W |
| Address bar | Ctrl+L | ⌘L |
| Reload | Ctrl+R, F5 | ⌘R |
| Back / forward | Alt+← / Alt+→ | ⌘[ / ⌘] |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | ⌃Tab / ⌃⇧Tab |

## Building

Rust 1.88 or newer.

**Linux** (Debian/Ubuntu package names):

```sh
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev
cargo run --release
```

**macOS:**

```sh
cargo run --release
```

**Windows** (MSVC toolchain, PowerShell):

```powershell
cargo build --release
.\scripts\fetch-webkit-windows.ps1 -Destination target\release
.\target\release\webby.exe
```

webby loads `WebKit2.dll` when it starts, so the build itself needs no
WebKit SDK. Apple publishes no Windows binaries of WebKit; the script
downloads the build of WebKit's Windows port that the Playwright project
publishes for browser testing and unpacks it next to `webby.exe`. To use
a WebKit you built yourself from [webkit.org](https://webkit.org/webkit-on-windows/)
sources, zip its `bin` directory and point `WEBBY_WEBKIT_URL` at it, or
copy the files next to `webby.exe` by hand.

## Installers

| Platform | Output | Built by |
| --- | --- | --- |
| Linux | `dist/webby_<version>_<arch>.deb` | `packaging/linux/build-deb.sh` |
| macOS | `dist/webby-<version>-macos.dmg` (universal) | `packaging/macos/build-dmg.sh` |
| Windows | `webby-<version>-windows-x64-setup.exe` | Inno Setup, `packaging/windows/webby.iss` |

CI builds all three on every push and attaches them to the run. Pushing a
tag such as `v0.1.0` also publishes them as a GitHub release.

None of the installers is code signed. macOS will refuse to open the app
on first launch (right-click the app and choose Open, or sign it with a
Developer ID), and Windows SmartScreen will warn before running the
setup. The Windows installer includes WebKit and installs per user, with
no administrator prompt.

History and the session file live in the platform data directory
(`~/.local/share/webby`, `~/Library/Application Support/webby`,
`%APPDATA%\webby`). Set `WEBBY_PROFILE` to use another directory.

## How it is put together

```
crates/core   Tab model, address-bar parsing, history, session, start page.
              No platform code; fully unit tested.
  ui/chrome.html   The tab strip and toolbar, as one HTML document.
crates/app    The binary. One module per platform implementing core's
              `Shell` trait on that platform's WebKit:
                platform/linux.rs     gtk4 + webkit6 crates
                platform/macos.rs     objc2 bindings to AppKit and WebKit
                platform/windows.rs   Win32 + platform/windows/wk.rs, hand-
                                      written bindings to WebKit's C API
```

The window holds one small web view for the browser's own UI and one web
view per tab. The UI is the same HTML on all three platforms; it posts JSON
commands to the core through a script message handler that only that view
has, so web pages cannot reach it. The core answers by calling
`webby.update(state)` in the UI. Each platform therefore only has to
create windows and web views and forward their state changes: a few
hundred lines each.

## Not there yet

Downloads, bookmarks, error pages on macOS and Windows (a failed load
leaves the previous page),  find in page, private windows, a history view,
permission prompts (camera, location, notifications), JavaScript dialogs
(`alert`, `confirm`) on macOS and Windows, per-site zoom, content blocking, code signing, and an icon embedded in the Windows executable. On Windows the DPI scale is read once at
startup.

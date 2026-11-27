<h1><img src="packaging/icons/webby-wordmark.svg" alt="webby" width="220"></h1>

A small web browser written in Rust that renders with **WebKit on every
platform**, including Windows, where Safari has not been available since
2012.

![webby showing its start page](docs/screenshot.png)

## Download

Get the installer for your system from the
[latest release](https://github.com/rcdelacruz/webby/releases/latest).

| System | File | Install |
| --- | --- | --- |
| Windows 10/11 (64-bit) | `webby-<version>-windows-x64-setup.exe` | Run it. Installs for your user only; no administrator prompt. WebKit is included. |
| macOS 11 or later (Apple silicon and Intel) | `webby-<version>-macos.dmg` | Open it and drag webby to Applications. |
| Debian / Ubuntu (64-bit) | `webby_<version>_amd64.deb` | `sudo apt install ./webby_<version>_amd64.deb` |

The installers are not code signed yet, so the system will warn you the
first time:

- **Windows:** SmartScreen shows "Windows protected your PC". Choose
  *More info*, then *Run anyway*.
- **macOS:** the first launch shows "Apple could not verify webby is free
  of malware". Click *Done*, open *System Settings → Privacy & Security*,
  scroll to *Security* and click *Open Anyway*. Or run
  `xattr -dr com.apple.quarantine /Applications/webby.app` in Terminal.

## Status

| Platform | Engine | State |
| --- | --- | --- |
| Windows | webby's own build of WebKit's Windows port (`WebKit2.dll`); no WebView2, no Chromium | Tested by hand on Windows 11 |
| macOS | System WebKit (`WKWebView`), the engine Safari uses | Builds and packages in CI; not yet tested by hand |
| Linux | WebKitGTK 6.0 (GTK 4) | Runs; tested end to end |

If something misbehaves, please open an issue.

## Features

- Tabs: open, close, switch, middle-click to close. `window.open` and
  `target=_blank` open as a tab next to the current one.
- Address bar that accepts URLs, bare hosts (`example.com`,
  `localhost:3000`) and search terms (DuckDuckGo).
- Back, forward, reload, stop, load progress and an HTTPS indicator.
- Start page listing the sites you visit most.
- History and session restore: tabs reopen where you left off.
- Light and dark appearance, following the system.
- On Windows: streaming audio and video (YouTube, HLS, Web Audio), video
  kept on the GPU, smooth scrolling and page fullscreen, through webby's
  own build of WebKit. See [webkit/](webkit/).
- `webby:media` in the address bar shows what the build reports to web
  pages about audio and video.

### Keyboard shortcuts

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| New tab | Ctrl+T | ⌘T |
| Close tab | Ctrl+W | ⌘W |
| Address bar | Ctrl+L | ⌘L |
| Reload | Ctrl+R, F5 | ⌘R |
| Back / forward | Alt+← / Alt+→ | ⌘[ / ⌘] |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | ⌃Tab / ⌃⇧Tab |

### Where your data lives

History and the open-tabs session are plain files in the platform data
directory:

| System | Directory |
| --- | --- |
| Linux | `~/.local/share/webby` |
| macOS | `~/Library/Application Support/webby` |
| Windows | `%APPDATA%\webby` |

Set the `WEBBY_PROFILE` environment variable to use another directory.

## Building from source

You need Rust 1.88 or newer.

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

On Windows, webby loads `WebKit2.dll` when it starts, so the build itself
needs no WebKit SDK. Apple publishes no Windows binaries of WebKit; the
script downloads the build of WebKit's Windows port that the Playwright
project publishes for browser testing and unpacks it next to `webby.exe`.
That build has no streaming media and always looks light. The released
installer ships webby's own build instead: WebKit from
[webkit.org](https://webkit.org/webkit-on-windows/) sources with the
patches in [webkit/](webkit/). To run against a WebKit you built
yourself, use `scripts/package-windows.ps1 -StageOnly`, or zip its files
and point `WEBBY_WEBKIT_URL` at the zip.

### Building the installers

| Platform | Command | Output |
| --- | --- | --- |
| Linux | `packaging/linux/build-deb.sh` (after `cargo build --release`) | `dist/webby_<version>_<arch>.deb` |
| macOS | `packaging/macos/build-dmg.sh` | `dist/webby-<version>-macos.dmg` |
| Windows | `scripts/package-windows.ps1` (needs a WebKit build and Inno Setup 6) | `dist/webby-<version>-windows-x64-setup.exe` |

CI builds the macOS and Linux installers on every push and attaches them
to the workflow run. Pushing a tag such as `v0.1.0` publishes them as a
GitHub release. The Windows installer is built on the build machine,
with webby's own WebKit, and added to the release by hand.

## How it works

The window holds one small web view for the browser's own interface (the
tab strip and toolbar) and one web view per tab. All browser logic lives
in a platform-independent core; each platform only creates windows and
web views and reports what they do.

```mermaid
flowchart TB
    subgraph core["crates/core: shared, no platform code"]
        browser["Browser<br/>tabs, address bar, history, session"]
        ui["ui/chrome.html<br/>tab strip and toolbar"]
    end

    shell{{"Shell trait"}}
    browser --> shell

    subgraph app["crates/app: one backend per platform"]
        linux["platform/linux.rs<br/>gtk4 + webkit6 crates"]
        macos["platform/macos.rs<br/>objc2 bindings"]
        windows["platform/windows.rs<br/>Win32 + hand-written C API bindings"]
    end

    shell --> linux
    shell --> macos
    shell --> windows

    linux --> webkitgtk[("WebKitGTK")]
    macos --> wkwebview[("System WebKit")]
    windows --> webkit2[("WebKit2.dll")]
```

The interface is the same HTML document on all three platforms. It talks
to the core through a script message handler that only that one web view
has, so web pages can never send commands to the browser.

```mermaid
sequenceDiagram
    participant UI as Toolbar (chrome.html)
    participant Core as Browser (core)
    participant Shell as Platform backend
    participant Page as Tab web view

    UI->>Core: navigate "example.com"
    Core->>Core: resolve to https://example.com
    Core->>Shell: navigate(tab, url)
    Shell->>Page: load URL
    Page-->>Shell: URL, title, progress changed
    Shell-->>Core: ViewState snapshot
    Core-->>UI: webby.update(state)
```

Every event goes through one queue (`Runtime`), so the browser is never
re-entered when a toolkit fires a callback from inside the call that
caused it.

| Path | What it holds |
| --- | --- |
| `crates/core/src/browser.rs` | Tab model and the rules that drive it |
| `crates/core/src/omnibox.rs` | Turns address-bar input into a URL or a search |
| `crates/core/src/store.rs` | History and session files |
| `crates/core/src/startpage.rs` | The new-tab page |
| `crates/core/ui/chrome.html` | The tab strip and toolbar |
| `crates/app/src/platform/` | The three platform backends |
| `packaging/` | Icons and installer scripts |

## Not there yet

- Downloads, bookmarks, find in page, a history view, private windows
- Permission prompts (camera, location, notifications)
- Error pages and JavaScript dialogs (`alert`, `confirm`) on macOS and
  Windows
- Encrypted (DRM) video on Windows
- Per-site zoom and content blocking
- Code signing, and an icon embedded in the Windows executable

## License

webby is open source under the [MIT License](LICENSE).

WebKit itself is a separate project under its own LGPL and BSD licenses.
The Windows installer bundles webby's build of WebKit together with
GStreamer, FFmpeg and the other libraries it needs, each under its own
licence; their texts are installed in the `licenses` folder. macOS and
Linux use the WebKit already provided by the system.

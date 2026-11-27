---
title: How it works
description: One platform-independent core, three thin platform backends.
order: 6
---

The window holds one small web view for the browser's own interface, the
tab strip and toolbar, and one web view per tab. All browser logic lives
in a platform-independent core. Each platform only creates windows and
web views and reports what they do.

| Layer | Where | What it holds |
| --- | --- | --- |
| Core | `crates/core` | Tabs, the address bar, history, session restore, the start page. No platform code. |
| Interface | `crates/core/ui/chrome.html` | The tab strip and toolbar: one HTML document shared by every platform. |
| Linux backend | `crates/app/src/platform/linux.rs` | GTK 4 and WebKitGTK through the `gtk4` and `webkit6` crates. |
| macOS backend | `crates/app/src/platform/macos.rs` | System WebKit through `objc2` bindings. |
| Windows backend | `crates/app/src/platform/windows.rs` | Win32 and hand-written bindings to WebKit's C API. |

## A navigation, step by step

1. You type `example.com` in the toolbar. The toolbar sends `navigate` to
   the core.
2. The core resolves it to `https://example.com` and asks the platform
   backend to load it in the tab's web view.
3. The web view reports its URL, title and progress as they change.
4. The backend hands the core a snapshot of that state, and the core
   updates the toolbar.

## Two rules that keep it simple

**Pages cannot drive the browser.** The toolbar talks to the core through
a script message handler that only its own web view has. A web page has
no such channel, so it can never send commands to the browser.

**One queue.** Every event goes through a single queue, so the browser is
never re-entered when a toolkit fires a callback from inside the call
that caused it.

The [README](https://github.com/rcdelacruz/webby#how-it-works) has the
same material with diagrams.

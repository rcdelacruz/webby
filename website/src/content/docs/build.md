---
title: Build from source
description: Build and run webby yourself, and package the installers.
order: 5
---

You need Rust 1.88 or newer.

## Linux

Debian and Ubuntu package names:

```sh
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev
cargo run --release
```

## macOS

```sh
cargo run --release
```

## Windows

With the MSVC toolchain, in PowerShell:

```powershell
cargo build --release
.\scripts\fetch-webkit-windows.ps1 -Destination target\release
.\target\release\webby.exe
```

webby loads `WebKit2.dll` when it starts, so the build itself needs no
WebKit SDK. Apple publishes no Windows binaries of WebKit. By default the
script downloads the build of WebKit's Windows port that the Playwright
project publishes for browser testing, and unpacks it next to
`webby.exe`. That build has no streaming media and always looks light.

To use another WebKit, such as one built with the patches in
[`webkit/patches`](https://github.com/rcdelacruz/webby/tree/main/webkit),
run `scripts/package-windows.ps1 -StageOnly`, which collects it with
`webby.exe` into a folder you can run from.

## Installers

| Platform | Command | Output |
| --- | --- | --- |
| Linux | `packaging/linux/build-deb.sh`, after `cargo build --release` | `dist/webby_<version>_<arch>.deb` |
| macOS | `packaging/macos/build-dmg.sh` | `dist/webby-<version>-macos.dmg` |
| Windows | `scripts/package-windows.ps1`, which needs a WebKit build and Inno Setup 6 | `dist/webby-<version>-windows-x64-setup.exe` |

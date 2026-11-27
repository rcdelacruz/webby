---
title: Your data
description: Where webby keeps your history and open tabs, and how to use another directory.
order: 3
---

webby has no account, no sync and no telemetry. History and the open-tabs
session are plain files in your system's data directory:

| System | Directory |
| --- | --- |
| Linux | `~/.local/share/webby` |
| macOS | `~/Library/Application Support/webby` |
| Windows | `%APPDATA%\webby` |

## Using another directory

Set the `WEBBY_PROFILE` environment variable to keep a profile somewhere
else, for example to try webby without touching your usual history.

On Linux and macOS:

```sh
WEBBY_PROFILE=/tmp/webby-test webby
```

On Windows, in PowerShell:

```powershell
$env:WEBBY_PROFILE = "$env:TEMP\webby-test"
webby
```

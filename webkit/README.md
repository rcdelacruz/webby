# webby's WebKit for Windows

WebKit's Windows port is built without Media Source Extensions, Web Audio
and encrypted media, so streaming audio and video do not play in it. This
directory holds what webby needs to build its own WebKit for Windows with
those added.

- `.github/workflows/webkit-windows.yml` builds WebKit's Windows port from
  source and uploads the binaries webby loads (`WebKit2.dll` and the files
  beside it).
- `patches/` holds webby's changes to WebKit, applied in name order before
  the build.

The WebKit revision being built is pinned by `WEBKIT_REF` in the workflow.

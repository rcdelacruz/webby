export const REPO = "rcdelacruz/webby";
export const GITHUB = `https://github.com/${REPO}`;
export const RELEASES = `${GITHUB}/releases`;
export const LATEST = `${RELEASES}/latest`;

export type PlatformKey = "windows" | "macos" | "linux";

export interface Platform {
  key: PlatformKey;
  name: string;
  file: string;
  /** How the installer's file name ends; used to find it in a release. */
  suffix: string;
  engine: string;
  requires: string;
  status: string;
  blurb: string;
}

export const PLATFORMS: Platform[] = [
  {
    key: "windows",
    name: "Windows",
    file: ".exe",
    suffix: "-setup.exe",
    engine: "WebKit's Windows port (WebKit2.dll)",
    requires: "Windows 10 or 11, 64-bit",
    status: "Tested by hand on Windows 11",
    blurb: "WebKit's own Windows port, bundled in the installer. No WebView2, no Chromium. Installs for your user without an administrator prompt.",
  },
  {
    key: "macos",
    name: "macOS",
    file: ".dmg",
    suffix: ".dmg",
    engine: "System WebKit (WKWebView)",
    requires: "macOS 11 or later, Apple silicon and Intel",
    status: "Builds and packages in CI; not yet tested by hand",
    blurb: "Runs on the system WebKit, the engine Safari uses, so pages look and behave as they do there.",
  },
  {
    key: "linux",
    name: "Linux",
    file: ".deb",
    suffix: ".deb",
    engine: "WebKitGTK 6.0 (GTK 4)",
    requires: "Debian or Ubuntu, 64-bit",
    status: "Tested end to end",
    blurb: "Built on WebKitGTK and GTK 4, so it feels at home on a modern desktop and uses the WebKit your distribution keeps patched.",
  },
];

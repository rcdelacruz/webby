// Point the download buttons at the visitor's platform, and at the exact
// files of the latest release when GitHub's API is reachable. Without
// JavaScript, or if the request fails, every button still leads to the
// releases page.
(() => {
  const REPO = "rcdelacruz/webby";
  const NAMES = { windows: "Windows", macos: "macOS", linux: "Linux" };
  const SUFFIX = { windows: "-setup.exe", macos: ".dmg", linux: ".deb" };

  const ua = navigator.userAgent;
  const mobile = /Android|iPhone|iPad|iPod/i.test(ua);
  const os = mobile ? null : /Windows/i.test(ua) ? "windows" : /Mac OS X|Macintosh/i.test(ua) ? "macos" : /Linux|X11/i.test(ua) ? "linux" : null;

  const main = document.querySelectorAll("[data-download]");
  if (os) {
    document.querySelectorAll("[data-download-label]").forEach(el => (el.textContent = `Download for ${NAMES[os]}`));
    document.querySelector(`[data-os="${os}"]`)?.classList.add("current");
  }

  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } })
    .then(response => (response.ok ? response.json() : Promise.reject(response.status)))
    .then(release => {
      const version = String(release.tag_name || "").replace(/^v/, "");
      if (version) document.querySelectorAll("[data-version]").forEach(el => (el.textContent = version));
      const assets = {};
      for (const [key, suffix] of Object.entries(SUFFIX)) {
        const match = (release.assets || []).find(asset => asset.name.endsWith(suffix));
        if (match) assets[key] = match.browser_download_url;
      }
      document.querySelectorAll("[data-asset]").forEach(el => {
        if (assets[el.dataset.asset]) el.href = assets[el.dataset.asset];
      });
      if (os && assets[os]) main.forEach(el => (el.href = assets[os]));
    })
    .catch(() => {});
})();

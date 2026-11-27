import { defineConfig } from "astro/config";

// SITE_URL (for example https://webby.example) makes the canonical and
// social-preview links absolute. The site builds and works without it.
export default defineConfig({
  site: process.env.SITE_URL || undefined,
  trailingSlash: "always",
  build: { format: "directory" },
  markdown: { shikiConfig: { themes: { light: "github-light", dark: "github-dark" } } },
});

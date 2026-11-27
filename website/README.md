# webby website

The site for webby, built with [Astro](https://astro.build) into plain
static files. Fonts are bundled, so the only request a page makes to
another server is one call to the GitHub API to find the latest release.

## Develop

Needs Node 22.23.4 or newer.

```sh
cd website
npm install
npm run dev      # http://localhost:4321, reloads as you edit
npm run build    # type-checks, then writes dist/
npm run preview  # serves dist/
```

## Where things are

| Path | What it holds |
| --- | --- |
| `src/content/releases/` | One Markdown file per release. The newest is the version the home page shows and the "New in" section describes. |
| `src/content/docs/` | The docs pages. `order` in the front matter sets their sequence. |
| `src/data/site.ts` | Repository links and the three platforms: engine, requirements, state. |
| `src/pages/` | Home, download, docs, changelog and the 404 page. |
| `src/styles/global.css` | All styles and the colour tokens, light and dark. |

## Releasing a version

Add `src/content/releases/<version>.md` with the release notes. Set
`highlights` in its front matter to show a "New in" section on the home
page. Deploy after the GitHub release is published: the download buttons
and the version shown follow GitHub's latest release once the page loads.

## Deploy on Coolify

1. Add a new resource from this Git repository.
2. Build pack: **Dockerfile**.
3. Base directory: `/website`. Dockerfile location: `/Dockerfile`.
4. Port: `80`.
5. Optional: a build argument `SITE_URL` with the public address, which
   makes the canonical and social-preview links absolute.
6. Set your domain and deploy.

Under *Advanced*, a watch path of `website/**` keeps browser-only commits
from redeploying the site. A health check is available at `/health`.

Or run the container anywhere:

```sh
docker build -t webby-site website
docker run --rm -p 8080:80 webby-site
```

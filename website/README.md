# webby website

The marketing site for webby: plain HTML, CSS and a little JavaScript, with
no build step and no external requests except one call to the GitHub API
to find the latest release.

## Preview locally

```sh
python3 -m http.server 8080 -d website
```

## Deploy on Coolify

1. Add a new resource from this Git repository.
2. Build pack: **Dockerfile**.
3. Base directory: `/website`. Dockerfile location: `/Dockerfile`.
4. Port: `80`.
5. Set your domain and deploy.

Under *Advanced*, a watch path of `website/**` keeps browser-only commits
from redeploying the site. A health check is available at `/health`.

Or run the container anywhere:

```sh
docker build -t webby-site website
docker run --rm -p 8080:80 webby-site
```

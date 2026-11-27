//! The page shown in a new tab.

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The wordmark, shown above the search box.
const LOGO: &str = include_str!("../../../packaging/icons/webby-wordmark.svg");

pub fn html(top_sites: &[(String, String)]) -> String {
    let mut tiles = String::new();
    for (url, title) in top_sites {
        let label = if title.is_empty() { url } else { title };
        let initial = label.chars().find(|c| c.is_alphanumeric()).unwrap_or('•').to_uppercase();
        tiles.push_str(&format!(
            "<a class=tile href=\"{}\"><span class=badge>{}</span><span class=label>{}</span></a>",
            escape(url),
            initial,
            escape(label)
        ));
    }
    let sites = if tiles.is_empty() { String::new() } else { format!("<h2>Frequently visited</h2><div class=grid>{tiles}</div>") };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>New Tab</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
:root {{ color-scheme: light dark; --bg:#f6f6f4; --fg:#1d1d1f; --muted:#6e6e73; --card:#fff; --line:#d9d9d6; --accent:#0a6cff; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#09090b; --fg:#f4f4f5; --muted:#8a8c97; --card:#141418; --line:#26262d; --accent:#4c9bff; }} }}
* {{ box-sizing: border-box; }}
body {{ margin:0; background:var(--bg); color:var(--fg); font:15px/1.4 system-ui,-apple-system,"Segoe UI",sans-serif; }}
main {{ max-width:720px; margin:0 auto; padding:12vh 24px 48px; }}
h1 {{ width:250px; margin:0 auto 30px; }} h1 svg {{ display:block; }}
form input {{ width:100%; padding:13px 18px; font:inherit; color:inherit; background:var(--card); border:1px solid var(--line); border-radius:12px; outline:none; }}
form input:focus {{ border-color:var(--accent); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent); }}
h2 {{ font-size:12px; font-weight:600; text-transform:uppercase; letter-spacing:.06em; color:var(--muted); margin:44px 0 12px; }}
.grid {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(150px,1fr)); gap:10px; }}
.tile {{ display:flex; align-items:center; gap:10px; padding:10px 12px; background:var(--card); border:1px solid var(--line); border-radius:10px; color:inherit; text-decoration:none; min-width:0; }}
.tile:hover {{ border-color:var(--accent); }}
.badge {{ flex:none; width:28px; height:28px; border-radius:7px; background:var(--accent); color:#fff; display:grid; place-items:center; font-weight:600; font-size:13px; }}
.label {{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:13px; }}
</style></head><body><main>
<h1>{logo}</h1>
<form action="https://duckduckgo.com/" method="get"><input name="q" placeholder="Search the web" autocomplete="off"></form>
{sites}
</main></body></html>"#,
        logo = LOGO
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn titles_and_urls_are_escaped() {
        let page = super::html(&[("https://x.test/?a=\"1\"&b".into(), "<script>".into())]);
        assert!(page.contains("&lt;script&gt;"));
        assert!(page.contains("href=\"https://x.test/?a=&quot;1&quot;&amp;b\""));
        assert!(!page.contains("<script>"));
    }
}

//! Turns whatever was typed in the address bar into a URL to load.

const SEARCH_URL: &str = "https://duckduckgo.com/?q=";

/// Schemes the address bar will load directly. Anything else that merely
/// looks like a scheme (`javascript:`, `foo:bar`) is sent to search.
const SCHEMES: &[&str] = &["http", "https", "file", "about", "data", "blob", "view-source"];

/// Resolve address-bar input. Returns `None` for blank input.
pub fn resolve(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if let Some((scheme, _)) = input.split_once(':')
        && SCHEMES.contains(&scheme.to_ascii_lowercase().as_str())
    {
        return Some(input.to_string());
    }
    if looks_like_host(input) {
        let scheme = if is_local(host_of(input)) { "http" } else { "https" };
        return Some(format!("{scheme}://{input}"));
    }
    Some(format!("{SEARCH_URL}{}", encode_query(input)))
}

fn host_of(input: &str) -> &str {
    let authority = input.split(['/', '?', '#']).next().unwrap_or(input);
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    authority.rsplit_once(':').map_or(authority, |(h, _)| h)
}

fn is_local(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.contains(':')
        || (host.split('.').count() == 4 && host.split('.').all(|p| p.parse::<u8>().is_ok()))
}

fn looks_like_host(input: &str) -> bool {
    if input.contains(char::is_whitespace) {
        return false;
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or(input);
    if authority.starts_with('[') {
        return authority.contains(']');
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if let Some(p) = port
        && (p.is_empty() || p.parse::<u16>().is_err())
    {
        return false;
    }
    if host.is_empty() || !host.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '.') {
        return false;
    }
    if host.eq_ignore_ascii_case("localhost") || port.is_some() {
        return true;
    }
    // A dot with a plausible TLD: "example.com" yes, "3.14" or "foo." no.
    match host.rsplit_once('.') {
        Some((name, tld)) => !name.is_empty() && (is_local(host) || (tld.len() >= 2 && tld.chars().all(char::is_alphabetic))),
        None => false,
    }
}

fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resolve;

    fn r(s: &str) -> String {
        resolve(s).unwrap()
    }

    #[test]
    fn blank_is_none() {
        assert_eq!(resolve("   "), None);
    }

    #[test]
    fn urls_pass_through() {
        assert_eq!(r("https://example.com/a b"), "https://example.com/a b");
        assert_eq!(r("about:blank"), "about:blank");
        assert_eq!(r("FILE:///tmp/x.html"), "FILE:///tmp/x.html");
    }

    #[test]
    fn bare_hosts_get_a_scheme() {
        assert_eq!(r("example.com"), "https://example.com");
        assert_eq!(r(" webkit.org/blog?x=1 "), "https://webkit.org/blog?x=1");
        assert_eq!(r("localhost:8080/x"), "http://localhost:8080/x");
        assert_eq!(r("127.0.0.1"), "http://127.0.0.1");
        assert_eq!(r("[::1]:3000"), "http://[::1]:3000");
        assert_eq!(r("intranet:8080"), "https://intranet:8080");
    }

    #[test]
    fn everything_else_is_a_search() {
        assert_eq!(r("rust webkit"), "https://duckduckgo.com/?q=rust+webkit");
        assert_eq!(r("3.14"), "https://duckduckgo.com/?q=3.14");
        assert_eq!(r("what is c++?"), "https://duckduckgo.com/?q=what+is+c%2B%2B%3F");
        assert_eq!(r("javascript:alert(1)"), "https://duckduckgo.com/?q=javascript%3Aalert%281%29");
        assert_eq!(r("define:word"), "https://duckduckgo.com/?q=define%3Aword");
    }
}

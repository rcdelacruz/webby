//! On-disk history and session state. All I/O is best effort: a browser
//! that cannot write its profile should still browse.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const HISTORY_READ_LIMIT: usize = 5000;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Visit {
    url: String,
    title: String,
    ts: u64,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Session {
    pub tabs: Vec<String>,
    pub active: usize,
}

pub struct Store {
    dir: Option<PathBuf>,
    visits: Vec<Visit>,
}

impl Store {
    /// A store persisting under `dir`. `None` keeps everything in memory.
    pub fn open(dir: Option<PathBuf>) -> Self {
        let mut visits = Vec::new();
        if let Some(dir) = &dir {
            let _ = fs::create_dir_all(dir);
            if let Ok(text) = fs::read_to_string(dir.join("history.jsonl")) {
                let lines: Vec<&str> = text.lines().collect();
                let skip = lines.len().saturating_sub(HISTORY_READ_LIMIT);
                visits = lines[skip..].iter().filter_map(|l| serde_json::from_str(l).ok()).collect();
            }
        }
        Store { dir, visits }
    }

    pub fn record_visit(&mut self, url: &str, title: &str) {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let visit = Visit { url: url.to_string(), title: title.to_string(), ts };
        if let (Some(dir), Ok(line)) = (&self.dir, serde_json::to_string(&visit))
            && let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dir.join("history.jsonl"))
        {
            let _ = writeln!(f, "{line}");
        }
        self.visits.push(visit);
    }

    /// Most-visited pages as `(url, title)`, most frequent first.
    pub fn top_sites(&self, limit: usize) -> Vec<(String, String)> {
        let mut counts: HashMap<&str, (usize, usize)> = HashMap::new();
        for (i, v) in self.visits.iter().enumerate() {
            let e = counts.entry(&v.url).or_insert((0, i));
            e.0 += 1;
            e.1 = i; // latest visit carries the freshest title
        }
        let mut ranked: Vec<_> = counts.into_iter().collect();
        ranked.sort_by_key(|site| std::cmp::Reverse(site.1));
        ranked.into_iter().take(limit).map(|(url, (_, i))| (url.to_string(), self.visits[i].title.clone())).collect()
    }

    pub fn save_session(&self, session: &Session) {
        if let (Some(dir), Ok(json)) = (&self.dir, serde_json::to_string(session)) {
            let _ = fs::write(dir.join("session.json"), json);
        }
    }

    pub fn load_session(&self) -> Option<Session> {
        let text = fs::read_to_string(self.dir.as_ref()?.join("session.json")).ok()?;
        serde_json::from_str(&text).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_sites_rank_by_frequency_then_recency() {
        let mut s = Store::open(None);
        s.record_visit("https://a.test/", "A old");
        s.record_visit("https://b.test/", "B");
        s.record_visit("https://a.test/", "A new");
        s.record_visit("https://c.test/", "C");
        let top = s.top_sites(2);
        assert_eq!(top[0], ("https://a.test/".into(), "A new".into()));
        assert_eq!(top[1].0, "https://c.test/");
    }

    #[test]
    fn history_and_session_round_trip() {
        let dir = std::env::temp_dir().join(format!("webby-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut s = Store::open(Some(dir.clone()));
        s.record_visit("https://a.test/", "A");
        let session = Session { tabs: vec!["https://a.test/".into(), String::new()], active: 1 };
        s.save_session(&session);
        let s2 = Store::open(Some(dir.clone()));
        assert_eq!(s2.top_sites(5).len(), 1);
        assert_eq!(s2.load_session(), Some(session));
        let _ = fs::remove_dir_all(&dir);
    }
}

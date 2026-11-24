//! The tab model and the rules that drive it.

use crate::runtime::Event;
use crate::store::{Session, Store};
use crate::{Shell, TabId, omnibox, startpage};
use serde::{Deserialize, Serialize};

/// A snapshot of a content web view, sent by the platform on any change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    /// Load progress in `0.0..=1.0`.
    pub progress: f64,
    pub can_back: bool,
    pub can_forward: bool,
}

/// Commands the chrome UI posts as JSON, e.g. `{"cmd":"select_tab","id":3}`.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum UiCommand {
    Ready,
    Navigate {
        input: String,
    },
    NewTab,
    CloseTab {
        id: TabId,
    },
    SelectTab {
        id: TabId,
    },
    Back,
    Forward,
    Reload,
    Stop,
    /// The user left the address bar without navigating.
    FocusPage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shortcut {
    NewTab,
    CloseTab,
    FocusAddress,
    Reload,
    Back,
    Forward,
    NextTab,
    PrevTab,
}

struct Tab {
    id: TabId,
    view: ViewState,
    /// Presenting the built-in start page rather than a web document.
    start_page: bool,
    /// The view's current document is the start page. Differs from
    /// `start_page` while a navigation away from it is in flight.
    start_doc: bool,
    /// Last URL written to history, to avoid duplicates from reloads.
    recorded: String,
    /// A finished load is waiting for its title before being recorded.
    unrecorded: bool,
}

impl Tab {
    fn display_url(&self) -> &str {
        if self.start_page { "" } else { &self.view.url }
    }
}

#[derive(Serialize)]
struct TabJson<'a> {
    id: TabId,
    title: &'a str,
    url: &'a str,
    loading: bool,
    progress: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChromeJson<'a> {
    tabs: Vec<TabJson<'a>>,
    active: Option<TabId>,
    can_back: bool,
    can_forward: bool,
}

pub struct Browser {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    next_id: TabId,
    store: Store,
    started: bool,
    pushed: String,
}

impl Browser {
    pub fn new(store: Store) -> Self {
        Browser { tabs: Vec::new(), active: None, next_id: 1, store, started: false, pushed: String::new() }
    }

    pub fn handle(&mut self, shell: &mut dyn Shell, event: Event) {
        match event {
            Event::Ui(json) => match serde_json::from_str(&json) {
                Ok(cmd) => self.on_ui(shell, cmd),
                Err(e) => eprintln!("webby: ignoring malformed UI message: {e}"),
            },
            Event::View(id, state) => self.on_view(id, state),
            Event::OpenTab(url) => {
                if let Some(url) = omnibox::resolve(&url) {
                    self.open_tab(shell, Some(&url));
                }
            }
            Event::Shortcut(s) => self.on_shortcut(shell, s),
            Event::Closing => {}
        }
        self.save_session();
        self.push_chrome(shell);
    }

    fn on_ui(&mut self, shell: &mut dyn Shell, cmd: UiCommand) {
        match cmd {
            UiCommand::Ready => {
                // The chrome document (re)loaded: it needs the full state.
                self.pushed.clear();
                if !self.started {
                    self.started = true;
                    self.restore(shell);
                }
            }
            UiCommand::Navigate { input } => {
                if let (Some(id), Some(url)) = (self.active, omnibox::resolve(&input)) {
                    self.load(shell, id, &url);
                    shell.focus_view(id);
                }
            }
            UiCommand::NewTab => self.open_tab(shell, None),
            UiCommand::CloseTab { id } => self.close_tab(shell, id),
            UiCommand::SelectTab { id } => self.select(shell, id),
            UiCommand::Back => self.with_active(|id| shell.go_back(id)),
            UiCommand::Forward => self.with_active(|id| shell.go_forward(id)),
            UiCommand::Reload => self.reload(shell),
            UiCommand::Stop => self.with_active(|id| shell.stop(id)),
            UiCommand::FocusPage => self.with_active(|id| shell.focus_view(id)),
        }
    }

    fn on_shortcut(&mut self, shell: &mut dyn Shell, shortcut: Shortcut) {
        match shortcut {
            Shortcut::NewTab => self.open_tab(shell, None),
            Shortcut::CloseTab => {
                if let Some(id) = self.active {
                    self.close_tab(shell, id);
                }
            }
            Shortcut::FocusAddress => focus_address(shell),
            Shortcut::Reload => self.reload(shell),
            Shortcut::Back => self.with_active(|id| shell.go_back(id)),
            Shortcut::Forward => self.with_active(|id| shell.go_forward(id)),
            Shortcut::NextTab => self.cycle(shell, 1),
            Shortcut::PrevTab => self.cycle(shell, -1),
        }
    }

    fn on_view(&mut self, id: TabId, state: ViewState) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else { return };
        let is_web = |u: &str| u.starts_with("http://") || u.starts_with("https://");
        // The start page lives at about:blank. Leaving that URL means it
        // was navigated away from; settling back on it means the
        // navigation never happened and the start page is still shown.
        if !state.url.is_empty() && state.url != "about:blank" {
            tab.start_page = false;
            tab.start_doc = false;
        } else if tab.start_doc && !state.loading {
            tab.start_page = true;
        }
        // A finished load is recorded once its title is known; titles can
        // arrive just after the load ends. If the tab moves on first, the
        // visit is recorded with whatever title it had.
        if tab.unrecorded && state.url != tab.view.url {
            tab.unrecorded = false;
            self.store.record_visit(&tab.view.url, &tab.view.title);
        }
        let finished = tab.view.loading && !state.loading;
        tab.view = state;
        if finished && !tab.start_page && is_web(&tab.view.url) && tab.view.url != tab.recorded {
            tab.recorded = tab.view.url.clone();
            tab.unrecorded = true;
        }
        if tab.unrecorded && !tab.view.loading && !tab.view.title.is_empty() {
            tab.unrecorded = false;
            self.store.record_visit(&tab.view.url, &tab.view.title);
        }
    }

    fn with_active(&self, f: impl FnOnce(TabId)) {
        if let Some(id) = self.active {
            f(id);
        }
    }

    fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn restore(&mut self, shell: &mut dyn Shell) {
        let session = self.store.load_session().unwrap_or_default();
        for url in &session.tabs {
            self.open_tab(shell, Some(url.as_str()).filter(|u| !u.is_empty()));
        }
        match self.tabs.get(session.active).map(|t| t.id) {
            Some(id) => self.select(shell, id),
            None if self.tabs.is_empty() => self.open_tab(shell, None),
            None => {}
        }
    }

    /// Open a tab next to the active one and select it. `None` shows the
    /// start page.
    fn open_tab(&mut self, shell: &mut dyn Shell, url: Option<&str>) {
        let id = self.next_id;
        self.next_id += 1;
        let at = self.active.and_then(|a| self.index_of(a)).map_or(self.tabs.len(), |i| i + 1);
        self.tabs.insert(
            at,
            Tab { id, view: ViewState::default(), start_page: false, start_doc: false, recorded: String::new(), unrecorded: false },
        );
        shell.create_view(id);
        match url {
            Some(url) => self.load(shell, id, url),
            None => {
                self.tabs[at].start_page = true;
                self.tabs[at].start_doc = true;
                shell.load_html(id, &startpage::html(&self.store.top_sites(8)));
            }
        }
        self.select(shell, id);
    }

    fn load(&mut self, shell: &mut dyn Shell, id: TabId, url: &str) {
        if let Some(i) = self.index_of(id) {
            self.tabs[i].start_page = false;
            // Show the destination immediately; the view confirms it later.
            self.tabs[i].view.url = url.to_string();
            shell.navigate(id, url);
        }
    }

    fn reload(&mut self, shell: &mut dyn Shell) {
        let Some(i) = self.active.and_then(|a| self.index_of(a)) else { return };
        let id = self.tabs[i].id;
        if self.tabs[i].start_page {
            shell.load_html(id, &startpage::html(&self.store.top_sites(8)));
        } else {
            shell.reload(id);
        }
    }

    /// Show a tab and give it keyboard focus. A start page has nothing to
    /// type into, so focus goes to the address bar instead.
    fn select(&mut self, shell: &mut dyn Shell, id: TabId) {
        let Some(i) = self.index_of(id) else { return };
        if self.active == Some(id) {
            return;
        }
        self.active = Some(id);
        shell.show_view(id);
        if self.tabs[i].start_page {
            focus_address(shell);
        } else {
            shell.focus_view(id);
        }
    }

    fn cycle(&mut self, shell: &mut dyn Shell, step: isize) {
        let Some(i) = self.active.and_then(|a| self.index_of(a)) else { return };
        let n = self.tabs.len() as isize;
        let next = self.tabs[((i as isize + step).rem_euclid(n)) as usize].id;
        self.select(shell, next);
    }

    fn close_tab(&mut self, shell: &mut dyn Shell, id: TabId) {
        let Some(i) = self.index_of(id) else { return };
        if self.tabs.len() == 1 {
            // Keep the last tab in the session so it reopens next launch.
            self.save_session();
            shell.close_window();
            return;
        }
        self.tabs.remove(i);
        if self.active == Some(id) {
            self.active = None;
            let neighbour = self.tabs[i.min(self.tabs.len() - 1)].id;
            self.select(shell, neighbour);
        }
        shell.destroy_view(id);
    }

    fn save_session(&self) {
        if !self.started {
            return;
        }
        // Blank tabs are saved as empty and reopen on the start page.
        let tabs = self.tabs.iter().map(|t| t.display_url()).map(|u| if u == "about:blank" { "" } else { u }.to_string()).collect();
        let active = self.active.and_then(|a| self.index_of(a)).unwrap_or(0);
        self.store.save_session(&Session { tabs, active });
    }

    /// Send the chrome UI its state, if it changed since the last push.
    fn push_chrome(&mut self, shell: &mut dyn Shell) {
        if !self.started {
            return;
        }
        let active = self.active.and_then(|a| self.index_of(a)).map(|i| &self.tabs[i]);
        let state = ChromeJson {
            tabs: self
                .tabs
                .iter()
                .map(|t| TabJson {
                    id: t.id,
                    title: if t.start_page { "New Tab" } else { &t.view.title },
                    url: t.display_url(),
                    loading: t.view.loading,
                    progress: t.view.progress,
                })
                .collect(),
            active: self.active,
            can_back: active.is_some_and(|t| t.view.can_back),
            can_forward: active.is_some_and(|t| t.view.can_forward),
        };
        let Ok(json) = serde_json::to_string(&state) else { return };
        if json == self.pushed {
            return;
        }
        let title = active.map(|t| if t.start_page { "New Tab" } else { t.view.title.as_str() }).unwrap_or("");
        shell.set_window_title(&if title.is_empty() { "Webby".to_string() } else { format!("{title} — Webby") });
        shell.eval_chrome(&format!("webby.update({json})"));
        self.pushed = json;
    }
}

fn focus_address(shell: &mut dyn Shell) {
    shell.focus_chrome();
    shell.eval_chrome("webby.focusAddress()");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log(Vec<String>);

    impl Shell for Log {
        fn create_view(&mut self, id: TabId) {
            self.0.push(format!("create {id}"));
        }
        fn destroy_view(&mut self, id: TabId) {
            self.0.push(format!("destroy {id}"));
        }
        fn show_view(&mut self, id: TabId) {
            self.0.push(format!("show {id}"));
        }
        fn navigate(&mut self, id: TabId, url: &str) {
            self.0.push(format!("navigate {id} {url}"));
        }
        fn load_html(&mut self, id: TabId, _: &str) {
            self.0.push(format!("html {id}"));
        }
        fn go_back(&mut self, id: TabId) {
            self.0.push(format!("back {id}"));
        }
        fn go_forward(&mut self, id: TabId) {
            self.0.push(format!("forward {id}"));
        }
        fn reload(&mut self, id: TabId) {
            self.0.push(format!("reload {id}"));
        }
        fn stop(&mut self, id: TabId) {
            self.0.push(format!("stop {id}"));
        }
        fn eval_chrome(&mut self, script: &str) {
            self.0.push(format!("eval {script}"));
        }
        fn focus_chrome(&mut self) {
            self.0.push("focus chrome".into());
        }
        fn focus_view(&mut self, id: TabId) {
            self.0.push(format!("focus {id}"));
        }
        fn set_window_title(&mut self, title: &str) {
            self.0.push(format!("title {title}"));
        }
        fn close_window(&mut self) {
            self.0.push("close window".into());
        }
    }

    fn ui(b: &mut Browser, s: &mut Log, json: &str) {
        b.handle(s, Event::Ui(json.into()));
    }

    fn started() -> (Browser, Log) {
        let (mut b, mut s) = (Browser::new(Store::open(None)), Log::default());
        ui(&mut b, &mut s, r#"{"cmd":"ready"}"#);
        (b, s)
    }

    fn has(s: &Log, needle: &str) -> bool {
        s.0.iter().any(|l| l == needle)
    }

    #[test]
    fn first_ready_opens_a_start_page_tab() {
        let (_, s) = started();
        assert_eq!(&s.0[..3], ["create 1", "html 1", "show 1"]);
        assert!(has(&s, "focus chrome"));
        assert!(has(&s, "title New Tab — Webby"));
        assert!(s.0.last().unwrap().starts_with(r#"eval webby.update({"tabs":[{"id":1,"title":"New Tab","url":"","#));
    }

    #[test]
    fn navigate_resolves_input_and_focuses_the_page() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"navigate","input":"example.com"}"#);
        assert!(has(&s, "navigate 1 https://example.com"));
        assert!(has(&s, "focus 1"));
        assert!(s.0.last().unwrap().contains(r#""url":"https://example.com""#));
    }

    #[test]
    fn closing_the_active_tab_selects_its_neighbour() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"new_tab"}"#);
        ui(&mut b, &mut s, r#"{"cmd":"new_tab"}"#);
        ui(&mut b, &mut s, r#"{"cmd":"select_tab","id":2}"#);
        s.0.clear();
        ui(&mut b, &mut s, r#"{"cmd":"close_tab","id":2}"#);
        assert_eq!(s.0[0], "show 3");
        assert!(has(&s, "destroy 2"));
        assert_eq!(b.active, Some(3));
    }

    #[test]
    fn closing_the_last_tab_closes_the_window() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"close_tab","id":1}"#);
        assert!(has(&s, "close window"));
        assert!(!has(&s, "destroy 1"));
    }

    #[test]
    fn pages_open_new_windows_as_adjacent_tabs() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"new_tab"}"#);
        ui(&mut b, &mut s, r#"{"cmd":"select_tab","id":1}"#);
        b.handle(&mut s, Event::OpenTab("https://popup.test/".into()));
        let order: Vec<TabId> = b.tabs.iter().map(|t| t.id).collect();
        assert_eq!(order, [1, 3, 2]);
        assert_eq!(b.active, Some(3));
    }

    #[test]
    fn finished_loads_are_recorded_once() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"navigate","input":"https://a.test/"}"#);
        let mut v = ViewState { url: "https://a.test/".into(), title: "A".into(), loading: true, ..Default::default() };
        for _ in 0..2 {
            v.loading = true;
            b.handle(&mut s, Event::View(1, v.clone()));
            v.loading = false;
            b.handle(&mut s, Event::View(1, v.clone()));
        }
        assert_eq!(b.store.top_sites(9), [("https://a.test/".to_string(), "A".to_string())]);
    }

    #[test]
    fn late_titles_still_reach_history() {
        let (mut b, mut s) = started();
        let mut v = ViewState { url: "https://a.test/".into(), loading: true, ..Default::default() };
        b.handle(&mut s, Event::View(1, v.clone()));
        v.loading = false;
        b.handle(&mut s, Event::View(1, v.clone()));
        assert!(b.store.top_sites(9).is_empty());
        v.title = "Late".into();
        b.handle(&mut s, Event::View(1, v.clone()));
        assert_eq!(b.store.top_sites(9)[0].1, "Late");
        // An untitled page is recorded when the tab navigates away.
        v = ViewState { url: "https://b.test/".into(), loading: true, ..Default::default() };
        b.handle(&mut s, Event::View(1, v.clone()));
        v.loading = false;
        b.handle(&mut s, Event::View(1, v.clone()));
        v.url = "https://c.test/".into();
        b.handle(&mut s, Event::View(1, v));
        assert_eq!(b.store.top_sites(9).len(), 2);
    }

    #[test]
    fn an_abandoned_navigation_falls_back_to_the_start_page() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"navigate","input":"nowhere.invalid"}"#);
        assert!(s.0.last().unwrap().contains(r#""url":"https://nowhere.invalid""#));
        b.handle(&mut s, Event::View(1, ViewState { url: "about:blank".into(), ..Default::default() }));
        assert!(s.0.last().unwrap().contains(r#""title":"New Tab","url":"","#));
    }

    #[test]
    fn unchanged_state_is_not_pushed_again() {
        let (mut b, mut s) = started();
        s.0.clear();
        b.handle(&mut s, Event::View(1, ViewState { url: "about:blank".into(), ..Default::default() }));
        assert!(s.0.is_empty(), "{:?}", s.0);
    }

    #[test]
    fn shortcuts_cycle_tabs_and_malformed_messages_are_ignored() {
        let (mut b, mut s) = started();
        ui(&mut b, &mut s, r#"{"cmd":"new_tab"}"#);
        b.handle(&mut s, Event::Shortcut(Shortcut::NextTab));
        assert_eq!(b.active, Some(1));
        b.handle(&mut s, Event::Shortcut(Shortcut::PrevTab));
        assert_eq!(b.active, Some(2));
        ui(&mut b, &mut s, "not json");
        ui(&mut b, &mut s, r#"{"cmd":"format_disk"}"#);
        assert_eq!(b.tabs.len(), 2);
    }
}

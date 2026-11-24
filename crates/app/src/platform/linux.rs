//! Linux: GTK 4 window, WebKitGTK 6.0 web views.

use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use webby_core::{Browser, CHROME_HEIGHT, CHROME_HTML, Event, MESSAGE_HANDLER, Runtime, Shell, Shortcut, TabId, ViewState};
use webkit6::prelude::*;
use webkit6::{CookiePersistentStorage, NetworkSession, Settings, UserContentManager, WebView};

type Rt = Runtime<GtkShell>;

const SHORTCUTS: &[(&str, Shortcut)] = &[
    ("<Control>t", Shortcut::NewTab),
    ("<Control>w", Shortcut::CloseTab),
    ("<Control>l", Shortcut::FocusAddress),
    ("<Control>r", Shortcut::Reload),
    ("F5", Shortcut::Reload),
    ("<Alt>Left", Shortcut::Back),
    ("<Alt>Right", Shortcut::Forward),
    ("<Control>Tab", Shortcut::NextTab),
    ("<Control>Page_Down", Shortcut::NextTab),
    ("<Control><Shift>Tab", Shortcut::PrevTab),
    ("<Control><Shift>ISO_Left_Tab", Shortcut::PrevTab),
    ("<Control>Page_Up", Shortcut::PrevTab),
];

struct GtkShell {
    rt: Weak<Rt>,
    window: gtk::ApplicationWindow,
    chrome: WebView,
    stack: gtk::Stack,
    views: HashMap<TabId, WebView>,
    session: Option<NetworkSession>,
    settings: Settings,
}

pub fn run(browser: Browser, profile: Option<PathBuf>) {
    let app = gtk::Application::new(Some("co.coderstudio.Webby"), gio::ApplicationFlags::NON_UNIQUE);
    let pending = RefCell::new(Some(browser));
    app.connect_activate(move |app| {
        if let Some(browser) = pending.take() {
            build(app, browser, profile.as_ref());
        }
    });
    app.run_with_args::<&str>(&[]);
}

fn build(app: &gtk::Application, browser: Browser, profile: Option<&PathBuf>) {
    let window = gtk::ApplicationWindow::builder().application(app).title("Webby").default_width(1200).default_height(800).build();

    let messages = UserContentManager::new();
    messages.register_script_message_handler(MESSAGE_HANDLER, None);
    let chrome = WebView::builder().user_content_manager(&messages).build();
    chrome.set_size_request(-1, CHROME_HEIGHT);

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&chrome);
    layout.append(&stack);
    window.set_child(Some(&layout));

    // One persistent network session shared by every tab.
    let session = profile.map(|dir| {
        let s = NetworkSession::new(dir.join("data").to_str(), dir.join("cache").to_str());
        if let (Some(cookies), Some(path)) = (s.cookie_manager(), dir.join("cookies.sqlite").to_str()) {
            cookies.set_persistent_storage(path, CookiePersistentStorage::Sqlite);
        }
        s
    });
    let settings = Settings::new();
    settings.set_enable_developer_extras(true);

    let rt = Rc::new_cyclic(|rt| {
        let shell =
            GtkShell { rt: rt.clone(), window: window.clone(), chrome: chrome.clone(), stack, views: HashMap::new(), session, settings };
        Runtime::new(browser, shell)
    });

    let r = rt.clone();
    messages.connect_script_message_received(Some(MESSAGE_HANDLER), move |_, value| r.post(Event::Ui(value.to_str().into())));

    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
    for &(keys, shortcut) in SHORTCUTS {
        let r = rt.clone();
        let action = gtk::CallbackAction::new(move |_, _| {
            r.post(Event::Shortcut(shortcut));
            glib::Propagation::Stop
        });
        shortcuts.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(keys), Some(action)));
    }
    window.add_controller(shortcuts);

    // The runtime (and with it every web view) lives as long as the window.
    window.connect_close_request(move |_| {
        rt.post(Event::Closing);
        glib::Propagation::Proceed
    });

    chrome.load_html(CHROME_HTML, None);
    window.present();
}

fn snapshot(view: &WebView) -> ViewState {
    ViewState {
        url: view.uri().map(Into::into).unwrap_or_default(),
        title: view.title().map(Into::into).unwrap_or_default(),
        loading: view.is_loading(),
        progress: view.estimated_load_progress(),
        can_back: view.can_go_back(),
        can_forward: view.can_go_forward(),
    }
}

impl GtkShell {
    fn view(&self, id: TabId) -> Option<&WebView> {
        self.views.get(&id)
    }
}

impl Shell for GtkShell {
    fn create_view(&mut self, id: TabId) {
        let mut builder = WebView::builder().settings(&self.settings);
        if let Some(session) = &self.session {
            builder = builder.network_session(session);
        }
        let view = builder.build();

        let changed = {
            let rt = self.rt.clone();
            move |view: &WebView| {
                if let Some(rt) = rt.upgrade() {
                    rt.post(Event::View(id, snapshot(view)));
                }
            }
        };
        view.connect_uri_notify(changed.clone());
        view.connect_title_notify(changed.clone());
        view.connect_is_loading_notify(changed.clone());
        view.connect_estimated_load_progress_notify(changed.clone());
        // Back/forward availability settles as the load progresses.
        view.connect_load_changed(move |view, _| changed(view));

        // window.open() and target=_blank become tabs.
        let rt = self.rt.clone();
        view.connect_create(move |_, action| {
            let uri = action.clone().request().and_then(|r| r.uri());
            if let (Some(rt), Some(uri)) = (rt.upgrade(), uri) {
                rt.post(Event::OpenTab(uri.into()));
            }
            None
        });

        self.stack.add_child(&view);
        self.views.insert(id, view);
    }

    fn destroy_view(&mut self, id: TabId) {
        if let Some(view) = self.views.remove(&id) {
            self.stack.remove(&view);
        }
    }

    fn show_view(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            self.stack.set_visible_child(view);
        }
    }

    fn navigate(&mut self, id: TabId, url: &str) {
        self.view(id).inspect(|v| v.load_uri(url));
    }

    fn load_html(&mut self, id: TabId, html: &str) {
        self.view(id).inspect(|v| v.load_html(html, None));
    }

    fn go_back(&mut self, id: TabId) {
        self.view(id).inspect(|v| v.go_back());
    }

    fn go_forward(&mut self, id: TabId) {
        self.view(id).inspect(|v| v.go_forward());
    }

    fn reload(&mut self, id: TabId) {
        self.view(id).inspect(|v| v.reload());
    }

    fn stop(&mut self, id: TabId) {
        self.view(id).inspect(|v| v.stop_loading());
    }

    fn eval_chrome(&mut self, script: &str) {
        self.chrome.evaluate_javascript(script, None, None, gio::Cancellable::NONE, |_| {});
    }

    fn focus_chrome(&mut self) {
        self.chrome.grab_focus();
    }

    fn focus_view(&mut self, id: TabId) {
        self.view(id).inspect(|v| {
            v.grab_focus();
        });
    }

    fn set_window_title(&mut self, title: &str) {
        self.window.set_title(Some(title));
    }

    fn close_window(&mut self) {
        // Closing tears the shell down, so do it once this call unwinds.
        let window = self.window.clone();
        glib::idle_add_local_once(move || window.close());
    }
}

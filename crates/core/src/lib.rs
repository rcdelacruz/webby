//! Platform-independent browser logic for Webby.
//!
//! The core owns the tab model, address-bar parsing, history and session
//! persistence. It talks to the outside world only through the [`Shell`]
//! trait, which each platform implements on top of its native WebKit port.

mod browser;
pub mod omnibox;
mod runtime;
mod startpage;
mod store;

pub use browser::{Browser, Shortcut, UiCommand, ViewState};
pub use runtime::{Event, Runtime};
pub use store::Store;

/// Identifies one tab and the native web view backing it.
pub type TabId = u32;

/// Height of the browser chrome (tab strip + toolbar) in logical pixels.
pub const CHROME_HEIGHT: i32 = 84;

/// Name of the script message handler the chrome UI posts commands to
/// (`window.webkit.messageHandlers.webby.postMessage(json)`).
pub const MESSAGE_HANDLER: &str = "webby";

/// The chrome UI document, shared verbatim by every platform.
pub const CHROME_HTML: &str = include_str!("../ui/chrome.html");

/// What the core needs from a platform: a window holding one chrome web
/// view and any number of content web views, of which one is visible.
pub trait Shell {
    fn create_view(&mut self, id: TabId);
    fn destroy_view(&mut self, id: TabId);
    /// Make `id` the visible content view.
    fn show_view(&mut self, id: TabId);
    fn navigate(&mut self, id: TabId, url: &str);
    /// Render a document the core generated (the start page).
    fn load_html(&mut self, id: TabId, html: &str);
    fn go_back(&mut self, id: TabId);
    fn go_forward(&mut self, id: TabId);
    fn reload(&mut self, id: TabId);
    fn stop(&mut self, id: TabId);
    /// Run a script in the chrome web view.
    fn eval_chrome(&mut self, script: &str);
    fn focus_chrome(&mut self);
    fn focus_view(&mut self, id: TabId);
    fn set_window_title(&mut self, title: &str);
    fn close_window(&mut self);
}

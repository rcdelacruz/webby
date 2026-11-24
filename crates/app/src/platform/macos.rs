//! macOS: AppKit window, system WKWebView (the same WebKit Safari uses).

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSAutoresizingMaskOptions, NSBackingStoreType,
    NSEventModifierFlags, NSMenu, NSMenuItem, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSDictionary, NSKeyValueChangeKey, NSKeyValueObservingOptions, NSNotification, NSObject, NSObjectNSKeyValueObserverRegistration,
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest, ns_string,
};
use objc2_web_kit::{
    WKNavigation, WKNavigationAction, WKNavigationDelegate, WKScriptMessage, WKScriptMessageHandler, WKUIDelegate, WKUserContentController,
    WKWebView, WKWebViewConfiguration, WKWindowFeatures,
};
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;
use webby_core::{Browser, CHROME_HEIGHT, CHROME_HTML, Event, MESSAGE_HANDLER, Runtime, Shell, Shortcut, TabId, ViewState};

type Rt = Runtime<MacShell>;
type Views = Rc<RefCell<HashMap<TabId, Retained<WKWebView>>>>;

/// Properties whose changes are forwarded to the core, via KVO.
const OBSERVED: &[&str] = &["URL", "title", "loading", "estimatedProgress", "canGoBack", "canGoForward"];

/// Menu items carry an index into this table as their tag.
const SHORTCUTS: &[Shortcut] = &[
    Shortcut::NewTab,
    Shortcut::CloseTab,
    Shortcut::FocusAddress,
    Shortcut::Reload,
    Shortcut::Back,
    Shortcut::Forward,
    Shortcut::NextTab,
    Shortcut::PrevTab,
];

#[derive(Default)]
struct Ivars {
    rt: OnceCell<Rc<Rt>>,
    views: Views,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Controller does
    // not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct Controller;

    unsafe impl NSObjectProtocol for Controller {}

    unsafe impl NSApplicationDelegate for Controller {
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window(&self, _app: &NSApplication) -> bool {
            true
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.post(Event::Closing);
        }
    }

    unsafe impl WKScriptMessageHandler for Controller {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive_message(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            if let Ok(body) = unsafe { message.body() }.downcast::<NSString>() {
                self.post(Event::Ui(body.to_string()));
            }
        }
    }

    unsafe impl WKNavigationDelegate for Controller {
        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish(&self, view: &WKWebView, _navigation: Option<&WKNavigation>) {
            self.refresh(view);
        }
    }

    unsafe impl WKUIDelegate for Controller {
        // window.open() and target=_blank become tabs.
        #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        fn create_web_view(
            &self,
            _view: &WKWebView,
            _configuration: &WKWebViewConfiguration,
            action: &WKNavigationAction,
            _features: &WKWindowFeatures,
        ) -> Option<Retained<WKWebView>> {
            let url = unsafe { action.request().URL() }.and_then(|u| u.absoluteString());
            if let Some(url) = url {
                self.post(Event::OpenTab(url.to_string()));
            }
            None
        }
    }

    impl Controller {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            _key_path: Option<&NSString>,
            object: Option<&AnyObject>,
            _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
            _context: *mut c_void,
        ) {
            if let Some(view) = object.and_then(|o| o.downcast_ref::<WKWebView>()) {
                self.refresh(view);
            }
        }

        #[unsafe(method(webbyShortcut:))]
        fn shortcut(&self, sender: &NSMenuItem) {
            if let Some(&shortcut) = SHORTCUTS.get(sender.tag() as usize) {
                self.post(Event::Shortcut(shortcut));
            }
        }
    }
);

impl Controller {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        unsafe { msg_send![super(this), init] }
    }

    fn post(&self, event: Event) {
        if let Some(rt) = self.ivars().rt.get() {
            rt.post(event);
        }
    }

    /// Send the core a fresh snapshot of `view`, if it backs a tab.
    fn refresh(&self, view: &WKWebView) {
        let id = self.ivars().views.borrow().iter().find(|(_, v)| std::ptr::eq(&***v, view)).map(|(id, _)| *id);
        if let Some(id) = id {
            self.post(Event::View(id, snapshot(view)));
        }
    }
}

fn snapshot(view: &WKWebView) -> ViewState {
    unsafe {
        ViewState {
            url: view.URL().and_then(|u| u.absoluteString()).map(|s| s.to_string()).unwrap_or_default(),
            title: view.title().map(|s| s.to_string()).unwrap_or_default(),
            loading: view.isLoading(),
            progress: view.estimatedProgress(),
            can_back: view.canGoBack(),
            can_forward: view.canGoForward(),
        }
    }
}

struct MacShell {
    mtm: MainThreadMarker,
    controller: Retained<Controller>,
    window: Retained<NSWindow>,
    chrome: Retained<WKWebView>,
    views: Views,
}

impl MacShell {
    fn view(&self, id: TabId) -> Option<Retained<WKWebView>> {
        self.views.borrow().get(&id).cloned()
    }

    /// The area below the chrome, in the content view's coordinates
    /// (origin at the bottom left).
    fn content_frame(&self) -> NSRect {
        let size = self.window.contentView().map(|v| v.bounds().size).unwrap_or(NSSize::new(0.0, 0.0));
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size.width, (size.height - CHROME_HEIGHT as f64).max(0.0)))
    }
}

impl Shell for MacShell {
    fn create_view(&mut self, id: TabId) {
        let view = unsafe {
            let configuration = WKWebViewConfiguration::new(self.mtm);
            let view = WKWebView::initWithFrame_configuration(WKWebView::alloc(self.mtm), self.content_frame(), &configuration);
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*self.controller)));
            view.setUIDelegate(Some(ProtocolObject::from_ref(&*self.controller)));
            view.setAllowsBackForwardNavigationGestures(true);
            view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
            view.setHidden(true);
            for key in OBSERVED {
                view.addObserver_forKeyPath_options_context(
                    &self.controller,
                    &NSString::from_str(key),
                    NSKeyValueObservingOptions::New,
                    std::ptr::null_mut(),
                );
            }
            view
        };
        if let Some(content) = self.window.contentView() {
            content.addSubview(&view);
        }
        self.views.borrow_mut().insert(id, view);
    }

    fn destroy_view(&mut self, id: TabId) {
        let removed = self.views.borrow_mut().remove(&id);
        if let Some(view) = removed {
            unsafe {
                for key in OBSERVED {
                    view.removeObserver_forKeyPath(&self.controller, &NSString::from_str(key));
                }
                view.removeFromSuperview();
            }
        }
    }

    fn show_view(&mut self, id: TabId) {
        let frame = self.content_frame();
        for (other, view) in self.views.borrow().iter() {
            if *other == id {
                view.setFrame(frame);
            }
            view.setHidden(*other != id);
        }
    }

    fn navigate(&mut self, id: TabId, url: &str) {
        if let (Some(view), Some(url)) = (self.view(id), NSURL::URLWithString(&NSString::from_str(url))) {
            unsafe { view.loadRequest(&NSURLRequest::requestWithURL(&url)) };
        }
    }

    fn load_html(&mut self, id: TabId, html: &str) {
        if let Some(view) = self.view(id) {
            unsafe { view.loadHTMLString_baseURL(&NSString::from_str(html), None) };
        }
    }

    fn go_back(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            unsafe { view.goBack() };
        }
    }

    fn go_forward(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            unsafe { view.goForward() };
        }
    }

    fn reload(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            unsafe { view.reload() };
        }
    }

    fn stop(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            unsafe { view.stopLoading() };
        }
    }

    fn eval_chrome(&mut self, script: &str) {
        unsafe { self.chrome.evaluateJavaScript_completionHandler(&NSString::from_str(script), None) };
    }

    fn focus_chrome(&mut self) {
        self.window.makeFirstResponder(Some(&self.chrome));
    }

    fn focus_view(&mut self, id: TabId) {
        if let Some(view) = self.view(id) {
            self.window.makeFirstResponder(Some(&view));
        }
    }

    fn set_window_title(&mut self, title: &str) {
        self.window.setTitle(&NSString::from_str(title));
    }

    fn close_window(&mut self) {
        self.window.close();
    }
}

pub fn run(browser: Browser, _profile: Option<PathBuf>) {
    let mtm = MainThreadMarker::new().expect("webby must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let controller = Controller::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*controller)));
    app.setMainMenu(Some(&main_menu(mtm, &controller)));

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1200.0, 800.0)),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // The shell owns the window; AppKit must not free it on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(ns_string!("webby"));
    window.setContentMinSize(NSSize::new(480.0, 320.0));
    window.center();

    // The chrome sits along the top edge. Only it gets the message handler,
    // so web content can never send commands to the browser.
    let chrome = unsafe {
        let configuration = WKWebViewConfiguration::new(mtm);
        configuration
            .userContentController()
            .addScriptMessageHandler_name(ProtocolObject::from_ref(&*controller), &NSString::from_str(MESSAGE_HANDLER));
        let frame = NSRect::new(NSPoint::new(0.0, 800.0 - CHROME_HEIGHT as f64), NSSize::new(1200.0, CHROME_HEIGHT as f64));
        let chrome = WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &configuration);
        chrome.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin);
        chrome
    };
    if let Some(content) = window.contentView() {
        content.addSubview(&chrome);
    }

    let views = controller.ivars().views.clone();
    let shell = MacShell { mtm, controller: controller.clone(), window: window.clone(), chrome: chrome.clone(), views };
    let _ = controller.ivars().rt.set(Rc::new(Runtime::new(browser, shell)));

    unsafe { chrome.loadHTMLString_baseURL(&NSString::from_str(CHROME_HTML), None) };
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    app.run();
}

fn main_menu(mtm: MainThreadMarker, controller: &Controller) -> Retained<NSMenu> {
    const CMD: NSEventModifierFlags = NSEventModifierFlags::Command;
    let item = |title: &str, action: Option<Sel>, key: &str, modifiers: NSEventModifierFlags| {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        };
        item.setKeyEquivalentModifierMask(modifiers);
        item
    };
    // An item that posts SHORTCUTS[index] to the core.
    let shortcut = |title: &str, index: usize, key: &str, modifiers: NSEventModifierFlags| {
        let item = item(title, Some(sel!(webbyShortcut:)), key, modifiers);
        item.setTag(index as isize);
        unsafe { item.setTarget(Some(controller)) };
        item
    };
    // Standard actions travel the responder chain to the focused web view.
    let standard = |title: &str, action: Sel, key: &str| item(title, Some(action), key, CMD);

    let control_shift = NSEventModifierFlags::Control | NSEventModifierFlags::Shift;
    let menus: Vec<(&str, Vec<Retained<NSMenuItem>>)> = vec![
        ("webby", vec![standard("Hide webby", sel!(hide:), "h"), standard("Quit webby", sel!(terminate:), "q")]),
        ("File", vec![shortcut("New Tab", 0, "t", CMD), shortcut("Open Location…", 2, "l", CMD), shortcut("Close Tab", 1, "w", CMD)]),
        (
            "Edit",
            vec![
                standard("Undo", sel!(undo:), "z"),
                item("Redo", Some(sel!(redo:)), "z", CMD | NSEventModifierFlags::Shift),
                NSMenuItem::separatorItem(mtm),
                standard("Cut", sel!(cut:), "x"),
                standard("Copy", sel!(copy:), "c"),
                standard("Paste", sel!(paste:), "v"),
                standard("Select All", sel!(selectAll:), "a"),
            ],
        ),
        ("View", vec![shortcut("Reload Page", 3, "r", CMD)]),
        ("History", vec![shortcut("Back", 4, "[", CMD), shortcut("Forward", 5, "]", CMD)]),
        (
            "Window",
            vec![
                standard("Minimize", sel!(performMiniaturize:), "m"),
                shortcut("Show Next Tab", 6, "\t", NSEventModifierFlags::Control),
                shortcut("Show Previous Tab", 7, "\t", control_shift),
            ],
        ),
    ];

    let bar = NSMenu::new(mtm);
    for (title, items) in menus {
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
        for item in &items {
            menu.addItem(item);
        }
        let holder = NSMenuItem::new(mtm);
        holder.setSubmenu(Some(&menu));
        bar.addItem(&holder);
    }
    bar
}

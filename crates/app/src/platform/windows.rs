//! Windows: Win32 window, WebKit's own Windows port (`WebKit2.dll`) for
//! the web views. No WebView2, no Chromium.

// Nearly every line here is an FFI call; the unsafe functions below are
// only ever invoked by Windows or WebKit with the arguments they document.
#![allow(unsafe_op_in_unsafe_fn)]

#[path = "windows/wk.rs"]
mod wk;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use webby_core::{Browser, CHROME_HEIGHT, CHROME_HTML, Event, MESSAGE_HANDLER, Runtime, Shell, Shortcut, TabId, ViewState};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Ole::OleInitialize;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_UNAWARE, GetDpiForWindow, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_F5, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_SHIFT, VK_TAB,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

type Rt = Runtime<WinShell>;

thread_local! {
    static RUNTIME: RefCell<Option<Rc<Rt>>> = const { RefCell::new(None) };
    /// Window handles the window procedure lays out. Kept apart from the
    /// shell so resizing never has to borrow the runtime.
    static LAYOUT: Cell<Layout> = const { Cell::new(Layout { chrome: null_mut(), active: null_mut(), chrome_height: 0 }) };
}

#[derive(Clone, Copy)]
struct Layout {
    chrome: HWND,
    active: HWND,
    chrome_height: i32,
}

fn post(event: Event) {
    // Clone out of the cell first: handling the event may re-enter here.
    let rt = RUNTIME.with(|r| r.borrow().clone());
    if let Some(rt) = rt {
        rt.post(event);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// One WebKit view plus what its callbacks need to know about it.
struct View {
    id: TabId,
    view: wk::Ref,
    page: wk::Ref,
    hwnd: HWND,
    /// WebKit reports loading as change notifications without a getter.
    loading: Cell<bool>,
}

impl View {
    /// Create a view as a child of `parent`. Boxed so the address handed to
    /// WebKit as callback context stays put.
    unsafe fn new(id: TabId, configuration: wk::Ref, parent: HWND) -> Box<View> {
        let api = wk::api();
        let empty = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let view = (api.WKViewCreate)(empty, configuration, parent);
        (api.WKViewSetIsInWindow)(view, true);
        let page = (api.WKViewGetPage)(view);
        Box::new(View { id, view, page, hwnd: (api.WKViewGetWindow)(view), loading: Cell::new(false) })
    }

    fn snapshot(&self) -> ViewState {
        let api = wk::api();
        unsafe {
            ViewState {
                url: wk::take_url((api.WKPageCopyActiveURL)(self.page)),
                title: wk::take_string((api.WKPageCopyTitle)(self.page)),
                loading: self.loading.get(),
                progress: (api.WKPageGetEstimatedProgress)(self.page),
                can_back: (api.WKPageCanGoBack)(self.page),
                can_forward: (api.WKPageCanGoForward)(self.page),
            }
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        let api = wk::api();
        unsafe {
            // Detach the callbacks first: they hold a pointer to this View.
            (api.WKPageSetPageStateClient)(self.page, null_mut());
            (api.WKPageSetPageUIClient)(self.page, null());
            ShowWindow(self.hwnd, SW_HIDE);
            (api.WKPageClose)(self.page);
            (api.WKRelease)(self.view);
        }
    }
}

unsafe extern "C" fn on_loading_changed(info: *const c_void) {
    let view = &*(info as *const View);
    view.loading.set(!view.loading.get());
    post(Event::View(view.id, view.snapshot()));
}

unsafe extern "C" fn on_state_changed(info: *const c_void) {
    let view = &*(info as *const View);
    post(Event::View(view.id, view.snapshot()));
}

/// window.open() and target=_blank become tabs.
unsafe extern "C" fn on_create_new_page(
    _page: wk::Ref,
    _configuration: wk::Ref,
    action: wk::Ref,
    _features: wk::Ref,
    _info: *const c_void,
) -> wk::Ref {
    let api = wk::api();
    let request = wk::Owned((api.WKNavigationActionCopyRequest)(action));
    if !request.0.is_null() {
        let url = wk::take_url((api.WKURLRequestCopyURL)(request.0));
        if !url.is_empty() {
            post(Event::OpenTab(url));
        }
    }
    null()
}

unsafe extern "C" fn on_script_message(message: wk::Ref, reply: wk::Ref, _context: *const c_void) {
    let api = wk::api();
    let body = (api.WKScriptMessageGetBody)(message);
    let text = (!body.is_null() && (api.WKGetTypeID)(body) == (api.WKStringGetTypeID)()).then(|| wk::to_string(body));
    // WebKit expects every message to be answered; the chrome ignores it.
    (api.WKCompletionListenerComplete)(reply, null());
    if let Some(text) = text {
        post(Event::Ui(text));
    }
}

struct WinShell {
    window: HWND,
    /// Shared by every content view: process pool, settings, site data.
    configuration: wk::Ref,
    chrome: Box<View>,
    views: HashMap<TabId, Box<View>>,
}

impl WinShell {
    fn page(&self, id: TabId) -> Option<wk::Ref> {
        self.views.get(&id).map(|v| v.page)
    }
}

impl Shell for WinShell {
    fn create_view(&mut self, id: TabId) {
        let api = wk::api();
        unsafe {
            let view = View::new(id, self.configuration, self.window);
            ShowWindow(view.hwnd, SW_HIDE);
            let info = &*view as *const View as *const c_void;

            let mut state: wk::PageStateClientV0 = std::mem::zeroed();
            state.base.client_info = info;
            state.didChangeIsLoading = Some(on_loading_changed);
            state.didChangeTitle = Some(on_state_changed);
            state.didChangeActiveURL = Some(on_state_changed);
            state.didChangeEstimatedProgress = Some(on_state_changed);
            state.didChangeCanGoBack = Some(on_state_changed);
            state.didChangeCanGoForward = Some(on_state_changed);
            (api.WKPageSetPageStateClient)(view.page, &mut state.base);

            let mut ui: wk::PageUIClientV6 = std::mem::zeroed();
            ui.base.version = 6;
            ui.base.client_info = info;
            ui.createNewPage = Some(on_create_new_page);
            (api.WKPageSetPageUIClient)(view.page, &ui.base);

            self.views.insert(id, view);
        }
    }

    fn destroy_view(&mut self, id: TabId) {
        self.views.remove(&id);
    }

    fn show_view(&mut self, id: TabId) {
        let Some(active) = self.views.get(&id).map(|v| v.hwnd) else { return };
        let mut layout = LAYOUT.get();
        layout.active = active;
        LAYOUT.set(layout);
        unsafe {
            apply_layout(self.window);
            for view in self.views.values() {
                ShowWindow(view.hwnd, if view.hwnd == active { SW_SHOW } else { SW_HIDE });
            }
        }
    }

    fn navigate(&mut self, id: TabId, url: &str) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageLoadURL)(page, wk::url(url).0) };
        }
    }

    fn load_html(&mut self, id: TabId, html: &str) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageLoadHTMLString)(page, wk::string(html).0, wk::url("about:blank").0) };
        }
    }

    fn go_back(&mut self, id: TabId) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageGoBack)(page) };
        }
    }

    fn go_forward(&mut self, id: TabId) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageGoForward)(page) };
        }
    }

    fn reload(&mut self, id: TabId) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageReload)(page) };
        }
    }

    fn stop(&mut self, id: TabId) {
        if let Some(page) = self.page(id) {
            unsafe { (wk::api().WKPageStopLoading)(page) };
        }
    }

    fn eval_chrome(&mut self, script: &str) {
        unsafe { (wk::api().WKPageEvaluateJavaScriptInMainFrame)(self.chrome.page, wk::string(script).0, null_mut(), None) };
    }

    fn focus_chrome(&mut self) {
        unsafe { SetFocus(self.chrome.hwnd) };
    }

    fn focus_view(&mut self, id: TabId) {
        if let Some(view) = self.views.get(&id) {
            unsafe { SetFocus(view.hwnd) };
        }
    }

    fn set_window_title(&mut self, title: &str) {
        unsafe { SetWindowTextW(self.window, wide(title).as_ptr()) };
    }

    fn close_window(&mut self) {
        // Posted, so the shell is not torn down underneath this call.
        unsafe { PostMessageW(self.window, WM_CLOSE, 0, 0) };
    }
}

/// The chrome's height in physical pixels at `dpi`. WebKit scales each
/// view by the window's real DPI (fractional scales included) for both
/// painting and input, so the layout must use that same factor and must
/// not override the page's backing scale.
fn chrome_height(dpi: u32) -> i32 {
    (CHROME_HEIGHT as f64 * dpi as f64 / 96.0).round() as i32
}

/// Stack the chrome above the active content view, filling the window.
unsafe fn apply_layout(window: HWND) {
    let layout = LAYOUT.get();
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(window, &mut client);
    let (width, height) = (client.right, client.bottom);
    if !layout.chrome.is_null() {
        MoveWindow(layout.chrome, 0, 0, width, layout.chrome_height, 1);
    }
    if !layout.active.is_null() {
        MoveWindow(layout.active, 0, layout.chrome_height, width, (height - layout.chrome_height).max(0), 1);
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_SIZE => {
            apply_layout(window);
            0
        }
        // Moved to a monitor with a different scale: take the size Windows
        // suggests and lay out again for the new DPI.
        WM_DPICHANGED => {
            let mut layout = LAYOUT.get();
            layout.chrome_height = chrome_height((wparam >> 16) as u32 & 0xffff);
            LAYOUT.set(layout);
            let suggested = &*(lparam as *const RECT);
            let (width, height) = (suggested.right - suggested.left, suggested.bottom - suggested.top);
            SetWindowPos(window, null_mut(), suggested.left, suggested.top, width, height, SWP_NOZORDER | SWP_NOACTIVATE);
            apply_layout(window);
            0
        }
        WM_SETFOCUS => {
            let active = LAYOUT.get().active;
            if !active.is_null() {
                SetFocus(active);
            }
            0
        }
        // The child views cover the whole client area.
        WM_ERASEBKGND => 1,
        WM_CLOSE => {
            post(Event::Closing);
            DestroyWindow(window);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

/// Browser shortcuts are picked out of the message loop so they work no
/// matter which web view has keyboard focus.
unsafe fn shortcut_for(message: &MSG) -> Option<Shortcut> {
    if message.message != WM_KEYDOWN && message.message != WM_SYSKEYDOWN {
        return None;
    }
    let down = |key: u16| GetKeyState(key as i32) < 0;
    let (ctrl, shift, alt) = (down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU));
    let key = message.wParam as u16;
    match (ctrl, alt, key) {
        (false, false, VK_F5) => Some(Shortcut::Reload),
        (false, true, VK_LEFT) => Some(Shortcut::Back),
        (false, true, VK_RIGHT) => Some(Shortcut::Forward),
        (true, false, VK_TAB) if shift => Some(Shortcut::PrevTab),
        (true, false, VK_TAB) | (true, false, VK_NEXT) => Some(Shortcut::NextTab),
        (true, false, VK_PRIOR) => Some(Shortcut::PrevTab),
        (true, false, _) if !shift => match key as u8 {
            b'T' => Some(Shortcut::NewTab),
            b'W' => Some(Shortcut::CloseTab),
            b'L' => Some(Shortcut::FocusAddress),
            b'R' => Some(Shortcut::Reload),
            _ => None,
        },
        _ => None,
    }
}

fn fatal(text: &str) -> ! {
    unsafe { MessageBoxW(null_mut(), wide(text).as_ptr(), wide("webby").as_ptr(), MB_OK | MB_ICONERROR) };
    std::process::exit(1);
}

pub fn run(browser: Browser, _profile: Option<PathBuf>) {
    // webby is independent of the Windows display scale: it always lays
    // out, paints and handles input at 100%, and Windows magnifies the
    // finished window. Declared before WebKit loads so nothing else can
    // pick a different mode for the process.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) };

    if let Err(error) = wk::load() {
        fatal(&format!(
            "webby could not load WebKit ({error}).\n\nWebKit2.dll and the files that ship with it must sit next to webby.exe. See the README for how to fetch them."
        ));
    }
    let api = wk::api();
    unsafe {
        OleInitialize(null());

        let instance = GetModuleHandleW(null());
        let class_name = wide("webbyWindow");
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&class);
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("webby").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if window.is_null() {
            fatal("webby could not create its window.");
        }

        let context = (api.WKContextCreateWithConfiguration)(wk::Owned((api.WKContextConfigurationCreate)()).0);
        let preferences = (api.WKPreferencesCreate)();
        (api.WKPreferencesSetDeveloperExtrasEnabled)(preferences, true);
        // GPU compositing in the bundled WebKit draws pages misplaced and
        // out of step with mouse input; the project that publishes that
        // build always runs it with compositing off on Windows. Setting
        // WEBBY_GPU=1 turns it back on for testing.
        let gpu = std::env::var_os("WEBBY_GPU").is_some_and(|v| v == "1");
        (api.WKPreferencesSetAcceleratedCompositingEnabled)(preferences, gpu);
        let new_configuration = || {
            let configuration = (api.WKPageConfigurationCreate)();
            (api.WKPageConfigurationSetContext)(configuration, context);
            (api.WKPageConfigurationSetPreferences)(configuration, preferences);
            (api.WKPageConfigurationSetWebsiteDataStore)(configuration, (api.WKWebsiteDataStoreGetDefaultDataStore)());
            configuration
        };

        // Only the chrome's configuration gets the message handler, so web
        // content can never send commands to the browser.
        let chrome_configuration = wk::Owned(new_configuration());
        let messages = wk::Owned((api.WKUserContentControllerCreate)());
        (api.WKUserContentControllerAddScriptMessageHandler)(messages.0, wk::string(MESSAGE_HANDLER).0, on_script_message, null());
        (api.WKPageConfigurationSetUserContentController)(chrome_configuration.0, messages.0);
        let chrome = View::new(0, chrome_configuration.0, window);

        LAYOUT.set(Layout { chrome: chrome.hwnd, active: null_mut(), chrome_height: chrome_height(GetDpiForWindow(window)) });
        let chrome_page = chrome.page;
        let shell = WinShell { window, configuration: new_configuration(), chrome, views: HashMap::new() };
        RUNTIME.with(|r| *r.borrow_mut() = Some(Rc::new(Runtime::new(browser, shell))));

        ShowWindow(window, SW_SHOWDEFAULT);
        apply_layout(window);
        (api.WKPageLoadHTMLString)(chrome_page, wk::string(CHROME_HTML).0, wk::url("about:blank").0);

        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if let Some(shortcut) = shortcut_for(&message) {
                post(Event::Shortcut(shortcut));
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    // Exiting without unwinding leaves WebKit's teardown to the OS; the
    // session was already saved when the window closed.
    std::process::exit(0);
}

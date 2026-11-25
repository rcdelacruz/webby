//! The default engine on Windows: WebView2, the Chromium-based web view
//! that ships with Windows 10 and 11. Unlike WebKit's Windows port it has
//! Media Source Extensions, Web Audio, DRM and the system audio decoders,
//! so streaming audio and video play.

use super::{chrome_height, shortcut_for_key};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use webby_core::{Browser, CHROME_HTML, Event, Runtime, Shell, TabId, ViewState};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::*;
use windows::Win32::Foundation::{E_POINTER, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR, w};

type Rt = Runtime<EdgeShell>;

/// Posted to the main window when [`PENDING`] has events to process.
const WM_DRAIN: u32 = WM_APP + 1;

thread_local! {
    static RUNTIME: RefCell<Option<Rc<Rt>>> = const { RefCell::new(None) };
    /// The controllers the window procedure lays out. Kept apart from the
    /// shell so resizing never has to borrow the runtime.
    static LAYOUT: RefCell<Layout> = const { RefCell::new(Layout { chrome: None, active: None, chrome_height: 0 }) };
    static WINDOW: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
    /// Events raised inside WebView2 callbacks. They are handled from the
    /// message loop instead, because handling one can create a web view,
    /// which must not happen while WebView2 is calling into us.
    static PENDING: RefCell<VecDeque<Event>> = const { RefCell::new(VecDeque::new()) };
}

struct Layout {
    chrome: Option<ICoreWebView2Controller>,
    active: Option<ICoreWebView2Controller>,
    chrome_height: i32,
}

/// Queue an event raised by WebView2 for the message loop.
fn defer(event: Event) {
    PENDING.with_borrow_mut(|queue| queue.push_back(event));
    unsafe {
        let _ = PostMessageW(Some(WINDOW.get()), WM_DRAIN, WPARAM(0), LPARAM(0));
    }
}

fn drain() {
    let runtime = RUNTIME.with_borrow(|runtime| runtime.clone());
    let Some(runtime) = runtime else { return };
    loop {
        // Take one event at a time: handling it may queue more.
        let next = PENDING.with_borrow_mut(|queue| queue.pop_front());
        let Some(event) = next else { break };
        runtime.post(event);
    }
}

/// Start WebView2, storing its profile under `profile`.
pub fn environment(profile: Option<&Path>) -> std::result::Result<ICoreWebView2Environment, String> {
    let data = profile.map(|dir| HSTRING::from(dir.join("webview2").as_os_str()));
    let (sender, receiver) = mpsc::channel();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let folder = data.as_ref().map_or(PCWSTR::null(), |path| PCWSTR(path.as_ptr()));
            CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), folder, None, &handler).map_err(Error::WindowsError)
        }),
        Box::new(move |result, environment| {
            result?;
            let _ = sender.send(environment);
            Ok(())
        }),
    )
    .map_err(|error| format!("{error:?}"))?;
    receiver.recv().ok().flatten().ok_or_else(|| "no environment was created".to_string())
}

/// Create a web view inside `window`. Blocks, pumping messages, until
/// WebView2 has it ready.
fn create_controller(environment: &ICoreWebView2Environment, window: HWND) -> windows::core::Result<ICoreWebView2Controller> {
    let (sender, receiver) = mpsc::channel();
    let environment = environment.clone();
    let created = CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe { environment.CreateCoreWebView2Controller(window, &handler).map_err(Error::WindowsError) }),
        Box::new(move |result, controller| {
            result?;
            let _ = sender.send(controller);
            Ok(())
        }),
    );
    match created {
        Ok(()) => receiver.recv().ok().flatten().ok_or_else(|| E_POINTER.into()),
        Err(Error::WindowsError(error)) => Err(error),
        Err(_) => Err(E_POINTER.into()),
    }
}

/// Read a string WebView2 hands back through an out parameter.
fn string(read: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
    let mut value = PWSTR::null();
    match read(&mut value) {
        Ok(()) => take_pwstr(value),
        Err(_) => String::new(),
    }
}

fn flag(read: impl FnOnce(*mut BOOL) -> windows::core::Result<()>) -> bool {
    let mut value = BOOL(0);
    read(&mut value).is_ok() && value.as_bool()
}

/// Turn key presses inside a web view into browser shortcuts.
fn watch_shortcuts(controller: &ICoreWebView2Controller) -> windows::core::Result<()> {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(|_, args| {
        let Some(args) = args else { return Ok(()) };
        unsafe {
            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP;
            args.KeyEventKind(&mut kind)?;
            if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN {
                let mut key = 0u32;
                args.VirtualKey(&mut key)?;
                if let Some(shortcut) = shortcut_for_key(key as u16) {
                    args.SetHandled(true)?;
                    defer(Event::Shortcut(shortcut));
                }
            }
        }
        Ok(())
    }));
    let mut token = 0;
    unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token) }
}

struct View {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
}

impl View {
    fn new(environment: &ICoreWebView2Environment, window: HWND) -> windows::core::Result<View> {
        let controller = create_controller(environment, window)?;
        watch_shortcuts(&controller)?;
        let webview = unsafe { controller.CoreWebView2()? };
        Ok(View { controller, webview })
    }
}

fn snapshot(webview: &ICoreWebView2, loading: bool) -> ViewState {
    unsafe {
        ViewState {
            url: string(|out| webview.Source(out)),
            title: string(|out| webview.DocumentTitle(out)),
            loading,
            // WebView2 reports no load progress, only start and finish.
            progress: if loading { 0.5 } else { 1.0 },
            can_back: flag(|out| webview.CanGoBack(out)),
            can_forward: flag(|out| webview.CanGoForward(out)),
        }
    }
}

/// Forward everything the core tracks about a tab's web view.
fn watch_tab(id: TabId, webview: &ICoreWebView2) -> windows::core::Result<()> {
    let loading = Rc::new(Cell::new(false));
    let mut token = 0;
    let report = {
        let (webview, loading) = (webview.clone(), loading.clone());
        move || defer(Event::View(id, snapshot(&webview, loading.get())))
    };
    unsafe {
        let (state, changed) = (loading.clone(), report.clone());
        webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, _| {
                state.set(true);
                changed();
                Ok(())
            })),
            &mut token,
        )?;
        let (state, changed) = (loading.clone(), report.clone());
        webview.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |_, _| {
                state.set(false);
                changed();
                Ok(())
            })),
            &mut token,
        )?;
        let changed = report.clone();
        webview.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |_, _| {
                changed();
                Ok(())
            })),
            &mut token,
        )?;
        let changed = report.clone();
        webview.add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(move |_, _| {
                changed();
                Ok(())
            })),
            &mut token,
        )?;
        let changed = report;
        webview.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |_, _| {
                changed();
                Ok(())
            })),
            &mut token,
        )?;
        // window.open() and target=_blank become tabs.
        webview.add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let url = string(|out| args.Uri(out));
                args.SetHandled(true)?;
                if !url.is_empty() {
                    defer(Event::OpenTab(url));
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

struct EdgeShell {
    window: HWND,
    environment: ICoreWebView2Environment,
    chrome: View,
    views: HashMap<TabId, View>,
}

impl EdgeShell {
    fn webview(&self, id: TabId) -> Option<&ICoreWebView2> {
        self.views.get(&id).map(|view| &view.webview)
    }
}

impl Shell for EdgeShell {
    fn create_view(&mut self, id: TabId) {
        let view = View::new(&self.environment, self.window).and_then(|view| {
            unsafe { view.controller.SetIsVisible(false)? };
            watch_tab(id, &view.webview)?;
            Ok(view)
        });
        match view {
            Ok(view) => {
                self.views.insert(id, view);
            }
            Err(error) => eprintln!("webby: could not create a tab: {error}"),
        }
    }

    fn destroy_view(&mut self, id: TabId) {
        if let Some(view) = self.views.remove(&id) {
            unsafe {
                let _ = view.controller.Close();
            }
        }
    }

    fn show_view(&mut self, id: TabId) {
        let Some(active) = self.views.get(&id).map(|view| view.controller.clone()) else { return };
        LAYOUT.with_borrow_mut(|layout| layout.active = Some(active));
        apply_layout(self.window);
        for (other, view) in &self.views {
            unsafe {
                let _ = view.controller.SetIsVisible(*other == id);
            }
        }
    }

    fn navigate(&mut self, id: TabId, url: &str) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.Navigate(&HSTRING::from(url));
            }
        }
    }

    fn load_html(&mut self, id: TabId, html: &str) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.NavigateToString(&HSTRING::from(html));
            }
        }
    }

    fn go_back(&mut self, id: TabId) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.GoBack();
            }
        }
    }

    fn go_forward(&mut self, id: TabId) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.GoForward();
            }
        }
    }

    fn reload(&mut self, id: TabId) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.Reload();
            }
        }
    }

    fn stop(&mut self, id: TabId) {
        if let Some(webview) = self.webview(id) {
            unsafe {
                let _ = webview.Stop();
            }
        }
    }

    fn eval_chrome(&mut self, script: &str) {
        unsafe {
            let _ = self.chrome.webview.ExecuteScript(&HSTRING::from(script), None);
        }
    }

    fn focus_chrome(&mut self) {
        unsafe {
            let _ = self.chrome.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        }
    }

    fn focus_view(&mut self, id: TabId) {
        if let Some(view) = self.views.get(&id) {
            unsafe {
                let _ = view.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }

    fn set_window_title(&mut self, title: &str) {
        unsafe {
            let _ = SetWindowTextW(self.window, &HSTRING::from(title));
        }
    }

    fn close_window(&mut self) {
        // Posted, so the shell is not torn down underneath this call.
        unsafe {
            let _ = PostMessageW(Some(self.window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Stack the chrome above the active content view, filling the window.
fn apply_layout(window: HWND) {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }
    LAYOUT.with_borrow(|layout| unsafe {
        let split = layout.chrome_height.min(client.bottom);
        if let Some(chrome) = &layout.chrome {
            let _ = chrome.SetBounds(RECT { left: 0, top: 0, right: client.right, bottom: split });
        }
        if let Some(active) = &layout.active {
            let _ = active.SetBounds(RECT { left: 0, top: split, right: client.right, bottom: client.bottom });
        }
    });
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_SIZE => {
            apply_layout(window);
            LRESULT(0)
        }
        // WebView2 positions its pop-ups relative to the window.
        WM_MOVE => {
            LAYOUT.with_borrow(|layout| {
                for controller in [&layout.chrome, &layout.active].into_iter().flatten() {
                    let _ = unsafe { controller.NotifyParentWindowPositionChanged() };
                }
            });
            LRESULT(0)
        }
        WM_SETFOCUS => {
            let active = LAYOUT.with_borrow(|layout| layout.active.clone());
            if let Some(active) = active {
                let _ = unsafe { active.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
            }
            LRESULT(0)
        }
        WM_DRAIN => {
            drain();
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CLOSE => {
            let runtime = RUNTIME.with_borrow(|runtime| runtime.clone());
            if let Some(runtime) = runtime {
                runtime.post(Event::Closing);
            }
            let _ = unsafe { DestroyWindow(window) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn fatal(text: &str) -> ! {
    unsafe { MessageBoxW(None, &HSTRING::from(text), w!("webby"), MB_OK | MB_ICONERROR) };
    std::process::exit(1);
}

pub fn run(browser: Browser, environment: ICoreWebView2Environment) {
    let window = unsafe {
        let instance = GetModuleHandleW(None).map(Into::into).unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("webbyWindow"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("webbyWindow"),
            w!("webby"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(instance),
            None,
        )
    };
    let Ok(window) = window else { fatal("webby could not create its window.") };
    WINDOW.set(window);

    let chrome = match View::new(&environment, window) {
        Ok(chrome) => chrome,
        Err(error) => fatal(&format!("webby could not start its web view ({error}).")),
    };
    unsafe {
        // The toolbar posts its commands as web messages. Only this view
        // listens for them, so web content can never command the browser.
        let mut token = 0;
        let _ = chrome.webview.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    let text = string(|out| args.TryGetWebMessageAsString(out));
                    if !text.is_empty() {
                        defer(Event::Ui(text));
                    }
                }
                Ok(())
            })),
            &mut token,
        );
        if let Ok(settings) = chrome.webview.Settings() {
            let _ = settings.SetIsZoomControlEnabled(false);
            let _ = settings.SetIsStatusBarEnabled(false);
        }
        let _ = chrome.controller.SetIsVisible(true);
    }

    let height = chrome_height(unsafe { GetDpiForWindow(window) });
    LAYOUT.with_borrow_mut(|layout| *layout = Layout { chrome: Some(chrome.controller.clone()), active: None, chrome_height: height });
    let toolbar = chrome.webview.clone();
    let shell = EdgeShell { window, environment, chrome, views: HashMap::new() };
    RUNTIME.with_borrow_mut(|runtime| *runtime = Some(Rc::new(Runtime::new(browser, shell))));

    unsafe {
        let _ = ShowWindow(window, SW_SHOWDEFAULT);
        apply_layout(window);
        let _ = toolbar.NavigateToString(&HSTRING::from(CHROME_HTML));

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    std::process::exit(0);
}

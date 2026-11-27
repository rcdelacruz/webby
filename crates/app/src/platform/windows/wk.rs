//! Hand-written bindings to the WebKit C API exported by `WebKit2.dll`,
//! the library built by WebKit's Windows port. Declarations follow the
//! headers in `Source/WebKit/UIProcess/API/C` and `Source/WebKit/Shared/API/c`
//! of the WebKit repository.
//!
//! The library is loaded at runtime rather than linked, so a missing or
//! incompatible WebKit produces an error dialog instead of a loader failure.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_void};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{HWND, RECT};

/// Every `WK…Ref` is an opaque reference-counted pointer.
pub type Ref = *const c_void;

pub type StateCallback = unsafe extern "C" fn(client_info: *const c_void);
pub type ScriptMessageCallback = unsafe extern "C" fn(message: Ref, reply: Ref, context: *const c_void);
pub type EvaluateCallback = unsafe extern "C" fn(result: Ref, error: Ref, context: *mut c_void);
pub type CreateNewPageCallback =
    unsafe extern "C" fn(page: Ref, configuration: Ref, action: Ref, features: Ref, client_info: *const c_void) -> Ref;
pub type WillEnterFullScreenCallback = unsafe extern "C" fn(page: Ref, listener: Ref, client_info: *const c_void);
pub type BeganEnterFullScreenCallback = unsafe extern "C" fn(page: Ref, initial: Rect, target: Rect, client_info: *const c_void);
pub type ExitFullScreenCallback = unsafe extern "C" fn(page: Ref, client_info: *const c_void);
pub type BeganExitFullScreenCallback =
    unsafe extern "C" fn(page: Ref, initial: Rect, target: Rect, listener: Ref, client_info: *const c_void);

/// `WKRect`: an origin and a size, in doubles.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
pub struct ClientBase {
    pub version: c_int,
    pub client_info: *const c_void,
}

/// `WKPageStateClientV0`: a will/did callback pair per observable property.
#[repr(C)]
pub struct PageStateClientV0 {
    pub base: ClientBase,
    pub willChangeIsLoading: Option<StateCallback>,
    pub didChangeIsLoading: Option<StateCallback>,
    pub willChangeTitle: Option<StateCallback>,
    pub didChangeTitle: Option<StateCallback>,
    pub willChangeActiveURL: Option<StateCallback>,
    pub didChangeActiveURL: Option<StateCallback>,
    pub willChangeHasOnlySecureContent: Option<StateCallback>,
    pub didChangeHasOnlySecureContent: Option<StateCallback>,
    pub willChangeEstimatedProgress: Option<StateCallback>,
    pub didChangeEstimatedProgress: Option<StateCallback>,
    pub willChangeCanGoBack: Option<StateCallback>,
    pub didChangeCanGoBack: Option<StateCallback>,
    pub willChangeCanGoForward: Option<StateCallback>,
    pub didChangeCanGoForward: Option<StateCallback>,
    pub willChangeNetworkRequestsInProgress: Option<StateCallback>,
    pub didChangeNetworkRequestsInProgress: Option<StateCallback>,
    pub willChangeCertificateInfo: Option<StateCallback>,
    pub didChangeCertificateInfo: Option<StateCallback>,
    pub willChangeWebProcessIsResponsive: Option<StateCallback>,
    pub didChangeWebProcessIsResponsive: Option<StateCallback>,
    pub didSwapWebProcesses: Option<StateCallback>,
}

/// `WKPageUIClientV6`, the first version with the current `createNewPage`
/// signature. webby only sets that one callback; the 57 function
/// pointers of versions 0–5 before it and the four after it stay null.
#[repr(C)]
pub struct PageUIClientV6 {
    pub base: ClientBase,
    pub earlier: [*const c_void; 57],
    pub createNewPage: Option<CreateNewPageCallback>,
    pub later: [*const c_void; 4],
}

/// `WKPageFullScreenClientV0`. WebKit asks the application to take its window
/// in and out of fullscreen, since the application owns that window.
#[repr(C)]
pub struct PageFullScreenClientV0 {
    pub base: ClientBase,
    pub willEnterFullScreen: Option<WillEnterFullScreenCallback>,
    pub beganEnterFullScreen: Option<BeganEnterFullScreenCallback>,
    pub exitFullScreen: Option<ExitFullScreenCallback>,
    pub beganExitFullScreen: Option<BeganExitFullScreenCallback>,
}

macro_rules! webkit_api {
    ($(fn $name:ident($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        pub struct Api {
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
            _library: libloading::Library,
        }

        impl Api {
            unsafe fn load() -> Result<Self, libloading::Error> {
                let library = libloading::Library::new("WebKit2.dll")?;
                Ok(Api {
                    $($name: *library.get(concat!(stringify!($name), "\0").as_bytes())?,)*
                    _library: library,
                })
            }
        }
    };
}

webkit_api! {
    fn WKRelease(Ref);
    fn WKGetTypeID(Ref) -> usize;

    fn WKStringGetTypeID() -> usize;
    fn WKStringCreateWithUTF8CString(*const c_char) -> Ref;
    fn WKStringGetMaximumUTF8CStringSize(Ref) -> usize;
    fn WKStringGetUTF8CString(Ref, *mut c_char, usize) -> usize;
    fn WKURLCreateWithUTF8CString(*const c_char) -> Ref;
    fn WKURLCopyString(Ref) -> Ref;
    fn WKURLRequestCopyURL(Ref) -> Ref;
    fn WKNavigationActionCopyRequest(Ref) -> Ref;

    fn WKContextConfigurationCreate() -> Ref;
    fn WKContextCreateWithConfiguration(Ref) -> Ref;
    fn WKWebsiteDataStoreGetDefaultDataStore() -> Ref;
    fn WKPreferencesCreate() -> Ref;
    fn WKPreferencesSetDeveloperExtrasEnabled(Ref, bool);
    fn WKPreferencesSetAcceleratedCompositingEnabled(Ref, bool);
    fn WKPreferencesSetFullScreenEnabled(Ref, bool);
    fn WKPreferencesSetBoolValueForKeyForTesting(Ref, bool, Ref);

    fn WKPageConfigurationCreate() -> Ref;
    fn WKPageConfigurationSetContext(Ref, Ref);
    fn WKPageConfigurationSetPreferences(Ref, Ref);
    fn WKPageConfigurationSetWebsiteDataStore(Ref, Ref);
    fn WKPageConfigurationSetUserContentController(Ref, Ref);

    fn WKUserContentControllerCreate() -> Ref;
    fn WKUserContentControllerAddScriptMessageHandler(Ref, Ref, ScriptMessageCallback, *const c_void);
    fn WKUserContentControllerAddUserScript(Ref, Ref);
    fn WKUserScriptCreateWithSource(Ref, c_int, bool) -> Ref;
    fn WKScriptMessageGetBody(Ref) -> Ref;
    fn WKCompletionListenerComplete(Ref, Ref);

    fn WKViewCreate(RECT, Ref, HWND) -> Ref;
    fn WKViewGetWindow(Ref) -> HWND;
    fn WKViewGetPage(Ref) -> Ref;
    fn WKViewSetIsInWindow(Ref, bool);

    fn WKPageLoadURL(Ref, Ref);
    fn WKPageLoadHTMLString(Ref, Ref, Ref);
    fn WKPageStopLoading(Ref);
    fn WKPageReload(Ref);
    fn WKPageGoBack(Ref);
    fn WKPageGoForward(Ref);
    fn WKPageCanGoBack(Ref) -> bool;
    fn WKPageCanGoForward(Ref) -> bool;
    fn WKPageCopyTitle(Ref) -> Ref;
    fn WKPageCopyActiveURL(Ref) -> Ref;
    fn WKPageGetEstimatedProgress(Ref) -> f64;
    fn WKPageSetCustomUserAgent(Ref, Ref);
    fn WKPageSetPageStateClient(Ref, *mut ClientBase);
    fn WKPageSetPageUIClient(Ref, *const ClientBase);
    fn WKPageSetFullScreenClientForTesting(Ref, *const ClientBase);
    fn WKPageSetUseDarkAppearanceForTesting(Ref, bool);
    fn WKPageRequestExitFullScreen(Ref);
    fn WKPageEvaluateJavaScriptInMainFrame(Ref, Ref, *mut c_void, Option<EvaluateCallback>);
}

static API: OnceLock<Api> = OnceLock::new();

/// Load `WebKit2.dll` and resolve every function webby uses.
pub fn load() -> Result<(), String> {
    if API.get().is_none() {
        // SAFETY: loading WebKit runs its initialisers, which is the point;
        // the signatures above mirror WebKit's public C headers.
        let api = unsafe { Api::load() }.map_err(|e| e.to_string())?;
        let _ = API.set(api);
    }
    Ok(())
}

/// The loaded API. Panics if [`load`] has not succeeded.
pub fn api() -> &'static Api {
    API.get().expect("WebKit is not loaded")
}

/// An owned `WKStringRef`/`WKURLRef`, released on drop.
pub struct Owned(pub Ref);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { (api().WKRelease)(self.0) };
        }
    }
}

fn c_string(s: &str) -> std::ffi::CString {
    // Interior NULs cannot be represented; drop them rather than fail.
    std::ffi::CString::new(s.replace('\0', "")).unwrap_or_default()
}

pub fn string(s: &str) -> Owned {
    Owned(unsafe { (api().WKStringCreateWithUTF8CString)(c_string(s).as_ptr()) })
}

pub fn url(s: &str) -> Owned {
    Owned(unsafe { (api().WKURLCreateWithUTF8CString)(c_string(s).as_ptr()) })
}

/// Copy a `WKStringRef` into a Rust string. Null yields an empty string.
pub fn to_string(s: Ref) -> String {
    if s.is_null() {
        return String::new();
    }
    let wk = api();
    unsafe {
        let mut buffer = vec![0u8; (wk.WKStringGetMaximumUTF8CStringSize)(s)];
        // The returned length counts the terminating NUL.
        let written = (wk.WKStringGetUTF8CString)(s, buffer.as_mut_ptr().cast(), buffer.len());
        buffer.truncate(written.saturating_sub(1));
        String::from_utf8_lossy(&buffer).into_owned()
    }
}

/// Convert and release a string returned by a `…Copy…` function.
pub fn take_string(s: Ref) -> String {
    to_string(Owned(s).0)
}

/// Convert and release a URL returned by a `…Copy…` function.
pub fn take_url(u: Ref) -> String {
    let u = Owned(u);
    if u.0.is_null() {
        return String::new();
    }
    take_string(unsafe { (api().WKURLCopyString)(u.0) })
}

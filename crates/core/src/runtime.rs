//! Serialises everything that happens (UI commands, web view changes,
//! shortcuts) into one queue so the browser is never re-entered: native
//! toolkits love to fire callbacks from inside the very call that caused
//! them.

use crate::{Browser, Shell, Shortcut, TabId, ViewState};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

pub enum Event {
    /// A raw JSON command posted by the chrome UI.
    Ui(String),
    /// Fresh state of a content view.
    View(TabId, ViewState),
    /// A page asked for a new window; it opens as a tab.
    OpenTab(String),
    Shortcut(Shortcut),
    /// The window is going away; persist the session.
    Closing,
}

pub struct Runtime<S: Shell> {
    state: RefCell<(Browser, S)>,
    queue: RefCell<VecDeque<Event>>,
    draining: Cell<bool>,
}

impl<S: Shell> Runtime<S> {
    pub fn new(browser: Browser, shell: S) -> Self {
        Runtime { state: RefCell::new((browser, shell)), queue: RefCell::default(), draining: Cell::new(false) }
    }

    /// Queue an event and process it unless a caller further up the stack
    /// is already draining the queue, in which case it runs right after.
    pub fn post(&self, event: Event) {
        self.queue.borrow_mut().push_back(event);
        if self.draining.replace(true) {
            return;
        }
        loop {
            let next = self.queue.borrow_mut().pop_front();
            let Some(event) = next else { break };
            let (browser, shell) = &mut *self.state.borrow_mut();
            browser.handle(shell, event);
        }
        self.draining.set(false);
    }
}

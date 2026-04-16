use std::cell::RefCell;
use std::rc::Rc;

use iced_servo::{ServoRuntime, ServoWebViewController, WebViewConfig};
use url::Url;

pub struct Tab {
    pub id: usize,
    pub controller: ServoWebViewController,
    pub url_input: String,
    pub last_seen_url: Option<String>,
    pub pending_new_tab: Rc<RefCell<Option<Url>>>,
}

impl Tab {
    pub fn new(id: usize, runtime: &ServoRuntime, url: &str) -> Self {
        let controller =
            ServoWebViewController::new(runtime, WebViewConfig::default().url(url), 1.0)
                .expect("failed to build Servo controller");
        let pending_new_tab: Rc<RefCell<Option<Url>>> = Rc::new(RefCell::new(None));
        {
            let slot = Rc::clone(&pending_new_tab);
            controller.on_new_webview_requested(move |url| {
                *slot.borrow_mut() = Some(url);
            });
        }
        controller.deactivate();
        Self {
            id,
            controller,
            url_input: url.to_string(),
            last_seen_url: None,
            pending_new_tab,
        }
    }

    pub fn display_title(&self) -> String {
        self.controller
            .title()
            .filter(|t| !t.is_empty())
            .or_else(|| self.controller.url())
            .unwrap_or_else(|| "New tab".into())
    }

    /// Keep the URL bar in sync with the page's notion of the current
    /// URL, but only when it actually changes.
    pub fn sync_url(&mut self) {
        let current = self.controller.url();
        if current != self.last_seen_url {
            if let Some(url) = current.clone() {
                self.url_input = url;
            }
            self.last_seen_url = current;
        }
    }
}

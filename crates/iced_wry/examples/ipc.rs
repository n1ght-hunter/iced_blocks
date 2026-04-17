use iced::{
    Element, Fill, Subscription, Task,
    widget::{column, scrollable, text},
    window,
};
use iced_wry::{IpcMessage, WebViewConfig, WebViewController, frame};

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    iced::application(App::new, App::update, App::view)
        .title("WebView IPC")
        .subscription(App::subscription)
        .run()
}

const HTML: &str = r#"<!DOCTYPE html>
<html>
<body style="font-family:sans-serif;padding:2rem;background:#1e1e2e;color:#cdd6f4">
  <h2>IPC Example</h2>
  <button onclick="window.ipc.postMessage('button_clicked')"
          style="padding:0.5rem 1rem;font-size:1rem;cursor:pointer">
    Send IPC Message
  </button>
  <script>
    setInterval(() => window.ipc.postMessage('ping'), 3000);
  </script>
</body>
</html>"#;

struct App {
    controller: WebViewController,
    messages: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    GotWindow(Option<window::Id>),
    WebViewReady(Result<(), String>),
    Ipc(IpcMessage),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let config = WebViewConfig::default().html(HTML).devtools(true);

        (
            Self {
                controller: WebViewController::new(config),
                messages: Vec::new(),
            },
            window::oldest().map(Message::GotWindow),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GotWindow(Some(id)) => self.controller.create_task(id, Message::WebViewReady),
            Message::GotWindow(None) => Task::none(),
            Message::WebViewReady(Ok(())) => {
                self.controller.take_staged();
                Task::none()
            }
            Message::WebViewReady(Err(e)) => {
                eprintln!("WebView failed: {e}");
                Task::none()
            }
            Message::Ipc(msg) => {
                self.messages.push(msg.body);
                if self.messages.len() > 20 {
                    self.messages.remove(0);
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        self.controller.ipc_subscription().map(Message::Ipc)
    }

    fn view(&self) -> Element<'_, Message> {
        let log = self
            .messages
            .iter()
            .rev()
            .fold(column![].spacing(4), |col, msg| {
                col.push(text(format!("← {msg}")).size(14))
            });

        column![
            frame(&self.controller.frame_handle()).height(200),
            scrollable(log).height(Fill),
        ]
        .into()
    }
}

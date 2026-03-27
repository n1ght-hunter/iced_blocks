//! JS->Rust IPC channel bridging `window.ipc.postMessage()` to the app.

use std::sync::{Arc, Mutex};

use futures::channel::mpsc;

#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub body: String,
}

pub(crate) type IpcSender = mpsc::UnboundedSender<IpcMessage>;
pub(crate) type IpcReceiver = Arc<Mutex<Option<mpsc::UnboundedReceiver<IpcMessage>>>>;

pub(crate) fn ipc_channel() -> (IpcSender, IpcReceiver) {
    let (tx, rx) = mpsc::unbounded();
    (tx, Arc::new(Mutex::new(Some(rx))))
}

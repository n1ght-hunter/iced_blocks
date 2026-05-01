/// Errors produced by the wgpusink element.
#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error("GStreamer error: {0}")]
    Gst(String),

    #[error("texture import failed: {0}")]
    Import(String),

    #[error("wrong wgpu backend for this memory type")]
    WrongBackend,

    #[error("unsupported memory type: {0}")]
    UnsupportedMemory(String),
}

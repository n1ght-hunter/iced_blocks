use glow::HasContext;

use crate::ImportError;

/// Blit from one GL framebuffer to another, flipping vertically
/// (OpenGL's bottom-left origin → top-left).
///
/// # Safety
///
/// Caller must ensure the GL context is current and both framebuffers
/// are valid.
pub unsafe fn blit_framebuffer(
    gl: &glow::Context,
    read_fbo: Option<glow::NativeFramebuffer>,
    draw_fbo: Option<glow::NativeFramebuffer>,
    width: i32,
    height: i32,
) -> Result<(), ImportError> {
    unsafe {
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, read_fbo);
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw_fbo);
        gl.blit_framebuffer(
            0,
            0,
            width,
            height,
            0,
            height,
            width,
            0,
            glow::COLOR_BUFFER_BIT,
            glow::NEAREST,
        );
        gl.flush();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
    }
    Ok(())
}

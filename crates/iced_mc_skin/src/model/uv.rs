//! UV coordinates for each face of a cube on the 64x64 Minecraft skin texture.
//! Each face is defined by (x, y) of the top-left corner in pixel coordinates.
//! The face layout for a cube of size (w, h, d) in the skin texture is:
//!
//! ```text
//!            +------+------+
//!            | top  | bot  |   (top row: d tall)
//!  +-----+-------+------+------+
//!  | right | front | left | back |   (main row: h tall)
//!  +-----+-------+------+------+
//! ```
//!
//! Widths: right=d, front=w, left=d, back=w
//! Heights: top/bot=d, sides=h

const TEX_W: f32 = 64.0;
const TEX_H: f32 = 64.0;

#[derive(Debug, Clone, Copy)]
pub struct FaceUv {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl FaceUv {
    const fn px(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            u0: x / TEX_W,
            v0: y / TEX_H,
            u1: (x + w) / TEX_W,
            v1: (y + h) / TEX_H,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CubeFaceUvs {
    pub front: FaceUv,
    pub back: FaceUv,
    pub left: FaceUv,
    pub right: FaceUv,
    pub top: FaceUv,
    pub bottom: FaceUv,
}

fn cube_uvs(ox: f32, oy: f32, w: f32, h: f32, d: f32) -> CubeFaceUvs {
    CubeFaceUvs {
        right: FaceUv::px(ox, oy + d, d, h),
        front: FaceUv::px(ox + d, oy + d, w, h),
        left: FaceUv::px(ox + d + w, oy + d, d, h),
        back: FaceUv::px(ox + d + w + d, oy + d, w, h),
        top: FaceUv::px(ox + d, oy, w, d),
        bottom: FaceUv::px(ox + d + w, oy, w, d),
    }
}

pub fn head() -> CubeFaceUvs {
    cube_uvs(0.0, 0.0, 8.0, 8.0, 8.0)
}

pub fn head_layer() -> CubeFaceUvs {
    cube_uvs(32.0, 0.0, 8.0, 8.0, 8.0)
}

pub fn body() -> CubeFaceUvs {
    cube_uvs(16.0, 16.0, 8.0, 12.0, 4.0)
}

pub fn body_layer() -> CubeFaceUvs {
    cube_uvs(16.0, 32.0, 8.0, 12.0, 4.0)
}

pub fn right_arm(arm_width: f32) -> CubeFaceUvs {
    cube_uvs(40.0, 16.0, arm_width, 12.0, 4.0)
}

pub fn right_arm_layer(arm_width: f32) -> CubeFaceUvs {
    cube_uvs(40.0, 32.0, arm_width, 12.0, 4.0)
}

pub fn left_arm(arm_width: f32) -> CubeFaceUvs {
    cube_uvs(32.0, 48.0, arm_width, 12.0, 4.0)
}

pub fn left_arm_layer(arm_width: f32) -> CubeFaceUvs {
    cube_uvs(48.0, 48.0, arm_width, 12.0, 4.0)
}

pub fn right_leg() -> CubeFaceUvs {
    cube_uvs(0.0, 16.0, 4.0, 12.0, 4.0)
}

pub fn right_leg_layer() -> CubeFaceUvs {
    cube_uvs(0.0, 32.0, 4.0, 12.0, 4.0)
}

pub fn left_leg() -> CubeFaceUvs {
    cube_uvs(16.0, 48.0, 4.0, 12.0, 4.0)
}

pub fn left_leg_layer() -> CubeFaceUvs {
    cube_uvs(0.0, 48.0, 4.0, 12.0, 4.0)
}

//! Iced shader widget for rendering 3D Minecraft player skins with animation
//! and orbit camera controls.

pub mod animation;
pub mod camera;
pub mod model;
pub(crate) mod pipeline;
pub(crate) mod primitive;
pub mod source;
pub(crate) mod vertex;
pub mod widget;

pub use animation::AnimationMode;
pub use model::ArmVariant;
pub use source::Source;

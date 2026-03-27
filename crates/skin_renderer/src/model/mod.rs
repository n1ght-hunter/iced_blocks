//! Minecraft player model: body part definitions, geometry, and UV mapping.

pub mod geometry;
pub mod uv;

use glam::Vec3;

use crate::vertex::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArmVariant {
    #[default]
    Classic,
    Slim,
}

impl std::fmt::Display for ArmVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Classic => write!(f, "Classic"),
            Self::Slim => write!(f, "Slim"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPart {
    Head,
    Body,
    RightArm,
    LeftArm,
    RightLeg,
    LeftLeg,
    HeadLayer,
    BodyLayer,
    RightArmLayer,
    LeftArmLayer,
    RightLegLayer,
    LeftLegLayer,
}

impl BodyPart {
    pub fn is_layer(self) -> bool {
        matches!(
            self,
            Self::HeadLayer
                | Self::BodyLayer
                | Self::RightArmLayer
                | Self::LeftArmLayer
                | Self::RightLegLayer
                | Self::LeftLegLayer
        )
    }
}

pub struct BodyPartMesh {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) part: BodyPart,
    pub(crate) pivot: Vec3,
}

use glam::{Mat4, Vec3, Vec4};

use super::{ArmVariant, BodyPart, BodyPartMesh, uv::CubeFaceUvs};
use crate::{animation::LimbRotations, vertex::Vertex};

#[allow(clippy::too_many_arguments)]
fn quad(
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    normal: Vec3,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) -> [Vertex; 6] {
    [
        Vertex {
            position: p0.into(),
            uv: [u0, v0],
            normal: normal.into(),
        },
        Vertex {
            position: p1.into(),
            uv: [u1, v0],
            normal: normal.into(),
        },
        Vertex {
            position: p2.into(),
            uv: [u1, v1],
            normal: normal.into(),
        },
        Vertex {
            position: p0.into(),
            uv: [u0, v0],
            normal: normal.into(),
        },
        Vertex {
            position: p2.into(),
            uv: [u1, v1],
            normal: normal.into(),
        },
        Vertex {
            position: p3.into(),
            uv: [u0, v1],
            normal: normal.into(),
        },
    ]
}

fn cube(pos: Vec3, size: Vec3, uvs: &CubeFaceUvs) -> Vec<Vertex> {
    let Vec3 {
        x: x0,
        y: y0,
        z: z0,
    } = pos;
    let Vec3 { x: w, y: h, z: d } = size;
    let x1 = x0 + w;
    let y1 = y0 + h;
    let z1 = z0 + d;

    let mut verts = Vec::with_capacity(36);

    // Front face (+Z)
    let f = &uvs.front;
    verts.extend(quad(
        Vec3::new(x0, y1, z1),
        Vec3::new(x1, y1, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x0, y0, z1),
        Vec3::Z,
        f.u0,
        f.v0,
        f.u1,
        f.v1,
    ));

    // Back face (-Z)
    let b = &uvs.back;
    verts.extend(quad(
        Vec3::new(x1, y1, z0),
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y0, z0),
        Vec3::new(x1, y0, z0),
        -Vec3::Z,
        b.u0,
        b.v0,
        b.u1,
        b.v1,
    ));

    // Right face (+X)
    let r = &uvs.right;
    verts.extend(quad(
        Vec3::new(x1, y1, z1),
        Vec3::new(x1, y1, z0),
        Vec3::new(x1, y0, z0),
        Vec3::new(x1, y0, z1),
        Vec3::X,
        r.u0,
        r.v0,
        r.u1,
        r.v1,
    ));

    // Left face (-X)
    let l = &uvs.left;
    verts.extend(quad(
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y1, z1),
        Vec3::new(x0, y0, z1),
        Vec3::new(x0, y0, z0),
        -Vec3::X,
        l.u0,
        l.v0,
        l.u1,
        l.v1,
    ));

    // Top face (+Y)
    let t = &uvs.top;
    verts.extend(quad(
        Vec3::new(x0, y1, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
        Vec3::Y,
        t.u0,
        t.v0,
        t.u1,
        t.v1,
    ));

    // Bottom face (-Y)
    let bo = &uvs.bottom;
    verts.extend(quad(
        Vec3::new(x0, y0, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y0, z0),
        Vec3::new(x0, y0, z0),
        -Vec3::Y,
        bo.u0,
        bo.v0,
        bo.u1,
        bo.v1,
    ));

    verts
}

fn inflate_cube(pos: Vec3, size: Vec3, amount: f32) -> (Vec3, Vec3) {
    (pos - Vec3::splat(amount), size + Vec3::splat(amount * 2.0))
}

pub fn player_parts(arm_variant: ArmVariant) -> Vec<BodyPartMesh> {
    use super::uv;

    let arm_w = match arm_variant {
        ArmVariant::Classic => 4.0,
        ArmVariant::Slim => 3.0,
    };
    let inflate = 0.5;

    let mut parts = Vec::with_capacity(12);

    // Head: 8x8x8 at (-4, 24, -4), pivot at (0, 24, 0)
    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(-4.0, 24.0, -4.0),
            Vec3::new(8.0, 8.0, 8.0),
            &uv::head(),
        ),
        part: BodyPart::Head,
        pivot: Vec3::new(0.0, 24.0, 0.0),
    });
    let (hp, hs) = inflate_cube(
        Vec3::new(-4.0, 24.0, -4.0),
        Vec3::new(8.0, 8.0, 8.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(hp, hs, &uv::head_layer()),
        part: BodyPart::HeadLayer,
        pivot: Vec3::new(0.0, 24.0, 0.0),
    });

    // Body: 8x12x4 at (-4, 12, -2)
    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(-4.0, 12.0, -2.0),
            Vec3::new(8.0, 12.0, 4.0),
            &uv::body(),
        ),
        part: BodyPart::Body,
        pivot: Vec3::ZERO,
    });
    let (bp, bs) = inflate_cube(
        Vec3::new(-4.0, 12.0, -2.0),
        Vec3::new(8.0, 12.0, 4.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(bp, bs, &uv::body_layer()),
        part: BodyPart::BodyLayer,
        pivot: Vec3::ZERO,
    });

    // Right Arm
    let ra_x = -4.0 - arm_w;
    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(ra_x, 12.0, -2.0),
            Vec3::new(arm_w, 12.0, 4.0),
            &uv::right_arm(arm_w),
        ),
        part: BodyPart::RightArm,
        pivot: Vec3::new(ra_x + arm_w / 2.0, 22.0, 0.0),
    });
    let (rap, ras) = inflate_cube(
        Vec3::new(ra_x, 12.0, -2.0),
        Vec3::new(arm_w, 12.0, 4.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(rap, ras, &uv::right_arm_layer(arm_w)),
        part: BodyPart::RightArmLayer,
        pivot: Vec3::new(ra_x + arm_w / 2.0, 22.0, 0.0),
    });

    // Left Arm
    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(4.0, 12.0, -2.0),
            Vec3::new(arm_w, 12.0, 4.0),
            &uv::left_arm(arm_w),
        ),
        part: BodyPart::LeftArm,
        pivot: Vec3::new(4.0 + arm_w / 2.0, 22.0, 0.0),
    });
    let (lap, las) = inflate_cube(
        Vec3::new(4.0, 12.0, -2.0),
        Vec3::new(arm_w, 12.0, 4.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(lap, las, &uv::left_arm_layer(arm_w)),
        part: BodyPart::LeftArmLayer,
        pivot: Vec3::new(4.0 + arm_w / 2.0, 22.0, 0.0),
    });

    // Legs offset by 0.1 inward to avoid z-fighting where they meet
    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(-3.9, 0.0, -2.0),
            Vec3::new(4.0, 12.0, 4.0),
            &uv::right_leg(),
        ),
        part: BodyPart::RightLeg,
        pivot: Vec3::new(-1.9, 12.0, 0.0),
    });
    let (rlp, rls) = inflate_cube(
        Vec3::new(-3.9, 0.0, -2.0),
        Vec3::new(4.0, 12.0, 4.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(rlp, rls, &uv::right_leg_layer()),
        part: BodyPart::RightLegLayer,
        pivot: Vec3::new(-1.9, 12.0, 0.0),
    });

    parts.push(BodyPartMesh {
        vertices: cube(
            Vec3::new(-0.1, 0.0, -2.0),
            Vec3::new(4.0, 12.0, 4.0),
            &uv::left_leg(),
        ),
        part: BodyPart::LeftLeg,
        pivot: Vec3::new(1.9, 12.0, 0.0),
    });
    let (llp, lls) = inflate_cube(
        Vec3::new(-0.1, 0.0, -2.0),
        Vec3::new(4.0, 12.0, 4.0),
        inflate,
    );
    parts.push(BodyPartMesh {
        vertices: cube(llp, lls, &uv::left_leg_layer()),
        part: BodyPart::LeftLegLayer,
        pivot: Vec3::new(1.9, 12.0, 0.0),
    });

    parts
}

fn transform_vertex(v: &Vertex, mat: &Mat4) -> Vertex {
    let pos = *mat * Vec4::new(v.position[0], v.position[1], v.position[2], 1.0);
    let norm = mat
        .transform_vector3(Vec3::from(v.normal))
        .normalize_or_zero();
    Vertex {
        position: [pos.x, pos.y, pos.z],
        uv: v.uv,
        normal: norm.into(),
    }
}

pub fn animated_vertices(parts: &[BodyPartMesh], rotations: &LimbRotations) -> Vec<Vertex> {
    let mut all = Vec::with_capacity(parts.len() * 36);

    parts.iter().for_each(|part| {
        let angle = match part.part {
            BodyPart::Head | BodyPart::HeadLayer => rotations.head_pitch,
            BodyPart::RightArm | BodyPart::RightArmLayer => rotations.right_arm_pitch,
            BodyPart::LeftArm | BodyPart::LeftArmLayer => rotations.left_arm_pitch,
            BodyPart::RightLeg | BodyPart::RightLegLayer => rotations.right_leg_pitch,
            BodyPart::LeftLeg | BodyPart::LeftLegLayer => rotations.left_leg_pitch,
            BodyPart::Body | BodyPart::BodyLayer => 0.0,
        };

        if angle.abs() < 1e-6 {
            all.extend_from_slice(&part.vertices);
        } else {
            let pivot = part.pivot;
            let mat = Mat4::from_translation(pivot)
                * Mat4::from_rotation_x(angle)
                * Mat4::from_translation(-pivot);
            part.vertices.iter().for_each(|v| {
                all.push(transform_vertex(v, &mat));
            });
        }
    });

    all
}

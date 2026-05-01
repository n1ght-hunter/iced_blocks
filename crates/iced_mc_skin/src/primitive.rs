//! Shader primitive that packages per-frame skin rendering data.
use iced::{
    Rectangle, wgpu,
    widget::shader::{self, Viewport},
};

use crate::{pipeline::SkinPipeline, skin::Skin, vertex::Vertex};

#[derive(Debug)]
pub struct SkinPrimitive {
    vertices: Vec<Vertex>,
    view_proj: [f32; 16],
    skin: Skin,
}

impl SkinPrimitive {
    pub fn new(vertices: Vec<Vertex>, view_proj: glam::Mat4, skin: Skin) -> Self {
        Self {
            vertices,
            view_proj: view_proj.to_cols_array(),
            skin,
        }
    }
}

impl shader::Primitive for SkinPrimitive {
    type Pipeline = SkinPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        pipeline.update_vertices(queue, &self.vertices);
        pipeline.update_uniforms(queue, &self.view_proj);

        pipeline.update_skin(device, queue, &self.skin);

        let w = viewport.physical_width();
        let h = viewport.physical_height();
        pipeline.ensure_depth(device, w, h);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(dv) = &pipeline.depth_view else {
            return;
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("skin_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: dv,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );
        pass.set_viewport(
            clip_bounds.x as f32,
            clip_bounds.y as f32,
            clip_bounds.width as f32,
            clip_bounds.height as f32,
            0.0,
            1.0,
        );

        pass.set_pipeline(&pipeline.render_pipeline);
        pass.set_bind_group(0, &pipeline.uniform_bind_group, &[]);
        pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
        pass.set_vertex_buffer(0, pipeline.vertex_buffer.slice(..));
        pass.draw(0..pipeline.vertex_count, 0..1);
    }
}

//! wgpu `Primitive` + `Pipeline` for drawing an offscreen frame as a
//! textured quad. The pipeline owns a persistent texture, a sampler
//! that tracks [`FilterMode`], and a small uniform buffer
//! (`uv_transform: vec4<f32>`, xy = scale, zw = offset) that controls
//! content fit and alignment.

use std::sync::{Arc, Mutex};

use dpi::PhysicalSize;
use iced::widget::shader::{self, Viewport};
use iced::{Rectangle, Size};

use crate::{Alignment, ContentFit, FilterMode, Frame};

type SizeRequestInner = Arc<Mutex<Option<(PhysicalSize<u32>, f32)>>>;

#[derive(Clone, Debug, Default)]
pub struct SizeRequestSlot(SizeRequestInner);

impl SizeRequestSlot {
    /// Creates a new [`SizeRequestSlot`] with no requested size.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current requested size and scale factor, if any.
    pub fn size(&self) -> Option<(PhysicalSize<u32>, f32)> {
        *self.0.lock().unwrap()
    }

    fn set_size(&self, size: PhysicalSize<u32>, scale_factor: f32) {
        *self.0.lock().unwrap() = Some((size, scale_factor));
    }
}

#[derive(Debug)]
pub struct FramePrimitive {
    pub(crate) frame_slot: Arc<Mutex<Option<Frame>>>,
    pub(crate) size_request: SizeRequestSlot,
    pub(crate) logical_bounds: Size<f32>,
    pub(crate) content_fit: ContentFit,
    pub(crate) alignment: Alignment,
    pub(crate) filter: FilterMode,
}

#[derive(Debug)]
pub struct FramePipeline {
    device: wgpu::Device,
    texture: wgpu::Texture,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uv_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    texture_size: (u32, u32),
    texture_format: wgpu::TextureFormat,
    filter: FilterMode,
}

impl FramePipeline {
    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iced_frame.texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn create_sampler(device: &wgpu::Device, filter: FilterMode) -> wgpu::Sampler {
        let f = filter.to_wgpu();
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iced_frame.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: f,
            min_filter: f,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        })
    }

    fn build_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
        uv_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iced_frame.bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uv_buf.as_entire_binding(),
                },
            ],
        })
    }

    fn update_if_needed(&mut self, frame: &Frame, filter: FilterMode) {
        let size_changed =
            self.texture_size != (frame.width, frame.height) || self.texture_format != frame.format;
        let filter_changed = self.filter != filter;

        if !size_changed && !filter_changed {
            return;
        }

        if size_changed {
            self.texture =
                Self::create_texture(&self.device, frame.width, frame.height, frame.format);
            self.texture_format = frame.format;
            self.texture_size = (frame.width, frame.height);
        }

        if filter_changed {
            self.sampler = Self::create_sampler(&self.device, filter);
            self.filter = filter;
        }

        self.bind_group = Self::build_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.texture,
            &self.sampler,
            &self.uv_buf,
        );
    }

    fn upload(&self, queue: &wgpu::Queue, frame: &Frame) {
        let (w, h) = (frame.width, frame.height);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl shader::Pipeline for FramePipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced_frame.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("primitive.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced_frame.bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced_frame.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced_frame.render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let default_format = wgpu::TextureFormat::Rgba8Unorm;
        let default_filter = FilterMode::default();
        let texture = Self::create_texture(device, 1, 1, default_format);
        let sampler = Self::create_sampler(device, default_filter);
        let uv_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("iced_frame.uv_transform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group =
            Self::build_bind_group(device, &bind_group_layout, &texture, &sampler, &uv_buf);

        Self {
            device: device.clone(),
            texture,
            bind_group_layout,
            sampler,
            uv_buf,
            bind_group,
            pipeline,
            texture_size: (1, 1),
            texture_format: default_format,
            filter: default_filter,
        }
    }
}

impl shader::Primitive for FramePrimitive {
    type Pipeline = FramePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor();
        let widget_w = (self.logical_bounds.width * scale).round().max(1.0) as u32;
        let widget_h = (self.logical_bounds.height * scale).round().max(1.0) as u32;
        self.size_request
            .set_size(PhysicalSize::new(widget_w, widget_h), scale);

        if let Some(frame) = self.frame_slot.lock().unwrap().take() {
            pipeline.update_if_needed(&frame, self.filter);
            pipeline.upload(queue, &frame);
        }

        let transform = compute_uv_transform(
            pipeline.texture_size,
            widget_w,
            widget_h,
            self.content_fit,
            self.alignment,
        );
        queue.write_buffer(
            &pipeline.uv_buf,
            0,
            &transform.map(f32::to_ne_bytes).concat(),
        );
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
        true
    }
}

/// Compute `[uv_scale.x, uv_scale.y, uv_offset.x, uv_offset.y]`.
///
/// The shader applies `uv = base_uv * scale + offset` per vertex.
/// Fragments with `uv` outside [0,1] are discarded.
fn compute_uv_transform(
    tex: (u32, u32),
    ww: u32,
    wh: u32,
    fit: ContentFit,
    align: Alignment,
) -> [f32; 4] {
    let tw = tex.0.max(1) as f32;
    let th = tex.1.max(1) as f32;
    let ww = ww.max(1) as f32;
    let wh = wh.max(1) as f32;

    let uv_scale = match fit {
        ContentFit::Fill => [1.0_f32, 1.0_f32],
        ContentFit::Contain => {
            let r = (ww / tw).min(wh / th);
            [ww / (tw * r), wh / (th * r)]
        }
        ContentFit::Cover => {
            let r = (ww / tw).max(wh / th);
            [ww / (tw * r), wh / (th * r)]
        }
        ContentFit::FitWidth => {
            let r = ww / tw;
            [1.0, wh / (th * r)]
        }
        ContentFit::FitHeight => {
            let r = wh / th;
            [ww / (tw * r), 1.0]
        }
        ContentFit::None => [ww / tw, wh / th],
    };

    let extra_x = 1.0 - uv_scale[0];
    let extra_y = 1.0 - uv_scale[1];

    let (ox, oy) = match align {
        Alignment::TopLeft => (0.0, 0.0),
        Alignment::TopCenter => (extra_x / 2.0, 0.0),
        Alignment::TopRight => (extra_x, 0.0),
        Alignment::CenterLeft => (0.0, extra_y / 2.0),
        Alignment::Center => (extra_x / 2.0, extra_y / 2.0),
        Alignment::CenterRight => (extra_x, extra_y / 2.0),
        Alignment::BottomLeft => (0.0, extra_y),
        Alignment::BottomCenter => (extra_x / 2.0, extra_y),
        Alignment::BottomRight => (extra_x, extra_y),
    };

    let uv_offset = [ox, oy];

    [uv_scale[0], uv_scale[1], uv_offset[0], uv_offset[1]]
}

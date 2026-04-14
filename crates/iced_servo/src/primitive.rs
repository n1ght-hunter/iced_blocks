//! wgpu `Primitive` + `Pipeline` for drawing Servo's latest rendered
//! frame from inside the custom [`ServoWidget`](crate::widget::ServoWidget).
//! The pipeline is a single textured quad; the texture is recreated
//! when the frame size changes and re-uploaded when a new `RgbaImage`
//! lands in the shared frame slot.

use std::sync::{Arc, Mutex};

use dpi::PhysicalSize;
use iced::widget::shader::{self, Viewport};
use iced::{Rectangle, Size};
use image::RgbaImage;

/// Request from `Primitive::prepare` back to the controller: "render at
/// this physical-pixel size, the host wgpu surface is at this DPI scale".
/// The controller's `tick()` drains the latest value (debounced) and calls
/// `webview.resize`.
pub(crate) type SizeRequestSlot = Arc<Mutex<Option<(PhysicalSize<u32>, f32)>>>;

/// Per-draw primitive handed to iced. Cheap to construct — holds only
/// `Send + Sync` handles so it can live inside the shader-widget pipeline.
/// The actual GPU work happens in
/// [`prepare`](shader::Primitive::prepare) via the pipeline.
#[derive(Debug)]
pub struct ServoTexturePrimitive {
    pub(crate) frame_slot: Arc<Mutex<Option<RgbaImage>>>,
    pub(crate) size_request: SizeRequestSlot,
    pub(crate) logical_bounds: Size<f32>,
}

/// Persistent per-widget GPU state, kept in the shader `Storage`.
#[derive(Debug)]
pub struct ServoTexturePipeline {
    device: wgpu::Device,
    texture: wgpu::Texture,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    texture_size: (u32, u32),
}

impl ServoTexturePipeline {
    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iced_servo.frame_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Servo's `read_to_image` returns sRGB-encoded pixel bytes
            // already in the target display color space. If we use
            // `Rgba8UnormSrgb` wgpu would decode them to linear on
            // sample; then the sRGB-encoded render target would re-encode
            // the same values, double-processing the gamma. `Rgba8Unorm`
            // passes the bytes through unchanged, letting the render
            // target handle exactly one conversion at write time.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn build_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iced_servo.frame_bind_group"),
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
            ],
        })
    }

    fn resize_if_needed(&mut self, width: u32, height: u32) {
        if self.texture_size == (width, height) {
            return;
        }
        self.texture = Self::create_texture(&self.device, width, height);
        self.bind_group = Self::build_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.texture,
            &self.sampler,
        );
        self.texture_size = (width, height);
    }

    fn upload(&self, queue: &wgpu::Queue, frame: &RgbaImage) {
        let (w, h) = (frame.width(), frame.height());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            frame.as_raw(),
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

impl shader::Pipeline for ServoTexturePipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced_servo.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("primitive.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced_servo.bind_group_layout"),
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced_servo.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced_servo.render_pipeline"),
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

        let texture = Self::create_texture(device, 1, 1);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iced_servo.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = Self::build_bind_group(device, &bind_group_layout, &texture, &sampler);

        Self {
            device: device.clone(),
            texture,
            bind_group_layout,
            sampler,
            bind_group,
            pipeline,
            texture_size: (1, 1),
        }
    }
}

impl shader::Primitive for ServoTexturePrimitive {
    type Pipeline = ServoTexturePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        // Ask Servo to render at the physical-pixel size of the widget.
        // `viewport.scale_factor()` is the real DPI iced uses for the
        // wgpu surface, so rendering at
        // `logical_bounds * scale_factor` matches pixel-for-pixel with
        // the iced render target — no upscale, no blurriness.
        let scale = viewport.scale_factor();
        let physical_w = (self.logical_bounds.width * scale).round().max(1.0) as u32;
        let physical_h = (self.logical_bounds.height * scale).round().max(1.0) as u32;
        *self.size_request.lock().unwrap() =
            Some((PhysicalSize::new(physical_w, physical_h), scale));

        // Upload the latest Servo frame if one is waiting. If not, the
        // last frame stays bound to the texture.
        let Some(frame) = self.frame_slot.lock().unwrap().take() else {
            return;
        };
        pipeline.resize_if_needed(frame.width(), frame.height());
        pipeline.upload(queue, &frame);
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        // iced has already set the render pass viewport + scissor to our
        // widget bounds. We just draw a fullscreen quad (in NDC), which
        // gets clipped to those bounds by the scissor.
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
        true
    }
}

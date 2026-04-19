//! Iced example: imports textures from available source types and
//! displays them as labeled `shader::Program` widgets.
//!
//! On DX12: D3D12Resource + D3D11SharedHandle + VulkanImage + GlesTexture.
//!
//! ```bash
//! cargo run -p wgpu_interop --example import_iced
//! ```

#![cfg(target_os = "windows")]

#[path = "common/mod.rs"]
mod common;

use iced::widget::{Shader, column, row, shader, text};
use iced::{Element, Length, Rectangle, wgpu};
use wgpu_interop::DeviceInterop;

/// HANDLE wrapper that is Send + Sync for use in iced primitives.
/// Shared NTHANDLE is process-wide and not thread-affine.
#[derive(Clone, Copy)]
struct SyncHandle(windows::Win32::Foundation::HANDLE);
unsafe impl Send for SyncHandle {}
unsafe impl Sync for SyncHandle {}
impl std::fmt::Debug for SyncHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SyncHandle({:?})", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
enum PreparedSource {
    D3D12Resource,
    D3D11SharedHandle(SyncHandle),
    VulkanImage { seed: u64 },
    GlesTexture { seed: u64 },
}

struct InteropProgram {
    slot: usize,
    source: PreparedSource,
}

impl InteropProgram {
    fn view(&self) -> Shader<(), &Self> {
        Shader::new(self).width(Length::Fill).height(Length::Fill)
    }
}

#[derive(Debug)]
struct InteropPrimitive {
    slot: usize,
    source: PreparedSource,
}

impl<Message> shader::Program<Message> for InteropProgram {
    type State = ();
    type Primitive = InteropPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: iced::mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        InteropPrimitive {
            slot: self.slot,
            source: self.source,
        }
    }
}

struct InteropPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_groups: Vec<Option<wgpu::BindGroup>>,
    skipped: std::collections::HashSet<usize>,
}

impl shader::Pipeline for InteropPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("interop_bgl"),
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("interop_shader"),
            source: wgpu::ShaderSource::Wgsl(common::FULLSCREEN_TRIANGLE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("interop_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("interop_rp"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            bind_groups: Vec::new(),
            skipped: std::collections::HashSet::new(),
        }
    }
}

impl InteropPipeline {
    fn import_texture(
        &mut self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        slot: usize,
        source: PreparedSource,
    ) {
        use wgpu_interop::TextureSourceTypes;

        let supported = device.supported_sources();
        let required = match source {
            PreparedSource::D3D12Resource => TextureSourceTypes::D3D12Resource,
            PreparedSource::D3D11SharedHandle(_) => TextureSourceTypes::D3D11SharedHandle,
            PreparedSource::VulkanImage { .. } => TextureSourceTypes::VulkanImage,
            PreparedSource::GlesTexture { .. } => TextureSourceTypes::GlesTexture,
        };
        if !supported.contains(required) {
            self.skipped.insert(slot);
            return;
        }

        let desc = common::texture_desc();
        let tex = match source {
            PreparedSource::D3D12Resource => {
                let d3d12_src = unsafe { common::create_d3d12_resource(device, 0xD3D12) };
                unsafe {
                    device
                        .import_external_texture(d3d12_src, &desc)
                        .expect("import D3D12Resource")
                }
            }
            PreparedSource::D3D11SharedHandle(sync_handle) => {
                let handle = sync_handle.0;
                let tex = unsafe {
                    device
                        .import_external_texture(wgpu_interop::D3D11SharedHandle { handle }, &desc)
                        .expect("import D3D11SharedHandle")
                };
                unsafe { windows::Win32::Foundation::CloseHandle(handle) }.ok();
                tex
            }
            PreparedSource::VulkanImage { seed } => {
                let source = common::create_vulkan_image(seed);
                unsafe {
                    device
                        .import_external_texture(source, &desc)
                        .expect("import VulkanImage")
                }
            }
            PreparedSource::GlesTexture { seed } => {
                let (name, _wgl_ctx, interop) = common::create_gles_texture(seed);
                let source = wgpu_interop::GlesTexture {
                    gl: &_wgl_ctx.gl,
                    name,
                    interop: Some(&interop),
                };
                match unsafe { device.import_external_texture(source, &desc) } {
                    Ok(tex) => tex,
                    Err(e) => {
                        eprintln!("  Skipping GlesTexture: {e}");
                        self.skipped.insert(slot);
                        return;
                    }
                }
            }
        };

        let view = tex.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("interop_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.bind_groups[slot] = Some(bind_group);
    }
}

impl shader::Primitive for InteropPrimitive {
    type Pipeline = InteropPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        if pipeline.skipped.contains(&self.slot) {
            return;
        }
        if pipeline.bind_groups.len() <= self.slot {
            pipeline.bind_groups.resize_with(self.slot + 1, || None);
        }
        if pipeline.bind_groups[self.slot].is_none() {
            pipeline.import_texture(device, queue, self.slot, self.source);
        }
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(bind_group) = &pipeline.bind_groups[self.slot] else {
            return;
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("interop_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
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
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

struct App {
    programs: Vec<(String, InteropProgram)>,
}

impl App {
    fn new() -> (Self, iced::Task<()>) {
        let handle = common::create_d3d11_shared_handle(0xD3D11);

        let programs = vec![
            (
                "D3D12Resource".into(),
                InteropProgram {
                    slot: 0,
                    source: PreparedSource::D3D12Resource,
                },
            ),
            (
                "D3D11SharedHandle".into(),
                InteropProgram {
                    slot: 1,
                    source: PreparedSource::D3D11SharedHandle(SyncHandle(handle)),
                },
            ),
            (
                "VulkanImage".into(),
                InteropProgram {
                    slot: 2,
                    source: PreparedSource::VulkanImage { seed: 0x71CA },
                },
            ),
            (
                "GlesTexture".into(),
                InteropProgram {
                    slot: 3,
                    source: PreparedSource::GlesTexture { seed: 0x61E5 },
                },
            ),
        ];

        eprintln!("{} programs created", programs.len());

        (App { programs }, iced::Task::none())
    }

    fn update(&mut self, _message: ()) -> iced::Task<()> {
        iced::Task::none()
    }

    fn view(&self) -> Element<'_, ()> {
        let items: Vec<Element<'_, ()>> = self
            .programs
            .iter()
            .map(|(label, program)| {
                column![text(label.as_str()).size(16), program.view()]
                    .spacing(8)
                    .width(Length::Fill)
                    .into()
            })
            .collect();

        row(items)
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("wgpu_interop — iced demo")
        .window_size((700.0, 400.0))
        .run()
}

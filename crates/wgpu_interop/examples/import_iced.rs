//! Iced example: imports textures from available source types and
//! displays them as labeled `shader::Program` widgets.
//!
//! Source set depends on OS — see `common::platform_sources()`:
//! - Windows (DX12): D3D12Resource + D3D11SharedHandle + VulkanImage + GlesTexture
//! - macOS (Metal):  MetalTexture
//! - Linux (Vulkan): VulkanImage

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use common::SourceFactory;
use iced::widget::{Shader, column, row, shader, text};
use iced::{Element, Length, Rectangle, wgpu};
use wgpu_interop::DeviceInterop;

struct InteropProgram {
    slot: usize,
    factory: Arc<dyn SourceFactory>,
}

impl InteropProgram {
    fn view(&self) -> Shader<(), &Self> {
        Shader::new(self).width(Length::Fill).height(Length::Fill)
    }
}

struct InteropPrimitive {
    slot: usize,
    factory: Arc<dyn SourceFactory>,
}

impl std::fmt::Debug for InteropPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteropPrimitive")
            .field("slot", &self.slot)
            .field("label", &self.factory.label())
            .finish()
    }
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
            factory: Arc::clone(&self.factory),
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
    fn import_texture(&mut self, device: &wgpu::Device, slot: usize, factory: &dyn SourceFactory) {
        // Skip factories whose source type isn't supported by this
        // wgpu backend (e.g. `MetalTexture` on a Vulkan/MoltenVK device).
        if !device
            .supported_sources()
            .contains(factory.required_source())
        {
            self.skipped.insert(slot);
            eprintln!("  Skipping {} (not supported by backend)", factory.label());
            return;
        }
        let tex = unsafe { factory.import(device) };
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
        _queue: &wgpu::Queue,
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
            pipeline.import_texture(device, self.slot, &*self.factory);
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
    programs: Vec<InteropProgram>,
}

impl App {
    fn new() -> (Self, iced::Task<()>) {
        let programs: Vec<InteropProgram> = common::platform_sources()
            .into_iter()
            .enumerate()
            .map(|(slot, factory)| InteropProgram { slot, factory })
            .collect();
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
            .map(|program| {
                column![text(program.factory.label()).size(16), program.view()]
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

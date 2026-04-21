//! Imports textures from all available source types and renders them
//! side-by-side in a winit window with labels.
//!
//! - **Windows (DX12)**: D3D12Resource + D3D11SharedHandle + VulkanImage + GlesTexture
//! - **macOS (Metal)**: MetalTexture
//! - **Linux (Vulkan)**: VulkanImage
//!
//! ```bash
//! cargo run -p wgpu_interop --example import_winit
//! ```

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const MARGIN: f32 = 20.0;
const GAP: f32 = 20.0;
const LABEL_HEIGHT: f32 = 30.0;
const LABEL_GAP: f32 = 8.0;

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x11, 0x01, 0x02, 0x04, 0x04, 0x04],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        'a' => [0x00, 0x00, 0x0E, 0x01, 0x0F, 0x11, 0x0F],
        'b' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1E],
        'c' => [0x00, 0x00, 0x0E, 0x10, 0x10, 0x11, 0x0E],
        'd' => [0x01, 0x01, 0x0D, 0x13, 0x11, 0x11, 0x0F],
        'e' => [0x00, 0x00, 0x0E, 0x11, 0x1F, 0x10, 0x0E],
        'f' => [0x06, 0x09, 0x08, 0x1C, 0x08, 0x08, 0x08],
        'g' => [0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'h' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
        'i' => [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x0E],
        'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0C],
        'k' => [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
        'l' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'm' => [0x00, 0x00, 0x1A, 0x15, 0x15, 0x11, 0x11],
        'n' => [0x00, 0x00, 0x16, 0x19, 0x11, 0x11, 0x11],
        'o' => [0x00, 0x00, 0x0E, 0x11, 0x11, 0x11, 0x0E],
        'p' => [0x00, 0x00, 0x1E, 0x11, 0x1E, 0x10, 0x10],
        'q' => [0x00, 0x00, 0x0D, 0x13, 0x0F, 0x01, 0x01],
        'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        's' => [0x00, 0x00, 0x0E, 0x10, 0x0E, 0x01, 0x1E],
        't' => [0x08, 0x08, 0x1C, 0x08, 0x08, 0x09, 0x06],
        'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0D],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'w' => [0x00, 0x00, 0x11, 0x11, 0x15, 0x15, 0x0A],
        'x' => [0x00, 0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11],
        'y' => [0x00, 0x00, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'z' => [0x00, 0x00, 0x1F, 0x02, 0x04, 0x08, 0x1F],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

const FONT_CHAR_W: usize = 5;
const FONT_CHAR_H: usize = 7;
const FONT_SCALE: usize = 2;
const FONT_SPACING: usize = 1;
const FONT_PAD: usize = 2;

/// Renders a text string into a wgpu texture using the embedded bitmap font.
fn render_text_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    text: &str,
) -> (wgpu::Texture, u32, u32) {
    let scaled_w = FONT_CHAR_W * FONT_SCALE;
    let scaled_h = FONT_CHAR_H * FONT_SCALE;
    let scaled_spacing = FONT_SPACING * FONT_SCALE;
    let scaled_pad = FONT_PAD * FONT_SCALE;

    let char_count = text.chars().count();
    let text_w = char_count * scaled_w + char_count.saturating_sub(1) * scaled_spacing;
    let total_w = text_w + 2 * scaled_pad;
    let total_h = scaled_h + 2 * scaled_pad;

    let mut pixels = vec![0u8; total_w * total_h * 4];

    for (ci, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        for (row, &bits) in g.iter().enumerate().take(FONT_CHAR_H) {
            for col in 0..FONT_CHAR_W {
                if bits & (1 << (4 - col)) != 0 {
                    for sy in 0..FONT_SCALE {
                        for sx in 0..FONT_SCALE {
                            let px = scaled_pad
                                + ci * (scaled_w + scaled_spacing)
                                + col * FONT_SCALE
                                + sx;
                            let py = scaled_pad + row * FONT_SCALE + sy;
                            let idx = (py * total_w + px) * 4;
                            pixels[idx] = 220;
                            pixels[idx + 1] = 220;
                            pixels[idx + 2] = 220;
                            pixels[idx + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    let w = total_w as u32;
    let h = total_h as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("label"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        view_formats: &[],
    });

    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );

    (texture, w, h)
}

struct Block {
    label_bind_group: wgpu::BindGroup,
    label_tex_w: u32,
    label_tex_h: u32,
    texture_bind_group: wgpu::BindGroup,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    blocks: Vec<Block>,
}

struct App {
    state: Option<RenderState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("wgpu_interop — all sources")
                        .with_inner_size(winit::dpi::LogicalSize::new(800, 400)),
                )
                .expect("create window"),
        );

        // Honor `WGPU_BACKEND=vulkan` etc. so the same example can run
        // on either Metal or MoltenVK on macOS.
        let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("request adapter");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("request device");

        eprintln!("Backend: {:?}", adapter.get_info().backend);

        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("get default surface config");
        surface.configure(&device, &config);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
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

        let make_bind_group = |label: &str, view: &wgpu::TextureView| -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };

        let supported = wgpu_interop::DeviceInterop::supported_sources(&device);
        eprintln!("Supported sources: {:#010b}", supported.bits());

        let mut blocks = Vec::new();
        for factory in common::platform_sources() {
            if !supported.contains(factory.required_source()) {
                eprintln!(
                    "  Skipping {} (not supported by {:?})",
                    factory.label(),
                    adapter.get_info().backend
                );
                continue;
            }
            let tex = unsafe { factory.import(&device) };
            let tex_view = tex.create_view(&Default::default());

            let (label_tex, label_tex_w, label_tex_h) =
                render_text_texture(&device, &queue, factory.label());
            let label_view = label_tex.create_view(&Default::default());

            blocks.push(Block {
                label_bind_group: make_bind_group("label", &label_view),
                label_tex_w,
                label_tex_h,
                texture_bind_group: make_bind_group("bg", &tex_view),
            });
            eprintln!("  Imported {}", factory.label());
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(common::FULLSCREEN_TRIANGLE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rp"),
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
                    format: config.format,
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

        self.state = Some(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            blocks,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match state.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Outdated) => return,
                    Err(e) => panic!("surface error: {e}"),
                };
                let view = frame.texture.create_view(&Default::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render"),
                        });

                let n = state.blocks.len() as f32;
                let win_w = state.config.width as f32;
                let win_h = state.config.height as f32;
                let col_w = (win_w - 2.0 * MARGIN - (n - 1.0) * GAP) / n;
                let tex_y = MARGIN + LABEL_HEIGHT + LABEL_GAP;
                let tex_h = (win_h - tex_y - MARGIN).max(1.0);

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("rp"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.08,
                                    g: 0.08,
                                    b: 0.08,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });

                    rpass.set_pipeline(&state.pipeline);

                    for (i, block) in state.blocks.iter().enumerate() {
                        let block_x = MARGIN + i as f32 * (col_w + GAP);

                        // Label: render at native aspect ratio, left-aligned
                        let label_scale = (LABEL_HEIGHT / block.label_tex_h as f32)
                            .min(col_w / block.label_tex_w as f32);
                        let label_w = block.label_tex_w as f32 * label_scale;
                        let label_h = block.label_tex_h as f32 * label_scale;
                        rpass.set_viewport(block_x, MARGIN, label_w, label_h, 0.0, 1.0);
                        rpass.set_bind_group(0, &block.label_bind_group, &[]);
                        rpass.draw(0..3, 0..1);

                        // Imported texture
                        rpass.set_viewport(block_x, tex_y, col_w, tex_h, 0.0, 1.0);
                        rpass.set_bind_group(0, &block.texture_bind_group, &[]);
                        rpass.draw(0..3, 0..1);
                    }
                }

                state.queue.submit(Some(encoder.finish()));
                frame.present();
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App { state: None };
    event_loop.run_app(&mut app).expect("run app");
}

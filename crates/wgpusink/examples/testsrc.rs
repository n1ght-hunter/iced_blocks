//! Renders a GStreamer `videotestsrc` test pattern in a winit + wgpu window.
//!
//! ```bash
//! cargo run -p wgpusink --example testsrc
//! ```

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;

use common::AppEvent;
use gst::prelude::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

struct App {
    state: Option<RenderState>,
    proxy: EventLoopProxy<AppEvent>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    frame_slot: wgpusink::FrameSlot,
    gst_pipeline: gst::Pipeline,
    current_bind_group: Option<wgpu::BindGroup>,
    current_frame: Option<wgpusink::WgpuFrame>,
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("wgpusink · testsrc"))
                .expect("create window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
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

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("testsrc"),
            ..Default::default()
        }))
        .expect("request device");

        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        surface.configure(&device, &config);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (render_pipeline, bind_group_layout) =
            common::create_render_pipeline(&device, config.format);

        let (frame_slot, gst_pipeline) =
            common::spawn_pipeline(device.clone(), queue.clone(), self.proxy.clone());

        self.state = Some(RenderState {
            window,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            bind_group_layout,
            sampler,
            frame_slot,
            gst_pipeline,
            current_bind_group: None,
            current_frame: None,
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::NewFrame => {
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
            }
        }
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
            WindowEvent::CloseRequested => {
                let _ = state.gst_pipeline.set_state(gst::State::Null);
                state.current_frame.take();
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Some(frame) = state.frame_slot.take() {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    state.current_bind_group =
                        Some(state.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("frame_bg"),
                            layout: &state.bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::Sampler(&state.sampler),
                                },
                            ],
                        }));
                    state.current_frame = Some(frame);
                }

                let Some(bind_group) = state.current_bind_group.as_ref() else {
                    return;
                };

                let output = match state.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Outdated) => return,
                    Err(e) => panic!("surface error: {e}"),
                };
                let output_view = output.texture.create_view(&Default::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render"),
                        });

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("render"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &output_view,
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

                    rpass.set_pipeline(&state.render_pipeline);
                    rpass.set_bind_group(0, bind_group, &[]);
                    rpass.draw(0..3, 0..1);
                }

                state.queue.submit(Some(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    gst::init().expect("GStreamer init");
    wgpusink::plugin_register_static().expect("register wgpusink plugin");

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .expect("create event loop");
    let proxy = event_loop.create_proxy();

    let mut app = App { state: None, proxy };
    event_loop.run_app(&mut app).expect("run app");
}

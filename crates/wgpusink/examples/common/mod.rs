use gst::prelude::*;
use winit::event_loop::EventLoopProxy;

pub const FULLSCREEN_TRIANGLE_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    let x = f32(i32(idx & 1u)) * 4.0 - 1.0;
    let y = f32(i32(idx >> 1u)) * 4.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4f(x, y, 0.0, 1.0);
    out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return textureSample(tex, samp, in.uv);
}
"#;

#[derive(Debug)]
pub enum AppEvent {
    NewFrame,
}

pub fn create_render_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fullscreen_triangle"),
        source: wgpu::ShaderSource::Wgsl(FULLSCREEN_TRIANGLE_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frame_bgl"),
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
        label: Some("frame_pl"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("frame_rp"),
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
                format: surface_format,
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

    (pipeline, bind_group_layout)
}

/// Builds a GStreamer pipeline with `videotestsrc` feeding into `wgpuvideosink`.
///
/// The `new-frame` signal wakes the winit event loop via the proxy.
/// Frames are pulled from the sink's internal `frame-slot` property.
pub fn spawn_pipeline(
    device: wgpu::Device,
    queue: wgpu::Queue,
    proxy: EventLoopProxy<AppEvent>,
) -> (wgpusink::FrameSlot, gst::Pipeline) {
    let sink_element = wgpusink::WgpuSinkBuilder::new(device, queue)
        .build()
        .expect("failed to create wgpuvideosink");

    let slot: wgpusink::FrameSlot = sink_element.property("frame-slot");

    let sink = sink_element
        .downcast_ref::<wgpusink::WgpuVideoSink>()
        .expect("element is WgpuVideoSink");
    sink.connect_new_frame(move |_| {
        let _ = proxy.send_event(AppEvent::NewFrame);
    });

    let pipeline = gst::Pipeline::new();

    let src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .build()
        .expect("videotestsrc");

    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .expect("videoconvert");

    pipeline
        .add_many([&src, &convert, &sink_element])
        .expect("add elements");
    gst::Element::link_many([&src, &convert, &sink_element]).expect("link elements");

    pipeline
        .set_state(gst::State::Playing)
        .expect("set pipeline to Playing");

    (slot, pipeline)
}

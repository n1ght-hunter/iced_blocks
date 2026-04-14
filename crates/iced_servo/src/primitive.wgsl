// Fullscreen textured quad. Two triangles, six vertices, all derived from
// vertex_index — no vertex buffer needed. The render pass viewport is set
// by iced to the widget's bounds, so the quad fills exactly the widget.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    // wgpu NDC has +Y up; texture UVs have +Y down. Flip V so the
    // bottom-left vertex (NDC -1,-1) samples the bottom-left of the
    // texture (UV 0,1).
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(tex, samp, in.uv);
    // Servo paints in RGBA; wgpu surface is usually Bgra8Unorm. The iced
    // shader widget expects us to output the final color in the target's
    // format space — we leave the sample as-is and let the target format
    // conversion happen at store time. If colors look swapped on some
    // platforms, flip to `color.bgra`.
    return color;
}

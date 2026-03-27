struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(0) var skin_texture: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.normal = input.normal;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(skin_texture, skin_sampler, input.uv);

    if (tex_color.a < 0.5) {
        discard;
    }

    let sun_dir = normalize(vec3<f32>(0.2, 1.0, 0.6));
    let ambient = 0.55;
    let diffuse = max(dot(normalize(input.normal), sun_dir), 0.0) * 0.45;
    let light = ambient + diffuse;

    return vec4<f32>(tex_color.rgb * light, tex_color.a);
}

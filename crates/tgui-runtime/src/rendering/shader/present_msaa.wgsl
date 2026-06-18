struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local_position: vec2<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) corner_radius: f32,
    @location(5) clip_local_position: vec2<f32>,
    @location(6) clip_rect_size: vec2<f32>,
    @location(7) clip_corner_radius: f32,
    @location(8) clip_enabled: f32,
    @location(9) opacity: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var scene_texture: texture_multisampled_2d<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texture_size = vec2<f32>(textureDimensions(scene_texture));
    let pixel = vec2<i32>(clamp(input.uv * texture_size, vec2<f32>(0.0), texture_size - vec2<f32>(1.0)));
    var color = vec4<f32>(0.0);
    let samples = textureNumSamples(scene_texture);
    for (var sample = 0u; sample < samples; sample = sample + 1u) {
        color = color + textureLoad(scene_texture, pixel, i32(sample));
    }
    return color / f32(max(samples, 1u));
}

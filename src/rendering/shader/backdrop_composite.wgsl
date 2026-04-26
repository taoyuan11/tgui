struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) local_position: vec2<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) corner_radius: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
};

@group(0) @binding(0) var blurred_texture: texture_2d<f32>;
@group(0) @binding(1) var original_texture: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

fn rounded_box_sdf(local_position: vec2<f32>, rect_size: vec2<f32>, radius: f32) -> f32 {
    let half_size = rect_size * 0.5;
    let center_relative = local_position - half_size;
    let inner_half = max(half_size - vec2<f32>(radius, radius), vec2<f32>(0.0, 0.0));
    let delta = abs(center_relative) - inner_half;
    let outside = length(max(delta, vec2<f32>(0.0, 0.0)));
    let inside = min(max(delta.x, delta.y), 0.0);
    return outside + inside - radius;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_box_sdf(input.local_position, input.rect_size, input.corner_radius);
    let mask = clamp(0.5 - distance, 0.0, 1.0);
    if mask <= 0.0 {
        discard;
    }

    let blurred = textureSample(blurred_texture, source_sampler, input.uv);
    let original = textureSample(original_texture, source_sampler, input.uv);
    return mix(original, blurred, mask);
}

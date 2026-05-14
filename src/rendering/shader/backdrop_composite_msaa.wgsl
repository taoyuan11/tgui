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
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) clip_local_position: vec2<f32>,
    @location(5) clip_rect_size: vec2<f32>,
    @location(6) clip_corner_radius: f32,
    @location(7) clip_enabled: f32,
};

@group(0) @binding(0) var blurred_texture: texture_multisampled_2d<f32>;
@group(0) @binding(1) var original_texture: texture_multisampled_2d<f32>;

fn rounded_box_sdf(local_position: vec2<f32>, rect_size: vec2<f32>, radius: f32) -> f32 {
    let half_size = rect_size * 0.5;
    let center_relative = local_position - half_size;
    let inner_half = max(half_size - vec2<f32>(radius, radius), vec2<f32>(0.0, 0.0));
    let delta = abs(center_relative) - inner_half;
    let outside = length(max(delta, vec2<f32>(0.0, 0.0)));
    let inside = min(max(delta.x, delta.y), 0.0);
    return outside + inside - radius;
}

fn clip_mask_alpha(
    local_position: vec2<f32>,
    rect_size: vec2<f32>,
    radius: f32,
    enabled: f32,
) -> f32 {
    if enabled < 0.5 {
        return 1.0;
    }

    let distance = rounded_box_sdf(local_position, rect_size, radius);
    return clamp(0.5 - distance, 0.0, 1.0);
}

fn load_msaa_average(texture: texture_multisampled_2d<f32>, pixel: vec2<i32>) -> vec4<f32> {
    var color = vec4<f32>(0.0);
    let samples = textureNumSamples(texture);
    for (var sample = 0u; sample < samples; sample = sample + 1u) {
        color = color + textureLoad(texture, pixel, i32(sample));
    }
    return color / f32(max(samples, 1u));
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    output.clip_local_position = input.clip_local_position;
    output.clip_rect_size = input.clip_rect_size;
    output.clip_corner_radius = input.clip_corner_radius;
    output.clip_enabled = input.clip_enabled;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_box_sdf(input.local_position, input.rect_size, input.corner_radius);
    let mask = clamp(0.5 - distance, 0.0, 1.0);
    let clip_alpha = clip_mask_alpha(
        input.clip_local_position,
        input.clip_rect_size,
        input.clip_corner_radius,
        input.clip_enabled,
    );
    let combined_mask = mask * clip_alpha;
    if combined_mask <= 0.0 {
        discard;
    }

    let texture_size = vec2<f32>(textureDimensions(blurred_texture));
    let pixel = vec2<i32>(clamp(input.uv * texture_size, vec2<f32>(0.0), texture_size - vec2<f32>(1.0)));
    let blurred = load_msaa_average(blurred_texture, pixel);
    let original = load_msaa_average(original_texture, pixel);
    return mix(original, blurred, mask) * combined_mask;
}

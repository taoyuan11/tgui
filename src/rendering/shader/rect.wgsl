struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local_position: vec2<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) corner_radius: f32,
    @location(5) stroke_width: f32,
    @location(6) clip_local_position: vec2<f32>,
    @location(7) clip_rect_size: vec2<f32>,
    @location(8) clip_corner_radius: f32,
    @location(9) clip_enabled: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) stroke_width: f32,
    @location(5) clip_local_position: vec2<f32>,
    @location(6) clip_rect_size: vec2<f32>,
    @location(7) clip_corner_radius: f32,
    @location(8) clip_enabled: f32,
};

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

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    output.stroke_width = input.stroke_width;
    output.clip_local_position = input.clip_local_position;
    output.clip_rect_size = input.clip_rect_size;
    output.clip_corner_radius = input.clip_corner_radius;
    output.clip_enabled = input.clip_enabled;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_box_sdf(
        input.local_position,
        input.rect_size,
        input.corner_radius,
    );
    let outer_alpha = clamp(0.5 - distance, 0.0, 1.0);

    var alpha = outer_alpha;
    if input.stroke_width > 0.0 {
        let inner_alpha = clamp(0.5 + distance + input.stroke_width, 0.0, 1.0);
        alpha = outer_alpha * inner_alpha;
    }

    let clip_alpha = clip_mask_alpha(
        input.clip_local_position,
        input.clip_rect_size,
        input.clip_corner_radius,
        input.clip_enabled,
    );

    return vec4<f32>(input.color.rgb, input.color.a * alpha * clip_alpha);
}

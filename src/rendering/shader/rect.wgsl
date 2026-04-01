struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local_position: vec2<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) corner_radius: f32,
    @location(5) stroke_width: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) stroke_width: f32,
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

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    output.stroke_width = input.stroke_width;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let outer_distance = rounded_box_sdf(
        input.local_position,
        input.rect_size,
        input.corner_radius,
    );
    let aa = max(fwidth(outer_distance), 1.0);
    let outer_alpha = 1.0 - smoothstep(-aa, aa, outer_distance);

    var alpha = outer_alpha;
    if input.stroke_width > 0.0 {
        let inner_size = max(
            input.rect_size - vec2<f32>(input.stroke_width * 2.0, input.stroke_width * 2.0),
            vec2<f32>(0.0, 0.0),
        );
        if inner_size.x > 0.0 && inner_size.y > 0.0 {
            let inner_local = input.local_position - vec2<f32>(input.stroke_width, input.stroke_width);
            let inner_radius = max(input.corner_radius - input.stroke_width, 0.0);
            let inner_distance = rounded_box_sdf(inner_local, inner_size, inner_radius);
            let inner_aa = max(fwidth(inner_distance), 1.0);
            let inner_alpha = smoothstep(-inner_aa, inner_aa, inner_distance);
            alpha = outer_alpha * inner_alpha;
        }
    }

    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}

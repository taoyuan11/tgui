struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) brush_meta: vec4<f32>,
    @location(5) gradient_data0: vec4<f32>,
    @location(6) gradient_data1: vec4<f32>,
    @location(7) stop_offsets0: vec4<f32>,
    @location(8) stop_offsets1: vec4<f32>,
    @location(9) stop_color0: vec4<f32>,
    @location(10) stop_color1: vec4<f32>,
    @location(11) stop_color2: vec4<f32>,
    @location(12) stop_color3: vec4<f32>,
    @location(13) stop_color4: vec4<f32>,
    @location(14) stop_color5: vec4<f32>,
    @location(15) stop_color6: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) rect_size: vec2<f32>,
    @location(2) corner_radius: f32,
    @location(3) brush_meta: vec4<f32>,
    @location(4) gradient_data0: vec4<f32>,
    @location(5) gradient_data1: vec4<f32>,
    @location(6) stop_offsets0: vec4<f32>,
    @location(7) stop_offsets1: vec4<f32>,
    @location(8) stop_color0: vec4<f32>,
    @location(9) stop_color1: vec4<f32>,
    @location(10) stop_color2: vec4<f32>,
    @location(11) stop_color3: vec4<f32>,
    @location(12) stop_color4: vec4<f32>,
    @location(13) stop_color5: vec4<f32>,
    @location(14) stop_color6: vec4<f32>,
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

fn stop_offset(input: VertexOutput, index: i32) -> f32 {
    switch index {
        case 0: { return input.stop_offsets0.x; }
        case 1: { return input.stop_offsets0.y; }
        case 2: { return input.stop_offsets0.z; }
        case 3: { return input.stop_offsets0.w; }
        case 4: { return input.stop_offsets1.x; }
        case 5: { return input.stop_offsets1.y; }
        case 6: { return input.stop_offsets1.z; }
        default: { return input.stop_offsets1.z; }
    }
}

fn stop_color(input: VertexOutput, index: i32) -> vec4<f32> {
    switch index {
        case 0: { return input.stop_color0; }
        case 1: { return input.stop_color1; }
        case 2: { return input.stop_color2; }
        case 3: { return input.stop_color3; }
        case 4: { return input.stop_color4; }
        case 5: { return input.stop_color5; }
        default: { return input.stop_color6; }
    }
}

fn gradient_color(input: VertexOutput, t_raw: f32) -> vec4<f32> {
    let count = i32(input.brush_meta.y);
    if count <= 0 {
        return vec4<f32>(0.0);
    }

    let t = clamp(t_raw, 0.0, 1.0);
    var previous_offset = stop_offset(input, 0);
    var previous_color = stop_color(input, 0);

    if count == 1 || t <= previous_offset {
        return previous_color;
    }

    for (var index = 1; index < 7; index = index + 1) {
        if index >= count {
            break;
        }

        let next_offset = stop_offset(input, index);
        let next_color = stop_color(input, index);
        if t <= next_offset {
            let span = max(next_offset - previous_offset, 0.0001);
            let local_t = clamp((t - previous_offset) / span, 0.0, 1.0);
            return mix(previous_color, next_color, local_t);
        }

        previous_offset = next_offset;
        previous_color = next_color;
    }

    return previous_color;
}

fn brush_color(input: VertexOutput) -> vec4<f32> {
    let brush_kind = i32(input.brush_meta.x);
    if brush_kind == 0 {
        return input.stop_color0;
    }

    if brush_kind == 1 {
        let start = input.gradient_data0.xy;
        let end = input.gradient_data0.zw;
        let axis = end - start;
        let axis_length_sq = max(dot(axis, axis), 0.0001);
        let t = dot(input.local_position - start, axis) / axis_length_sq;
        return gradient_color(input, t);
    }

    let center = input.gradient_data1.xy;
    let radius = max(input.gradient_data1.z, 0.0001);
    let t = distance(input.local_position, center) / radius;
    return gradient_color(input, t);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    output.brush_meta = input.brush_meta;
    output.gradient_data0 = input.gradient_data0;
    output.gradient_data1 = input.gradient_data1;
    output.stop_offsets0 = input.stop_offsets0;
    output.stop_offsets1 = input.stop_offsets1;
    output.stop_color0 = input.stop_color0;
    output.stop_color1 = input.stop_color1;
    output.stop_color2 = input.stop_color2;
    output.stop_color3 = input.stop_color3;
    output.stop_color4 = input.stop_color4;
    output.stop_color5 = input.stop_color5;
    output.stop_color6 = input.stop_color6;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_box_sdf(input.local_position, input.rect_size, input.corner_radius);
    let alpha = clamp(0.5 - distance, 0.0, 1.0);
    let color = brush_color(input);
    return vec4<f32>(color.rgb, color.a * alpha);
}

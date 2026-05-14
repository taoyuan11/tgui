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

struct CompositeParams {
    data0: vec4<f32>,
    data1: vec4<f32>,
    data2: vec4<f32>,
    data3: vec4<f32>,
};

@group(0) @binding(0) var content_texture: texture_multisampled_2d<f32>;
@group(0) @binding(1) var scene_texture: texture_multisampled_2d<f32>;
@group(0) @binding(2) var mask_texture: texture_multisampled_2d<f32>;
@group(0) @binding(3) var<uniform> params: CompositeParams;

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

fn blend_channel(src: f32, dst: f32, mode: i32) -> f32 {
    switch mode {
        case 1: { return src * dst; }
        case 2: { return src + dst - src * dst; }
        case 3: {
            return select(2.0 * src * dst, 1.0 - 2.0 * (1.0 - src) * (1.0 - dst), dst > 0.5);
        }
        case 4: { return min(src, dst); }
        case 5: { return max(src, dst); }
        case 6: { return select(0.0, min(1.0, dst / max(1.0 - src, 0.0001)), src < 1.0); }
        case 7: { return select(1.0, 1.0 - min(1.0, (1.0 - dst) / max(src, 0.0001)), src > 0.0); }
        case 8: {
            return select(2.0 * src * dst, 1.0 - 2.0 * (1.0 - src) * (1.0 - dst), src > 0.5);
        }
        case 9: {
            return (1.0 - 2.0 * src) * dst * dst + 2.0 * src * dst;
        }
        case 10: { return abs(dst - src); }
        case 11: { return dst + src - 2.0 * dst * src; }
        case 12: { return min(1.0, src + dst); }
        default: { return src; }
    }
}

fn composite(src: vec4<f32>, dst: vec4<f32>, mode: i32) -> vec4<f32> {
    let src_alpha = clamp(src.a, 0.0, 1.0);
    let dst_alpha = clamp(dst.a, 0.0, 1.0);
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0001 {
        return vec4<f32>(0.0);
    }

    var blended = vec3<f32>(0.0);
    for (var i = 0; i < 3; i = i + 1) {
        blended[i] = blend_channel(src[i], dst[i], mode);
    }
    let color = (
        (1.0 - dst_alpha) * src.rgb * src_alpha +
        (1.0 - src_alpha) * dst.rgb * dst_alpha +
        blended * src_alpha * dst_alpha
    ) / out_alpha;
    return vec4<f32>(color, out_alpha);
}

fn color_filter(color: vec4<f32>) -> vec4<f32> {
    let multiply = params.data1;
    let add = params.data2;
    return clamp(color * multiply + add, vec4<f32>(0.0), vec4<f32>(1.0));
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
    let shape_alpha = clamp(0.5 - distance, 0.0, 1.0);
    let clip_alpha = clip_mask_alpha(
        input.clip_local_position,
        input.clip_rect_size,
        input.clip_corner_radius,
        input.clip_enabled,
    );
    let combined_alpha = shape_alpha * clip_alpha;
    if combined_alpha <= 0.0 {
        discard;
    }

    let opacity = params.data0.x;
    let blend_mode = i32(params.data0.y);
    let has_mask = params.data0.z;
    let texture_size = vec2<f32>(textureDimensions(content_texture));
    let pixel = vec2<i32>(clamp(input.uv * texture_size, vec2<f32>(0.0), texture_size - vec2<f32>(1.0)));
    let content = color_filter(load_msaa_average(content_texture, pixel));
    let scene = load_msaa_average(scene_texture, pixel);
    let mask_alpha = select(1.0, load_msaa_average(mask_texture, pixel).a, has_mask > 0.5);
    let src = vec4<f32>(content.rgb, content.a * opacity * mask_alpha * combined_alpha);
    return composite(src, scene, blend_mode);
}

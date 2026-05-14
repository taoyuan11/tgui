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
    data4: vec4<f32>,
};

@group(0) @binding(0) var content_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_texture: texture_2d<f32>;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var source_sampler: sampler;
@group(0) @binding(4) var<uniform> params: CompositeParams;

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

fn sample_box_blur(uv: vec2<f32>, radius: f32) -> vec4<f32> {
    if radius <= 0.5 {
        return textureSample(content_texture, source_sampler, uv);
    }

    let size = vec2<f32>(textureDimensions(content_texture));
    let texel = vec2<f32>(1.0) / max(size, vec2<f32>(1.0));
    let r = i32(clamp(radius, 1.0, 8.0));
    var sum = vec4<f32>(0.0);
    var count = 0.0;
    for (var y = -r; y <= r; y = y + 1) {
        for (var x = -r; x <= r; x = x + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            sum = sum + textureSample(content_texture, source_sampler, uv + offset);
            count = count + 1.0;
        }
    }
    return sum / max(count, 1.0);
}

fn sample_box_blur_alpha(uv: vec2<f32>, radius: f32, uv_offset: vec2<f32>) -> f32 {
    if radius <= 0.5 {
        return textureSample(content_texture, source_sampler, uv + uv_offset).a;
    }

    let size = vec2<f32>(textureDimensions(content_texture));
    let texel = vec2<f32>(1.0) / max(size, vec2<f32>(1.0));
    let r = i32(clamp(radius, 1.0, 8.0));
    var sum = 0.0;
    var count = 0.0;
    for (var y = -r; y <= r; y = y + 1) {
        for (var x = -r; x <= r; x = x + 1) {
            let offset = uv_offset + vec2<f32>(f32(x), f32(y)) * texel;
            sum = sum + textureSample(content_texture, source_sampler, uv + offset).a;
            count = count + 1.0;
        }
    }
    return sum / max(count, 1.0);
}

fn apply_inner_shadow(content: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let enabled = params.data4.w;
    if enabled <= 0.5 {
        return content;
    }

    let texture_size = vec2<f32>(textureDimensions(content_texture));
    let uv_offset = vec2<f32>(
        params.data4.x / max(texture_size.x, 1.0),
        params.data4.y / max(texture_size.y, 1.0),
    );
    let shifted_alpha = sample_box_blur_alpha(uv, params.data4.z, uv_offset);
    let shadow_alpha = clamp(content.a * (1.0 - shifted_alpha) * params.data3.a, 0.0, 1.0);
    let shadow = vec4<f32>(params.data3.rgb, shadow_alpha);
    return composite(shadow, content, 0);
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
    let blur_radius = params.data0.w;
    let content = apply_inner_shadow(color_filter(sample_box_blur(input.uv, blur_radius)), input.uv);
    let scene = textureSample(scene_texture, source_sampler, input.uv);
    let mask_alpha = select(1.0, textureSample(mask_texture, source_sampler, input.uv).a, has_mask > 0.5);
    let src = vec4<f32>(content.rgb, content.a * opacity * mask_alpha * combined_alpha);
    return composite(src, scene, blend_mode);
}

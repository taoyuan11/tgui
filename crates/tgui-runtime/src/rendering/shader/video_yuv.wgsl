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
    @location(1) local_position: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) clip_local_position: vec2<f32>,
    @location(5) clip_rect_size: vec2<f32>,
    @location(6) clip_corner_radius: f32,
    @location(7) clip_enabled: f32,
    @location(8) opacity: f32,
};

struct YuvUniform {
    format_matrix_range: vec4<f32>,
};

@group(0) @binding(0) var y_texture: texture_2d<f32>;
@group(0) @binding(1) var u_or_uv_texture: texture_2d<f32>;
@group(0) @binding(2) var v_texture: texture_2d<f32>;
@group(0) @binding(3) var yuv_sampler: sampler;
@group(0) @binding(4) var<uniform> yuv: YuvUniform;

fn rounded_box_sdf(local_position: vec2<f32>, rect_size: vec2<f32>, radius: f32) -> f32 {
    let half_size = rect_size * 0.5;
    let center_relative = local_position - half_size;
    let inner_half = max(half_size - vec2<f32>(radius, radius), vec2<f32>(0.0, 0.0));
    let delta = abs(center_relative) - inner_half;
    let outside = length(max(delta, vec2<f32>(0.0, 0.0)));
    let inside = min(max(delta.x, delta.y), 0.0);
    return outside + inside - radius;
}

struct PushTranslate {
    offset_ndc: vec2<f32>,
    offset_physical: vec2<f32>,
}

const pc: PushTranslate = PushTranslate(vec2<f32>(0.0), vec2<f32>(0.0));

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
    output.position = vec4<f32>(input.position + pc.offset_ndc, 0.0, 1.0);
    output.clip_local_position = input.clip_local_position + pc.offset_physical;
    output.uv = input.uv;
    output.local_position = input.local_position;
    output.rect_size = input.rect_size;
    output.corner_radius = input.corner_radius;
    output.clip_rect_size = input.clip_rect_size;
    output.clip_corner_radius = input.clip_corner_radius;
    output.clip_enabled = input.clip_enabled;
    output.opacity = input.opacity;
    return output;
}

fn yuv_to_rgb_bt601(y: f32, u: f32, v: f32) -> vec3<f32> {
    return vec3<f32>(
        y + 1.402 * v,
        y - 0.344136 * u - 0.714136 * v,
        y + 1.772 * u,
    );
}

fn yuv_to_rgb_bt709(y: f32, u: f32, v: f32) -> vec3<f32> {
    return vec3<f32>(
        y + 1.5748 * v,
        y - 0.187324 * u - 0.468124 * v,
        y + 1.8556 * u,
    );
}

fn yuv_to_rgb_bt2020(y: f32, u: f32, v: f32) -> vec3<f32> {
    return vec3<f32>(
        y + 1.4746 * v,
        y - 0.164553 * u - 0.571353 * v,
        y + 1.8814 * u,
    );
}

fn sampled_yuv(uv: vec2<f32>) -> vec3<f32> {
    let y_sample = textureSample(y_texture, yuv_sampler, uv).r;
    let format = yuv.format_matrix_range.x;
    let matrix = yuv.format_matrix_range.y;
    let range = yuv.format_matrix_range.z;

    var u = 0.0;
    var v = 0.0;
    if format < 0.5 {
        let uv_sample = textureSample(u_or_uv_texture, yuv_sampler, uv).rg;
        u = uv_sample.r;
        v = uv_sample.g;
    } else {
        u = textureSample(u_or_uv_texture, yuv_sampler, uv).r;
        v = textureSample(v_texture, yuv_sampler, uv).r;
    }

    var y_value = y_sample;
    if range < 0.5 {
        y_value = clamp((y_sample - (16.0 / 255.0)) * (255.0 / 219.0), 0.0, 1.0);
        u = (u - 0.5) * (255.0 / 224.0);
        v = (v - 0.5) * (255.0 / 224.0);
    } else {
        u = u - 0.5;
        v = v - 0.5;
    }

    if matrix < 0.5 {
        return yuv_to_rgb_bt601(y_value, u, v);
    }
    if matrix < 1.5 {
        return yuv_to_rgb_bt709(y_value, u, v);
    }
    return yuv_to_rgb_bt2020(y_value, u, v);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var alpha = 1.0;
    if (input.corner_radius > 0.0) {
        let radius = min(input.corner_radius, min(input.rect_size.x, input.rect_size.y) * 0.5);
        let dist = rounded_box_sdf(input.local_position, input.rect_size, radius);
        alpha = clamp(0.5 - dist, 0.0, 1.0);
    }

    let clip_alpha = clip_mask_alpha(
        input.clip_local_position,
        input.clip_rect_size,
        input.clip_corner_radius,
        input.clip_enabled,
    );
    let combined_alpha = alpha * clip_alpha;
    if (combined_alpha <= 0.0) {
        discard;
    }

    let rgb = clamp(sampled_yuv(input.uv), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, combined_alpha * input.opacity);
}

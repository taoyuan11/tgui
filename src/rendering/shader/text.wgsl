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

@group(0) @binding(0) var text_texture: texture_2d<f32>;
@group(0) @binding(1) var text_sampler: sampler;

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
    let sampled = textureSample(text_texture, text_sampler, input.uv);
    if (input.corner_radius <= 0.0) {
        return sampled;
    }

    let radius = min(input.corner_radius, min(input.rect_size.x, input.rect_size.y) * 0.5);
    let q = abs(input.local_position - input.rect_size * 0.5) - (input.rect_size * 0.5 - vec2<f32>(radius, radius));
    let dist = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
    if (dist > 0.0) {
        discard;
    }
    return sampled;
}

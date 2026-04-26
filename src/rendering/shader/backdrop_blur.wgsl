struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct BlurParams {
    direction: vec2<f32>,
    texel_size: vec2<f32>,
    radius: f32,
    _pad: f32,
};

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> blur_params: BlurParams;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let direction = blur_params.direction * blur_params.texel_size;
    let radius = clamp(blur_params.radius, 0.0, 24.0);
    let sigma = max(radius * 0.5, 0.5);

    var color = textureSample(source_texture, source_sampler, input.uv);
    var weight_sum = 1.0;

    for (var index = 1; index <= 12; index = index + 1) {
        let sample_distance = f32(index);
        if sample_distance > radius {
            break;
        }

        let weight = exp(-0.5 * pow(sample_distance / sigma, 2.0));
        let offset = direction * sample_distance;
        color = color + textureSample(source_texture, source_sampler, input.uv + offset) * weight;
        color = color + textureSample(source_texture, source_sampler, input.uv - offset) * weight;
        weight_sum = weight_sum + weight * 2.0;
    }

    return color / max(weight_sum, 0.0001);
}

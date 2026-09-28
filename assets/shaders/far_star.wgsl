#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::view

struct FarStarMaterial {
    min_radius: f32,
    angular_factor: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(2) @binding(0) var<uniform> material: FarStarMaterial;
@group(2) @binding(1) var base_texture: texture_2d<f32>;
@group(2) @binding(2) var base_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let model = get_world_from_local(v.instance_index);
    let world_center = (model * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
    let base_scale = length(model[0].xyz);

    let cam_pos = view.world_position;
    let to_cam = cam_pos - world_center;
    let dist = length(to_cam);

    let angular_scale = dist * material.angular_factor;
    let scale = max(angular_scale, base_scale);

    let fwd = to_cam / max(dist, 0.001);
    var up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(up, fwd));
    up = cross(fwd, right);

    let offset = right * v.position.x * scale + up * v.position.y * scale;
    let world_pos = world_center + offset;

    out.clip_position = view.clip_from_world * vec4<f32>(world_pos, 1.0);
    out.uv = v.uv;
    return out;
}

struct FragmentInput {
    @location(0) uv: vec2<f32>,
};

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    let tex = textureSample(base_texture, base_sampler, in.uv);
    return vec4<f32>(tex.rgb * 3.0, tex.a);
}

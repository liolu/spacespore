// Dôme du ciel (0.13 C2) : une sphère centrée sur la caméra, collée au fond de la profondeur (z = 0,
// profondeur inversée) : elle ne cache que le vide. La couleur du ciel y est dessinée, plus les
// étoiles de la cubemap, que le jour efface (`stars.x`).

#import bevy_pbr::{
    mesh_functions,
    forward_io::Vertex,
    mesh_view_bindings::view,
}

struct Sky {
    // rgb : couleur du ciel
    sky: vec4<f32>,
    // x : part des étoiles visibles, y : éclat
    stars: vec4<f32>,
};

@group(2) @binding(0) var sky_tex: texture_cube<f32>;
@group(2) @binding(1) var sky_sampler: sampler;
@group(2) @binding(2) var<uniform> sky: Sky;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) dir: vec3<f32>,
};

@vertex
fn vertex(v: Vertex) -> VOut {
    var out: VOut;
    // Sans passer par la position du monde : loin de l'origine (1e8) un f32 n'a plus la precision pour
    // une sphere de 50, et le ciel tremblait. On ne garde que la rotation de la camera.
    let view_pos = (view.view_from_world * vec4<f32>(v.position, 0.0)).xyz;
    let clip = view.clip_from_view * vec4<f32>(view_pos, 1.0);
    // Tout au fond : la profondeur inversée vaut 0 au plan lointain
    out.position = vec4<f32>(clip.xy, 0.0, clip.w);
    // La sphère est centrée sur la caméra et ne tourne pas : sa position locale est la direction du monde
    out.dir = normalize(v.position);
    return out;
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let stars = textureSample(sky_tex, sky_sampler, normalize(in.dir)).rgb;
    return vec4<f32>(sky.sky.rgb + stars * sky.stars.x * sky.stars.y, 1.0);
}

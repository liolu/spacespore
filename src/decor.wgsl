// Décor vivant (0.14 D3) : les plantes ondulent au vent (sommets déplacés selon leur hauteur dans la
// plante, alpha du sommet = hauteur 0..1, +0,5 pour le feuillage caduc), le feuillage caduc prend la
// couleur de la saison, les plantes lumineuses luisent la nuit, les arbres pétrifiés sont gris.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    view_transformations::position_world_to_clip,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct Sway {
    // xyz : déplacement du sommet des plantes (unités, repère monde), w : temps (s)
    wind: vec4<f32>,
    // rgb : couleur du feuillage de saison, a : part (0 : vert, 1 : tout teinté)
    tint: vec4<f32>,
    // x : nuit (0..1), y : lueur des plantes (0 ou 1), z : pétrifié (0 ou 1), w : rafale (0..1)
    night: vec4<f32>,
};

@group(2) @binding(100) var<uniform> sway: Sway;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
#ifdef VERTEX_COLORS
    let a = vertex.color.a;
    let leaf = select(0.0, 1.0, a >= 0.5);
    let height = (a - 0.5 * leaf) * 2.0;
    // Chaque plante a sa phase (sa position) : la forêt ne bouge pas d'un seul bloc
    let origin = world_from_local[3].xyz;
    let phase = dot(origin, vec3<f32>(0.37, 0.53, 0.71));
    let t = sway.wind.w;
    let s = 0.55 + 0.45 * sin(t * 1.9 + phase) + 0.3 * sin(t * 4.3 + phase * 1.7) * sway.night.w;
    world_position = vec4<f32>(world_position.xyz + sway.wind.xyz * (height * height) * s, 1.0);
    out.color = vertex.color;
#endif
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
    out.instance_index = vertex.instance_index;
    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
#ifdef VERTEX_COLORS
    let leaf = select(0.0, 1.0, in.color.a >= 0.5);
#else
    let leaf = 0.0;
#endif
    var base = pbr_input.material.base_color.rgb;
    // Feuillage de saison (caduc seulement)
    base = mix(base, sway.tint.rgb, sway.tint.a * leaf);
    // Pétrifié : la pierre garde la forme, pas la couleur
    let lum = dot(base, vec3<f32>(0.3, 0.59, 0.11));
    base = mix(base, vec3<f32>(lum * 1.05, lum, lum * 0.95), sway.night.z);
    pbr_input.material.base_color = vec4<f32>(base, 1.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Lueur la nuit des plantes lumineuses
    out.color = vec4<f32>(out.color.rgb + base * sway.night.x * sway.night.y * 1.6, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}

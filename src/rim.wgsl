// Liseré de l'atmosphère (0.13 P8) : une coque un peu plus grande que l'astre, qui brille là où on la
// voit de biais (le bord du disque), de la couleur du ciel, côté jour surtout.

#import bevy_pbr::{
    mesh_view_bindings::view,
    forward_io::VertexOutput,
}

struct Rim {
    // rgb : couleur, w : force (0..1)
    color: vec4<f32>,
    // xyz : direction du soleil (monde), w : épaisseur de la coque / rayon de l'astre
    sun: vec4<f32>,
    // xyz : centre de l'astre (monde), w : rayon de la coque
    center: vec4<f32>,
};

@group(2) @binding(0) var<uniform> rim: Rim;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position.xyz - in.world_position.xyz);
    let f = clamp(dot(n, v), 0.0, 1.0);
    // Au bord du disque la coque est vue presque de profil
    let edge = pow(1.0 - f, 2.5);
    // Plus lumineux du côté du soleil, et un peu autour du terminateur
    let lit = smoothstep(-0.25, 0.5, dot(n, rim.sun.xyz));
    let a = edge * lit * rim.color.w;
    return vec4<f32>(rim.color.rgb * (0.6 + 0.8 * edge), a);
}

// Sol des tuiles (0.13 O2) : le matériau standard, plus la lumière du fond marin. Les sommets du
// fond sous l'eau ont l'alpha 0,5 (`terrain::submerged`). Sous le niveau de la mer : la lumière du
// soleil est absorbée avec la profondeur (le rouge d'abord) et des caustiques dansent sur le fond.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct Caustics {
    // lignes de la matrice monde -> astre (xyz) et centre de l'astre (w)
    to0: vec4<f32>,
    to1: vec4<f32>,
    to2: vec4<f32>,
    // x : rayon de la mer (0 : pas d'eau), y : lumière du jour (0..1), z : temps (s), w : colonne (unités)
    sea: vec4<f32>,
    // xyz : direction du soleil (repère de l'astre), w : force des caustiques
    sun: vec4<f32>,
    // absorption par colonne de profondeur (rgb)
    absorb: vec4<f32>,
};

@group(2) @binding(100) var<uniform> caustics: Caustics;

const TAU: f32 = 6.28318530718;

// Caustiques : motif de lignes brillantes qui bougent (réseau de sinus déformés)
fn caustic(uv: vec2<f32>, time: f32) -> f32 {
    let p = uv * TAU - 250.0;
    var i = p;
    var c = 1.0;
    let inten = 0.005;
    for (var n = 0; n < 4; n++) {
        let t = time * (1.0 - (3.5 / f32(n + 1)));
        i = p + vec2<f32>(cos(t - i.x) + sin(t + i.y), sin(t - i.y) + cos(t + i.x));
        c += 1.0 / length(vec2<f32>(p.x / (sin(i.x + t) / inten), p.y / (cos(i.y + t) / inten)));
    }
    c /= 4.0;
    c = 1.17 - pow(c, 1.4);
    return pow(abs(c), 8.0);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);

    // Fond marin (alpha de sommet 0,5) sous le niveau de la mer
#ifdef VERTEX_COLORS
    let under = in.color.a < 0.75;
#else
    let under = false;
#endif
    if (under && caustics.sea.x > 0.0) {
        let rel = in.world_position.xyz - vec3<f32>(caustics.to0.w, caustics.to1.w, caustics.to2.w);
        let lp = vec3<f32>(dot(caustics.to0.xyz, rel), dot(caustics.to1.xyz, rel), dot(caustics.to2.xyz, rel));
        let cell = max(caustics.sea.w, 1e-4);
        let depth = (caustics.sea.x - length(lp)) / cell;
        if (depth > 0.0) {
            let t = exp(-caustics.absorb.rgb * depth);
            let sun = caustics.sun.xyz;
            let n = pbr_input.N;
            let nl = vec3<f32>(dot(caustics.to0.xyz, n), dot(caustics.to1.xyz, n), dot(caustics.to2.xyz, n));
            let facing = clamp(dot(nl, sun), 0.0, 1.0);
            // Axes du plan perpendiculaire aux rayons du soleil
            var a = cross(sun, vec3<f32>(0.0, 1.0, 0.0));
            if (length(a) < 0.1) {
                a = cross(sun, vec3<f32>(1.0, 0.0, 0.0));
            }
            a = normalize(a);
            let b = cross(sun, a);
            let uv = vec2<f32>(dot(lp, a), dot(lp, b)) / (cell * 9.0);
            let shallow = smoothstep(0.0, 1.5, depth) * exp(-depth * 0.035);
            let light = caustic(uv, caustics.sea.z * 0.9) * shallow * facing * caustics.sea.y * caustics.sun.w;
            out.color = vec4<f32>(out.color.rgb * t + pbr_input.material.base_color.rgb * t * light * 2.2, out.color.a);
        }
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}

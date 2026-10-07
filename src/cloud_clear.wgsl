// Nuages (0.13 P4) : les dalles de nuages s'effacent (tramage) autour du vaisseau et le long de son
// sillage, qui se referme ; le reste est le matériau standard.

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

struct Clear {
    // xyz : position (monde), w : rayon de la bulle libre (0 : inutilisée)
    slots: array<vec4<f32>, 8>,
};

@group(2) @binding(100) var<uniform> clear: Clear;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Plus près d'un trou, moins le nuage est dessiné (tramage)
    var keep = 1.0;
    for (var i = 0; i < 8; i++) {
        let s = clear.slots[i];
        if (s.w > 0.0) {
            let d = distance(in.world_position.xyz, s.xyz);
            keep = min(keep, smoothstep(s.w * 0.72, s.w, d));
        }
    }
    if (keep < 1.0 && keep < hash(in.position.xy)) {
        discard;
    }

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}

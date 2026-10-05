// Surface de l'eau (0.13 O1 / O2) : couleur et opacité selon la profondeur (alpha des couleurs de
// sommets), Fresnel, reflet du ciel et du soleil, vagues (sommets + normales), écume au rivage et
// sur les crêtes, vue d'en dessous (fenêtre de Snell). Tout est calculé dans le repère fixe de
// l'astre : les tuiles y sont posées, le monde tourne autour.
//
// Les vagues ne dépendent que des UV (coordonnées de colonne modulo 4096) et du temps : elles se
// raccordent d'une tuile à l'autre, quel que soit le niveau de détail.

#import bevy_pbr::{
    mesh_functions,
    forward_io::Vertex,
    view_transformations::position_world_to_clip,
    pbr_functions,
    pbr_types,
}

struct Water {
    // xyz : direction du soleil (repère de l'astre), w : force de la lumière (0..1)
    sun: vec4<f32>,
    // rgb : couleur du ciel, w : lumière de nuit (0..1)
    sky: vec4<f32>,
    // xyz : caméra (repère de l'astre), w : taille d'une colonne (unités)
    cam: vec4<f32>,
    // x : temps (s), y : hauteur des vagues (colonnes), z : gravité (colonnes / s^2), w : écume
    time: vec4<f32>,
    // poids des vagues 1 à 4 (le signe donne le sens de marche)
    w1: vec4<f32>,
    // x : poids de la vague 5, y : 1 sous l'eau, z / w : distance des vagues en sommets (colonnes)
    w2: vec4<f32>,
};

@group(2) @binding(0) var<uniform> water: Water;

const TAU: f32 = 6.2831853;
const PERIOD: f32 = 4096.0;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    // dans le repère de l'astre
    @location(1) local_position: vec3<f32>,
    @location(2) local_normal: vec3<f32>,
    @location(3) tint_depth: vec4<f32>,
    @location(4) uv: vec2<f32>,
};

// Une vague : x = hauteur (colonnes), yz = pente par rapport à (u, v)
fn wave(m: vec2<f32>, amp: f32, phase: f32, weight: f32, uv: vec2<f32>) -> vec3<f32> {
    let k = m * (TAU / PERIOD);
    let kl = length(k);
    let omega = sqrt(water.time.z * kl);
    let dir = select(1.0, -1.0, weight < 0.0);
    let a = abs(weight) * amp * water.time.y;
    let arg = dot(k, uv) - dir * omega * water.time.x + phase;
    return vec3<f32>(a * sin(arg), a * cos(arg) * k.x, a * cos(arg) * k.y);
}

fn waves(uv: vec2<f32>) -> vec3<f32> {
    var w = wave(vec2<f32>(96.0, 0.0), 1.0, 0.0, water.w1.x, uv);
    w += wave(vec2<f32>(-70.0, 150.0), 0.55, 1.3, water.w1.y, uv);
    w += wave(vec2<f32>(190.0, 90.0), 0.32, 2.6, water.w1.z, uv);
    w += wave(vec2<f32>(-60.0, -310.0), 0.18, 3.9, water.w1.w, uv);
    w += wave(vec2<f32>(410.0, -120.0), 0.10, 5.2, water.w2.x, uv);
    return w;
}

@vertex
fn vertex(v: Vertex) -> VOut {
    var out: VOut;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let n = normalize(v.normal);
    // Vagues en relief près de la caméra seulement (les sommets sont des colonnes)
    let cell = max(water.cam.w, 1e-4);
    let dist = distance(v.position, water.cam.xyz) / cell;
    let fade = 1.0 - smoothstep(water.w2.z, water.w2.w, dist);
    let shore = smoothstep(0.0, 3.0, v.color.a / cell);
    let h = waves(v.uv).x * cell * fade * shore;
    let p = v.position + n * h;
    out.local_position = p;
    out.local_normal = n;
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(p, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.tint_depth = v.color;
    out.uv = v.uv;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let cell = max(water.cam.w, 1e-4);
    let up = normalize(in.local_normal);
    let tint = in.tint_depth.rgb;
    let depth = in.tint_depth.a / cell;
    let to_cam = water.cam.xyz - in.local_position;
    let dist = length(to_cam) / cell;
    let view = to_cam / max(length(to_cam), 1e-4);

    // Normale des vagues : la pente par rapport aux UV devient une pente dans le plan tangent
    let w = waves(in.uv);
    let dp1 = dpdx(in.local_position);
    let dp2 = dpdy(in.local_position);
    let du1 = dpdx(in.uv);
    let du2 = dpdy(in.uv);
    let det = du1.x * du2.y - du1.y * du2.x;
    var n = up;
    let nfade = (1.0 - smoothstep(600.0, 6000.0, dist)) * smoothstep(0.0, 2.0, depth);
    if (abs(det) > 1e-9) {
        let eu = (dp1 * du2.y - dp2 * du1.y) / det;
        let ev = (dp2 * du1.x - dp1 * du2.x) / det;
        let lu = max(length(eu), 1e-6);
        let lv = max(length(ev), 1e-6);
        // pente (colonnes de haut par colonne) : même unité en haut et en bas
        n = normalize(up - (eu / lu) * (w.y * nfade) - (ev / lv) * (w.z * nfade));
    }

    let under = view_below(view, up);
    let day = water.sun.w;
    let lit = 0.10 + 0.90 * day * clamp(dot(up, water.sun.xyz) * 0.7 + 0.3, 0.0, 1.0) + water.sky.w * 0.25;
    var cos_t = dot(view, n);
    if (under) {
        n = -n;
        cos_t = dot(view, n);
    }
    cos_t = clamp(cos_t, 0.0, 1.0);

    var color = vec3<f32>(0.0);
    var alpha = 1.0;
    if (under) {
        // Vue d'en dessous : fenêtre de Snell vers le ciel, miroir sombre au-delà
        let fog = tint * lit * 0.55;
        let window = smoothstep(0.55, 0.78, cos_t);
        color = mix(fog * 0.7, mix(fog, water.sky.rgb * 0.9, 0.55), window);
        alpha = 0.88;
        let glint_u = pow(clamp(dot(refract(-view, n, 1.33), water.sun.xyz), 0.0, 1.0), 60.0) * day * window;
        color += vec3<f32>(1.0, 0.95, 0.8) * glint_u * 0.8;
    } else {
        // Fresnel (Schlick) : reflet du ciel de plus en plus fort vers l'horizon
        let f0 = 0.02;
        let fres = f0 + (1.0 - f0) * pow(1.0 - cos_t, 5.0);
        let refl = reflect(-view, n);
        let horizon = pow(1.0 - clamp(dot(refl, up), 0.0, 1.0), 3.0);
        let sky = water.sky.rgb * (0.85 + 0.45 * horizon);
        let rough = 140.0 + 900.0 * (1.0 - clamp(water.time.y * 0.4, 0.0, 1.0));
        let glint = pow(clamp(dot(refl, water.sun.xyz), 0.0, 1.0), rough) * day * (rough / 120.0);
        // Absorption : le rouge disparaît le premier ; le chemin est plus long de biais
        let absorb = (vec3<f32>(1.0) - tint) * 0.05 + vec3<f32>(0.004);
        let path = depth / max(cos_t, 0.2);
        let t = exp(-absorb * path);
        let tm = (t.x + t.y + t.z) / 3.0;
        let body = tint * lit;
        var rgb = body * (1.0 - tm) * (1.0 - fres) + sky * fres + vec3<f32>(1.0, 0.96, 0.85) * glint;
        var a = 1.0 - tm * (1.0 - fres);

        // Écume : au rivage (bande qui avance et recule) et sur les crêtes par vent fort
        let hn = w.x / max(water.time.y * 1.6, 1e-3);
        let swash = 0.5 + 0.5 * sin(water.time.x * 0.9 + in.uv.x * 0.07 + in.uv.y * 0.05);
        let band = (1.0 + 1.6 * swash) ;
        let n1 = vnoise(in.uv * 0.45 + vec2<f32>(water.time.x * 0.25, 0.0));
        let n2 = vnoise(in.uv * 1.3 - vec2<f32>(0.0, water.time.x * 0.4));
        let lace = smoothstep(0.35, 0.8, n1 * 0.6 + n2 * 0.5);
        let shore = (1.0 - smoothstep(0.0, band, depth)) * (0.35 + 0.65 * lace);
        let crest = smoothstep(0.55, 1.0, hn) * water.time.w * (0.4 + 0.6 * lace) * (1.0 - smoothstep(300.0, 2500.0, dist));
        let foam = clamp(shore * 0.9 + crest, 0.0, 1.0);
        rgb = mix(rgb, vec3<f32>(0.92, 0.95, 0.97) * (0.25 + 0.75 * lit), foam);
        a = mix(a, 0.96, foam);

        color = rgb;
        alpha = a;
    }

    var out_color = vec4<f32>(color / max(alpha, 0.02), alpha);
    out_color = vec4<f32>(min(out_color.rgb, vec3<f32>(4.0)), out_color.a);
    // Brouillard de l'horizon, étalonnage : comme les autres matériaux
    var pbr_input = pbr_types::pbr_input_new();
    pbr_input.material.flags = pbr_types::STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT;
    pbr_input.world_position = in.world_position;
    pbr_input.frag_coord = in.position;
    return pbr_functions::main_pass_post_lighting_processing(pbr_input, out_color);
}

// La caméra voit la surface d'en dessous : la normale de la surface la regarde de dos.
fn view_below(view: vec3<f32>, up: vec3<f32>) -> bool {
    return dot(view, up) < 0.0;
}

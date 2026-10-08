// Trou noir de Kerr (0.13 V1), porté du modèle `trou_noir` (GLSL, macroquad) : géodésiques nulles dans la
// métrique de Kerr en coordonnées de Kerr-Schild cartésiennes
//     g_μν = η_μν + f k_μ k_ν,  f = 2 M r³ / (r⁴ + a² z²),
//     k_μ = (1, (r x + a y)/(r²+a²), (r y − a x)/(r²+a²), z/r)
//     dx/dλ = p − f (k·p) k ;  dp/dλ = ½ (k·p)² ∇f + f (k·p) J·p
// Unités : rayon de Schwarzschild = 1 (M = 0,5) ; z = axe de rotation.
//
// Différences avec le modèle : le rayon part du bord de la sphère de lentille (ou de la caméra si elle est
// dedans) ; celui qui ressort prend le VRAI fond du jeu dans sa nouvelle direction (image déjà rendue
// derrière le trou noir, sinon la skybox fixe) ; pas de tonemap (le jeu le fait) ; profondeur écrite au
// centre du trou noir (ce qui est derrière est caché : on le voit dévié ; ce qui est devant reste devant) ;
// une étoile aspirée en spirale.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, view_transmission_texture, view_transmission_sampler},
}

struct Hole {
    center: vec4<f32>,
    ax: vec4<f32>,
    ay: vec4<f32>,
    az: vec4<f32>,
    misc: vec4<f32>,
    star: vec4<f32>,
};

@group(2) @binding(0) var<uniform> hole: Hole;
@group(2) @binding(1) var sky_tex: texture_cube<f32>;
@group(2) @binding(2) var sky_samp: sampler;

const PI: f32 = 3.14159265;
const M: f32 = 0.5;

// pow sans NaN (pow(0, y) vaut NaN sur certains GPU)
fn spow(x: f32, y: f32) -> f32 {
    return pow(max(x, 1e-6), y);
}

// ---------- Repère ----------
fn to_local(p: vec3<f32>) -> vec3<f32> {
    let d = p - hole.center.xyz;
    return vec3<f32>(dot(d, hole.ax.xyz), dot(d, hole.ay.xyz), dot(d, hole.az.xyz)) / hole.center.w;
}

fn dir_local(d: vec3<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(dot(d, hole.ax.xyz), dot(d, hole.ay.xyz), dot(d, hole.az.xyz)));
}

fn dir_world(d: vec3<f32>) -> vec3<f32> {
    return normalize(hole.ax.xyz * d.x + hole.ay.xyz * d.y + hole.az.xyz * d.z);
}

// ---------- Géométrie de Kerr ----------
struct KD {
    r: f32,
    f: f32,
    k: vec3<f32>,
    df: vec3<f32>,
    jx: vec3<f32>,
    jy: vec3<f32>,
    jz: vec3<f32>,
};

fn kd(a: f32, pos: vec3<f32>) -> KD {
    let x = pos.x;
    let y = pos.y;
    let z = pos.z;
    let a2 = a * a;
    let rho2 = x * x + y * y + z * z;
    let z2 = z * z;
    // r : racine positive de r⁴ − (ρ²−a²) r² − a² z² = 0
    let A = rho2 - a2;
    let B = sqrt(max(A * A + 4.0 * a2 * z2, 1e-12));
    let r2 = 0.5 * (A + B);
    let r = sqrt(max(r2, 1e-12));
    let r3 = r2 * r;
    let r4 = r2 * r2;
    let W = r4 + a2 * z2;
    let W2 = W * W;
    let invB = 1.0 / B;
    let drdx = x * r * invB;
    let drdy = y * r * invB;
    let drdz = z * (r2 + a2) * invB / r;
    let f = 2.0 * M * r3 / W;
    let c = 2.0 * M * r2 * (-r4 + 3.0 * a2 * z2) / W2;
    let df = vec3<f32>(c * drdx, c * drdy, c * drdz - 4.0 * M * a2 * z * r3 / W2);
    let denom = r2 + a2;
    let d2 = denom * denom;
    let rkx = r * x + a * y;
    let rky = r * y - a * x;
    let tr = 2.0 * r;
    var o: KD;
    o.r = r;
    o.f = f;
    o.k = vec3<f32>(rkx / denom, rky / denom, z / r);
    o.df = df;
    o.jx = vec3<f32>(((drdx * x + r) * denom - rkx * tr * drdx) / d2, ((drdx * y - a) * denom - rky * tr * drdx) / d2, -z * drdx / r2);
    o.jy = vec3<f32>(((drdy * x + a) * denom - rkx * tr * drdy) / d2, ((drdy * y + r) * denom - rky * tr * drdy) / d2, -z * drdy / r2);
    o.jz = vec3<f32>((drdz * x * denom - rkx * tr * drdz) / d2, (drdz * y * denom - rky * tr * drdz) / d2, (r - z * drdz) / r2);
    return o;
}

struct Deriv {
    dpos: vec3<f32>,
    dp: vec3<f32>,
};

fn eom(a: f32, pos: vec3<f32>, p: vec3<f32>) -> Deriv {
    let q = kd(a, pos);
    let kp = -1.0 + dot(q.k, p);
    var o: Deriv;
    o.dpos = p - q.f * kp * q.k;
    let jp = vec3<f32>(dot(p, q.jx), dot(p, q.jy), dot(p, q.jz));
    o.dp = 0.5 * kp * kp * q.df + q.f * kp * jp;
    return o;
}

// α tel que P = α·dir vérifie la condition nulle H = 0
fn init_alpha(a: f32, pos: vec3<f32>, dir: vec3<f32>) -> f32 {
    let q = kd(a, pos);
    let c = dot(q.k, dir);
    let fc2 = q.f * c * c;
    let disc = 1.0 + q.f - fc2;
    if (disc < 0.0) {
        return 1.0;
    }
    let denom = 1.0 - fc2;
    if (abs(denom) < 1e-4) {
        return 1.0;
    }
    return (-q.f * c + sqrt(disc)) / denom;
}

// ---------- Disque d'accrétion ----------
fn disk_color(hit: vec3<f32>, pdir: vec3<f32>, time: f32) -> vec3<f32> {
    let rho = length(hit.xy);
    let phi = atan2(hit.y, hit.x);
    let t = clamp((rho - 2.5) / (14.0 - 2.5), 0.0, 1.0);
    let hot = vec3<f32>(1.0, 0.98, 0.90);
    let warm = vec3<f32>(1.0, 0.55, 0.15);
    let cool = vec3<f32>(0.45, 0.10, 0.03);
    var c = mix(warm, cool, (t - 0.3) / 0.7);
    if (t < 0.3) {
        c = mix(hot, warm, t / 0.3);
    }
    let bright = spow(1.0 - t, 1.6);
    var n = 0.65 + 0.35 * sin(phi * 5.0 + rho * 1.6 - time * 1.5) * cos(rho * 2.4 - phi * 3.0 + time);
    n *= 0.75 + 0.25 * sin(phi * 17.0 + rho * 4.0);
    // Vitesse orbitale (képlérienne), effet Doppler relativiste
    let tang = normalize(vec3<f32>(-hit.y, hit.x, 0.0));
    let v_orb = min(sqrt(M / max(rho - M, 0.15)), 0.88);
    let gamma = 1.0 / sqrt(max(1.0 - v_orb * v_orb, 0.02));
    let n_photon = -normalize(pdir);
    let bdn = v_orb * dot(tang, n_photon);
    let doppler = clamp(1.0 / (gamma * (1.0 - bdn)), 0.2, 6.0);
    // Décalage gravitationnel
    let grav = sqrt(max(1.0 - 2.0 * M / max(rho, 1e-3), 0.02));
    let shift = doppler * grav;
    let boost = clamp(spow(shift, 3.0), 0.1, 10.0);
    var shifted = mix(c, vec3<f32>(c.r, c.g * 0.8, c.b * 0.6), min((1.0 - shift) * 0.7, 1.0));
    if (shift > 1.0) {
        shifted = mix(c, vec3<f32>(c.b, c.g, c.r), min((shift - 1.0) * 0.5, 1.0));
    }
    return shifted * bright * n * boost;
}

fn disk_opacity(hit: vec3<f32>) -> f32 {
    let rho = length(hit.xy);
    let t = clamp((rho - 2.5) / (14.0 - 2.5), 0.0, 1.0);
    let u = (t - 0.3) * 3.0;
    var alpha = 0.80 * exp(-u * u);
    alpha *= smoothstep(0.0, 0.15, t) * smoothstep(1.0, 0.85, t);
    return clamp(alpha, 0.0, 1.0);
}

// ---------- Jets polaires ----------
fn jet_density(pos: vec3<f32>) -> f32 {
    let h = abs(pos.z);
    let rho = length(pos.xy);
    let cone_r = 0.12 + 0.18 * h;
    let u = rho / cone_r;
    let in_jet = exp(-u * u * 1.5);
    var h_prof = smoothstep(1.5, 4.0, h) * exp(-h / 18.0);
    h_prof *= smoothstep(1.5, 3.0, length(pos));
    return in_jet * h_prof;
}

fn jet_color(pos: vec3<f32>, pdir: vec3<f32>) -> vec3<f32> {
    var vz = -1.0;
    if (pos.z > 0.0) {
        vz = 1.0;
    }
    let beta = 0.85;
    let gamma = 1.0 / sqrt(1.0 - beta * beta);
    let n_photon = -normalize(pdir);
    let doppler = 1.0 / (gamma * (1.0 - dot(vec3<f32>(0.0, 0.0, vz * beta), n_photon)));
    let boost = clamp(spow(doppler, 3.0), 0.05, 15.0);
    let base = vec3<f32>(0.55, 0.72, 1.0);
    var shifted = mix(base, vec3<f32>(0.50, 0.30, 0.55), min((1.0 - doppler) * 0.8, 1.0));
    if (doppler > 1.0) {
        shifted = mix(base, vec3<f32>(0.92, 0.96, 1.0), min((doppler - 1.0) * 0.6, 1.0));
    }
    return shifted * boost * 0.06;
}

// ---------- Étoile aspirée ----------
// Une étoile compagne sur une orbite large : les marées lui arrachent un courant de gaz qui s'enroule en
// spirale jusqu'au disque, de plus en plus chaud en tombant. Émission volumique : elle est déviée par la
// lentille comme le reste.
fn star_emission(pos: vec3<f32>) -> vec3<f32> {
    if (hole.star.z < 0.5) {
        return vec3<f32>(0.0);
    }
    let r_star = hole.star.x;
    let phase = hole.star.y;
    let sp = vec3<f32>(r_star * cos(phase), r_star * sin(phase), 0.0);
    // L'étoile (déformée en goutte vers le trou noir)
    let ds = pos - sp;
    let toward = -normalize(sp);
    let along = dot(ds, toward);
    // Goutte : la partie tournée vers le trou noir est étirée (marée), pas l'autre
    let k = select(1.0, 0.55, along > 0.0);
    let stretched = ds - toward * along * (1.0 - k);
    let core = exp(-dot(stretched, stretched) / 0.8);
    var col = vec3<f32>(1.0, 0.82, 0.6) * core * 1.6 + vec3<f32>(1.0, 0.6, 0.3) * exp(-dot(stretched, stretched) / 6.0) * 0.12;
    // Le courant en spirale : de l'étoile (rho = r_star) au bord du disque (rho = 14)
    let rho = length(pos.xy);
    if (rho > 9.0 && rho < r_star + 1.0) {
        let phi = atan2(pos.y, pos.x);
        // Retard angulaire croissant vers l'intérieur (le gaz plus proche tourne plus vite)
        let arm = phase + 3.2 * log(r_star / rho);
        var dphi = fract((phi - arm) / (2.0 * PI) + 0.5) - 0.5;
        dphi *= 2.0 * PI;
        let width = 0.25 + 0.035 * (r_star - rho);
        let u = dphi * rho / width;
        let v = pos.z / (0.3 + 0.03 * (r_star - rho));
        let fall = clamp((r_star - rho) / (r_star - 9.0), 0.0, 1.0);
        let dens = exp(-u * u - v * v) * smoothstep(9.0, 12.0, rho) * (0.4 + 0.6 * fall);
        let hot = mix(vec3<f32>(1.0, 0.55, 0.25), vec3<f32>(0.85, 0.9, 1.0), fall);
        col += hot * dens * 0.5;
    }
    return col;
}

// ---------- Le vrai fond dans la direction `d` (monde) ----------
// L'image déjà rendue derrière le trou noir quand la direction tombe dans l'écran, la skybox fixe sinon
// (fondu près des bords).
fn background(d: vec3<f32>) -> vec3<f32> {
    let sky = textureSampleLevel(sky_tex, sky_samp, d, 0.0).rgb;
    let clip = view.clip_from_world * vec4<f32>(d, 0.0);
    if (clip.w <= 1e-6) {
        return sky;
    }
    let ndc = clip.xy / clip.w;
    let uv = ndc * vec2<f32>(0.5, -0.5) + 0.5;
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let w = smoothstep(0.0, 0.06, edge);
    if (w <= 0.0) {
        return sky;
    }
    let scene = textureSampleLevel(view_transmission_texture, view_transmission_sampler, clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb;
    return mix(sky, scene, w);
}

struct Out {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> Out {
    let R = hole.misc.z;
    let a = hole.misc.x;
    let time = hole.misc.y;
    let cam_w = view.world_position;
    let cam = to_local(cam_w);
    let inside = length(cam) < R * 0.999;
    // Dehors : les faces avant ; dedans : les faces arrière (une seule fois par pixel)
    if (inside == front) {
        discard;
    }
    let ray_w = normalize(in.world_position.xyz - cam_w);
    let dir = dir_local(ray_w);
    var pos = cam;
    if (!inside) {
        pos = to_local(in.world_position.xyz);
    }
    var p = init_alpha(a, pos, dir) * dir;
    var accum = vec3<f32>(0.0);
    var trans = 1.0;
    var escaped = false;
    let r_hor = M + sqrt(max(M * M - a * a, 0.0));
    for (var i = 0; i < 300; i++) {
        let r = length(pos);
        // Horizon : absorbé (l'ombre)
        if (r < r_hor * 1.005) {
            break;
        }
        let speed = max(length(p), 0.01);
        // Pas plus fin que le modèle : sans ça, la déviation fait des anneaux en escalier dans le fond
        let dl = clamp(0.1 * r / speed, 0.005, 0.8);
        // Jets (optiquement minces) et étoile aspirée
        let jd = jet_density(pos);
        if (jd > 0.001) {
            accum += trans * jet_color(pos, p) * jd;
        }
        if (r > 8.0) {
            accum += trans * star_emission(pos) * dl;
        }
        // RK4 (le RK2 du modèle laissait des anneaux en escalier dans le fond dévié)
        let k1 = eom(a, pos, p);
        let k2 = eom(a, pos + 0.5 * dl * k1.dpos, p + 0.5 * dl * k1.dp);
        let k3 = eom(a, pos + 0.5 * dl * k2.dpos, p + 0.5 * dl * k2.dp);
        let k4 = eom(a, pos + dl * k3.dpos, p + dl * k3.dp);
        let npos = pos + dl / 6.0 * (k1.dpos + 2.0 * k2.dpos + 2.0 * k3.dpos + k4.dpos);
        let np = p + dl / 6.0 * (k1.dp + 2.0 * k2.dp + 2.0 * k3.dp + k4.dp);
        // Traversée du plan du disque (z = 0)
        if (pos.z * npos.z < 0.0 && abs(npos.z - pos.z) > 1e-9) {
            let t = pos.z / (pos.z - npos.z);
            let hit = mix(pos, npos, t);
            let rho = length(hit.xy);
            if (rho > 2.5 && rho < 14.0) {
                let al = disk_opacity(hit);
                accum += trans * al * disk_color(hit, p, time);
                trans *= 1.0 - al;
                if (trans < 0.005) {
                    break;
                }
            }
        }
        // Ressorti de la sphère : la direction est prise exactement sur son bord (interpolée dans le pas),
        // sinon le pas qui dépasse plus ou moins le bord faisait des anneaux en escalier dans le fond
        let rn = length(npos);
        if (rn > R && dot(npos, np) > 0.0) {
            let u = clamp((R - r) / max(rn - r, 1e-6), 0.0, 1.0);
            p = mix(p, np, u);
            escaped = true;
            break;
        }
        pos = npos;
        p = np;
    }
    // Un rayon qui n'a pas fini sa course : il ressort seulement s'il est loin et s'éloigne (sinon il tourne
    // autour de l'ombre ou y tombe : noir)
    if (!escaped && length(pos) > 6.0 && dot(pos, p) > 0.0) {
        escaped = true;
    }
    var col = accum * hole.misc.w * 2.5;
    if (escaped && trans > 0.0) {
        col += trans * background(dir_world(normalize(p)));
    }
    // Profondeur : celle du centre du trou noir (ce qui est plus loin est caché et vu dévié, ce qui est
    // plus près reste devant). Caméra au-delà du centre vers l'arrière : tout au fond.
    let cc = view.clip_from_world * vec4<f32>(hole.center.xyz, 1.0);
    var depth = 0.0;
    if (cc.w > 0.0) {
        depth = clamp(cc.z / cc.w, 0.0, 1.0);
    }
    var o: Out;
    o.color = vec4<f32>(col, 1.0);
    o.depth = depth;
    return o;
}

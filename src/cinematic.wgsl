// Séquences plein écran (0.13 V2, V4) : creusement d'un tunnel du sub-espace par la foreuse, vol dans un
// tunnel, voyage entre galaxies. Tout est calculé par pixel (ray marching et formules), sans maillage :
// la séquence recouvre le jeu (nœud d'interface plein écran) et le rend à la fin.
//
// Le creusement vient du prototype `foreuse.rs` (GLSL, macroquad), porté et étendu : une vise (foret en
// spirale) à l'avant, la grille 2D puis 3D de la zone, la zone proche qui tourne vers la droite en
// déformant l'espace comme un trou noir, le tunnel qui se forme derrière la foreuse.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Cine {
    // x, y : résolution ; z : temps de la séquence (s) ; w : mode (0 foreuse, 1 tunnel, 2 galaxies)
    res: vec4<f32>,
    // caméra : position, cible
    cam: vec4<f32>,
    look: vec4<f32>,
    // foreuse : position ; w = vitesse des anneaux
    drill: vec4<f32>,
    // x : lueur, y : grille 2D, z : grille 3D, w : tourbillon
    fx: vec4<f32>,
    // x : lentille, y : fondu, z : longueur du tunnel formé derrière la foreuse, w : flash
    fx2: vec4<f32>,
    // tunnel : x voie (-1..1), y vitesse (0..1), z position (distance), w sens (+1 avant, -1 arrière)
    // galaxies : x, y = teintes de départ et d'arrivée, z = graine, w = durée
    extra: vec4<f32>,
};

@group(1) @binding(0) var<uniform> cine: Cine;

const PI: f32 = 3.14159265;

fn fmod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

fn hash13(p0: vec3<f32>) -> f32 {
    var p = fract(p0 * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

fn hash12(p0: vec2<f32>) -> f32 {
    var p = fract(vec3<f32>(p0.xyx) * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash12(i), hash12(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash12(i + vec2<f32>(0.0, 1.0)), hash12(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// ─────────────────────────── SDF ───────────────────────────

fn sdCylinderZ(p: vec3<f32>, r: f32, h: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xy), p.z)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sdTorusXY(p: vec3<f32>, big: f32, small: f32) -> f32 {
    let q = vec2<f32>(length(p.xy) - big, p.z);
    return length(q) - small;
}

fn sdEllipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / max(k1, 1e-5);
}

fn sdBox(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// La vise : un foret conique à trois goujures hélicoïdales, de la base de la foreuse (z = 0) à la pointe (z = 6).
fn bitSDF(q: vec3<f32>, time: f32) -> f32 {
    let h = 6.0;
    let zc = clamp(q.z, 0.0, h);
    let r = 0.82 * pow(1.0 - zc / h, 0.85);
    let ang = atan2(q.y, q.x);
    let spin = time * cine.drill.w * 3.0;
    let flute = 0.5 + 0.5 * cos(3.0 * ang - 4.6 * q.z + spin);
    let rr = r * (0.58 + 0.42 * flute);
    let radial = length(q.xy) - rr;
    let caps = max(-q.z, q.z - h);
    return max(radial, caps) * 0.55;
}

fn drillSDF(p: vec3<f32>, time: f32) -> f32 {
    var d = sdCylinderZ(p, 0.9, 3.5);
    // Collier qui porte la vise
    d = min(d, sdCylinderZ(p - vec3<f32>(0.0, 0.0, 3.6), 1.05, 0.22));
    d = min(d, bitSDF(p - vec3<f32>(0.0, 0.0, 3.7), time));
    d = min(d, sdEllipsoid(p - vec3<f32>(0.0, 0.0, -4.0), vec3<f32>(0.9, 0.9, 0.7)));
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let zpos = -3.0 + fi * 1.5;
        let sgn = select(-1.0, 1.0, fmod(fi, 2.0) < 0.5);
        let speed = cine.drill.w * (0.6 + 0.15 * fi) * sgn;
        let ang = time * speed;
        let ca = cos(ang);
        let sa = sin(ang);
        var rp = p;
        rp.z -= zpos;
        rp = vec3<f32>(rp.x * ca - rp.y * sa, rp.x * sa + rp.y * ca, rp.z);
        let big = 1.7 + 0.15 * sin(fi * 1.3);
        var dr = sdTorusXY(rp, big, 0.11);
        dr = min(dr, sdBox(rp, vec3<f32>(big, 0.05, 0.05)));
        let rp2 = vec3<f32>(rp.y, -rp.x, rp.z);
        dr = min(dr, sdBox(rp2, vec3<f32>(big, 0.05, 0.05)));
        d = min(d, dr);
    }
    return d;
}

fn drill_normal(p: vec3<f32>, time: f32) -> vec3<f32> {
    let e = vec2<f32>(0.012, 0.0);
    let d0 = drillSDF(p, time);
    return normalize(vec3<f32>(drillSDF(p + e.xyy, time) - d0, drillSDF(p + e.yxy, time) - d0, drillSDF(p + e.yyx, time) - d0));
}

// ─────────────────────────── Étoiles ───────────────────────────

fn starfield(dir: vec3<f32>) -> vec3<f32> {
    var col = vec3<f32>(0.002, 0.003, 0.008);
    for (var layer = 0; layer < 2; layer++) {
        let sc = select(130.0, 300.0, layer == 1);
        let p = dir * sc;
        let ip = floor(p);
        let h = hash13(ip + f32(layer) * 17.0);
        let th = select(0.9965, 0.9975, layer == 1);
        if (h > th) {
            let b = pow((h - th) / (1.0 - th), 0.8);
            let tint = vec3<f32>(hash13(ip + 1.7), hash13(ip + 3.3), hash13(ip + 5.9));
            col += b * (0.55 + 0.6 * tint) * select(1.0, 0.6, layer == 1);
        }
    }
    let n = sin(dir.x * 1.7) * cos(dir.y * 2.3 + dir.z * 1.1);
    col += 0.004 * vec3<f32>(0.4, 0.55, 0.85) * (0.5 + 0.5 * n);
    return col;
}

// ─────────────────────────── Espace tordu autour de la foreuse ───────────────────────────

// Le tourbillon : la zone proche de la foreuse tourne vers la droite (autour de son axe), de plus en plus
// vite vers le centre, comme l'espace entraîné par un trou noir.
fn swirl(p: vec3<f32>) -> vec3<f32> {
    let s = cine.fx.w;
    if (s < 0.001) {
        return p;
    }
    let c = cine.drill.xyz;
    var q = p - c;
    let r = length(q.xy);
    let w = length(q);
    let ang = s * 2.6 * exp(-w * w / 900.0) * (1.0 + 2.5 / (1.0 + 0.15 * w));
    let xy = rot2(-ang) * q.xy;
    q = vec3<f32>(xy, q.z);
    return q + c;
}

// ─────────────────────────── Grilles ───────────────────────────

fn grid_lines3(q: vec3<f32>, spacing: f32) -> f32 {
    let g = abs(fract(q / spacing + 0.5) - 0.5) * spacing;
    let dxy = length(g.xy);
    let dxz = length(g.xz);
    let dyz = length(g.yz);
    return min(dxy, min(dxz, dyz));
}

// Grille 2D : un plan quadrillé sous la foreuse, sur toute la zone.
fn grid2d(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let amount = cine.fx.y;
    if (amount < 0.001 || abs(rd.y) < 1e-4) {
        return vec3<f32>(0.0);
    }
    let y0 = cine.drill.y - 7.0;
    let t = (y0 - ro.y) / rd.y;
    if (t < 0.0) {
        return vec3<f32>(0.0);
    }
    let hit = ro + rd * t;
    let sw = swirl(hit);
    let spacing = 4.0;
    let g = abs(fract(sw.xz / spacing + 0.5) - 0.5) * spacing;
    let w = 0.06 + 0.0025 * t;
    let line = 1.0 - smoothstep(0.0, w, min(g.x, g.y));
    let extent = 1.0 - smoothstep(60.0, 95.0, max(abs(sw.x - cine.drill.x), abs(sw.z - cine.drill.z)));
    let fade = exp(-t * 0.004);
    // Les lignes principales (tous les 5) sont plus vives
    let gm = abs(fract(sw.xz / (spacing * 5.0) + 0.5) - 0.5) * spacing * 5.0;
    let major = 1.0 - smoothstep(0.0, w * 1.6, min(gm.x, gm.y));
    return vec3<f32>(0.25, 0.6, 1.0) * (line * 0.7 + major * 0.9) * extent * fade * amount * 1.5;
}

// Grille 3D : un treillis dans un volume autour de la foreuse, éclairé en passant près de ses lignes.
fn grid3d(ro: vec3<f32>, rd: vec3<f32>, tmax: f32) -> vec3<f32> {
    let amount = cine.fx.z;
    if (amount < 0.001) {
        return vec3<f32>(0.0);
    }
    let c = cine.drill.xyz;
    let half = vec3<f32>(45.0, 24.0, 55.0);
    // Boîte englobante (entrée et sortie du rayon)
    let inv = 1.0 / rd;
    let t0v = (c - half - ro) * inv;
    let t1v = (c + half - ro) * inv;
    let tn = max(max(min(t0v.x, t1v.x), min(t0v.y, t1v.y)), min(t0v.z, t1v.z));
    let tf = min(min(max(t0v.x, t1v.x), max(t0v.y, t1v.y)), max(t0v.z, t1v.z));
    if (tf < max(tn, 0.0)) {
        return vec3<f32>(0.0);
    }
    let ta = max(tn, 0.0);
    let tb = min(tf, tmax);
    if (tb <= ta) {
        return vec3<f32>(0.0);
    }
    var acc = 0.0;
    let n = 72;
    let dt = (tb - ta) / f32(n);
    for (var i = 0; i < n; i++) {
        let t = ta + (f32(i) + 0.5) * dt;
        let p = ro + rd * t;
        let sw = swirl(p);
        let d = grid_lines3(sw, 4.0);
        let edge = 1.0 - smoothstep(0.55, 1.0, length((p - c) / half));
        acc += exp(-d * 11.0) * dt * edge * exp(-t * 0.006);
    }
    return vec3<f32>(0.3, 0.62, 1.0) * acc * amount * 0.55;
}

// Le tunnel formé derrière la foreuse : un cylindre lumineux (parois à rubans), qui s'allonge.
fn tube_glow(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let len = cine.fx2.z;
    if (len < 0.01) {
        return vec3<f32>(0.0);
    }
    let big = 3.4;
    let z_end = cine.drill.z - 4.5;
    let z_start = z_end - len;
    let a = dot(rd.xy, rd.xy);
    if (a < 1e-6) {
        return vec3<f32>(0.0);
    }
    let b = 2.0 * dot(ro.xy, rd.xy);
    let c = dot(ro.xy, ro.xy) - big * big;
    let disc = b * b - 4.0 * a * c;
    if (disc < 0.0) {
        return vec3<f32>(0.0);
    }
    var col = vec3<f32>(0.0);
    let sq = sqrt(disc);
    for (var k = 0; k < 2; k++) {
        let t = (-b + select(-sq, sq, k == 1)) / (2.0 * a);
        if (t < 0.0) {
            continue;
        }
        let p = ro + rd * t;
        if (p.z < z_start || p.z > z_end) {
            continue;
        }
        let ang = atan2(p.y, p.x);
        let ribbon = pow(0.5 + 0.5 * cos(ang * 12.0), 14.0);
        let rings = pow(0.5 + 0.5 * sin(p.z * 1.4 - cine.res.z * 3.0), 10.0);
        let head = exp(-(z_end - p.z) * 0.35);
        col += vec3<f32>(0.18, 0.45, 0.95) * (0.18 + ribbon * 0.9 + rings * 0.6 + head * 1.6) * select(0.5, 1.0, k == 0);
    }
    return col * 0.8;
}

// ─────────────────────────── Foreuse : séquence ───────────────────────────

fn shade_drill(p: vec3<f32>, n: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let lp = p - cine.drill.xyz;
    let base = vec3<f32>(0.15, 0.16, 0.2);
    let light_dir = normalize(vec3<f32>(0.4, 0.7, -0.5));
    let diff = max(dot(n, light_dir), 0.0);
    var col = base * (0.14 + diff * 0.75);
    let fres = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
    col += vec3<f32>(0.4, 0.6, 0.9) * fres * 0.45;
    // Lumières bleues sur les anneaux
    var ring_glow = 0.0;
    for (var i = 0; i < 5; i++) {
        let zpos = -3.0 + f32(i) * 1.5;
        if (abs(lp.z - zpos) < 0.35) {
            let r = length(lp.xy);
            if (r > 1.4 && r < 2.15) {
                ring_glow = 1.0;
            }
        }
    }
    col += vec3<f32>(0.3, 0.7, 1.0) * ring_glow * cine.fx.x;
    // La vise : métal clair, pointe qui rougeoie
    if (lp.z > 3.7) {
        let tip = clamp((lp.z - 3.7) / 6.0, 0.0, 1.0);
        let ang = atan2(lp.y, lp.x);
        let edge = pow(0.5 + 0.5 * cos(3.0 * ang - 4.6 * lp.z + cine.res.z * cine.drill.w * 3.0), 6.0);
        col = mix(col, vec3<f32>(0.62, 0.66, 0.74) * (0.25 + diff * 0.9), 0.7);
        col += vec3<f32>(1.0, 0.55, 0.2) * pow(tip, 3.0) * cine.fx.x * (0.6 + 0.8 * edge);
        col += vec3<f32>(0.5, 0.8, 1.0) * edge * 0.18 * cine.fx.x;
    }
    col += vec3<f32>(0.2, 0.5, 0.9) * cine.fx.x * 0.08;
    return col;
}

fn render_drill(ro: vec3<f32>, rd0: vec3<f32>) -> vec3<f32> {
    let rd = rd0;
    var t = 0.0;
    var hit = false;
    var hit_pos = vec3<f32>(0.0);
    var tmax = 160.0;
    for (var i = 0; i < 200; i++) {
        let p = ro + rd * t;
        let d = drillSDF(p - cine.drill.xyz, cine.res.z);
        if (d < 0.004 * t + 0.002) {
            hit = true;
            hit_pos = p;
            break;
        }
        t += max(d * 0.9, 0.02);
        if (t > tmax) {
            break;
        }
    }
    // Fond : étoiles, qui tournent avec l'espace autour de la foreuse
    var bg_dir = rd;
    if (cine.fx.w > 0.001) {
        let to = normalize(cine.drill.xyz - ro);
        let along = dot(rd, to);
        let perp = rd - to * along;
        let amt = cine.fx.w * 1.6 * exp(-length(perp) * length(perp) * 6.0);
        // rotation de la direction autour de l'axe caméra - foreuse (vers la droite)
        let axis = to;
        let c = cos(-amt);
        let s = sin(-amt);
        bg_dir = rd * c + cross(axis, rd) * s + axis * dot(axis, rd) * (1.0 - c);
    }
    var col = starfield(bg_dir) * (1.0 - 0.93 * cine.fx.w);
    let tcap = select(tmax, t, hit);
    col += grid3d(ro, rd, tcap);
    col += grid2d(ro, rd);
    col += tube_glow(ro, rd);
    if (hit) {
        let n = drill_normal(hit_pos - cine.drill.xyz, cine.res.z);
        col = shade_drill(hit_pos, n, rd);
    }
    // Halo bleu du trou noir artificiel quand l'espace tourne
    if (cine.fx.w > 0.001) {
        let to = cine.drill.xyz - ro;
        let dist = length(to);
        let a = max(dot(rd, to / dist), 0.0);
        col += vec3<f32>(0.1, 0.25, 0.7) * pow(a, 60.0) * cine.fx.w * 1.2;
    }
    return col;
}

// ─────────────────────────── Tunnel (intérieur) ───────────────────────────

fn render_tunnel(ro_in: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let big = 6.0;
    let speed = cine.extra.y;
    let s = cine.extra.z;
    let dir = cine.extra.w;
    let lane = cine.extra.x;
    // Position dans la section : voie (-1, 0, 1) -> x
    let ro = vec3<f32>(lane * 3.0, -1.5, 0.0);
    // Parois : intersection avec le cylindre
    let a = dot(rd.xy, rd.xy);
    let b = 2.0 * dot(ro.xy, rd.xy);
    let c = dot(ro.xy, ro.xy) - big * big;
    let disc = max(b * b - 4.0 * a * c, 0.0);
    let t = (-b + sqrt(disc)) / (2.0 * max(a, 1e-5));
    let p = ro + rd * t;
    let zz = s + p.z * dir;
    let ang = atan2(p.y, p.x);
    // Anneaux de structure tous les 6 u, rubans lumineux, hexagones
    let ring = pow(0.5 + 0.5 * cos(zz * (2.0 * PI / 6.0)), 18.0);
    let ribbon = pow(0.5 + 0.5 * cos(ang * 8.0), 22.0);
    let hexa = hash12(floor(vec2<f32>(ang * 6.0, zz * 0.5)));
    var col = vec3<f32>(0.02, 0.05, 0.12) * (0.6 + 0.6 * hexa);
    col += vec3<f32>(0.1, 0.35, 0.9) * ring * 1.4;
    col += vec3<f32>(0.2, 0.5, 1.0) * ribbon * 0.35;
    // Les voies, tracées au sol : bleue (lente), verte (moyenne), or (rapide, à péage)
    let floor_y = p.y < -3.5;
    if (floor_y) {
        let lx = p.x;
        let line = abs(fract((lx + 1.5) / 3.0 + 0.5) - 0.5) * 3.0;
        let lane_idx = clamp(floor((lx + 4.5) / 3.0), 0.0, 2.0);
        var lc = vec3<f32>(0.2, 0.5, 1.0);
        if (lane_idx > 0.5) {
            lc = vec3<f32>(0.2, 0.9, 0.5);
        }
        if (lane_idx > 1.5) {
            lc = vec3<f32>(1.0, 0.75, 0.2);
        }
        let dash = smoothstep(0.35, 0.5, fract(zz * 0.25));
        col += lc * (1.0 - smoothstep(0.0, 0.07, line)) * (0.4 + 0.8 * dash);
        col += lc * 0.05 * (1.0 - smoothstep(0.0, 1.3, line));
    }
    let depth = exp(-t * 0.018);
    col *= 0.25 + 0.75 * depth;
    // Lointain : la lumière au bout du tunnel
    let center = max(rd.z * dir * dir, 0.0);
    col += vec3<f32>(0.85, 0.92, 1.0) * pow(center, 60.0) * 2.4;
    // Traits de vitesse : des étoiles qui filent le long des parois
    var streak = 0.0;
    for (var i = 0; i < 30; i++) {
        let fi = f32(i);
        let a0 = hash12(vec2<f32>(fi, 3.1)) * 6.2831;
        let r0 = 1.5 + hash12(vec2<f32>(fi, 7.7)) * 3.5;
        let zt = fmod(hash12(vec2<f32>(fi, 1.3)) * 80.0 - s * 0.9 * dir - cine.res.z * speed * 40.0, 80.0);
        let sp = vec3<f32>(cos(a0) * r0, sin(a0) * r0, zt) - ro;
        let d = length(sp);
        let al = max(dot(rd, sp / d), 0.0);
        streak += pow(al, 400.0 + 800.0 * (1.0 - speed)) * 12.0 * speed / (1.0 + d * 0.04);
    }
    col += vec3<f32>(0.7, 0.85, 1.0) * streak;
    // Distorsion périodique
    col += vec3<f32>(0.2, 0.5, 1.0) * max(sin(length(rd.xy) * 8.0 - cine.res.z * 6.0 * speed), 0.0) * 0.05 * speed;
    // Péage : un éclair doré quand on passe sur la voie rapide
    col += vec3<f32>(1.0, 0.75, 0.25) * cine.fx2.w * 0.5 * (1.0 - length(rd.xy));
    return col;
}

// ─────────────────────────── Voyage entre galaxies ───────────────────────────

fn hue(h: f32) -> vec3<f32> {
    let k = vec3<f32>(5.0, 3.0, 1.0) + vec3<f32>(h * 6.0);
    return clamp(abs(vec3<f32>(fmod(k.x, 6.0), fmod(k.y, 6.0), fmod(k.z, 6.0)) - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Une galaxie spirale vue de dessus : bras logarithmiques, noyau, poussière, étoiles.
fn galaxy_img(uv: vec2<f32>, seed: f32, tint: f32, spin: f32, tilt: f32) -> vec3<f32> {
    var p = rot2(spin) * uv;
    p.y = p.y / tilt;
    let r = length(p);
    let a = atan2(p.y, p.x);
    let arms = 2.0 + floor(seed * 3.0);
    let wind = 2.0 + seed * 2.0;
    let arm = pow(0.5 + 0.5 * cos(arms * (a - wind * log(r + 0.04))), 2.4 + seed * 2.0);
    let disc = exp(-r * 2.6) * (0.35 + 1.5 * arm * smoothstep(0.02, 0.2, r));
    let core = exp(-r * r * 160.0) * 2.4 + exp(-r * 14.0) * 0.5;
    let dust = 0.7 + 0.3 * vnoise(p * 14.0 + seed * 11.0);
    let n = vnoise(p * 70.0 + seed * 3.0);
    let stars = smoothstep(0.86, 1.0, n) * (0.35 + 1.0 * arm) * exp(-r * 2.0);
    let c_arm = mix(vec3<f32>(0.55, 0.7, 1.0), hue(tint), 0.5);
    let c_core = vec3<f32>(1.0, 0.85, 0.6);
    var col = c_arm * disc * dust + c_core * core + vec3<f32>(1.0) * stars * 1.4;
    col *= 1.0 - smoothstep(0.82, 1.0, r);
    return col;
}

fn render_galaxy(uv: vec2<f32>, aspect: f32) -> vec4<f32> {
    let t = cine.res.z;
    let dur = max(cine.extra.w, 1.0);
    let k = t / dur * 10.0;
    var p = vec2<f32>(uv.x * aspect, uv.y);
    let r = length(p);
    let ang = atan2(p.y, p.x);
    var col = vec3<f32>(0.0);
    let seed = cine.extra.z;

    // 0 - 3 s : chute dans la galaxie de départ (zoom exponentiel vers son cœur)
    let fall = smoothstep(0.0, 3.2, k);
    let tunnel_in = smoothstep(2.6, 4.2, k);
    let tunnel = tunnel_in * (1.0 - smoothstep(6.4, 7.0, k));
    if (k < 4.4) {
        let zoom = 1.0 / (1.0 + 60.0 * fall * fall * fall);
        let spin = 0.3 * k + 2.5 * fall * fall;
        let g = galaxy_img(p * zoom * 1.1, seed, cine.extra.x, spin, 0.62 + 0.38 * fall);
        // étoiles de fond
        let bg = starfield(normalize(vec3<f32>(p * 0.8, 1.0))) * (1.0 - 0.8 * fall);
        col = (g + bg) * (1.0 - tunnel_in);
    }
    // 2.6 - 7 s : tunnel hyperspatial, étoiles étirées, bleu -> blanc -> rouge
    if (tunnel > 0.001) {
        let heat = smoothstep(3.0, 6.6, k);
        let hcol = mix(vec3<f32>(0.35, 0.6, 1.0), mix(vec3<f32>(1.0, 0.95, 0.9), vec3<f32>(1.0, 0.35, 0.2), smoothstep(0.5, 1.0, heat)), smoothstep(0.0, 0.5, heat));
        var acc = vec3<f32>(0.0);
        for (var layer = 0; layer < 3; layer++) {
            let fl = f32(layer);
            let nrad = 18.0 + 10.0 * fl;
            let sector = floor(ang / (2.0 * PI) * nrad * 6.0);
            let h = hash12(vec2<f32>(sector, fl * 13.0 + seed * 7.0));
            // distance radiale qui s'éloigne du centre : z = 1/r est la profondeur
            let flow = fmod(1.0 / (r + 0.02) * 0.05 - k * (0.5 + heat * 2.2) * 0.5 + h * 9.0, 1.0);
            let streak = smoothstep(0.0, 0.04, flow) * (1.0 - smoothstep(0.04, 0.6 + 0.3 * heat, flow));
            let on = step(0.55, h);
            acc += hcol * streak * on * (0.5 + fl * 0.3) * (0.15 + 0.8 * r) * 0.45;
        }
        // anneau-portail au centre qui se resserre
        let ring_r = mix(0.9, 0.12, smoothstep(3.4, 6.8, k));
        let ring = exp(-pow((r - ring_r) * 14.0, 2.0)) * 0.9;
        let core = exp(-r * r * mix(2.0, 60.0, smoothstep(4.0, 6.8, k))) * (0.2 + 0.7 * heat);
        // aberration chromatique : le rouge part plus loin que le bleu
        acc += vec3<f32>(1.0, 0.5, 0.3) * ring * 0.5 * heat + hcol * ring + vec3<f32>(1.0) * core * 0.6;
        col += acc * tunnel;
    }
    // 6.6 - 7.4 s : l'iris se ferme, flash blanc
    let iris = smoothstep(6.5, 7.2, k);
    if (iris > 0.0) {
        let rad = mix(1.4, 0.0, iris * iris);
        col *= 1.0 - smoothstep(rad, rad + 0.05, r);
    }
    let flash = exp(-pow((k - 7.2) * 3.2, 2.0)) * 2.4;
    col += vec3<f32>(1.0, 0.97, 0.92) * flash;
    // 7.2 - 10 s : la galaxie d'arrivée éclot (petit point qui grandit, rotation lente)
    if (k > 7.0) {
        let bloom = smoothstep(7.1, 9.3, k);
        let scale = mix(14.0, 1.0, 1.0 - pow(1.0 - bloom, 3.0));
        let g = galaxy_img(p * scale * 1.1, seed + 0.37, cine.extra.y, 0.9 + 0.2 * k, 0.78);
        let bg = starfield(normalize(vec3<f32>(p * 0.8 + vec2<f32>(0.3, 0.2), 1.0)));
        col += (g + bg) * smoothstep(7.0, 7.7, k);
    }
    let alpha = 1.0 - smoothstep(9.4, 10.0, k);
    return vec4<f32>(col, alpha);
}

// ─────────────────────────── Fragment ───────────────────────────

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let aspect = cine.res.x / max(cine.res.y, 1.0);
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let mode = i32(cine.res.w + 0.5);
    var col = vec3<f32>(0.0);
    var alpha = 1.0;
    if (mode == 2) {
        let r = render_galaxy(ndc, aspect);
        col = r.rgb;
        alpha = r.a;
    } else {
        let forward = normalize(cine.look.xyz - cine.cam.xyz);
        var world_up = vec3<f32>(0.0, 1.0, 0.0);
        if (abs(forward.y) > 0.95) {
            world_up = vec3<f32>(0.0, 0.0, 1.0);
        }
        let right_v = normalize(cross(forward, world_up));
        let up_v = cross(right_v, forward);
        let fov = 0.9;
        let rd = normalize(forward + right_v * ndc.x * aspect * fov + up_v * ndc.y * fov);
        if (mode == 1) {
            // Tunnel : la caméra regarde toujours dans l'axe (+z) ; en marche arrière, vers l'arrière
            let look = vec3<f32>(ndc.x * aspect * fov, ndc.y * fov, 1.0);
            col = render_tunnel(cine.cam.xyz, normalize(look));
        } else {
            col = render_drill(cine.cam.xyz, rd);
        }
    }
    col *= (1.0 - cine.fx2.y);
    col += vec3<f32>(1.0) * cine.fx2.w * 0.0;
    // Tons : ACES et gamma (la sortie de l'interface est en sRGB)
    col = (col * (2.51 * col + 0.03)) / (col * (2.43 * col + 0.59) + 0.14);
    col = max(col, vec3<f32>(0.0));
    return vec4<f32>(col, alpha);
}

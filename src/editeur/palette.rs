//! La grande palette (E2, §2 de `roadmaps/fait/ROADMAP-0.12-editeur.md`).
//!
//! Rangée dans l'espace **OKLCH** (clarté perçue L, saturation C, teinte H) : deux couleurs de même
//! L paraissent vraiment aussi claires, contrairement au HSV (où les jaunes semblent plus clairs
//! que les bleus). Grille de 36 teintes (tous les 10°) × 12 clartés, pour 4 saturations (vif,
//! moyen, pastel, terne) : 1 728 couleurs sans trou ; les couleurs trop saturées pour l'écran sont
//! ramenées à la plus saturée possible de même teinte et de même clarté. Les gris sont à part.

use super::format::{Material, PaletteEntry};

pub const HUES: usize = 36;
pub const LIGHTS: usize = 12;
pub const GREYS: usize = 16;

/// Saturation de toute la grille.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Saturation {
    #[default]
    Vif,
    Moyen,
    Pastel,
    Terne,
}

impl Saturation {
    pub const ALL: [Saturation; 4] = [Saturation::Vif, Saturation::Moyen, Saturation::Pastel, Saturation::Terne];

    pub fn name(self) -> &'static str {
        match self {
            Saturation::Vif => "Vif",
            Saturation::Moyen => "Moyen",
            Saturation::Pastel => "Pastel",
            Saturation::Terne => "Terne",
        }
    }

    /// Saturation OKLCH visée (avant de la ramener dans l'écran).
    fn chroma(self) -> f32 {
        match self {
            Saturation::Vif => 0.32,
            Saturation::Moyen => 0.13,
            Saturation::Pastel => 0.075,
            Saturation::Terne => 0.04,
        }
    }

    /// Clartés de la grille : le pastel est plus clair.
    fn lightness(self, row: usize) -> f32 {
        let t = row as f32 / (LIGHTS - 1) as f32;
        match self {
            Saturation::Pastel => 0.55 + 0.42 * t,
            _ => 0.18 + 0.78 * t,
        }
    }
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// sRGB (0..1) -> OKLab.
pub fn srgb_to_oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(to_linear);
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab -> sRGB linéaire (peut sortir de 0..1 : hors de l'écran).
fn oklab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let [ll, a, b] = lab;
    let l = (ll + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (ll - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (ll - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

/// OKLCH -> sRGB 8 bits, la saturation réduite juste assez pour rester affichable.
pub fn oklch_to_rgb8(l: f32, c: f32, h_deg: f32) -> [u8; 3] {
    let h = h_deg.to_radians();
    let inside = |c: f32| oklab_to_linear([l, c * h.cos(), c * h.sin()]).iter().all(|v| (-1e-4..=1.0001).contains(v));
    let mut c = c;
    if !inside(c) {
        // Dichotomie sur la saturation
        let (mut lo, mut hi) = (0.0f32, c);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if inside(mid) { lo = mid } else { hi = mid }
        }
        c = lo;
    }
    oklab_to_linear([l, c * h.cos(), c * h.sin()]).map(|v| (to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8)
}

/// (L, C, H en degrés) d'une couleur 8 bits.
pub fn rgb8_to_oklch(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [l, a, b] = srgb_to_oklab(rgb.map(|c| c as f32 / 255.0));
    (l, (a * a + b * b).sqrt(), b.atan2(a).to_degrees().rem_euclid(360.0))
}

/// Couleur de la grille : teinte `col` (0..36), clarté `row` (0 = sombre).
pub fn grid_color(sat: Saturation, col: usize, row: usize) -> [u8; 3] {
    oklch_to_rgb8(sat.lightness(row), sat.chroma(), col as f32 * (360.0 / HUES as f32))
}

/// Les gris, du noir au blanc (clartés perçues régulières).
pub fn grey(i: usize) -> [u8; 3] {
    oklch_to_rgb8(i as f32 / (GREYS - 1) as f32, 0.0, 0.0)
}

/// Palettes thématiques.
pub const THEMES: [(&str, &[&str]); 7] = [
    ("Peaux", &["FFE0CC", "F5CBA7", "E8B48A", "D9A066", "C68642", "A86B3C", "8D5524", "6B3E1F", "4A2912", "F2C6C2", "C9E4B4", "B8D8F0"]),
    ("Cheveux", &["FAF0BE", "E6CE8A", "C9A15A", "A0522D", "8B4513", "5C3317", "2C1B10", "0E0E10", "B7B7B7", "E8E8E8", "C0392B", "8E44AD"]),
    ("Metaux", &["E8E8EC", "C0C0C8", "8C8C96", "5A5A64", "B87333", "D4AF37", "CD7F32", "4A6B8A", "6C7A89", "3B3B40"]),
    ("Coques", &["DADDE2", "B8BEC6", "8E97A3", "646E7A", "3E4651", "2A2F36", "C9C2B0", "7C8471", "4F5B66", "1E2329"]),
    ("Militaire", &["4B5320", "6B7B3A", "8A9A5B", "C2B280", "A0896B", "5B4E3A", "3B4A3A", "556B6E", "2F3A40", "8B8D7A"]),
    ("Neons", &["FF2D95", "FF6EC7", "B026FF", "4D4DFF", "00E5FF", "00FF9C", "B6FF00", "FFE600", "FF7A00", "FF3131"]),
    ("Pixel World", &[]),
];

/// Couleur hexadécimale « RRGGBB » (avec ou sans #).
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    let s = if s.len() == 3 { s.chars().flat_map(|c| [c, c]).collect::<String>() } else { s.to_string() };
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(&s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Couleurs d'un thème (le thème « Pixel World » : les blocs de l'ancien éditeur).
pub fn theme(i: usize) -> Vec<PaletteEntry> {
    let (name, list) = THEMES[i];
    if name == "Pixel World" {
        return super::edit::starter_palette();
    }
    let material = match name {
        "Metaux" => Material::Metal,
        "Neons" => Material::Lumineuse,
        _ => Material::Mate,
    };
    list.iter().filter_map(|h| parse_hex(h)).map(|rgb| PaletteEntry { rgb, material }).collect()
}

/// Ordre d'affichage de la palette d'un modèle : gris d'abord (du sombre au clair), puis par
/// teinte (tranches de 10°), puis par clarté. Renvoie les index de `entries`.
pub fn sorted(entries: &[PaletteEntry]) -> Vec<usize> {
    let mut keyed: Vec<(usize, (u8, i32, i32))> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let (l, c, h) = rgb8_to_oklch(e.rgb);
            let key = if c < 0.025 { (0, 0, (l * 1000.0) as i32) } else { (1, (h / 10.0) as i32, (l * 1000.0) as i32) };
            (i, key)
        })
        .collect();
    keyed.sort_by_key(|k| k.1);
    keyed.into_iter().map(|k| k.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trips_and_greys_are_grey() {
        for rgb in [[0u8, 0, 0], [255, 255, 255], [200, 30, 30], [12, 140, 250], [90, 200, 70]] {
            let (l, c, h) = rgb8_to_oklch(rgb);
            let back = oklch_to_rgb8(l, c, h);
            for k in 0..3 {
                assert!((back[k] as i32 - rgb[k] as i32).abs() <= 1, "{rgb:?} -> {back:?}");
            }
        }
        for i in 0..GREYS {
            let g = grey(i);
            assert!(g[0] == g[1] && g[1] == g[2], "{g:?}");
        }
        assert_eq!(grey(0), [0, 0, 0]);
        assert_eq!(grey(GREYS - 1), [255, 255, 255]);
    }

    #[test]
    fn the_grid_has_1728_colors_with_even_lightness() {
        let mut all = std::collections::HashSet::new();
        for sat in Saturation::ALL {
            for row in 0..LIGHTS {
                let mut lights = Vec::new();
                for col in 0..HUES {
                    let c = grid_color(sat, col, row);
                    all.insert((sat as u8, col, row, c));
                    lights.push(rgb8_to_oklch(c).0);
                }
                // Même ligne : même clarté perçue (c'est tout l'intérêt de OKLCH)
                let (lo, hi) = lights.iter().fold((1.0f32, 0.0f32), |a, l| (a.0.min(*l), a.1.max(*l)));
                assert!(hi - lo < 0.02, "{sat:?} ligne {row} : {lo}..{hi}");
            }
        }
        assert_eq!(all.len(), 4 * HUES * LIGHTS);
        // Le vif est plus saturé que le terne
        let c = |s: Saturation| rgb8_to_oklch(grid_color(s, 0, 6)).1;
        assert!(c(Saturation::Vif) > c(Saturation::Moyen) && c(Saturation::Moyen) > c(Saturation::Terne));
    }

    #[test]
    fn themes_hex_and_sorting() {
        assert_eq!(parse_hex("#FF8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("0f0"), Some([0, 255, 0]));
        assert_eq!(parse_hex("xyz"), None);
        for i in 0..THEMES.len() {
            assert!(theme(i).len() >= 10, "{}", THEMES[i].0);
        }
        assert!(theme(2).iter().all(|e| e.material == Material::Metal));
        let e = |rgb: [u8; 3]| PaletteEntry { rgb, material: Material::Mate };
        let list = [e([0, 0, 255]), e([200, 200, 200]), e([255, 0, 0]), e([20, 20, 20]), e([120, 0, 0])];
        // Gris (sombre puis clair), puis rouges (sombre puis clair), puis bleu
        assert_eq!(sorted(&list), vec![3, 1, 4, 2, 0]);
    }
}

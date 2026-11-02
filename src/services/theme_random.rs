//! Chromatic random palettes in OKLCH for Dark / Light theme modes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::config::{ThemeMode, ThemePalette};

static SEED_BUMP: AtomicU64 = AtomicU64::new(1);

/// Build a new palette for `mode` using a clock + counter seed.
/// `coherence`: 100 = harmonized ladders; 0 = fully independent RGB per role.
pub fn random_palette(mode: ThemeMode, coherence: u8) -> ThemePalette {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15);
    let bump = SEED_BUMP.fetch_add(1, Ordering::Relaxed);
    random_palette_seeded(
        mode,
        nanos ^ bump.wrapping_mul(0x9e3779b97f4a7c15),
        coherence,
    )
}

/// Deterministic palette for tests and reproducible draws.
pub fn random_palette_seeded(mode: ThemeMode, seed: u64, coherence: u8) -> ThemePalette {
    let coherence = coherence.min(100);
    let mut rng = Rng::new(seed);
    if coherence == 0 {
        return gen_wild(&mut rng);
    }
    let structured = match mode {
        ThemeMode::Dark => gen_dark(&mut rng),
        ThemeMode::Light => gen_light(&mut rng),
    };
    if coherence == 100 {
        return structured;
    }
    let wild = gen_wild(&mut rng);
    blend_palettes(&structured, &wild, coherence)
}

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid the zero state; xorshift locks there.
        Self {
            state: if seed == 0 { 0xdeadbeefcafeu64 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545f4914f6cdd1d)
    }

    fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f01()
    }

    fn bool_weighted(&mut self, true_parts: u32, of: u32) -> bool {
        self.f01() * f64::from(of) < f64::from(true_parts)
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }
}

/// Every role is an independent random sRGB color — no Dark/Light constraints.
fn gen_wild(rng: &mut Rng) -> ThemePalette {
    ThemePalette {
        bg: random_hex(rng),
        surface: random_hex(rng),
        surface_2: random_hex(rng),
        surface_press: random_hex(rng),
        text: random_hex(rng),
        muted: random_hex(rng),
        accent: random_hex(rng),
        accent_press: random_hex(rng),
        danger: random_hex(rng),
        danger_press: random_hex(rng),
        ok: random_hex(rng),
        text_on_accent: random_hex(rng),
    }
}

fn random_hex(rng: &mut Rng) -> String {
    format!("#{:02x}{:02x}{:02x}", rng.byte(), rng.byte(), rng.byte())
}

/// `coherence` 100 → structured; 0 → wild. Intermediate values lerp sRGB channels.
fn blend_palettes(structured: &ThemePalette, wild: &ThemePalette, coherence: u8) -> ThemePalette {
    let t = f64::from(coherence) / 100.0;
    ThemePalette {
        bg: lerp_hex(&structured.bg, &wild.bg, t),
        surface: lerp_hex(&structured.surface, &wild.surface, t),
        surface_2: lerp_hex(&structured.surface_2, &wild.surface_2, t),
        surface_press: lerp_hex(&structured.surface_press, &wild.surface_press, t),
        text: lerp_hex(&structured.text, &wild.text, t),
        muted: lerp_hex(&structured.muted, &wild.muted, t),
        accent: lerp_hex(&structured.accent, &wild.accent, t),
        accent_press: lerp_hex(&structured.accent_press, &wild.accent_press, t),
        danger: lerp_hex(&structured.danger, &wild.danger, t),
        danger_press: lerp_hex(&structured.danger_press, &wild.danger_press, t),
        ok: lerp_hex(&structured.ok, &wild.ok, t),
        text_on_accent: lerp_hex(&structured.text_on_accent, &wild.text_on_accent, t),
    }
}

fn lerp_hex(structured: &str, wild: &str, t: f64) -> String {
    let (sr, sg, sb) = parse_rgb(structured).unwrap_or((0, 0, 0));
    let (wr, wg, wb) = parse_rgb(wild).unwrap_or((0, 0, 0));
    let r = (f64::from(sr) * t + f64::from(wr) * (1.0 - t)).round() as u8;
    let g = (f64::from(sg) * t + f64::from(wg) * (1.0 - t)).round() as u8;
    let b = (f64::from(sb) * t + f64::from(wb) * (1.0 - t)).round() as u8;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn gen_dark(rng: &mut Rng) -> ThemePalette {
    let h = rng.range(0.0, 360.0);
    let accent_h = accent_harmony(rng, h);

    let bg_l = rng.range(0.15, 0.22);
    let surface_l = rng.range(bg_l + 0.04, (bg_l + 0.10).min(0.32));
    let surface_2_l = rng.range(surface_l + 0.03, (surface_l + 0.08).min(0.40));
    let press_l = rng.range(surface_2_l + 0.02, (surface_2_l + 0.07).min(0.48));
    let muted_l = rng.range(0.62, 0.74);
    let text_l = rng.range(0.90, 0.97);
    let accent_l = rng.range(0.64, 0.76);
    let accent_c = rng.range(0.12, 0.19);
    let neut_c = rng.range(0.015, 0.045);

    let bg = oklch_hex(bg_l, neut_c, h);
    let surface = oklch_hex(surface_l, neut_c * 0.9, h);
    let surface_2 = oklch_hex(surface_2_l, neut_c * 0.85, h);
    let surface_press = oklch_hex(press_l, neut_c * 0.8, h);
    let mut text = oklch_hex(text_l, neut_c * 0.4, h);
    let mut muted = oklch_hex(muted_l, neut_c * 0.7, h);
    let mut accent = oklch_hex(accent_l, accent_c, accent_h);

    let danger_h = wrap_hue(25.0 + rng.range(-10.0, 10.0));
    let danger_l = rng.range(0.62, 0.72);
    let danger_c = rng.range(0.16, 0.22);
    let mut danger = oklch_hex(danger_l, danger_c, danger_h);
    let danger_press = oklch_hex((danger_l - 0.07).max(0.40), danger_c * 0.95, danger_h);

    let ok_h = wrap_hue(150.0 + rng.range(-12.0, 12.0));
    let ok_l = rng.range(0.68, 0.78);
    let ok_c = rng.range(0.14, 0.20);
    let mut ok = oklch_hex(ok_l, ok_c, ok_h);

    ensure_contrast(&mut text, &bg, 7.0, true);
    ensure_contrast(&mut muted, &bg, 3.0, true);
    ensure_contrast(&mut accent, &bg, 3.0, true);
    ensure_contrast(&mut danger, &bg, 3.0, true);
    ensure_contrast(&mut ok, &bg, 3.0, true);
    let text_on_accent = resolve_text_on_accent(&mut accent, accent_h);
    let accent_press = press_of(&accent, 0.35);

    ThemePalette {
        bg,
        surface,
        surface_2,
        surface_press,
        text,
        muted,
        accent,
        accent_press,
        danger,
        danger_press,
        ok,
        text_on_accent,
    }
}

fn gen_light(rng: &mut Rng) -> ThemePalette {
    let h = rng.range(0.0, 360.0);
    let accent_h = accent_harmony(rng, h);

    let text_l = rng.range(0.22, 0.32);
    let muted_l = rng.range(text_l + 0.12, 0.52);
    let press_l = rng.range(0.82, 0.90);
    let surface_2_l = rng.range(press_l + 0.02, (press_l + 0.05).min(0.94));
    let bg_l = rng.range(0.94, 0.97);
    let surface_l = rng.range(0.985, 0.995);
    let accent_l = rng.range(0.48, 0.60);
    let accent_c = rng.range(0.12, 0.19);
    let neut_c = rng.range(0.010, 0.035);

    let bg = oklch_hex(bg_l, neut_c * 0.5, h);
    let surface = oklch_hex(surface_l, neut_c * 0.3, h);
    let surface_2 = oklch_hex(surface_2_l, neut_c * 0.7, h);
    let surface_press = oklch_hex(press_l, neut_c * 0.8, h);
    let mut text = oklch_hex(text_l, neut_c * 0.6, h);
    let mut muted = oklch_hex(muted_l, neut_c * 0.7, h);
    let mut accent = oklch_hex(accent_l, accent_c, accent_h);

    let danger_h = wrap_hue(25.0 + rng.range(-10.0, 10.0));
    let danger_l = rng.range(0.50, 0.60);
    let danger_c = rng.range(0.16, 0.22);
    let mut danger = oklch_hex(danger_l, danger_c, danger_h);
    let danger_press = oklch_hex((danger_l - 0.07).max(0.35), danger_c * 0.95, danger_h);

    let ok_h = wrap_hue(150.0 + rng.range(-12.0, 12.0));
    let ok_l = rng.range(0.48, 0.58);
    let ok_c = rng.range(0.14, 0.20);
    let mut ok = oklch_hex(ok_l, ok_c, ok_h);

    ensure_contrast(&mut text, &bg, 7.0, false);
    ensure_contrast(&mut muted, &bg, 3.0, false);
    ensure_contrast(&mut accent, &bg, 3.0, false);
    ensure_contrast(&mut danger, &bg, 3.0, false);
    ensure_contrast(&mut ok, &bg, 3.0, false);
    let text_on_accent = resolve_text_on_accent(&mut accent, accent_h);
    let accent_press = press_of(&accent, 0.30);

    ThemePalette {
        bg,
        surface,
        surface_2,
        surface_press,
        text,
        muted,
        accent,
        accent_press,
        danger,
        danger_press,
        ok,
        text_on_accent,
    }
}

fn accent_harmony(rng: &mut Rng, base_h: f64) -> f64 {
    // 3/5 same, 1/5 analogous, 1/5 complementary
    let roll = rng.f01() * 5.0;
    if roll < 3.0 {
        base_h
    } else if roll < 4.0 {
        let delta = rng.range(18.0, 36.0);
        if rng.bool_weighted(1, 2) {
            wrap_hue(base_h + delta)
        } else {
            wrap_hue(base_h - delta)
        }
    } else {
        wrap_hue(base_h + 180.0 + rng.range(-12.0, 12.0))
    }
}

fn wrap_hue(h: f64) -> f64 {
    let mut x = h % 360.0;
    if x < 0.0 {
        x += 360.0;
    }
    x
}

fn press_of(hex: &str, l_floor: f64) -> String {
    let (r, g, b) = parse_rgb(hex).unwrap_or((0, 0, 0));
    let (l, c, h) = srgb_to_oklch(r, g, b);
    oklch_hex((l - 0.07).max(l_floor), c * 0.95, h)
}

/// Choose white or near-black on accent; nudge accent lightness if needed for ≥4.5.
fn resolve_text_on_accent(accent: &mut String, accent_h: f64) -> String {
    let white = "#ffffff".to_string();
    let black = oklch_hex(0.20, 0.02, accent_h);
    let use_white = contrast_ratio(&white, accent) >= contrast_ratio(&black, accent);
    let mut on = if use_white { white } else { black };
    if contrast_ratio(&on, accent) >= 4.5 {
        return on;
    }
    // Mid yellows etc.: push accent away from the label until WCAG AA holds.
    let (r, g, b) = parse_rgb(accent).unwrap_or((0, 0, 0));
    let (l, c, h) = srgb_to_oklch(r, g, b);
    let step = if use_white { -0.02 } else { 0.02 };
    let mut nl = l;
    for _ in 0..40 {
        nl = (nl + step).clamp(0.15, 0.90);
        *accent = oklch_hex(nl, c, h);
        on = if use_white {
            "#ffffff".to_string()
        } else {
            oklch_hex(0.20, 0.02, accent_h)
        };
        if contrast_ratio(&on, accent) >= 4.5 {
            return on;
        }
    }
    on
}

/// Nudge `fg` lightness toward light (`toward_light`) or dark until contrast vs `bg` holds.
fn ensure_contrast(fg: &mut String, bg: &str, min_ratio: f64, toward_light: bool) {
    if contrast_ratio(fg, bg) >= min_ratio {
        return;
    }
    let (r, g, b) = parse_rgb(fg).unwrap_or((0, 0, 0));
    let (l, c, h) = srgb_to_oklch(r, g, b);
    let step = if toward_light { 0.02 } else { -0.02 };
    let mut nl = l;
    for _ in 0..40 {
        nl = (nl + step).clamp(0.05, 0.98);
        *fg = oklch_hex(nl, c, h);
        if contrast_ratio(fg, bg) >= min_ratio {
            return;
        }
    }
}

fn oklch_hex(l: f64, c: f64, h: f64) -> String {
    let (r, g, b) = oklch_to_srgb_gamut(l, c, h);
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// Reduce chroma until the color is inside sRGB, then convert.
fn oklch_to_srgb_gamut(l: f64, c: f64, h: f64) -> (u8, u8, u8) {
    let mut lo = 0.0;
    let mut hi = c.max(0.0);
    let mut best = (0u8, 0u8, 0u8);
    // Prefer largest in-gamut chroma.
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if let Some(rgb) = oklch_to_srgb_checked(l, mid, h) {
            best = rgb;
            lo = mid;
        } else {
            hi = mid;
        }
    }
    if let Some(rgb) = oklch_to_srgb_checked(l, lo, h) {
        rgb
    } else if let Some(rgb) = oklch_to_srgb_checked(l, 0.0, h) {
        rgb
    } else {
        best
    }
}

fn oklch_to_srgb_checked(l: f64, c: f64, h_deg: f64) -> Option<(u8, u8, u8)> {
    let h = h_deg.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();
    let (lr, lg, lb) = oklab_to_linear_srgb(l, a, b);
    if ![lr, lg, lb].iter().all(|v| (-0.001..=1.001).contains(v)) {
        return None;
    }
    Some((
        linear_to_srgb8(lr.clamp(0.0, 1.0)),
        linear_to_srgb8(lg.clamp(0.0, 1.0)),
        linear_to_srgb8(lb.clamp(0.0, 1.0)),
    ))
}

fn oklab_to_linear_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    (
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3,
    )
}

fn linear_to_srgb8(c: f64) -> u8 {
    let v = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb8_to_linear(c: u8) -> f64 {
    let v = f64::from(c) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_to_oklab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let lr = srgb8_to_linear(r);
    let lg = srgb8_to_linear(g);
    let lb = srgb8_to_linear(b);
    let l = 0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb;
    let m = 0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb;
    let s = 0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb;
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();
    (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )
}

fn srgb_to_oklch(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (l, a, bb) = srgb_to_oklab(r, g, b);
    let c = (a * a + bb * bb).sqrt();
    let h = if c < 1e-8 {
        0.0
    } else {
        wrap_hue(bb.atan2(a).to_degrees())
    };
    (l, c, h)
}

fn parse_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

fn relative_luminance(hex: &str) -> f64 {
    let (r, g, b) = parse_rgb(hex).unwrap_or((0, 0, 0));
    0.2126 * srgb8_to_linear(r) + 0.7152 * srgb8_to_linear(g) + 0.0722 * srgb8_to_linear(b)
}

fn contrast_ratio(a: &str, b: &str) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_hex_rrggbb(s: &str) -> bool {
        let Some(h) = s.strip_prefix('#') else {
            return false;
        };
        h.len() == 6
            && h.bytes().all(|b| b.is_ascii_hexdigit())
            && h.bytes().all(|b| !b.is_ascii_uppercase())
    }

    fn assert_palette_shape(p: &ThemePalette) {
        for (key, hex) in [
            ("bg", &p.bg),
            ("surface", &p.surface),
            ("surface_2", &p.surface_2),
            ("surface_press", &p.surface_press),
            ("text", &p.text),
            ("muted", &p.muted),
            ("accent", &p.accent),
            ("accent_press", &p.accent_press),
            ("danger", &p.danger),
            ("danger_press", &p.danger_press),
            ("ok", &p.ok),
            ("text_on_accent", &p.text_on_accent),
        ] {
            assert!(is_hex_rrggbb(hex), "{key}={hex}");
        }
    }

    fn lum(hex: &str) -> f64 {
        relative_luminance(hex)
    }

    fn hue_of(hex: &str) -> f64 {
        let (r, g, b) = parse_rgb(hex).expect("rgb");
        srgb_to_oklch(r, g, b).2
    }

    fn hue_near(h: f64, target: f64, tol: f64) -> bool {
        let d = (wrap_hue(h - target)).min(360.0 - wrap_hue(h - target));
        d <= tol
    }

    #[test]
    fn dark_seed_orders_and_contrasts() {
        // Includes a yellow-ish hue path via seed search coverage below.
        let p = random_palette_seeded(ThemeMode::Dark, 42, 100);
        assert_palette_shape(&p);
        assert!(lum(&p.bg) < lum(&p.surface), "bg < surface");
        assert!(lum(&p.surface) < lum(&p.text), "surface < text");
        assert!(contrast_ratio(&p.text, &p.bg) >= 7.0);
        assert!(contrast_ratio(&p.muted, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.accent, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.danger, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.ok, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.text_on_accent, &p.accent) >= 4.5);
        assert!(hue_near(hue_of(&p.danger), 25.0, 25.0), "danger red");
        assert!(hue_near(hue_of(&p.ok), 150.0, 30.0), "ok green");
    }

    #[test]
    fn light_seed_orders_and_contrasts() {
        let p = random_palette_seeded(ThemeMode::Light, 42, 100);
        assert_palette_shape(&p);
        assert!(lum(&p.text) < lum(&p.muted), "text < muted");
        assert!(lum(&p.bg) <= lum(&p.surface) + 1e-9, "bg <= surface");
        assert!(lum(&p.surface) > lum(&p.text), "surface lightest vs text");
        assert!(contrast_ratio(&p.text, &p.bg) >= 7.0);
        assert!(contrast_ratio(&p.muted, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.accent, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.danger, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.ok, &p.bg) >= 3.0);
        assert!(contrast_ratio(&p.text_on_accent, &p.accent) >= 4.5);
        assert!(hue_near(hue_of(&p.danger), 25.0, 25.0), "danger red");
        assert!(hue_near(hue_of(&p.ok), 150.0, 30.0), "ok green");
    }

    #[test]
    fn yellow_and_blue_seeds_stay_in_gamut() {
        // Seeds that historically push yellow (~95°) and blue (~250°) accents.
        for seed in [7u64, 99, 12345, 0xabcdef, 0xfeedface] {
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                let p = random_palette_seeded(mode, seed, 100);
                assert_palette_shape(&p);
                assert!(contrast_ratio(&p.text, &p.bg) >= 7.0);
                assert!(contrast_ratio(&p.text_on_accent, &p.accent) >= 4.5);
            }
        }
    }

    #[test]
    fn coherence_zero_is_wild_hex() {
        let p = random_palette_seeded(ThemeMode::Dark, 7, 0);
        assert_palette_shape(&p);
        // Wild draw is unconstrained — do not assert luminance order.
    }

    #[test]
    fn coherence_blend_is_between() {
        let structured = random_palette_seeded(ThemeMode::Dark, 99, 100);
        let mid = random_palette_seeded(ThemeMode::Dark, 99, 50);
        assert_palette_shape(&mid);
        // Same seed: structured half of the draw matches; mid accent should differ from 100%.
        assert_ne!(mid.accent, structured.accent);
    }
}

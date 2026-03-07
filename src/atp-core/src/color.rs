//! # Color — Color Manipulation & Accessibility
//!
//! Parse hex/RGB/HSL, contrast ratios, WCAG accessibility checks,
//! palette generation, ANSI-256 mapping.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An sRGB colour with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// HSL representation (0–360 hue, 0–100 saturation/lightness).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

/// Named colour format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorFormat {
    Hex,
    Rgb,
    Hsl,
}

/// WCAG conformance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WcagLevel {
    Fail,
    AA,
    AAA,
}

/// Accessibility report for a foreground/background pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityReport {
    pub foreground: String,
    pub background: String,
    pub contrast_ratio: f64,
    pub normal_text: WcagLevel,
    pub large_text: WcagLevel,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a hex string (#RGB, #RRGGBB, RRGGBB, RGB).
pub fn parse_hex(s: &str) -> Result<Rgb, String> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
            Ok(Rgb { r, g, b })
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&s[1..2], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&s[2..3], 16).map_err(|e| e.to_string())?;
            Ok(Rgb {
                r: r * 17,
                g: g * 17,
                b: b * 17,
            })
        }
        _ => Err(format!("invalid hex colour: {s}")),
    }
}

/// Parse "rgb(r, g, b)" or "r, g, b".
pub fn parse_rgb(s: &str) -> Result<Rgb, String> {
    let inner = s
        .trim()
        .trim_start_matches("rgb(")
        .trim_end_matches(')')
        .trim();
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return Err(format!("expected 3 components, got {}", parts.len()));
    }
    let r: u8 = parts[0].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let g: u8 = parts[1].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let b: u8 = parts[2].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    Ok(Rgb { r, g, b })
}

/// Parse "hsl(h, s%, l%)" or "h, s, l".
pub fn parse_hsl(s: &str) -> Result<Hsl, String> {
    let inner = s
        .trim()
        .trim_start_matches("hsl(")
        .trim_end_matches(')')
        .trim();
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim().trim_end_matches('%')).collect();
    if parts.len() != 3 {
        return Err(format!("expected 3 components, got {}", parts.len()));
    }
    let h: f64 = parts[0].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
    let s: f64 = parts[1].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
    let l: f64 = parts[2].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
    Ok(Hsl { h, s, l })
}

/// Auto-detect format and parse.
pub fn parse_color(s: &str) -> Result<Rgb, String> {
    let t = s.trim();
    if t.starts_with('#') || t.chars().all(|c| c.is_ascii_hexdigit()) && (t.len() == 3 || t.len() == 6) {
        parse_hex(t)
    } else if t.starts_with("rgb(") || t.starts_with("RGB(") {
        parse_rgb(t)
    } else if t.starts_with("hsl(") || t.starts_with("HSL(") {
        let hsl = parse_hsl(t)?;
        Ok(hsl_to_rgb(&hsl))
    } else if t.contains(',') {
        // Guess RGB
        parse_rgb(t)
    } else {
        Err(format!("cannot parse colour: {t}"))
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

/// RGB → hex string (#RRGGBB).
pub fn to_hex(c: &Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// RGB → HSL.
pub fn rgb_to_hsl(c: &Rgb) -> Hsl {
    let r = c.r as f64 / 255.0;
    let g = c.g as f64 / 255.0;
    let b = c.b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-10 {
        return Hsl {
            h: 0.0,
            s: 0.0,
            l: l * 100.0,
        };
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-10 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < 1e-10 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    Hsl {
        h: h * 60.0,
        s: s * 100.0,
        l: l * 100.0,
    }
}

/// HSL → RGB.
pub fn hsl_to_rgb(c: &Hsl) -> Rgb {
    let h = c.h / 360.0;
    let s = c.s / 100.0;
    let l = c.l / 100.0;

    if s.abs() < 1e-10 {
        let v = (l * 255.0).round() as u8;
        return Rgb { r: v, g: v, b: v };
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }

    let r = (hue_to_rgb(p, q, h + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue_to_rgb(p, q, h) * 255.0).round() as u8;
    let b = (hue_to_rgb(p, q, h - 1.0 / 3.0) * 255.0).round() as u8;
    Rgb { r, g, b }
}

// ---------------------------------------------------------------------------
// Contrast / Accessibility (WCAG 2.1)
// ---------------------------------------------------------------------------

/// Relative luminance (WCAG 2.1 definition).
pub fn relative_luminance(c: &Rgb) -> f64 {
    fn linearize(v: f64) -> f64 {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = linearize(c.r as f64 / 255.0);
    let g = linearize(c.g as f64 / 255.0);
    let b = linearize(c.b as f64 / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Contrast ratio between two colours (WCAG 2.1).
pub fn contrast_ratio(a: &Rgb, b: &Rgb) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG conformance for normal text.
pub fn wcag_normal(ratio: f64) -> WcagLevel {
    if ratio >= 7.0 {
        WcagLevel::AAA
    } else if ratio >= 4.5 {
        WcagLevel::AA
    } else {
        WcagLevel::Fail
    }
}

/// WCAG conformance for large text (≥18pt or ≥14pt bold).
pub fn wcag_large(ratio: f64) -> WcagLevel {
    if ratio >= 4.5 {
        WcagLevel::AAA
    } else if ratio >= 3.0 {
        WcagLevel::AA
    } else {
        WcagLevel::Fail
    }
}

/// Full accessibility report for a colour pair.
pub fn accessibility(fg: &Rgb, bg: &Rgb) -> AccessibilityReport {
    let ratio = contrast_ratio(fg, bg);
    AccessibilityReport {
        foreground: to_hex(fg),
        background: to_hex(bg),
        contrast_ratio: (ratio * 100.0).round() / 100.0,
        normal_text: wcag_normal(ratio),
        large_text: wcag_large(ratio),
    }
}

// ---------------------------------------------------------------------------
// Palette generation
// ---------------------------------------------------------------------------

/// Complementary colour (180° rotation on hue wheel).
pub fn complementary(c: &Rgb) -> Rgb {
    let mut hsl = rgb_to_hsl(c);
    hsl.h = (hsl.h + 180.0) % 360.0;
    hsl_to_rgb(&hsl)
}

/// Analogous palette (±30°).
pub fn analogous(c: &Rgb) -> [Rgb; 3] {
    let hsl = rgb_to_hsl(c);
    let mut h1 = hsl;
    h1.h = (hsl.h + 330.0) % 360.0;
    let mut h2 = hsl;
    h2.h = (hsl.h + 30.0) % 360.0;
    [hsl_to_rgb(&h1), *c, hsl_to_rgb(&h2)]
}

/// Triadic palette (120° separation).
pub fn triadic(c: &Rgb) -> [Rgb; 3] {
    let hsl = rgb_to_hsl(c);
    let mut h1 = hsl;
    h1.h = (hsl.h + 120.0) % 360.0;
    let mut h2 = hsl;
    h2.h = (hsl.h + 240.0) % 360.0;
    [*c, hsl_to_rgb(&h1), hsl_to_rgb(&h2)]
}

/// Generate `n` evenly-spaced hues at the given saturation and lightness.
pub fn palette(n: usize, saturation: f64, lightness: f64) -> Vec<Rgb> {
    if n == 0 {
        return Vec::new();
    }
    let step = 360.0 / n as f64;
    (0..n)
        .map(|i| {
            hsl_to_rgb(&Hsl {
                h: step * i as f64,
                s: saturation,
                l: lightness,
            })
        })
        .collect()
}

/// Lighten a colour by `amount` percent (0–100).
pub fn lighten(c: &Rgb, amount: f64) -> Rgb {
    let mut hsl = rgb_to_hsl(c);
    hsl.l = (hsl.l + amount).min(100.0);
    hsl_to_rgb(&hsl)
}

/// Darken a colour by `amount` percent (0–100).
pub fn darken(c: &Rgb, amount: f64) -> Rgb {
    let mut hsl = rgb_to_hsl(c);
    hsl.l = (hsl.l - amount).max(0.0);
    hsl_to_rgb(&hsl)
}

/// Mix two colours equally.
pub fn mix(a: &Rgb, b: &Rgb) -> Rgb {
    Rgb {
        r: ((a.r as u16 + b.r as u16) / 2) as u8,
        g: ((a.g as u16 + b.g as u16) / 2) as u8,
        b: ((a.b as u16 + b.b as u16) / 2) as u8,
    }
}

// ---------------------------------------------------------------------------
// ANSI-256 mapping
// ---------------------------------------------------------------------------

/// Map an RGB colour to the nearest ANSI-256 index.
pub fn to_ansi256(c: &Rgb) -> u8 {
    // First check grey ramp (232-255)
    if c.r == c.g && c.g == c.b {
        if c.r < 8 {
            return 16; // black
        }
        if c.r > 248 {
            return 231; // white
        }
        return (((c.r as f64 - 8.0) / 247.0 * 24.0).round() as u8) + 232;
    }
    // 6×6×6 colour cube (16-231)
    let ri = ((c.r as f64 / 255.0 * 5.0).round() as u8).min(5);
    let gi = ((c.g as f64 / 255.0 * 5.0).round() as u8).min(5);
    let bi = ((c.b as f64 / 255.0 * 5.0).round() as u8).min(5);
    16 + 36 * ri + 6 * gi + bi
}

/// ANSI escape code to set foreground colour.
pub fn ansi_fg(c: &Rgb) -> String {
    format!("\x1b[38;2;{};{};{}m", c.r, c.g, c.b)
}

/// ANSI escape code to set background colour.
pub fn ansi_bg(c: &Rgb) -> String {
    format!("\x1b[48;2;{};{};{}m", c.r, c.g, c.b)
}

/// ANSI reset.
pub fn ansi_reset() -> &'static str {
    "\x1b[0m"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_6() {
        let c = parse_hex("#FF8000").unwrap();
        assert_eq!(c, Rgb { r: 255, g: 128, b: 0 });
    }

    #[test]
    fn test_parse_hex_3() {
        let c = parse_hex("#F80").unwrap();
        assert_eq!(c, Rgb { r: 255, g: 136, b: 0 });
    }

    #[test]
    fn test_parse_rgb() {
        let c = parse_rgb("rgb(10, 20, 30)").unwrap();
        assert_eq!(c, Rgb { r: 10, g: 20, b: 30 });
    }

    #[test]
    fn test_parse_hsl() {
        let hsl = parse_hsl("hsl(120, 50%, 50%)").unwrap();
        assert!((hsl.h - 120.0).abs() < 0.01);
    }

    #[test]
    fn test_to_hex() {
        assert_eq!(to_hex(&Rgb { r: 255, g: 0, b: 128 }), "#FF0080");
    }

    #[test]
    fn test_rgb_hsl_roundtrip() {
        let c = Rgb { r: 100, g: 150, b: 200 };
        let hsl = rgb_to_hsl(&c);
        let back = hsl_to_rgb(&hsl);
        assert!((c.r as i16 - back.r as i16).unsigned_abs() <= 1);
        assert!((c.g as i16 - back.g as i16).unsigned_abs() <= 1);
        assert!((c.b as i16 - back.b as i16).unsigned_abs() <= 1);
    }

    #[test]
    fn test_contrast_bw() {
        let black = Rgb { r: 0, g: 0, b: 0 };
        let white = Rgb { r: 255, g: 255, b: 255 };
        let ratio = contrast_ratio(&black, &white);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn test_wcag_aaa() {
        assert_eq!(wcag_normal(8.0), WcagLevel::AAA);
        assert_eq!(wcag_large(5.0), WcagLevel::AAA);
    }

    #[test]
    fn test_wcag_fail() {
        assert_eq!(wcag_normal(2.0), WcagLevel::Fail);
    }

    #[test]
    fn test_accessibility_report() {
        let fg = Rgb { r: 0, g: 0, b: 0 };
        let bg = Rgb { r: 255, g: 255, b: 255 };
        let report = accessibility(&fg, &bg);
        assert!(report.contrast_ratio > 20.0);
        assert_eq!(report.normal_text, WcagLevel::AAA);
    }

    #[test]
    fn test_complementary() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        let comp = complementary(&red);
        // Complementary of red is cyan-ish
        assert!(comp.g > 200);
        assert!(comp.b > 200);
    }

    #[test]
    fn test_triadic() {
        let c = Rgb { r: 255, g: 0, b: 0 };
        let tri = triadic(&c);
        assert_eq!(tri.len(), 3);
    }

    #[test]
    fn test_palette() {
        let p = palette(6, 80.0, 50.0);
        assert_eq!(p.len(), 6);
    }

    #[test]
    fn test_lighten_darken() {
        let c = Rgb { r: 128, g: 64, b: 32 };
        let lighter = lighten(&c, 10.0);
        let darker = darken(&c, 10.0);
        let lo = rgb_to_hsl(&lighter);
        let do_ = rgb_to_hsl(&darker);
        assert!(lo.l > do_.l);
    }

    #[test]
    fn test_mix() {
        let a = Rgb { r: 0, g: 0, b: 0 };
        let b = Rgb { r: 200, g: 100, b: 50 };
        let m = mix(&a, &b);
        assert_eq!(m, Rgb { r: 100, g: 50, b: 25 });
    }

    #[test]
    fn test_ansi256() {
        let grey = Rgb { r: 128, g: 128, b: 128 };
        let idx = to_ansi256(&grey);
        assert!(idx >= 232);
    }

    #[test]
    fn test_ansi_fg() {
        let s = ansi_fg(&Rgb { r: 255, g: 0, b: 0 });
        assert!(s.contains("255;0;0"));
    }
}

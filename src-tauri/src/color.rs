use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorInfo {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
    pub nearest_name: String,
    pub nearest_hex: String,
    pub delta_e: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColorEntry {
    pub name: String,
    pub hex: String,
}

impl ColorInfo {
    pub fn from_rgb(r: u8, g: u8, b: u8, dictionary: &[ColorEntry]) -> Self {
        let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        let (nearest_name, nearest_hex, delta_e) =
            find_nearest_color(r, g, b, dictionary);
        ColorInfo {
            r,
            g,
            b,
            hex,
            nearest_name,
            nearest_hex,
            delta_e,
        }
    }
}

/// CIE76 color difference in Lab space (simplified sRGB → Lab conversion).
fn rgb_to_lab(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    // sRGB linearize
    let linearize = |v: u8| -> f64 {
        let c = v as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let lr = linearize(r);
    let lg = linearize(g);
    let lb = linearize(b);

    // sRGB → XYZ (D65)
    let x = lr * 0.4124564 + lg * 0.3575761 + lb * 0.1804375;
    let y = lr * 0.2126729 + lg * 0.7151522 + lb * 0.0721750;
    let z = lr * 0.0193339 + lg * 0.1191920 + lb * 0.9503041;

    // XYZ → Lab
    let fx = |t: f64| -> f64 {
        if t > 0.008856 {
            t.powf(1.0 / 3.0)
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let xn = 0.95047;
    let yn = 1.00000;
    let zn = 1.08883;

    let l = 116.0 * fx(y / yn) - 16.0;
    let a = 500.0 * (fx(x / xn) - fx(y / yn));
    let b_val = 200.0 * (fx(y / yn) - fx(z / zn));

    (l, a, b_val)
}

fn delta_e_cie76(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let (l1, a1, b1v) = rgb_to_lab(r1, g1, b1);
    let (l2, a2, b2v) = rgb_to_lab(r2, g2, b2);
    let dl = l1 - l2;
    let da = a1 - a2;
    let db = b1v - b2v;
    (dl * dl + da * da + db * db).sqrt()
}

fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some((r, g, b))
}

fn find_nearest_color(r: u8, g: u8, b: u8, dict: &[ColorEntry]) -> (String, String, f64) {
    let mut best_name = "Unknown".to_string();
    let mut best_hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
    let mut best_de = f64::MAX;

    for entry in dict {
        if let Some((er, eg, eb)) = hex_to_rgb(&entry.hex) {
            let de = delta_e_cie76(r, g, b, er, eg, eb);
            if de < best_de {
                best_de = de;
                best_name = entry.name.clone();
                best_hex = entry.hex.clone();
            }
        }
    }

    (best_name, best_hex, best_de)
}

/// Load the color dictionary from the bundled JSON file.
pub fn load_dictionary(json: &str) -> Result<Vec<ColorEntry>, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

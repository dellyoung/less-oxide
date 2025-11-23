#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    fn clamp(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
            a: self.a.clamp(0.0, 1.0),
        }
    }
}

pub fn parse_color(input: &str) -> Option<Rgba> {
    let trimmed = input.trim();
    if let Some(stripped) = trimmed.strip_prefix('#') {
        return parse_hex(stripped);
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("rgba") {
        return parse_rgb_function(&lowered, true);
    }
    if lowered.starts_with("rgb") {
        return parse_rgb_function(&lowered, false);
    }
    None
}

pub fn lighten(color: Rgba, amount: f64) -> Rgba {
    let (h, s, l) = rgb_to_hsl(color);
    let new_l = (l + amount).clamp(0.0, 1.0);
    hsl_to_rgb(h, s, new_l, color.a)
}

pub fn darken(color: Rgba, amount: f64) -> Rgba {
    let (h, s, l) = rgb_to_hsl(color);
    let new_l = (l - amount).clamp(0.0, 1.0);
    hsl_to_rgb(h, s, new_l, color.a)
}

pub fn fade(color: Rgba, amount: f64) -> Rgba {
    Rgba {
        a: amount.clamp(0.0, 1.0),
        ..color
    }
    .clamp()
}

pub fn overlay(top: Rgba, bottom: Rgba) -> Rgba {
    color_blend(blend_overlay, top, bottom)
}

pub fn format_hex(color: Rgba) -> String {
    let c = color.clamp();
    format!(
        "#{:02x}{:02x}{:02x}",
        to_channel(c.r),
        to_channel(c.g),
        to_channel(c.b)
    )
}

pub fn format_rgba(color: Rgba) -> String {
    let c = color.clamp();
    let alpha = format_float(c.a);
    format!(
        "rgba({}, {}, {}, {})",
        to_channel(c.r),
        to_channel(c.g),
        to_channel(c.b),
        alpha
    )
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    match hex.len() {
        3 => {
            let r = hex_value(&hex[0..1])?;
            let g = hex_value(&hex[1..2])?;
            let b = hex_value(&hex[2..3])?;
            Some(Rgba {
                r: (r * 17) as f64 / 255.0,
                g: (g * 17) as f64 / 255.0,
                b: (b * 17) as f64 / 255.0,
                a: 1.0,
            })
        }
        6 => {
            let r = hex_value(&hex[0..2])?;
            let g = hex_value(&hex[2..4])?;
            let b = hex_value(&hex[4..6])?;
            Some(Rgba {
                r: r as f64 / 255.0,
                g: g as f64 / 255.0,
                b: b as f64 / 255.0,
                a: 1.0,
            })
        }
        8 => {
            let r = hex_value(&hex[0..2])?;
            let g = hex_value(&hex[2..4])?;
            let b = hex_value(&hex[4..6])?;
            let a = hex_value(&hex[6..8])?;
            Some(Rgba {
                r: r as f64 / 255.0,
                g: g as f64 / 255.0,
                b: b as f64 / 255.0,
                a: a as f64 / 255.0,
            })
        }
        _ => None,
    }
}

fn parse_rgb_function(input: &str, has_alpha: bool) -> Option<Rgba> {
    let start = input.find('(')? + 1;
    let end = input.rfind(')')?;
    let body = &input[start..end];
    let parts: Vec<&str> = body.split(',').map(|s| s.trim()).collect();
    if (has_alpha && parts.len() != 4) || (!has_alpha && parts.len() != 3) {
        return None;
    }
    let r = parse_u8(parts[0])?;
    let g = parse_u8(parts[1])?;
    let b = parse_u8(parts[2])?;
    let a = if has_alpha {
        parse_alpha(parts[3])?
    } else {
        1.0
    };
    Some(Rgba {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a,
    })
}

fn parse_u8(input: &str) -> Option<u8> {
    input.parse().ok()
}

fn parse_alpha(input: &str) -> Option<f64> {
    if let Some(value) = input.strip_suffix('%') {
        let num: f64 = value.parse().ok()?;
        Some((num / 100.0).clamp(0.0, 1.0))
    } else {
        input.parse().ok().map(|v: f64| v.clamp(0.0, 1.0))
    }
}

fn color_blend<F>(mode: F, bottom: Rgba, top: Rgba) -> Rgba
where
    F: Fn(f64, f64) -> f64 + Copy,
{
    let ab = bottom.a;
    let at = top.a;
    let ar = at + ab * (1.0 - at);
    let bottom_channels = [bottom.r, bottom.g, bottom.b];
    let top_channels = [top.r, top.g, top.b];
    let mut result = [0.0; 3];
    for i in 0..3 {
        let cb = bottom_channels[i];
        let cs = top_channels[i];
        let mut cr = mode(cb, cs);
        if ar > 0.0 {
            cr = (at * cs + ab * (cb - at * (cb + cs - cr))) / ar;
        }
        result[i] = cr;
    }
    Rgba {
        r: result[0],
        g: result[1],
        b: result[2],
        a: ar,
    }
    .clamp()
}

fn blend_multiply(a: f64, b: f64) -> f64 {
    a * b
}

fn blend_screen(a: f64, b: f64) -> f64 {
    a + b - a * b
}

fn blend_overlay(base: f64, overlay: f64) -> f64 {
    if base <= 0.5 {
        blend_multiply(base * 2.0, overlay)
    } else {
        blend_screen(base * 2.0 - 1.0, overlay)
    }
}

fn hex_value(hex: &str) -> Option<u8> {
    u8::from_str_radix(hex, 16).ok()
}

fn rgb_to_hsl(color: Rgba) -> (f64, f64, f64) {
    let r = color.r;
    let g = color.g;
    let b = color.b;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f64::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;

    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64, alpha: f64) -> Rgba {
    if s <= 0.0 {
        return Rgba {
            r: l,
            g: l,
            b: l,
            a: alpha,
        };
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

    Rgba { r, g, b, a: alpha }.clamp()
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    match t {
        _ if t < 1.0 / 6.0 => p + (q - p) * 6.0 * t,
        _ if t < 1.0 / 2.0 => q,
        _ if t < 2.0 / 3.0 => p + (q - p) * (2.0 / 3.0 - t) * 6.0,
        _ => p,
    }
}

fn to_channel(value: f64) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

fn format_float(value: f64) -> String {
    let mut formatted = format!("{value:.3}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    if formatted.is_empty() {
        "0".to_string()
    } else {
        formatted
    }
}

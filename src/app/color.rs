use slint::Color;

/// Parse `#RRGGBB` or `#RGB`; fallback accent blue.
pub fn parse_hex(s: &str) -> Color {
    let s = s.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::from_rgb_u8(r, g, b);
        }
    } else if hex.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..1], 16),
            u8::from_str_radix(&hex[1..2], 16),
            u8::from_str_radix(&hex[2..3], 16),
        ) {
            return Color::from_rgb_u8(r * 17, g * 17, b * 17);
        }
    }
    Color::from_rgb_u8(0x3b, 0x82, 0xf6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rrggbb() {
        let c = parse_hex("#ef4444");
        assert_eq!(c.red(), 0xef);
        assert_eq!(c.green(), 0x44);
        assert_eq!(c.blue(), 0x44);
    }
}

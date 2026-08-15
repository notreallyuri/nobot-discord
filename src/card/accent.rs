use crate::error::AppError;

pub const DEFAULT: Rgb = Rgb(0x7c, 0x5c, 0xff);

const MIN_LUMINANCE: f64 = 0.12;
const HIGHLIGHT_MIX: f64 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    pub fn to_i32(self) -> i32 {
        i32::from(self.0) << 16 | i32::from(self.1) << 8 | i32::from(self.2)
    }

    pub fn from_i32(value: i32) -> Self {
        Self(
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        )
    }

    pub fn luminance(self) -> f64 {
        fn channel(value: u8) -> f64 {
            let v = f64::from(value) / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }

    pub fn lighten(self, amount: f64) -> Self {
        self.mix(Self(255, 255, 255), amount)
    }

    pub fn mix(self, other: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| {
            (f64::from(a) + (f64::from(b) - f64::from(a)) * amount)
                .round()
                .clamp(0.0, 255.0) as u8
        };

        Self(
            blend(self.0, other.0),
            blend(self.1, other.1),
            blend(self.2, other.2),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Accent {
    pub base: String,
    pub light: String,
    pub adjusted: bool,
}

impl Accent {
    pub fn new(colour: Rgb) -> Self {
        let (base, adjusted) = ensure_legible(colour);

        Self {
            light: base.lighten(HIGHLIGHT_MIX).to_hex(),
            base: base.to_hex(),
            adjusted,
        }
    }

    pub fn pair(base: Rgb, light: Rgb) -> Self {
        let (base, base_lifted) = ensure_legible(base);
        let (light, light_lifted) = ensure_legible(light);

        Self {
            base: base.to_hex(),
            light: light.to_hex(),
            adjusted: base_lifted || light_lifted,
        }
    }

    pub fn from_stored(colour: Option<i32>) -> Self {
        Self::new(colour.map_or(DEFAULT, Rgb::from_i32))
    }
}

impl Default for Accent {
    fn default() -> Self {
        Self::new(DEFAULT)
    }
}

fn ensure_legible(colour: Rgb) -> (Rgb, bool) {
    if colour.luminance() >= MIN_LUMINANCE {
        return (colour, false);
    }

    let mut lifted = colour;
    for _ in 0..24 {
        lifted = lifted.lighten(0.08);
        if lifted.luminance() >= MIN_LUMINANCE {
            break;
        }
    }

    (lifted, true)
}

pub fn parse(input: &str) -> Result<Rgb, AppError> {
    let hex = input.trim().trim_start_matches('#');

    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Message(invalid(input)));
    }

    let pair = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16).map_err(|_| AppError::Message(invalid(input)))
    };

    match hex.len() {
        3 => {
            let digit = |i: usize| pair(i..i + 1).map(|value| value * 17);
            Ok(Rgb(digit(0)?, digit(1)?, digit(2)?))
        }
        6 => Ok(Rgb(pair(0..2)?, pair(2..4)?, pair(4..6)?)),
        _ => Err(AppError::Message(invalid(input))),
    }
}

fn invalid(input: &str) -> String {
    let shown: String = input.chars().take(20).collect();
    format!("`{shown}` isn't a colour — use a hex code like `#7c5cff`.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_accepted_shape() {
        let expected = Rgb(0x7c, 0x5c, 0xff);
        for input in ["#7c5cff", "7c5cff", "#7C5CFF", "  #7c5cff  "] {
            assert_eq!(parse(input).expect(input), expected, "input: {input}");
        }

        assert_eq!(parse("#f0a").expect("shorthand"), Rgb(0xff, 0x00, 0xaa));
    }

    #[test]
    fn rejects_nonsense() {
        for input in [
            "", "#", "purple", "#12345", "#gggggg", "#1234567", "0x7c5cff",
        ] {
            assert!(parse(input).is_err(), "should have rejected: {input:?}");
        }
    }

    #[test]
    fn rejects_multibyte_input_without_panicking() {
        for input in ["€", "€abc", "日本", "ééé", "aé12", "#日本語", "🎨🎨"] {
            assert!(parse(input).is_err(), "should have rejected: {input:?}");
        }
    }

    #[test]
    fn round_trips_through_storage() {
        for colour in [DEFAULT, Rgb(0, 0, 0), Rgb(255, 255, 255), Rgb(1, 2, 3)] {
            assert_eq!(Rgb::from_i32(colour.to_i32()), colour);
        }
        assert_eq!(Rgb(255, 255, 255).to_i32(), 0xff_ff_ff);
    }

    #[test]
    fn bright_colours_are_left_alone() {
        let accent = Accent::new(DEFAULT);
        assert!(!accent.adjusted);
        assert_eq!(accent.base, "#7c5cff");
    }

    #[test]
    fn dark_colours_are_lifted_until_legible() {
        for colour in [Rgb(0, 0, 0), Rgb(0, 0, 0x40), Rgb(0x10, 0, 0)] {
            let accent = Accent::new(colour);
            assert!(accent.adjusted, "{colour:?} should have been lifted");

            let lifted = parse(&accent.base).expect("valid hex");
            assert!(
                lifted.luminance() >= MIN_LUMINANCE,
                "{colour:?} lifted to {:?}, luminance {}",
                accent.base,
                lifted.luminance()
            );
        }
    }

    #[test]
    fn the_highlight_is_lighter_than_the_base() {
        let accent = Accent::new(Rgb(0x30, 0x80, 0x40));
        let base = parse(&accent.base).expect("valid");
        let light = parse(&accent.light).expect("valid");
        assert!(light.luminance() > base.luminance());
    }

    #[test]
    fn a_pair_keeps_both_stops_it_was_given() {
        let accent = Accent::pair(Rgb(0x22, 0xd3, 0xee), Rgb(0xa7, 0x8b, 0xfa));

        assert_eq!(accent.base, "#22d3ee");
        assert_eq!(accent.light, "#a78bfa");
        assert!(!accent.adjusted);
    }

    #[test]
    fn a_pair_lifts_whichever_stop_is_too_dark() {
        let accent = Accent::pair(Rgb(0, 0, 0), Rgb(0xa7, 0x8b, 0xfa));

        assert!(accent.adjusted);
        assert_ne!(accent.base, "#000000");
        assert_eq!(accent.light, "#a78bfa", "the legible stop is left alone");
    }

    #[test]
    fn a_missing_stored_colour_falls_back_to_the_default() {
        assert_eq!(Accent::from_stored(None).base, Accent::default().base);
        assert_eq!(
            Accent::from_stored(Some(Rgb(0x22, 0xcc, 0x88).to_i32())).base,
            "#22cc88"
        );
    }
}

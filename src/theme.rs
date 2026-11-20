//! Colour palettes. Every colour used by the UI comes from here.

use clap::ValueEnum;
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ThemeName {
    Dark,
    Light,
    Mono,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dim: Color,
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
    pub rx: Color,
    pub tx: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            dim: Color::DarkGray,
            accent: Color::Cyan,
            good: Color::Green,
            warn: Color::Yellow,
            bad: Color::Red,
            rx: Color::LightGreen,
            tx: Color::LightMagenta,
            highlight_fg: Color::Black,
            highlight_bg: Color::Cyan,
        }
    }

    pub fn light() -> Self {
        Self {
            dim: Color::Gray,
            accent: Color::Blue,
            good: Color::Green,
            warn: Color::Rgb(180, 100, 0),
            bad: Color::Red,
            rx: Color::Green,
            tx: Color::Magenta,
            highlight_fg: Color::White,
            highlight_bg: Color::Blue,
        }
    }

    pub fn mono() -> Self {
        Self {
            dim: Color::Gray,
            accent: Color::White,
            good: Color::White,
            warn: Color::White,
            bad: Color::White,
            rx: Color::White,
            tx: Color::Gray,
            highlight_fg: Color::Black,
            highlight_bg: Color::White,
        }
    }

    /// Colour for a 0..=100 usage value: green, then yellow, then red.
    pub fn level(&self, percent: f64) -> Color {
        if percent < 50.0 {
            self.good
        } else if percent < 80.0 {
            self.warn
        } else {
            self.bad
        }
    }
}

impl From<ThemeName> for Theme {
    fn from(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::Mono => Self::mono(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_thresholds() {
        let t = Theme::dark();
        assert_eq!(t.level(0.0), t.good);
        assert_eq!(t.level(49.9), t.good);
        assert_eq!(t.level(50.0), t.warn);
        assert_eq!(t.level(79.9), t.warn);
        assert_eq!(t.level(80.0), t.bad);
        assert_eq!(t.level(100.0), t.bad);
    }
}

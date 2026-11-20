//! Small reusable pieces: bordered blocks, usage bars, popup placement.

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, LineGauge};

use crate::theme::Theme;

/// A rounded block with a coloured title.
pub fn block<'a>(title: impl Into<Line<'a>>, theme: &Theme) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.dim))
        .title(title.into().style(Style::new().fg(theme.accent).bold()))
}

/// A one-line usage bar, coloured green / yellow / red by `ratio`.
pub fn usage_bar<'a>(label: impl Into<Line<'a>>, ratio: f64, theme: &Theme) -> LineGauge<'a> {
    let ratio = ratio.clamp(0.0, 1.0);
    LineGauge::default()
        .filled_style(Style::new().fg(theme.level(ratio * 100.0)))
        .unfilled_style(Style::new().fg(theme.dim))
        .ratio(ratio)
        .label(label)
}

/// A `width` x `height` rectangle centred in `area` (shrunk to fit).
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_and_clamps() {
        let area = Rect::new(0, 0, 100, 40);
        assert_eq!(centered_rect(20, 10, area), Rect::new(40, 15, 20, 10));
        assert_eq!(centered_rect(200, 100, area), area);
    }
}

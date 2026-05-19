use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Calculates a centered rect within the given area.
pub fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Splits the content area (inside borders) into title, host list, and status bar.
pub fn content_layout(area: Rect) -> (Rect, Rect, Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // host list (takes remaining space)
            Constraint::Length(1), // status bar
        ])
        .split(area);

    (rows[0], rows[1], rows[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_calculation() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(area, 50, 70);

        // 50% of 100 = 50 wide, 70% of 50 = 35 tall
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 35);

        // Centered: x = 25, y = (100-70)/2% of 50 → 8 (rounded)
        assert_eq!(centered.x, 25);
        // y varies by rounding, just verify it's positive and reasonable
        assert!(centered.y > 0 && centered.y < 20);
    }

    #[test]
    fn test_layout_handles_small_terminal() {
        let area = Rect::new(0, 0, 20, 10);
        let centered = centered_rect(area, 50, 70);
        assert!(centered.width > 0);
        assert!(centered.height > 0);

        let (title, list, status) = content_layout(centered);
        assert_eq!(title.height, 1);
        assert_eq!(status.height, 1);
        assert!(list.height > 0);
    }
}

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Calculates a centered rect within the given area.
/// `percent_x` and `percent_y` are the percentage of the parent area to use.
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

/// Splits the main panel into title bar, host list, and status bar.
pub fn main_layout(area: Rect) -> (Rect, Rect, Rect) {
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // host list
            Constraint::Length(1), // status bar
        ])
        .split(area);

    (cols[0], cols[1], cols[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_calculation() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(area, 70, 80);

        // 70% of 100 = 70 wide, 80% of 50 = 40 tall
        assert_eq!(centered.width, 70);
        assert_eq!(centered.height, 40);

        // Should be centered: x = (100 - 70) / 2 = 15, y = (50 - 40) / 2 = 5
        assert_eq!(centered.x, 15);
        assert_eq!(centered.y, 5);
    }

    #[test]
    fn test_layout_handles_small_terminal() {
        let area = Rect::new(0, 0, 20, 10);
        // Should not panic with small dimensions
        let centered = centered_rect(area, 70, 80);
        assert!(centered.width > 0);
        assert!(centered.height > 0);

        let (title, list, status) = main_layout(centered);
        assert!(title.height == 1);
        assert!(status.height == 1);
        assert!(list.height > 0);
    }
}

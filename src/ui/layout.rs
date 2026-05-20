use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum width below which we render a "terminal too small" message
/// instead of the host list.
pub const MIN_TERMINAL_WIDTH: u16 = 60;
/// Minimum height below which we render a "terminal too small" message.
pub const MIN_TERMINAL_HEIGHT: u16 = 10;

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

/// Decides which optional columns are shown for the current inner width.
/// Status and Alias are always visible. Account is dropped first, then Port,
/// then Host. Thresholds are tuned for the centered panel; even compact
/// widths keep Account visible because the panel itself is narrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnVisibility {
    pub show_account: bool,
    pub show_host: bool,
    pub show_port: bool,
}

impl ColumnVisibility {
    /// `width` is the inner width available for the table (after border).
    pub fn for_width(width: u16) -> Self {
        Self {
            show_account: width >= 40,
            show_port: width >= 30,
            show_host: width >= 22,
        }
    }
}

/// Pre-computed widths for the five data columns. Header text lengths set
/// the lower bound so the headers never clip; actual host rows extend this.
#[derive(Debug, Clone, Copy)]
pub struct ColumnWidths {
    pub alias: u16,
    pub account: u16,
    pub host: u16,
    pub port: u16,
}

impl ColumnWidths {
    pub const HEADER_ALIAS: u16 = 5; // "Alias"
    pub const HEADER_ACCOUNT: u16 = 7; // "Account"
    pub const HEADER_HOST: u16 = 4; // "Host"
    pub const HEADER_PORT: u16 = 4; // "Port"

    pub fn header_baseline() -> Self {
        Self {
            alias: Self::HEADER_ALIAS,
            account: Self::HEADER_ACCOUNT,
            host: Self::HEADER_HOST,
            port: Self::HEADER_PORT,
        }
    }

    pub fn extend_with(
        &mut self,
        alias_len: usize,
        account_len: usize,
        host_len: usize,
        port_len: usize,
    ) {
        self.alias = self.alias.max(alias_len as u16);
        self.account = self.account.max(account_len as u16);
        self.host = self.host.max(host_len as u16);
        self.port = self.port.max(port_len as u16);
    }
}

/// Column constraints derived from the actual content widths. A small
/// `pad` (default 2) is added on top of each data column so the visual
/// separation stays consistent regardless of the longest row. A `Min(2)`
/// spacer immediately before Status absorbs any leftover width so Status
/// always sits at the right edge of the table area.
pub fn host_table_constraints_for(
    widths: &ColumnWidths,
    visibility: &ColumnVisibility,
) -> Vec<Constraint> {
    let pad: u16 = 2;
    let mut cols = Vec::with_capacity(6);
    cols.push(Constraint::Length(widths.alias + pad));
    if visibility.show_account {
        cols.push(Constraint::Length(widths.account + pad));
    }
    if visibility.show_host {
        cols.push(Constraint::Length(widths.host + pad));
    }
    if visibility.show_port {
        cols.push(Constraint::Length(widths.port + pad));
    }
    cols.push(Constraint::Min(2)); // spacer pushes Status to the right edge
    cols.push(Constraint::Length(2)); // Status
    cols
}

impl ColumnWidths {
    /// Sum of all visible column widths (incl. pad and Status) used to size
    /// the centered panel so the table sits flush against its border. The
    /// spacer column contributes its `Min(2)` floor.
    pub fn total_with_pad(&self, visibility: &ColumnVisibility) -> u16 {
        let pad: u16 = 2;
        // Column spacing of 1 between each pair of columns (ratatui default).
        let mut total: u16 = self.alias + pad;
        let mut col_count = 1;
        if visibility.show_account {
            total += self.account + pad;
            col_count += 1;
        }
        if visibility.show_host {
            total += self.host + pad;
            col_count += 1;
        }
        if visibility.show_port {
            total += self.port + pad;
            col_count += 1;
        }
        total += 2; // spacer minimum
        total += 2; // Status
        col_count += 2;
        // Inter-column spacing.
        total + (col_count as u16).saturating_sub(1)
    }
}

/// Splits the inner area (inside the outer border) into table area + status row.
pub fn host_panel_layout(area: Rect) -> (Rect, Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (rows[0], rows[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_calculation() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(area, 50, 70);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 35);
        assert_eq!(centered.x, 25);
    }

    #[test]
    fn test_column_visibility_full_width() {
        let v = ColumnVisibility::for_width(120);
        assert!(v.show_account);
        assert!(v.show_host);
        assert!(v.show_port);
    }

    #[test]
    fn test_column_visibility_keeps_account_in_centered_panel() {
        // A centered panel inner width is roughly 60-70; Account must stay.
        let v = ColumnVisibility::for_width(50);
        assert!(v.show_account);
        assert!(v.show_host);
        assert!(v.show_port);
    }

    #[test]
    fn test_column_visibility_hides_account_below_40() {
        let v = ColumnVisibility::for_width(35);
        assert!(!v.show_account);
    }

    #[test]
    fn test_column_visibility_hides_port_below_30() {
        let v = ColumnVisibility::for_width(25);
        assert!(!v.show_account);
        assert!(!v.show_port);
        assert!(v.show_host);
    }

    #[test]
    fn test_column_visibility_hides_host_below_22() {
        let v = ColumnVisibility::for_width(20);
        assert!(!v.show_account);
        assert!(!v.show_port);
        assert!(!v.show_host);
    }

    #[test]
    fn test_constraints_count_matches_visibility() {
        // 5 data slots + spacer + status; visibility may drop optional slots.
        let widths = ColumnWidths::header_baseline();
        let full = ColumnVisibility::for_width(120);
        assert_eq!(host_table_constraints_for(&widths, &full).len(), 6);

        let no_account = ColumnVisibility::for_width(35);
        assert_eq!(host_table_constraints_for(&widths, &no_account).len(), 5);

        let only_status_alias = ColumnVisibility::for_width(20);
        // Alias + spacer + Status = 3
        assert_eq!(
            host_table_constraints_for(&widths, &only_status_alias).len(),
            3
        );
    }

    #[test]
    fn test_column_widths_extend_picks_max() {
        let mut w = ColumnWidths::header_baseline();
        w.extend_with(20, 4, 18, 5);
        assert_eq!(w.alias, 20);
        assert_eq!(w.account, ColumnWidths::HEADER_ACCOUNT.max(4));
        assert_eq!(w.host, 18);
        assert_eq!(w.port, 5);
    }

    #[test]
    fn test_host_panel_layout_splits() {
        let area = Rect::new(0, 0, 80, 20);
        let (table, status) = host_panel_layout(area);
        assert_eq!(status.height, 1);
        assert!(table.height > 0);
    }
}

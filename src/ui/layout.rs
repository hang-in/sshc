use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum width below which we render a "terminal too small" message
/// instead of the host list.
pub const MIN_TERMINAL_WIDTH: u16 = 60;
/// Minimum height below which we render a "terminal too small" message.
pub const MIN_TERMINAL_HEIGHT: u16 = 10;

/// Decides which optional columns are shown for the current terminal width.
/// Status and Alias are always visible. Account is dropped first, then Port,
/// then Host (BRIEF_V3 §5 Q6 priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnVisibility {
    pub show_account: bool,
    pub show_host: bool,
    pub show_port: bool,
}

impl ColumnVisibility {
    /// `width` is the inner width available for the table (after border).
    pub fn for_width(width: u16) -> Self {
        // Thresholds account for the Status (2) + Alias (≥12) baseline.
        // Adding Account (12) brings the baseline to roughly 80; below 80 we
        // hide Account first, then Port, then Host.
        Self {
            show_account: width >= 60,
            show_port: width >= 38,
            show_host: width >= 30,
        }
    }
}

/// Column constraints for the 5-column host table, filtered by visibility.
/// Order: Alias, [Account], [Host], [Port], Status.
pub fn host_table_constraints(visibility: &ColumnVisibility) -> Vec<Constraint> {
    let mut cols = Vec::with_capacity(5);
    cols.push(Constraint::Min(12)); // Alias
    if visibility.show_account {
        cols.push(Constraint::Length(12)); // Account
    }
    if visibility.show_host {
        cols.push(Constraint::Min(15)); // Host
    }
    if visibility.show_port {
        cols.push(Constraint::Length(6)); // Port
    }
    cols.push(Constraint::Length(2)); // Status
    cols
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
    fn test_column_visibility_full_width() {
        let v = ColumnVisibility::for_width(120);
        assert!(v.show_account);
        assert!(v.show_host);
        assert!(v.show_port);
    }

    #[test]
    fn test_column_visibility_hides_account_first() {
        let v = ColumnVisibility::for_width(50);
        assert!(!v.show_account);
        assert!(v.show_host);
        assert!(v.show_port);
    }

    #[test]
    fn test_column_visibility_hides_port_next() {
        let v = ColumnVisibility::for_width(35);
        assert!(!v.show_account);
        assert!(!v.show_port);
        assert!(v.show_host);
    }

    #[test]
    fn test_column_visibility_hides_host_last() {
        let v = ColumnVisibility::for_width(28);
        assert!(!v.show_account);
        assert!(!v.show_port);
        assert!(!v.show_host);
    }

    #[test]
    fn test_constraints_count_matches_visibility() {
        let full = ColumnVisibility::for_width(120);
        assert_eq!(host_table_constraints(&full).len(), 5);

        let no_account = ColumnVisibility::for_width(50);
        assert_eq!(host_table_constraints(&no_account).len(), 4);

        let only_status_alias = ColumnVisibility::for_width(20);
        assert_eq!(host_table_constraints(&only_status_alias).len(), 2);
    }

    #[test]
    fn test_host_panel_layout_splits() {
        let area = Rect::new(0, 0, 80, 20);
        let (table, status) = host_panel_layout(area);
        assert_eq!(status.height, 1);
        assert!(table.height > 0);
    }
}

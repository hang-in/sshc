pub mod forms;
pub mod layout;
pub mod list;
pub mod modal;
pub mod status_bar;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::ui::layout::{
    host_panel_layout, ColumnVisibility, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH,
};

/// Minimum and maximum sizes for the dynamically-sized centered panel.
const MIN_PANEL_WIDTH: u16 = 50;
const MIN_PANEL_HEIGHT: u16 = 10;
const MAX_PANEL_WIDTH: u16 = 110;
const MAX_PANEL_HEIGHT: u16 = 32;
/// Bottom-status block (2 rows) + header row + 2 cells of border.
const PANEL_CHROME_HEIGHT: u16 = 5;
/// Border + 1 cell padding on each side.
const PANEL_CHROME_WIDTH: u16 = 2;

/// Renders the entire TUI. Panel is centered and sized to the data:
/// width matches the widest row content (clamped to MIN/MAX), height
/// matches the host count (also clamped). Falls back to a "too small"
/// notice if the terminal is below MIN_TERMINAL_*.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    if size.width < MIN_TERMINAL_WIDTH || size.height < MIN_TERMINAL_HEIGHT {
        let msg = format!(
            "terminal too small (need ≥{}x{})",
            MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        );
        let para = Paragraph::new(msg).alignment(Alignment::Center);
        f.render_widget(para, size);
        return;
    }

    let fallback_user = list::fallback_user();
    let widths = list::compute_column_widths(app, &fallback_user);

    // Decide visibility based on the *available* width: if the terminal is
    // wide enough to fit every column, show them all; otherwise hide
    // priority-by-priority.
    let available_inner = size.width.saturating_sub(PANEL_CHROME_WIDTH);
    let visibility = ColumnVisibility::for_width(available_inner);

    // Compute the desired panel size from the data.
    let desired_width = widths.total_with_pad(&visibility) + PANEL_CHROME_WIDTH;
    let desired_height = (app.host_count() as u16).saturating_add(PANEL_CHROME_HEIGHT);

    let panel_width = desired_width
        .clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH)
        .min(size.width);
    let panel_height = desired_height
        .clamp(MIN_PANEL_HEIGHT, MAX_PANEL_HEIGHT)
        .min(size.height);

    let panel_x = size.x + (size.width.saturating_sub(panel_width)) / 2;
    let panel_y = size.y + (size.height.saturating_sub(panel_height)) / 2;
    let panel = Rect::new(panel_x, panel_y, panel_width, panel_height);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(list::title_line(app.host_count(), app.total_host_count()));
    let inner = outer.inner(panel);
    f.render_widget(outer, panel);

    let (table_area, status_area) = host_panel_layout(inner);

    let (table, mut state) = list::create_host_table(app, table_area.width);
    let table = table.row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD),
    );
    f.render_stateful_widget(table, table_area, &mut state);

    let visible_msg = app.status_message.as_ref().filter(|m| m.is_visible());
    let (status_text, status_style) = if let Some(msg) = visible_msg {
        (
            ratatui::text::Text::from(vec![
                ratatui::text::Line::from(msg.text().to_string()),
                ratatui::text::Line::from(""),
            ]),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            list::status_line(app.filter_mode, &app.filter_query),
            Style::default().add_modifier(Modifier::DIM),
        )
    };
    let status_widget = Paragraph::new(status_text).style(status_style);
    f.render_widget(status_widget, status_area);
}

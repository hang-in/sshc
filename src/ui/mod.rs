pub mod forms;
pub mod layout;
pub mod list;
pub mod modal;
pub mod status_bar;

use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::ui::layout::{host_panel_layout, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH};

/// Renders the entire TUI. Layout: full-screen, single-cell border block,
/// 5-column host table (columns hide on narrow widths), bottom status row.
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

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(list::title_line(app.host_count(), app.total_host_count()));
    let inner = outer.inner(size);
    f.render_widget(outer, size);

    let (table_area, status_area) = host_panel_layout(inner);

    let (table, mut state) = list::create_host_table(app, table_area.width);
    let table = table.row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD),
    );
    f.render_stateful_widget(table, table_area, &mut state);

    let visible_msg = app.status_message.as_ref().filter(|m| m.is_visible());
    let (status_line, status_style) = if let Some(msg) = visible_msg {
        (
            ratatui::text::Line::from(msg.text().to_string()),
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
    let status_widget = Paragraph::new(status_line).style(status_style);
    f.render_widget(status_widget, status_area);
}

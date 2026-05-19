pub mod layout;
pub mod list;
pub mod status_bar;

use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::App;

/// Renders the entire TUI.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // Compact panel: ~50% width, ~70% height
    let panel = layout::centered_rect(size, 50, 70);

    // Bordered block with title — inner area excludes border cells
    let block = Block::default()
        .borders(Borders::ALL)
        .title(ratatui::widgets::block::Title::from(list::title_line(
            app.host_count(),
            app.total_host_count(),
        )));
    let inner = block.inner(panel);
    f.render_widget(block, panel);

    // Split inner area into header, list, status
    let (_title_area, list_area, status_area) = layout::content_layout(inner);

    // Draw column header
    let header = ratatui::widgets::Paragraph::new(ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            format!("{:<width$}", "Alias", width = list::ALIAS_WIDTH),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        ratatui::text::Span::styled("  Host", Style::default().add_modifier(Modifier::BOLD)),
        ratatui::text::Span::styled(
            format!("{:>width$}", "Port", width = list::PORT_WIDTH),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
    f.render_widget(header, _title_area);

    // Draw host list with highlight
    let (host_list, mut state) = list::create_host_list(app);
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD);
    let host_list = host_list.highlight_style(selected_style);
    f.render_stateful_widget(host_list, list_area, &mut state);

    // Draw status bar
    let status_line = list::status_line(app.filter_mode, &app.filter_query);
    let status_widget = ratatui::widgets::Paragraph::new(status_line)
        .style(Style::default().add_modifier(Modifier::DIM));
    f.render_widget(status_widget, status_area);
}

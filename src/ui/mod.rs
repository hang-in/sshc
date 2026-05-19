pub mod layout;
pub mod list;

use ratatui::style::{Modifier, Style};
use ratatui::Frame;

use crate::app::App;

/// Renders the entire TUI.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    let panel = layout::centered_rect(size, 70, 80);
    let (_title_area, list_area, status_area) = layout::main_layout(panel);

    // Draw borders around the panel
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(ratatui::widgets::block::Title::from(list::title_line(
            app.host_count(),
            app.total_host_count(),
        )));
    f.render_widget(block, panel);

    // Draw host list
    let (host_list, mut state) = list::create_host_list(app);

    // Highlight selected item
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

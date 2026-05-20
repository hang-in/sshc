pub mod forms;
pub mod layout;
pub mod list;
pub mod modal;
pub mod preview;
pub mod status_bar;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::ui::layout::{
    host_panel_layout, ColumnVisibility, MIN_TERMINAL_HEIGHT, MIN_TERMINAL_WIDTH,
};
use crate::ui::modal::{
    centered_rect, render_confirmation_body, render_info_body, render_modal_chrome, ModalKind,
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
/// v0.6: manage-mode preview panel shows host detail to the right of
/// the host table when the terminal is at least this wide. Below the
/// threshold, the preview is hidden and the layout falls back to the
/// pre-v0.6 single-pane look.
const PREVIEW_MIN_TERMINAL_WIDTH: u16 = 100;
/// Width of the preview pane itself (separator border included).
const PREVIEW_PANE_WIDTH: u16 = 36;

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

    // v0.6: show the right-side preview pane only when (a) the terminal
    // is wide enough that the host table doesn't have to shed columns,
    // (b) we're in the list view (modal forms don't share screen space),
    // and (c) there's actually a selected host to describe.
    let show_preview = matches!(app.mode, AppMode::List)
        && size.width >= PREVIEW_MIN_TERMINAL_WIDTH
        && app.selected_host().is_some();
    let preview_pane = if show_preview { PREVIEW_PANE_WIDTH } else { 0 };

    // Visibility is based on the table's share of the panel, not the
    // whole terminal — so the preview pane "borrowing" 36 cells doesn't
    // change which columns the table shows.
    let available_inner = size.width.saturating_sub(PANEL_CHROME_WIDTH + preview_pane);
    let visibility = ColumnVisibility::for_width(available_inner);

    // Compute the desired panel size from the data, then add the preview
    // pane width if it'll be drawn.
    let desired_width = widths.total_with_pad(&visibility) + PANEL_CHROME_WIDTH + preview_pane;
    let desired_height = (app.host_count() as u16).saturating_add(PANEL_CHROME_HEIGHT);

    // When the preview pane is visible the natural max grows so the
    // table + preview both fit; otherwise the v0.5 cap stays.
    let max_width = if show_preview {
        MAX_PANEL_WIDTH + PREVIEW_PANE_WIDTH
    } else {
        MAX_PANEL_WIDTH
    };
    let panel_width = desired_width
        .clamp(MIN_PANEL_WIDTH, max_width)
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

    // Split the table strip horizontally if the preview pane is on.
    let (list_area, preview_area) = if show_preview {
        let chunks = Layout::horizontal([
            Constraint::Min(MIN_PANEL_WIDTH - PANEL_CHROME_WIDTH),
            Constraint::Length(PREVIEW_PANE_WIDTH),
        ])
        .split(table_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (table_area, None)
    };

    let (table, mut state) = list::create_host_table(app, list_area.width);
    let table = table.row_highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD),
    );
    f.render_stateful_widget(table, list_area, &mut state);

    if let (Some(area), Some(host)) = (preview_area, app.selected_host()) {
        preview::render_preview(host, area, f);
    }

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
            list::status_line(
                app.filter_mode,
                &app.filter_query,
                app.selected_is_external(),
            ),
            Style::default().add_modifier(Modifier::DIM),
        )
    };
    let status_widget = Paragraph::new(status_text).style(status_style);
    f.render_widget(status_widget, status_area);

    // Overlay the active modal on top of the host panel. ratatui doesn't
    // auto-clear the cells under a widget, so render `Clear` first.
    if let AppMode::Modal(kind) = &app.mode {
        render_modal_overlay(f, panel, kind);
    }
}

fn render_modal_overlay(f: &mut Frame, panel: Rect, kind: &ModalKind) {
    match kind {
        ModalKind::Confirmation { prompt, .. } => {
            let area = centered_rect(panel, 70, 40);
            f.render_widget(Clear, area);
            render_modal_chrome(area, f, " Confirm ");
            render_confirmation_body(area, f, prompt);
        }
        ModalKind::Info { message, .. } => {
            let area = centered_rect(panel, 70, 50);
            f.render_widget(Clear, area);
            render_modal_chrome(area, f, " Help ");
            render_info_body(area, f, message);
        }
        ModalKind::Form(form) => {
            let area = centered_rect(panel, 70, 70);
            f.render_widget(Clear, area);
            form.render(area, f);
        }
    }
}

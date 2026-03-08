use crate::app::{AppState, PopupState};
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render(f: &mut Frame, state: &AppState) {
    let Some(popup) = &state.popup else { return };

    let area = f.area();
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    match popup {
        PopupState::SaveSearch { input, cursor } => {
            render_text_input(f, popup_area, "Save Search", input, *cursor);
        }
        PopupState::Rename { input, cursor, .. } => {
            render_text_input(f, popup_area, "Rename", input, *cursor);
        }
        PopupState::ConfirmDelete { name, .. } => {
            render_confirm(f, popup_area, name);
        }
    }
}

fn render_text_input(f: &mut Frame, area: Rect, title: &str, input: &str, cursor: usize) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::ACCENT))
        .title(format!(" {} ", title))
        .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 1 || inner.width < 2 {
        return;
    }

    let text_area = Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1);

    if input.is_empty() {
        let placeholder = Paragraph::new("Enter a name...")
            .style(Style::default().fg(theme::TEXT_DIM));
        f.render_widget(placeholder, text_area);
    } else {
        let paragraph = Paragraph::new(input)
            .style(Style::default().fg(theme::TEXT));
        f.render_widget(paragraph, text_area);
    }

    // Position cursor: count characters up to the byte cursor position
    let display_cursor = input[..cursor].chars().count() as u16;
    let cursor_x = text_area.x + display_cursor;
    let cursor_y = text_area.y;

    if cursor_x < text_area.right() {
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_confirm(f: &mut Frame, area: Rect, name: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::WARNING))
        .title(" Delete? ")
        .title_style(Style::default().fg(theme::WARNING).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 2 {
        return;
    }

    let text_area = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1);
    let hint_area = Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1);

    let prompt = format!("Delete \"{}\"?", name);
    let prompt_paragraph = Paragraph::new(prompt)
        .style(Style::default().fg(theme::TEXT));
    f.render_widget(prompt_paragraph, text_area);

    let hint = Paragraph::new("y/n")
        .style(Style::default().fg(theme::TEXT_DIM));
    f.render_widget(hint, hint_area);
}

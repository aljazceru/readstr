//! Landing screen: paste text or open a file to begin reading.

use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Element, Fill, Length};
use speedreading_app_core::AppState;

use crate::{HistoryRow, Message};

pub fn view<'a>(
    state: &'a AppState,
    paste_content: &'a text_editor::Content,
    history: &'a [HistoryRow],
    pending_delete: Option<&'a (String, String)>,
    file_not_found_error: Option<&'a str>,
) -> Element<'a, Message> {
    // Confirmation dialog overlay — shown instead of normal landing content
    if let Some((file_hash, file_name)) = pending_delete {
        let _ = file_hash; // file_hash is part of the tuple but not needed in dialog display
        let dialog = column![
            text(format!("Delete entry for {}?", file_name)).size(16),
            row![
                button("Keep Entry").on_press(Message::CancelDelete),
                button("Delete").on_press(Message::ConfirmDelete),
            ]
            .spacing(12),
        ]
        .spacing(16)
        .padding(32)
        .max_width(400);

        return container(dialog)
            .center_x(Fill)
            .center_y(Fill)
            .into();
    }

    let title = text("SpeedReader").size(36);

    let editor = text_editor(paste_content)
        .placeholder("Paste text here to start reading...")
        .on_action(Message::PasteAction)
        .height(180);

    let paste_btn = button("Start Reading").on_press(Message::LoadPastedText);

    let file_btn = button("Open File (txt / epub / pdf)").on_press(Message::OpenFile);

    let buttons = row![paste_btn, file_btn].spacing(12);

    // Status area: loading indicator, core error, or local file-not-found error
    let status: Element<'_, Message> = if state.is_loading {
        text("Loading file...").size(14).into()
    } else if let Some(err) = &state.error {
        text(format!("Error: {}", err))
            .size(14)
            .color([0.8, 0.2, 0.2])
            .into()
    } else if let Some(err) = file_not_found_error {
        text(err)
            .size(14)
            .color([0.8, 0.2, 0.2])
            .into()
    } else {
        text("").size(14).into()
    };

    // History section — only rendered when history is non-empty (D-01, D-03)
    let history_section: Option<Element<'_, Message>> = if history.is_empty() {
        None
    } else {
        let header = text("─ Recent Files ─").size(14);
        let rows: Vec<Element<'_, Message>> = history.iter().map(|hr| history_row(hr)).collect();
        let list = scrollable(column(rows).spacing(8));
        Some(column![header, list].spacing(8).into())
    };

    let mut content_items: Vec<Element<'_, Message>> = vec![
        title.into(),
        editor.into(),
        buttons.into(),
        status,
    ];
    if let Some(section) = history_section {
        content_items.push(section);
    }

    let content = column(content_items).spacing(16).padding(32).max_width(640);

    container(content).center_x(Fill).center_y(Fill).into()
}

fn history_row(hr: &HistoryRow) -> Element<'_, Message> {
    let icon = if hr.is_missing { "⚠" } else { "📄" };
    let pct = format!("{}%", hr.entry.progress_percent as u32);
    let file_hash = hr.entry.file_hash.clone();
    let file_hash_del = hr.entry.file_hash.clone();
    let file_name = hr.entry.file_name.clone();

    let name_col: Element<'_, Message> = if hr.is_missing {
        column![
            text(&hr.entry.file_name).color([0.5, 0.5, 0.5]),
            text("File not found").size(14).color([0.5, 0.5, 0.5]),
        ]
        .into()
    } else {
        text(&hr.entry.file_name).into()
    };

    let resume_msg = if hr.is_missing {
        Message::ResumeMissingFile(file_hash)
    } else {
        Message::ResumeFile(file_hash)
    };

    row![
        text(icon),
        name_col,
        text(pct).size(14),
        button("Resume").on_press(resume_msg),
        button("🗑").on_press(Message::ConfirmDeletePrompt(file_hash_del, file_name)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

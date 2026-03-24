//! Landing screen: paste text or open a file to begin reading.

use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Element, Fill, Length};
use speedreading_app_core::AppState;

use crate::Message;

pub fn view<'a>(
    state: &'a AppState,
    paste_content: &'a text_editor::Content,
) -> Element<'a, Message> {
    let title = text("SpeedReader").size(36);

    let editor = text_editor(paste_content)
        .placeholder("Paste text here to start reading...")
        .on_action(Message::PasteAction)
        .height(180);

    let paste_btn = button("Start Reading")
        .on_press(Message::LoadPastedText);

    let file_btn = button("Open File (txt / epub / pdf)")
        .on_press(Message::OpenFile);

    let buttons = row![paste_btn, file_btn].spacing(12);

    // Status area: loading spinner text or error message
    let status: Element<'_, Message> = if state.is_loading {
        text("Loading file...").size(14).into()
    } else if let Some(err) = &state.error {
        text(format!("Error: {}", err))
            .size(14)
            .color([0.8, 0.2, 0.2])
            .into()
    } else {
        text("").size(14).into()
    };

    let content = column![
        title,
        editor,
        buttons,
        status,
    ]
    .spacing(16)
    .padding(32)
    .max_width(640);

    container(content)
        .center_x(Fill)
        .center_y(Fill)
        .into()
}

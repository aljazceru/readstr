//! Landing screen: paste text or open a file to begin reading.

use iced::widget::{button, column, container, rich_text, row, span, text, text_editor};
use iced::{Background, Border, Element, Fill};
use speedreading_app_core::AppState;

use crate::{HistoryRow, Message, SYNE, ACCENT_ORANGE_DARK};

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
                button("Delete Entry").on_press(Message::ConfirmDelete),
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

    let logo_spans: Vec<iced::widget::text::Span<'static>> = vec![
        span("read").font(SYNE).size(32.0),
        span("str").font(SYNE).size(32.0).color(ACCENT_ORANGE_DARK),
    ];
    let logotype: Element<'_, Message> = rich_text(logo_spans).into();
    let subtitle: Element<'_, Message> = text("speed reading, focused")
        .size(14)
        .style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.weak.color),
        })
        .into();
    let title = column(vec![logotype, subtitle]).spacing(4);

    let editor = text_editor(paste_content)
        .placeholder("Paste text here to start reading...")
        .on_action(Message::PasteAction)
        .height(180);

    let paste_btn = button("Start Reading")
        .style(button::primary)
        .on_press(Message::LoadPastedText);

    let file_btn = button("Open File")
        .style(|theme: &iced::Theme, status| {
            use iced::widget::button::Status;
            use iced::{Background, Border, Color};
            let palette = theme.extended_palette();
            let primary_color = palette.primary.base.color;
            let alpha = match status {
                Status::Hovered => 0.8,
                Status::Pressed => 0.6,
                Status::Disabled => 0.3,
                Status::Active => 1.0,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: Color { a: alpha, ..primary_color },
                border: Border {
                    color: Color { a: alpha, ..primary_color },
                    width: 1.5,
                    radius: 2.0.into(),
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::OpenFile);

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
        let header = text("recent")
            .size(14)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.color),
            });
        let rows: Vec<Element<'_, Message>> = history.iter().map(|hr| history_row(hr)).collect();
        let list = column(rows).spacing(4);
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
    let icon = if hr.is_missing { "!" } else { "·" };
    let pct_value = hr.entry.progress_percent as u32;
    let pct = format!("{}%", pct_value);
    let file_hash = hr.entry.file_hash.clone();
    let file_hash_del = hr.entry.file_hash.clone();
    let file_name = hr.entry.file_name.clone();

    let name_col: Element<'_, Message> = if hr.is_missing {
        column![
            text(&hr.entry.file_name).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.color),
            }),
            text("File not found").size(12).style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.color),
            }),
        ]
        .into()
    } else {
        text(&hr.entry.file_name).into()
    };

    let progress_pill: Element<'_, Message> = container(
        text(pct)
            .size(12)
            .style(move |theme: &iced::Theme| iced::widget::text::Style {
                color: Some(if pct_value >= 1 {
                    // AccentOrange from theme primary
                    theme.extended_palette().primary.base.color
                } else {
                    theme.extended_palette().background.weak.color
                }),
            })
    )
    .padding([2, 8])
    .style(|theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(
            theme.extended_palette().background.strong.color
        )),
        border: Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into();

    let resume_msg = if hr.is_missing {
        Message::ResumeMissingFile(file_hash)
    } else {
        Message::ResumeFile(file_hash)
    };

    row![
        text(icon),
        name_col,
        progress_pill,
        button("Resume Reading").style(button::primary).on_press(resume_msg),
        button("Delete").style(button::text).on_press(Message::ConfirmDeletePrompt(file_hash_del, file_name)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

//! Reading screen: RSVP word display with full playback controls.

use iced::widget::{button, column, container, rich_text, row, slider, span, text};
use iced::{Border, Color, Element, Fill, Length};
use speedreading_app_core::{AppAction, AppState, WordDisplay};

use crate::widgets::seek_bar::seek_bar;
use crate::Message;

pub fn view(state: &AppState, wpm_preview: u32, group_preview: u32) -> Element<'_, Message> {
    let back_btn = button("← Back")
        .style(button::text)
        .on_press(Message::GoBack);

    // Word display area
    let word_area: Element<'_, Message> = match &state.display {
        Some(display) => word_display(display),
        None => {
            if state.is_loading {
                text("Loading...").size(48).into()
            } else {
                text("—").size(48).into()
            }
        }
    };

    // Play / Pause button
    let play_pause_label = if state.is_playing { "⏸ Pause" } else { "▶ Play" };
    let play_pause_btn = button(play_pause_label)
        .style(button::primary)
        .on_press(Message::Dispatch(AppAction::Toggle));

    // Progress / seek bar
    let progress = seek_bar(0.0_f32..=100.0_f32, state.progress_percent, |pct| {
        Message::Dispatch(AppAction::SeekToProgress { percent: pct })
    });

    let controls = row![play_pause_btn, progress]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    // WPM slider (100–1000, step 10) — dispatch on release only
    let wpm_slider = slider(100_u32..=1000_u32, wpm_preview, Message::WpmDragged)
        .step(10_u32)
        .on_release(Message::WpmCommitted);

    // Words-per-group slider (1–5, step 1) — dispatch on release only
    let group_slider = slider(1_u32..=5_u32, group_preview, Message::GroupDragged)
        .step(1_u32)
        .on_release(Message::GroupCommitted);

    let sliders = row![
        text(format!("{} WPM", wpm_preview)).width(80),
        wpm_slider,
        text(format!("×{}", group_preview)).width(32),
        group_slider,
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    // Replay button — only shown when reading is complete
    let is_finished = state.progress_percent >= 99.9 && !state.is_playing && state.total_words > 0;
    let replay_row: Element<'_, Message> = if is_finished {
        button("↩ Replay")
            .on_press(Message::Dispatch(AppAction::Replay))
            .into()
    } else {
        iced::widget::Space::new().into()
    };

    let content = column![
        back_btn,
        container(word_area)
            .style(rsvp_stage_style)
            .padding([24, 32])
            .center_x(Fill)
            .height(Length::Shrink),
        controls,
        sliders,
        replay_row,
    ]
    .spacing(20)
    .padding(32)
    .max_width(720);

    container(content)
        .center_x(Fill)
        .into()
}

/// Render a WordDisplay using rich_text with anchor letter highlighted.
/// Clones string fields — safe because WordSegment fields are small word fragments.
fn word_display(display: &WordDisplay) -> Element<'_, Message> {
    // Build spans as owned Strings to avoid lifetime issues with the view borrow
    let mut all_spans: Vec<iced::widget::text::Span<'static>> = Vec::new();

    for (i, seg) in display.words.iter().enumerate() {
        all_spans.push(span(seg.before.clone()).size(48.0));
        all_spans.push(
            span(seg.anchor.clone())
                .size(48.0)
                .color(iced::Color::from_rgb(1.0, 0.4, 0.0)), // ORP highlight: orange
        );
        all_spans.push(span(seg.after.clone()).size(48.0));

        // Add word separator for multi-word groups (not after the last word)
        if i < display.words.len() - 1 {
            all_spans.push(span(" ").size(48.0));
        }
    }

    container(rich_text(all_spans))
        .center_x(Fill)
        .into()
}

/// RSVP stage background style — dark panel in dark mode, light panel in light mode.
/// Contrast verified: dark #1A1A1A gives 5.8:1 against ORP orange (#FF6600). (UI-SPEC Color)
fn rsvp_stage_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let background_color = match theme {
        iced::Theme::Dark => Color::from_rgb(0.10, 0.10, 0.10),
        _ => Color::from_rgb(0.93, 0.93, 0.93),
    };
    iced::widget::container::Style {
        background: Some(background_color.into()),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

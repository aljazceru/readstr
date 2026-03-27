//! Reading screen: RSVP word display with full playback controls.

use iced::widget::{button, column, container, rich_text, row, slider, span, text};
use iced::font::{Family, Weight};
use iced::{Background, Border, Color, Element, Fill, Font, Length};
use readstr_core::{AppAction, AppState, WordDisplay};

use crate::widgets::seek_bar::seek_bar;
use crate::{ACCENT_ORANGE_DARK, ACCENT_ORANGE_LIGHT, JETBRAINS_MONO, Message};

pub fn view(state: &AppState, wpm_preview: u32, group_preview: u32, dark_mode: bool) -> Element<'_, Message> {
    let back_btn = button("← Back")
        .style(button::text)
        .on_press(Message::GoBack);

    let theme_label = if dark_mode { "Light" } else { "Dark" };
    let theme_btn = button(theme_label)
        .style(button::text)
        .on_press(Message::ToggleTheme);
    let nav_row = row![back_btn, iced::widget::Space::new().width(Fill), theme_btn]
        .align_y(iced::Alignment::Center);

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

    // Progress / seek bar — orange fill, mode-appropriate track
    let track = if dark_mode {
        Color::from_rgba(1.0, 1.0, 1.0, 0.15)
    } else {
        Color::from_rgba(0.0, 0.0, 0.0, 0.15)
    };
    let progress = seek_bar(0.0_f32..=100.0_f32, state.progress_percent, |pct| {
        Message::Dispatch(AppAction::SeekToProgress { percent: pct })
    })
    .track_color(track);

    let controls = row![play_pause_btn, progress]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    // WPM slider (100–1000, step 10) — dispatch on release only, orange style
    let wpm_slider = slider(100_u32..=1000_u32, wpm_preview, Message::WpmDragged)
        .step(10_u32)
        .on_release(Message::WpmCommitted)
        .style(orange_slider_style);

    // Words-per-group slider (1–5, step 1) — dispatch on release only, orange style
    let group_slider = slider(1_u32..=5_u32, group_preview, Message::GroupDragged)
        .step(1_u32)
        .on_release(Message::GroupCommitted)
        .style(orange_slider_style);

    // Android-style label layout: "speed" muted left / "{N} wpm" orange right
    let accent = if dark_mode { ACCENT_ORANGE_DARK } else { ACCENT_ORANGE_LIGHT };
    let muted = if dark_mode {
        Color::from_rgb(0.580, 0.565, 0.659)  // #9490A8
    } else {
        Color::from_rgb(0.369, 0.345, 0.439)  // #5E5870
    };

    let wpm_label_row = row![
        text("speed").size(14).color(muted),
        iced::widget::Space::new().width(Fill),
        text(format!("{} wpm", wpm_preview)).size(14).color(accent),
    ]
    .align_y(iced::Alignment::Center);

    let group_label_row = row![
        text("group").size(14).color(muted),
        iced::widget::Space::new().width(Fill),
        text(format!("{} words", group_preview)).size(14).color(accent),
    ]
    .align_y(iced::Alignment::Center);

    let sliders = column![
        wpm_label_row,
        wpm_slider,
        group_label_row,
        group_slider,
    ]
    .spacing(4);

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
        nav_row,
        container(word_area)
            .style(rsvp_stage_style(dark_mode))
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

/// Render a WordDisplay using rich_text with anchor letter highlighted in JetBrains Mono.
/// Non-anchor spans use Regular weight; anchor uses Bold weight + AccentOrange color.
fn word_display(display: &WordDisplay) -> Element<'_, Message> {
    let mut all_spans: Vec<iced::widget::text::Span<'static>> = Vec::new();

    for (i, seg) in display.words.iter().enumerate() {
        // Non-anchor before segment
        all_spans.push(span(seg.before.clone()).size(48.0).font(JETBRAINS_MONO));

        // Anchor letter — Bold weight + AccentOrange
        all_spans.push(
            span(seg.anchor.clone())
                .size(48.0)
                .color(ACCENT_ORANGE_DARK)
                .font(Font {
                    family: Family::Name("JetBrains Mono"),
                    weight: Weight::Bold,
                    ..Font::DEFAULT
                }),
        );

        // Non-anchor after segment
        all_spans.push(span(seg.after.clone()).size(48.0).font(JETBRAINS_MONO));

        // Word separator for multi-word groups (not after the last word)
        if i < display.words.len() - 1 {
            all_spans.push(span(" ").size(48.0).font(JETBRAINS_MONO));
        }
    }

    container(rich_text(all_spans))
        .center_x(Fill)
        .into()
}

/// RSVP stage background style — uses dark_mode: bool to avoid Pitfall 3.
/// Pitfall 3: matches!(theme, iced::Theme::Dark) breaks when a custom palette theme is used.
/// Instead, we capture dark_mode from app state and ignore the theme parameter entirely.
fn rsvp_stage_style(dark_mode: bool) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
    move |_theme| {
        let bg = if dark_mode {
            Color::from_rgb(0.086, 0.075, 0.125)  // #161320 DarkSurface
        } else {
            Color::from_rgb(0.929, 0.910, 0.965)  // #EDE8F6 LightSurfaceVar
        };
        iced::widget::container::Style {
            background: Some(bg.into()),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Orange slider style matching Android's accent color on thumb and active rail.
fn orange_slider_style(theme: &iced::Theme, status: iced::widget::slider::Status) -> iced::widget::slider::Style {
    let mut s = iced::widget::slider::default(theme, status);
    let orange = ACCENT_ORANGE_DARK;
    let inactive = theme.extended_palette().background.strong.color;
    s.rail = iced::widget::slider::Rail {
        backgrounds: (Background::Color(orange), Background::Color(inactive)),
        width: 4.0,
        border: Default::default(),
    };
    s.handle = iced::widget::slider::Handle {
        shape: iced::widget::slider::HandleShape::Circle { radius: 9.0 },
        background: Background::Color(orange),
        border_width: 0.0,
        border_color: Color::TRANSPARENT,
    };
    s
}

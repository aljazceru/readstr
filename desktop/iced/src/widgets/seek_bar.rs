//! SeekBar: a clickable progress bar widget.
//! The built-in iced ProgressBar is display-only; this widget publishes a seek message on click.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Tree;
use iced::advanced::{self, Shell};
use iced::mouse;
use iced::{Color, Element, Event, Length, Rectangle, Size};
use std::ops::RangeInclusive;

pub struct SeekBar<'a, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    on_seek: Box<dyn Fn(f32) -> Message + 'a>,
    width: Length,
    height: f32,
}

impl<'a, Message> SeekBar<'a, Message> {
    pub fn new(
        range: RangeInclusive<f32>,
        value: f32,
        on_seek: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            range,
            value,
            on_seek: Box::new(on_seek),
            width: Length::Fill,
            height: 8.0,
        }
    }
}

/// Convenience constructor — mirrors iced slider API style.
pub fn seek_bar<'a, Message>(
    range: RangeInclusive<f32>,
    value: f32,
    on_seek: impl Fn(f32) -> Message + 'a,
) -> SeekBar<'a, Message> {
    SeekBar::new(range, value, on_seek)
}

impl<'a, Message, Thm, Renderer> advanced::Widget<Message, Thm, Renderer>
    for SeekBar<'a, Message>
where
    Renderer: renderer::Renderer,
    Thm: iced::widget::progress_bar::Catalog,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Fixed(self.height + 8.0), // 8px padding top/bottom for click target
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let width = limits
            .resolve(self.width, Length::Fixed(self.height + 8.0), Size::ZERO)
            .width;
        layout::Node::new(Size::new(width, self.height + 8.0))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Thm,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: iced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let bar_height = self.height;
        let bar_y = bounds.y + (bounds.height - bar_height) / 2.0;
        let bar_bounds = Rectangle {
            x: bounds.x,
            y: bar_y,
            width: bounds.width,
            height: bar_height,
        };

        let range_len = self.range.end() - self.range.start();
        let fill_ratio = if range_len > 0.0 {
            ((self.value - self.range.start()) / range_len).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Background track
        renderer.fill_quad(
            renderer::Quad {
                bounds: bar_bounds,
                border: iced::Border {
                    radius: (bar_height / 2.0).into(),
                    ..Default::default()
                },
                shadow: Default::default(),
                snap: true,
            },
            Color::from_rgb(0.7, 0.7, 0.7),
        );

        // Filled portion
        if fill_ratio > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        width: bar_bounds.width * fill_ratio,
                        ..bar_bounds
                    },
                    border: iced::Border {
                        radius: (bar_height / 2.0).into(),
                        ..Default::default()
                    },
                    shadow: Default::default(),
                    snap: true,
                },
                Color::from_rgb(0.2, 0.5, 0.9),
            );
        }
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_over(layout.bounds()) {
                let bounds = layout.bounds();
                let ratio = ((pos.x - bounds.x) / bounds.width).clamp(0.0, 1.0);
                let range_len = self.range.end() - self.range.start();
                let seek_value = self.range.start() + ratio * range_len;
                shell.publish((self.on_seek)(seek_value));
            }
        }
    }
}

impl<'a, Message, Thm, Renderer> From<SeekBar<'a, Message>>
    for Element<'a, Message, Thm, Renderer>
where
    Message: 'a,
    Renderer: renderer::Renderer + 'a,
    Thm: iced::widget::progress_bar::Catalog + 'a,
{
    fn from(widget: SeekBar<'a, Message>) -> Self {
        Element::new(widget)
    }
}

//! This module draws text gizmos using a stroke font.

use crate::text_font::*;
use crate::{gizmos::GizmoBuffer, prelude::GizmoConfigGroup};
use bevy_color::Color;
use bevy_math::{vec2, Isometry2d, Isometry3d, Vec2, Vec3A};
use core::ops::Range;

/// A stroke font
pub struct StrokeFont<'a> {
    /// Baseline-to-baseline line height ratio.
    pub line_height: f32,
    /// Inclusive ASCII range covered by `glyphs`.
    pub ascii_range: Range<u8>,
    /// Full glyph height (cap + descender) in font units.
    pub height: f32,
    /// Cap height in font units.
    pub cap_height: f32,
    /// Advance used for unsupported glyphs.
    pub advance: i8,
    /// Raw glyph point positions.
    pub positions: &'a [[i8; 2]],
    /// Stroke ranges into `positions`.
    pub strokes: &'a [Range<usize>],
    /// Glyph advances and stroke ranges, indexed by ASCII code point.
    pub glyphs: &'a [(i8, Range<usize>)],
}

impl<'a> StrokeFont<'a> {
    /// Get the advance for the glyph corresponding to this char.
    /// Returns `self.advance` if there is no corresponding glyph.
    pub fn layout(&'a self, text: &'a str, font_size: f32) -> StrokeTextLayout<'a> {
        let scale = font_size / SIMPLEX_CAP_HEIGHT;
        let glyph_height = SIMPLEX_HEIGHT * scale;
        let line_height = LINE_HEIGHT * glyph_height;
        let margin_top = line_height - glyph_height;
        let space_advance = SIMPLEX_GLYPHS[0].0 as f32 * scale;
        StrokeTextLayout {
            font: self,
            scale,
            line_height,
            margin_top,
            space_advance,
            text,
        }
    }
}

/// Stroke text layout
pub struct StrokeTextLayout<'a> {
    /// The unscaled font
    font: &'a StrokeFont<'a>,
    /// The text
    text: &'a str,
    /// Scale applied to the raw glyph positions.
    scale: f32,
    /// Height of each line of text.
    line_height: f32,
    /// Space between top of line and cap height.
    margin_top: f32,
    /// Width of a space.
    space_advance: f32,
}

impl<'a> StrokeTextLayout<'a> {
    /// Get the advance for the glyph corresponding to this char.
    /// Returns `self.advance` if there is no corresponding glyph.
    pub fn advance(&self, c: char) -> f32 {
        u8::try_from(c)
            .ok()
            .filter(|c| self.font.ascii_range.contains(&c))
            .map(|c| self.font.glyphs[(c - self.font.ascii_range.start) as usize].0)
            .unwrap_or(self.font.advance) as f32
            * self.scale
    }

    /// Computes the width and height of a text layout with this font and
    /// the given text.
    ///
    /// Returns the layout size in pixels.
    pub fn measure(&self) -> Vec2 {
        let mut layout_size = vec2(0., self.line_height);

        let mut w = 0.;
        for c in self.text.chars() {
            if c == '\n' {
                layout_size.x = layout_size.x.max(w);
                w = 0.;
                layout_size.y += self.line_height;
                continue;
            }

            w += self.advance(c) as f32 * self.scale;
        }

        layout_size.x = layout_size.x.max(w);
        layout_size
    }

    /// Render lines
    pub fn render(&'a self) -> impl Iterator<Item = impl Iterator<Item = Vec2>> + 'a {
        let mut chars = self.text.chars();
        let mut rx = 0.0;
        let mut ry = -self.margin_top;
        let mut pending_strokes: Option<Range<usize>> = None;
        let mut pending_rx = 0.0;
        let mut pending_ry = 0.0;

        let font = self.font;
        let positions = self.font.positions;
        let scale = self.scale;
        let cap_height = self.font.cap_height;
        let line_height = self.line_height;
        let space_advance = self.space_advance;

        core::iter::from_fn(move || loop {
            if let Some(stroke_indices) = &mut pending_strokes {
                if let Some(stroke_index) = stroke_indices.next() {
                    let stroke = font.strokes[stroke_index].clone();
                    if stroke.len() < 2 {
                        continue;
                    }

                    let rx0 = pending_rx;
                    let ry0 = pending_ry;
                    return Some(stroke.map(move |index| {
                        let [x, y] = positions[index];
                        Vec2::new(
                            rx0 + scale * x as f32,
                            ry0 - scale * (cap_height - y as f32),
                        )
                    }));
                }

                pending_strokes = None;
            }

            let c = chars.next()?;
            if c == '\n' {
                rx = 0.0;
                ry -= line_height;
                continue;
            }

            let Some(code_point) = u8::try_from(c)
                .ok()
                .filter(|c| font.ascii_range.contains(&c))
            else {
                rx += space_advance;
                continue;
            };

            let glyph = &font.glyphs[(code_point - font.ascii_range.start) as usize];
            let advance = glyph.0 as f32 * scale;

            pending_strokes = Some(glyph.1.clone());
            pending_rx = rx;
            pending_ry = ry;

            rx += advance;
        })
    }
}

impl<Config, Clear> GizmoBuffer<Config, Clear>
where
    Config: GizmoConfigGroup,
    Clear: 'static + Send + Sync,
{
    /// Draw text using a stroke font with the given isometry applied.
    ///
    /// Only ASCII characters in the range 32–126 are supported.
    ///
    /// # Arguments
    ///
    /// - `isometry`: defines the translation and rotation of the text.
    /// - `text`: the text to be drawn.
    /// - `size`: the size of the text in pixels.
    /// - `anchor`: anchor point relative to the center of the text.
    /// - `color`: the color of the text.
    ///
    /// # Example
    /// ```
    /// # use bevy_gizmos::prelude::*;
    /// # use bevy_math::prelude::*;
    /// # use bevy_color::Color;
    /// fn system(mut gizmos: Gizmos) {
    ///     gizmos.text(Isometry3d::IDENTITY, "text gizmo", 25., Vec2::ZERO, Color::WHITE);
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    pub fn text(
        &mut self,
        isometry: impl Into<Isometry3d>,
        text: &str,
        font_size: f32,
        anchor: Vec2,
        color: impl Into<Color>,
    ) {
        let color = color.into();
        let layout = SIMPLEX_STROKE_FONT.layout(text, font_size);
        let layout_anchor = vec2(-0.5, 0.5) - anchor;
        let mut isometry: Isometry3d = isometry.into();
        isometry.translation += Vec3A::from((layout.measure() * layout_anchor).extend(0.));
        for points in layout.render() {
            self.linestrip(points.map(|point| isometry * point.extend(0.)), color);
        }
    }

    /// Draw text using a stroke font in 2d with the given isometry applied.
    ///
    /// Only ASCII characters in the range 32–126 are supported.
    ///
    /// # Arguments
    ///
    /// - `isometry`: defines the translation and rotation of the text.
    /// - `text`: the text to be drawn.
    /// - `size`: the size of the text.
    /// - `anchor`: anchor point relative to the center of the text.
    /// - `color`: the color of the text.
    ///
    /// # Example
    /// ```
    /// # use bevy_gizmos::prelude::*;
    /// # use bevy_math::prelude::*;
    /// # use bevy_color::Color;
    /// fn system(mut gizmos: Gizmos) {
    ///     gizmos.text_2d(Isometry2d::IDENTITY, "2D text gizmo", 25., Vec2::ZERO, Color::WHITE);
    /// }
    /// # bevy_ecs::system::assert_is_system(system);
    /// ```
    pub fn text_2d(
        &mut self,
        isometry: impl Into<Isometry2d>,
        text: &str,
        font_size: f32,
        anchor: Vec2,
        color: impl Into<Color>,
    ) {
        let color = color.into();
        let layout = SIMPLEX_STROKE_FONT.layout(text, font_size);
        let layout_anchor = vec2(-0.5, 0.5) - anchor;
        let mut isometry: Isometry2d = isometry.into();
        isometry.translation += layout.measure() * layout_anchor;
        for points in layout.render() {
            self.linestrip_2d(points.map(|point| isometry * point), color);
        }
    }
}

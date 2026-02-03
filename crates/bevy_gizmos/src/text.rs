//! This module draws text gizmos using a stroke font.

use crate::text_font::*;
use crate::{gizmos::GizmoBuffer, prelude::GizmoConfigGroup};
use bevy_color::Color;
use bevy_math::{vec2, Isometry2d, Isometry3d, Vec2, Vec3A};
use core::{ops::Range, str::Chars};

/// Computes the width and height of a text layout with the given text and
/// metrics when drawn with the builtin Simplex stroke font.
///
/// Returns the layout size in pixels.
pub fn measure_simplex_text(text: &str, metrics: StrokeTextMetrics) -> Vec2 {
    let mut layout_size = vec2(0., metrics.line_height);

    let mut w = 0.;
    for c in text.chars() {
        if c == '\n' {
            layout_size.x = layout_size.x.max(w);
            w = 0.;
            layout_size.y += metrics.line_height;
            continue;
        }

        let code_point = c as usize;
        if !(SIMPLEX_ASCII_START..=SIMPLEX_ASCII_END).contains(&code_point) {
            w += metrics.space_advance;
            continue;
        }

        let glyph = &SIMPLEX_GLYPHS[code_point - SIMPLEX_ASCII_START];
        w += glyph.0 as f32 * metrics.scale;
    }

    layout_size.x = layout_size.x.max(w);
    layout_size
}

/// Iterator that yields stroke line strips for a text string using the Simplex font.
///
/// Each item is a sequence of scaled `Vec2` points that can be passed to
/// `linestrip_2d` directly, or mapped to `Vec3` for `linestrip`.
pub struct StrokeTextIterator<'a> {
    chars: Chars<'a>,
    metrics: StrokeTextMetrics,
    rx: f32,
    ry: f32,
    strokes: Option<GlyphStrokeIterator>,
}

/// Scaled stroke font metrics for use during stroke text layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeTextMetrics {
    /// Scale applied to the raw glyph positions.
    pub scale: f32,
    /// Height of each line of text.
    pub line_height: f32,
    /// Space between top of line and cap height.
    pub margin_top: f32,
    /// Width of a space.
    pub space_advance: f32,
}

struct GlyphStrokeIterator {
    stroke_indices: Range<usize>,
    rx: f32,
    ry: f32,
}

impl<'a> StrokeTextIterator<'a> {
    /// Create a new iterator for the given text and font size.
    pub fn new(text: &'a str, metrics: StrokeTextMetrics) -> Self {
        Self {
            chars: text.chars(),
            rx: 0.0,
            metrics,
            ry: -metrics.margin_top,
            strokes: None,
        }
    }
}

impl Iterator for StrokeTextIterator<'_> {
    type Item = Vec<Vec2>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(pending) = &mut self.strokes {
                if let Some(stroke_index) = pending.stroke_indices.next() {
                    let stroke = SIMPLEX_STROKES[stroke_index].clone();
                    if stroke.len() < 2 {
                        continue;
                    }

                    let points = SIMPLEX_POSITIONS[stroke]
                        .iter()
                        .map(|&[x, y]| {
                            Vec2::new(
                                pending.rx + self.metrics.scale * x as f32,
                                pending.ry - self.metrics.scale * (SIMPLEX_CAP_HEIGHT - y as f32),
                            )
                        })
                        .collect();

                    return Some(points);
                }

                self.strokes = None;
            }

            let c = self.chars.next()?;
            if c == '\n' {
                self.rx = 0.0;
                self.ry -= self.metrics.line_height;
                continue;
            }

            let code_point = c as usize;
            if !(SIMPLEX_ASCII_START..=SIMPLEX_ASCII_END).contains(&code_point) {
                self.rx += self.metrics.space_advance;
                continue;
            }

            let glyph = &SIMPLEX_GLYPHS[code_point - SIMPLEX_ASCII_START];
            let advance = glyph.0 as f32 * self.metrics.scale;

            self.strokes = Some(GlyphStrokeIterator {
                stroke_indices: glyph.1.clone(),
                rx: self.rx,
                ry: self.ry,
            });

            self.rx += advance;
        }
    }
}

/// Build a stroke text iterator for the given text and font size.
pub fn stroke_text_iter(text: &str, font_size: f32) -> StrokeTextIterator<'_> {
    let scale = font_size / SIMPLEX_CAP_HEIGHT;
    let glyph_height = SIMPLEX_HEIGHT * scale;
    let line_height = LINE_HEIGHT * glyph_height;
    let margin_top = line_height - glyph_height;
    let space_advance = SIMPLEX_GLYPHS[0].0 as f32 * scale;

    StrokeTextIterator::new(
        text,
        StrokeTextMetrics {
            scale,
            line_height,
            margin_top,
            space_advance,
        },
    )
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
        let metrics = simplex_font_metrics(font_size);
        let layout_size = measure_simplex_text(text, metrics);
        let adjusted_anchor = -anchor + vec2(-0.5, 0.5);

        let mut isometry: Isometry3d = isometry.into();
        isometry.translation += Vec3A::from((layout_size * adjusted_anchor).extend(0.));

        for points in stroke_text_iter(text, font_size) {
            self.linestrip(
                points.into_iter().map(|point| isometry * point.extend(0.)),
                color,
            );
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
        size: f32,
        anchor: Vec2,
        color: impl Into<Color>,
    ) {
        let color = color.into();
        let metrics = simplex_font_metrics(size);
        let layout_size = measure_simplex_text(text, metrics);
        // Adjust anchor to top-left coords
        let adjusted_anchor = -anchor + vec2(-0.5, 0.5);

        let mut isometry: Isometry2d = isometry.into();
        isometry.translation += layout_size * adjusted_anchor;

        for points in stroke_text_iter(text, size) {
            self.linestrip_2d(points.into_iter().map(|point| isometry * point), color);
        }
    }
}

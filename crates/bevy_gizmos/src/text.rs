//! This module draws text gizmos using a stroke font.

use crate::text_font::*;
use crate::{gizmos::GizmoBuffer, prelude::GizmoConfigGroup};
use bevy_color::Color;
use bevy_math::{vec2, Isometry2d, Isometry3d, Vec2, Vec3A};
use core::{ops::Range, str::Chars};

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
    text: &'a str,
    /// Scale applied to the raw glyph positions.
    pub scale: f32,
    /// Height of each line of text.
    pub line_height: f32,
    /// Space between top of line and cap height.
    pub margin_top: f32,
    /// Width of a space.
    pub space_advance: f32,
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
        StrokeTextIterator::new(&self)
    }
}

/// Iterator that yields stroke line strips for a text string using the Simplex font.
///
/// Each item is a sequence of scaled `Vec2` points that can be passed to
/// `linestrip_2d` directly, or mapped to `Vec3` for `linestrip`.
struct StrokeTextIterator<'a> {
    chars: Chars<'a>,
    layout: &'a StrokeTextLayout<'a>,

    rx: f32,
    ry: f32,
    strokes: Option<GlyphStrokeIterator>,
}

impl<'a> StrokeTextIterator<'a> {
    /// Create a new iterator for the given text and font size.
    pub fn new(layout: &'a StrokeTextLayout) -> Self {
        Self {
            chars: layout.text.chars(),
            layout,
            rx: 0.0,
            ry: -layout.margin_top,
            strokes: None,
        }
    }
}

struct GlyphStrokeIterator {
    stroke_indices: Range<usize>,
    rx: f32,
    ry: f32,
}

/// Iterator over the points of a single stroke line strip.
struct StrokeLineStrip<'a> {
    positions: &'a [[i8; 2]],
    stroke: Range<usize>,
    rx: f32,
    ry: f32,
    scale: f32,
    cap_height: f32,
}

impl Iterator for StrokeLineStrip<'_> {
    type Item = Vec2;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.stroke.next()?;
        let [x, y] = self.positions[index];
        Some(Vec2::new(
            self.rx + self.scale * x as f32,
            self.ry - self.scale * (self.cap_height - y as f32),
        ))
    }
}

impl<'a> Iterator for StrokeTextIterator<'a> {
    type Item = StrokeLineStrip<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(pending) = &mut self.strokes {
                if let Some(stroke_index) = pending.stroke_indices.next() {
                    let stroke: Range<usize> = self.layout.font.strokes[stroke_index].clone();
                    if stroke.len() < 2 {
                        continue;
                    }

                    return Some(StrokeLineStrip {
                        positions: self.layout.font.positions,
                        stroke,
                        rx: pending.rx,
                        ry: pending.ry,
                        scale: self.layout.scale,
                        cap_height: self.layout.font.cap_height,
                    });
                }

                self.strokes = None;
            }

            let c = self.chars.next()?;
            if c == '\n' {
                self.rx = 0.0;
                self.ry -= self.layout.line_height;
                continue;
            }

            let Some(code_point) = u8::try_from(c)
                .ok()
                .filter(|c| self.layout.font.ascii_range.contains(&c))
            else {
                self.rx += self.layout.space_advance;
                continue;
            };

            let glyph = &self.layout.font.glyphs
                [(code_point - self.layout.font.ascii_range.start) as usize];
            let advance = glyph.0 as f32 * self.layout.scale;

            self.strokes = Some(GlyphStrokeIterator {
                stroke_indices: glyph.1.clone(),
                rx: self.rx,
                ry: self.ry,
            });

            self.rx += advance;
        }
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
        let adjusted_anchor = -anchor + vec2(-0.5, 0.5);

        let mut isometry: Isometry3d = isometry.into();
        isometry.translation += Vec3A::from((layout.measure() * adjusted_anchor).extend(0.));

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

        // Adjust anchor to top-left coords
        let adjusted_anchor = -anchor + vec2(-0.5, 0.5);

        let mut isometry: Isometry2d = isometry.into();
        isometry.translation += layout.measure() * adjusted_anchor;

        for points in layout.render() {
            self.linestrip_2d(points.map(|point| isometry * point), color);
        }
    }
}

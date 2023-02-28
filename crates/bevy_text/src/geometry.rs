use glyph_brush_layout::SectionText;
use crate::BreakLineOn;
use crate::GlyphBrush;
use crate::prelude::*;


pub struct ScaledFontGeometry {
 
}

impl ScaledFontGeometry {
    fn ascent() -> f32 {
    }

    fn descent() -> f32 {
    }

    fn h_advance() -> f32 {
    }
}

pub struct TextGeometry {

}

pub fn compute_geometry(
    brush: &GlyphBrush,
    sections: Vec<SectionText>,
    text_alignment: TextAlignment,
    linebreak_behaviour: BreakLineOn,
) {
    let glyphs = vec![];
    for section in sections {
        let section_glyphs = 
            brush.compute_glyphs(&section, bounds, text_alignment, linebreak_behaviour);
        glyphs.push(section_glyphs);
    }
}

pub fn compute_bounds(

) {
}
use crate::{Font, FontAtlas, FontSmoothing, TextFont};
use bevy_asset::{AssetEvent, AssetId};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    component::Component,
    message::MessageReader,
    resource::Resource,
    system::{Local, Query, ResMut},
};
use bevy_platform::collections::{HashMap, HashSet};
use smallvec::SmallVec;

/// Identifies the font atlases for a particular font in [`FontAtlasSet`]
///
/// Allows an `f32` font size to be used as a key in a `HashMap`, by its binary representation.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct FontAtlasKey(pub AssetId<Font>, pub u32, pub FontSmoothing);

impl From<&TextFont> for FontAtlasKey {
    fn from(font: &TextFont) -> Self {
        FontAtlasKey(
            font.font.id(),
            font.font_size.to_bits(),
            font.font_smoothing,
        )
    }
}

/// Set of rasterized fonts stored in [`FontAtlas`]es.
#[derive(Debug, Default, Resource, Deref, DerefMut)]
pub struct FontAtlasSet(HashMap<FontAtlasKey, Vec<FontAtlas>>);

impl FontAtlasSet {
    /// Checks whether the given subpixel-offset glyph is contained in any of the [`FontAtlas`]es for the font identified by the given [`FontAtlasKey`].
    pub fn has_glyph(&self, cache_key: cosmic_text::CacheKey, font_key: &FontAtlasKey) -> bool {
        self.get(font_key)
            .is_some_and(|font_atlas| font_atlas.iter().any(|atlas| atlas.has_glyph(cache_key)))
    }
}

/// A system that automatically frees unused texture atlases when a font asset is removed.
pub fn free_unused_font_atlases_system(
    mut font_atlas_sets: ResMut<FontAtlasSet>,
    mut font_events: MessageReader<AssetEvent<Font>>,
) {
    for event in font_events.read() {
        if let AssetEvent::Removed { id } = event {
            font_atlas_sets.retain(|key, _| key.0 != *id);
        }
    }
}

#[derive(Resource)]
/// Maximum number of font atlas sets.
pub struct MaxUnusedFontAtlasSets(pub usize);

impl Default for MaxUnusedFontAtlasSets {
    fn default() -> Self {
        Self(20)
    }
}

#[derive(Component, Default)]
/// Computed font derived from `TextFont` and the scale factor of the render target.
pub struct ComputedTextFonts(pub SmallVec<[FontAtlasKey; 1]>);

/// Automatically frees unused fonts when the total number of fonts
/// is greater than the [`MaxFonts`] value. Doesn't free in use fonts
/// even if the number of in use fonts is greater than [`MaxFonts`].
pub fn free_unused_font_atlases_computed_system(
    // list of unused fonts in order from least to most recently used
    mut least_recently_used: Local<Vec<FontAtlasKey>>,
    // fonts that were in use the previous frame
    mut previous_active_fonts: Local<HashSet<FontAtlasKey>>,
    mut active_fonts: Local<HashSet<FontAtlasKey>>,
    mut font_atlas_set: ResMut<FontAtlasSet>,
    max_fonts: ResMut<MaxUnusedFontAtlasSets>,
    active_fonts_query: Query<&ComputedTextFonts>,
) {
    // collect keys for all fonts currently in use by a text entity
    active_fonts.extend(
        active_fonts_query
            .iter()
            .flat_map(|computed_fonts| computed_fonts.0.iter()),
    );

    // remove any keys for fonts in use from the least recently used list
    least_recently_used.retain(|font| !active_fonts.contains(font));

    // push keys for any fonts no longer in use onto the least recently used list
    least_recently_used.extend(
        previous_active_fonts
            .difference(&active_fonts)
            .into_iter()
            .cloned(),
    );

    // If the total number of fonts is greater than max_fonts, free fonts from the least rcently used list
    // until the total is lower than max_fonts or the least recently used list is empty.
    let number_of_fonts_to_free = font_atlas_set
        .len()
        .saturating_sub(max_fonts.0)
        .min(least_recently_used.len());
    for font_atlas_key in least_recently_used.drain(..number_of_fonts_to_free) {
        font_atlas_set.remove(&font_atlas_key);
    }

    previous_active_fonts.clear();
    core::mem::swap(&mut *previous_active_fonts, &mut *active_fonts);
}

#[cfg(test)]
mod tests {
    use crate::free_unused_font_atlases_computed_system;
    use crate::ComputedTextFonts;
    use crate::FontAtlasKey;
    use crate::FontAtlasSet;
    use crate::MaxUnusedFontAtlasSets;
    use bevy_app::App;
    use bevy_app::Update;
    use bevy_asset::AssetId;
    use smallvec::smallvec;

    #[test]
    fn text_free_unused_font_atlases_computed_system() {
        let mut app = App::new();

        app.init_resource::<MaxUnusedFontAtlasSets>();
        app.init_resource::<FontAtlasSet>();

        app.add_systems(Update, free_unused_font_atlases_computed_system);

        let world = app.world_mut();

        let mut font_atlases = world.resource_mut::<FontAtlasSet>();

        let font_atlas_key_1 =
            FontAtlasKey(AssetId::default(), 10, crate::FontSmoothing::AntiAliased);
        let font_atlas_key_2 = FontAtlasKey(AssetId::default(), 10, crate::FontSmoothing::None);

        font_atlases.insert(font_atlas_key_1, vec![]);
        font_atlases.insert(font_atlas_key_2, vec![]);

        let e = world
            .spawn(ComputedTextFonts(smallvec![font_atlas_key_1]))
            .id();
        let f = world
            .spawn(ComputedTextFonts(smallvec![font_atlas_key_2]))
            .id();

        app.update();

        let world = app.world_mut();
        let font_atlases = world.resource_mut::<FontAtlasSet>();
        assert_eq!(font_atlases.len(), 2);

        world.despawn(f);

        app.update();

        let world = app.world_mut();
        let font_atlases = world.resource_mut::<FontAtlasSet>();
        assert_eq!(font_atlases.len(), 2);

        world.resource_mut::<MaxUnusedFontAtlasSets>().0 = 1;

        app.update();

        let world = app.world_mut();
        let font_atlases = world.resource_mut::<FontAtlasSet>();
        assert_eq!(font_atlases.len(), 1);
        assert!(font_atlases.contains_key(&font_atlas_key_1));
        assert!(!font_atlases.contains_key(&font_atlas_key_2));

        world.despawn(e);
        world.resource_mut::<MaxUnusedFontAtlasSets>().0 = 0;

        app.update();

        let world = app.world_mut();
        let font_atlases = world.resource_mut::<FontAtlasSet>();
        assert_eq!(font_atlases.len(), 0);
    }
}

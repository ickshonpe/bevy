mod error;
mod font;
mod font_atlas;
mod font_atlas_set;
mod font_loader;
mod glyph_brush;
mod pipeline;
mod text;
mod text2d;

pub use error::*;
pub use font::*;
pub use font_atlas::*;
pub use font_atlas_set::*;
pub use font_loader::*;
pub use glyph_brush::*;
pub use pipeline::*;
pub use text::*;
pub use text2d::*;

pub mod prelude {
    #[doc(hidden)]
    pub use crate::{Font, Text, Text2dBundle, TextAlignment, TextError, TextSection, TextStyle};
}

use bevy_app::prelude::*;
use bevy_asset::AddAsset;
use bevy_ecs::prelude::*;
use bevy_render::{camera::CameraUpdateSystem, ExtractSchedule, RenderApp};
use bevy_sprite::SpriteSystem;
use std::num::NonZeroUsize;

#[derive(Default)]
pub struct TextPlugin;

/// [`TextPlugin`] settings
#[derive(Resource)]
pub struct TextSettings {
    /// Maximum number of font atlases supported in a ['FontAtlasSet']
    pub max_font_atlases: NonZeroUsize,
    /// Allows font size to be set dynamically exceeding the amount set in max_font_atlases.
    /// Note each font size has to be generated which can have a strong performance impact.
    pub allow_dynamic_font_size: bool,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            max_font_atlases: NonZeroUsize::new(16).unwrap(),
            allow_dynamic_font_size: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct FontAtlasWarning {
    warned: bool,
}

/// Text is rendered for two different view projections, normal `Text2DBundle` is rendered with a
/// `BottomToTop` y axis, and UI is rendered with a `TopToBottom` y axis. This matters for text because
/// the glyph positioning is different in either layout.
pub enum YAxisOrientation {
    TopToBottom,
    BottomToTop,
}

pub struct FontSystem(pub (crate) cosmic_text::FontSystem);
pub struct SwashCache(pub (crate) cosmic_text::SwashCache);

impl FromWorld for SwashCache {
    fn from_world(world: &mut World) -> Self {
        let font_system = world.get_resource::<FontSystem>().unwrap();
        Self(cosmic_text::SwashCache::new(&font_system.0))
    }
}

impl FromWorld for FontSystem {
    fn from_world(world: &mut World) -> Self {
        Self(cosmic_text::FontSystem::new())
    }
}

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {

        app
            .init_resource::<FontSystem>()
            .init_resource::<SwashCache>()
            .add_asset::<Font>()
            .add_asset::<FontAtlasSet>()
            .register_type::<Text>()
            .register_type::<TextSection>()
            .register_type::<Vec<TextSection>>()
            .register_type::<TextStyle>()
            .register_type::<Text>()
            .register_type::<TextAlignment>()
            .init_asset_loader::<FontLoader>()
            .init_resource::<TextSettings>()
            .init_resource::<FontAtlasWarning>()
            .insert_resource(TextPipeline::default())
            .add_system(
                update_text2d_layout
                    .in_base_set(CoreSet::PostUpdate)
                    // Potential conflict: `Assets<Image>`
                    // In practice, they run independently since `bevy_render::camera_update_system`
                    // will only ever observe its own render target, and `update_text2d_layout`
                    // will never modify a pre-existing `Image` asset.
                    .ambiguous_with(CameraUpdateSystem),
            );

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_system_to_schedule(
                ExtractSchedule,
                extract_text2d_sprite.after(SpriteSystem::ExtractSprites),
            );
        }
    }
}

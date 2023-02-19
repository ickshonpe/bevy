use crate::{CalculatedSize, Measure, Node, UiScale};
use bevy_asset::Assets;
use bevy_ecs::{
    entity::Entity,
    query::{Changed, Or, With},
    system::{Commands, Local, ParamSet, Query, Res, ResMut},
};
use bevy_math::Vec2;
use bevy_render::texture::Image;
use bevy_sprite::TextureAtlas;
use bevy_text::{
    Font, FontAtlasSet, FontAtlasWarning, Text, TextError, TextLayoutInfo, TextPipeline,
    TextSettings, YAxisOrientation,
};
use bevy_window::{PrimaryWindow, Window};
use taffy::prelude::AvailableSpace;

fn scale_value(value: f32, factor: f64) -> f32 {
    (value as f64 * factor) as f32
}

#[derive(Clone)]
pub struct TextMeasure {
    pub size: Vec2,
    pub min_size: Vec2,
    pub max_size: Vec2,
    pub ideal_height: f32,
}

impl Measure for TextMeasure {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        _: AvailableSpace,
        _: AvailableSpace,
    ) -> Vec2 {
        let mut size = self.size;
        match (max_width, max_height) {
            (None, None) => {
                // with no constraints
                // ask for maximum width space for text with no wrapping
                size.x = self.max_size.x;
                size.y = self.min_size.y;
            }
            (Some(width), None) => {
                size.x = width;
                size.y = self.ideal_height;
            }
            (None, Some(height)) => {
                size.y = height;
                size.x = self.max_size.x;
            }
            (Some(width), Some(height)) => {
                size.x = width;
                size.y = height;
            }
        }
        size.x = size.x.ceil();
        size.y = size.y.ceil();
        size
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}

/// Updates the layout and size information whenever the text or style is changed.
/// This information is computed by the `TextPipeline` on insertion, then stored.
///
/// ## World Resources
///
/// [`ResMut<Assets<Image>>`](Assets<Image>) -- This system only adds new [`Image`] assets.
/// It does not modify or observe existing ones.
#[allow(clippy::too_many_arguments)]
pub fn text_system(
    mut commands: Commands,
    mut queued_text_ids: Local<Vec<Entity>>,
    mut last_scale_factor: Local<f64>,
    mut textures: ResMut<Assets<Image>>,
    fonts: Res<Assets<Font>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    text_settings: Res<TextSettings>,
    mut font_atlas_warning: ResMut<FontAtlasWarning>,
    ui_scale: Res<UiScale>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut font_atlas_set_storage: ResMut<Assets<FontAtlasSet>>,
    mut text_pipeline: ResMut<TextPipeline>,
    mut text_queries: ParamSet<(
        Query<Entity, Or<(Changed<Text>, Changed<Node>)>>,
        Query<Entity, (With<Text>, With<Node>)>,
        Query<(
            &Node,
            &Text,
            &mut CalculatedSize,
            Option<&mut TextLayoutInfo>,
        )>,
    )>,
) {
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let window_scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.);

    let scale_factor = ui_scale.scale * window_scale_factor;

    let inv_scale_factor = 1. / scale_factor;

    #[allow(clippy::float_cmp)]
    if *last_scale_factor == scale_factor {
        // Adds all entities where the text or the style has changed to the local queue
        for entity in text_queries.p0().iter() {
            queued_text_ids.push(entity);
        }
    } else {
        // If the scale factor has changed, queue all text
        for entity in text_queries.p1().iter() {
            queued_text_ids.push(entity);
        }
        *last_scale_factor = scale_factor;
    }

    if queued_text_ids.is_empty() {
        return;
    }

    // Computes all text in the local queue
    let mut new_queue = Vec::new();
    let mut query = text_queries.p2();
    for entity in queued_text_ids.drain(..) {
        if let Ok((node, text, mut calculated_size, text_layout_info)) = query.get_mut(entity) {
            let target_size = Vec2::new(
                scale_value(node.size().x, scale_factor),
                scale_value(node.size().y, scale_factor),
            );
            match text_pipeline.compute_sections(&fonts, &text.sections, scale_factor) {
                Ok((sections, scaled_fonts)) => {
                    let a_size = text_pipeline.compute_size(
                        &sections,
                        &scaled_fonts,
                        text.alignment,
                        text.linebreak_behaviour,
                        Vec2::new(0., f32::INFINITY),
                    );

                    let b_size = text_pipeline.compute_size(
                        &sections,
                        &scaled_fonts,
                        text.alignment,
                        text.linebreak_behaviour,
                        Vec2::splat(f32::INFINITY),
                    );

                    let min_x = a_size.x.min(b_size.x);
                    let max_x = a_size.x.max(b_size.x);
                    let min_y = a_size.y.min(b_size.y);
                    let max_y = a_size.y.max(b_size.y);
                    let min_size = Vec2::new(min_x, min_y);
                    let max_size = Vec2::new(max_x, max_y);

                    let ideal = if node.size() == Vec2::ZERO {
                        Vec2::new(max_size.x, min_size.y)
                    } else {
                        text_pipeline.compute_size(
                            &sections,
                            &scaled_fonts,
                            text.alignment,
                            text.linebreak_behaviour,
                            Vec2::new(target_size.x, f32::INFINITY),
                        )
                    };

                    let section_glyphs = if node.size() == Vec2::ZERO {
                        text_pipeline
                            .compute_section_glyphs(
                                &sections,
                                text.alignment,
                                text.linebreak_behaviour,
                                Vec2::splat(f32::INFINITY),
                            )
                            .unwrap()
                    } else {
                        text_pipeline
                            .compute_section_glyphs(
                                &sections,
                                text.alignment,
                                text.linebreak_behaviour,
                                Vec2::new(target_size.x, f32::INFINITY),
                            )
                            .unwrap()
                    };

                    let out = text_pipeline.queue_sections(
                        section_glyphs,
                        &scaled_fonts,
                        &fonts,
                        &sections,
                        &mut font_atlas_set_storage,
                        &mut texture_atlases,
                        &mut textures,
                        text_settings.as_ref(),
                        &mut font_atlas_warning,
                        YAxisOrientation::TopToBottom,
                    );
                    match out {
                        Err(TextError::NoSuchFont) => {
                            // There was an error processing the text layout, let's add this entity to the
                            // queue for further processing
                            new_queue.push(entity);
                        }
                        Err(e @ TextError::FailedToAddGlyph(_)) => {
                            panic!("Fatal error when processing text: {e}.");
                        }
                        Ok(info) => {
                            let inv_scale = |v: Vec2| {
                                Vec2::new(
                                    scale_value(v.x, inv_scale_factor),
                                    scale_value(v.y, inv_scale_factor),
                                )
                            };
                            let measure = TextMeasure {
                                size: inv_scale(info.size),
                                min_size: inv_scale(min_size),
                                max_size: inv_scale(max_size),
                                ideal_height: scale_value(ideal.y, inv_scale_factor),
                            };
                            calculated_size.measure = Box::new(measure);

                            match text_layout_info {
                                Some(mut t) => *t = info,
                                None => {
                                    commands.entity(entity).insert(info);
                                }
                            }
                        }
                    }
                }
                Err(TextError::NoSuchFont) => {
                    new_queue.push(entity);
                }
                Err(e @ TextError::FailedToAddGlyph(_)) => {
                    panic!("Fatal error when processing text: {e}.");
                }
            };
        }
    }

    *queued_text_ids = new_queue;
}

pub fn text_size_system(
    mut commands: Commands,
    mut queued_text_ids: Local<Vec<Entity>>,
    mut last_scale_factor: Local<f64>,
    mut textures: ResMut<Assets<Image>>,
    fonts: Res<Assets<Font>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    text_settings: Res<TextSettings>,
    mut font_atlas_warning: ResMut<FontAtlasWarning>,
    ui_scale: Res<UiScale>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut font_atlas_set_storage: ResMut<Assets<FontAtlasSet>>,
    mut text_pipeline: ResMut<TextPipeline>,
    mut text_queries: ParamSet<(
        Query<Entity, Or<(Changed<Text>, Changed<Node>)>>,
        Query<Entity, (With<Text>, With<Node>)>,
        Query<(
            &Node,
            &Text,
            &mut CalculatedSize,
            Option<&mut TextLayoutInfo>,
        )>,
    )>,
) {
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let window_scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.);

    let scale_factor = ui_scale.scale * window_scale_factor;

    let inv_scale_factor = 1. / scale_factor;

    #[allow(clippy::float_cmp)]
    if *last_scale_factor == scale_factor {
        // Adds all entities where the text or the style has changed to the local queue
        for entity in text_queries.p0().iter() {
            queued_text_ids.push(entity);
        }
    } else {
        // If the scale factor has changed, queue all text
        for entity in text_queries.p1().iter() {
            queued_text_ids.push(entity);
        }
        *last_scale_factor = scale_factor;
    }

    if queued_text_ids.is_empty() {
        return;
    }

    // Computes all text in the local queue
    let mut new_queue = Vec::new();
    let mut query = text_queries.p2();
    for entity in queued_text_ids.drain(..) {
        if let Ok((node, text, mut calculated_size, text_layout_info)) = query.get_mut(entity) {
            let target_size = Vec2::new(
                scale_value(node.size().x, scale_factor),
                scale_value(node.size().y, scale_factor),
            );
            match text_pipeline.compute_sections(&fonts, &text.sections, scale_factor) {
                Ok((sections, scaled_fonts)) => {
                    let a_size = text_pipeline.compute_size(
                        &sections,
                        &scaled_fonts,
                        text.alignment,
                        text.linebreak_behaviour,
                        Vec2::new(0., f32::INFINITY),
                    );

                    let b_size = text_pipeline.compute_size(
                        &sections,
                        &scaled_fonts,
                        text.alignment,
                        text.linebreak_behaviour,
                        Vec2::splat(f32::INFINITY),
                    );

                    let min_x = a_size.x.min(b_size.x);
                    let max_x = a_size.x.max(b_size.x);
                    let min_y = a_size.y.min(b_size.y);
                    let max_y = a_size.y.max(b_size.y);
                    let min_size = Vec2::new(min_x, min_y);
                    let max_size = Vec2::new(max_x, max_y);

                    let ideal = if node.size() == Vec2::ZERO {
                        Vec2::new(max_size.x, min_size.y)
                    } else {
                        text_pipeline.compute_size(
                            &sections,
                            &scaled_fonts,
                            text.alignment,
                            text.linebreak_behaviour,
                            Vec2::new(target_size.x, f32::INFINITY),
                        )
                    };

                    let section_glyphs = if node.size() == Vec2::ZERO {
                        text_pipeline
                            .compute_section_glyphs(
                                &sections,
                                text.alignment,
                                text.linebreak_behaviour,
                                Vec2::splat(f32::INFINITY),
                            )
                            .unwrap()
                    } else {
                        text_pipeline
                            .compute_section_glyphs(
                                &sections,
                                text.alignment,
                                text.linebreak_behaviour,
                                Vec2::new(target_size.x, f32::INFINITY),
                            )
                            .unwrap()
                    };

                    let out = text_pipeline.queue_sections(
                        section_glyphs,
                        &scaled_fonts,
                        &fonts,
                        &sections,
                        &mut font_atlas_set_storage,
                        &mut texture_atlases,
                        &mut textures,
                        text_settings.as_ref(),
                        &mut font_atlas_warning,
                        YAxisOrientation::TopToBottom,
                    );
                    match out {
                        Err(TextError::NoSuchFont) => {
                            // There was an error processing the text layout, let's add this entity to the
                            // queue for further processing
                            new_queue.push(entity);
                        }
                        Err(e @ TextError::FailedToAddGlyph(_)) => {
                            panic!("Fatal error when processing text: {e}.");
                        }
                        Ok(info) => {
                            let inv_scale = |v: Vec2| {
                                Vec2::new(
                                    scale_value(v.x, inv_scale_factor),
                                    scale_value(v.y, inv_scale_factor),
                                )
                            };
                            let measure = TextMeasure {
                                size: inv_scale(info.size),
                                min_size: inv_scale(min_size),
                                max_size: inv_scale(max_size),
                                ideal_height: scale_value(ideal.y, inv_scale_factor),
                            };
                            calculated_size.measure = Box::new(measure);

                            match text_layout_info {
                                Some(mut t) => *t = info,
                                None => {
                                    commands.entity(entity).insert(info);
                                }
                            }
                        }
                    }
                }
                Err(TextError::NoSuchFont) => {
                    new_queue.push(entity);
                }
                Err(e @ TextError::FailedToAddGlyph(_)) => {
                    panic!("Fatal error when processing text: {e}.");
                }
            };
        }
    }

    *queued_text_ids = new_queue;
}

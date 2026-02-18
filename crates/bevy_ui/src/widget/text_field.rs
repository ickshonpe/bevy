use std::hash::BuildHasher;

use crate::{ComputedNode, ComputedUiRenderTargetInfo, ContentSize, Node};
use bevy_asset::Assets;

use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::observer::{Observer, On};
use bevy_ecs::resource::Resource;
use bevy_ecs::world::DeferredWorld;
use bevy_ecs::{
    change_detection::DetectChanges,
    system::{Query, ResMut},
    world::Ref,
};
use bevy_image::prelude::*;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input::ButtonState;
use bevy_input_focus::FocusedInput;
use bevy_math::{Rect, UVec2, Vec2};
use bevy_platform::hash::FixedHasher;
use bevy_text::*;
use bevy_text::{
    add_glyph_to_atlas, get_glyph_atlas_info, ComputedTextBlock, FontAtlasKey, FontAtlasSet,
    FontCx, GlyphCacheKey, LayoutCx, LineHeight, RunGeometry, ScaleCx, TextColor, TextFont,
    TextLayoutInfo,
};
use parley::swash::FontRef;
use parley::{PlainEditor, PositionedLayoutItem};

#[derive(Component)]
pub struct TextEditor {
    editor: PlainEditor<(u32, FontSmoothing)>,
}

impl Default for TextEditor {
    fn default() -> Self {
        let mut editor = PlainEditor::new(20.);
        editor
            .edit_styles()
            .insert(parley::StyleProperty::OverflowWrap(
                parley::OverflowWrap::Anywhere,
            ));

        Self { editor }
    }
}

#[derive(Component)]
#[require(
    Node,
    TextFont,
    TextColor,
    ContentSize,
    ComputedTextBlock,
    LineHeight,
    TextEditor,
    TextLayoutInfo,
    ComputedUiRenderTargetInfo
)]
#[component(
    on_add = on_add_textinputnode,
)]
pub struct TextInput(pub String);

impl TextInput {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

fn on_add_textinputnode(mut world: DeferredWorld, context: HookContext) {
    println!("add text input observer");
    for mut observer in [Observer::new(on_focused_keyboard_input)] {
        observer.watch_entity(context.entity);
        world.commands().spawn(observer);
    }
}

#[derive(Resource, Default)]
pub struct EditorModifiers {
    pub shift: bool,
    pub command: bool,
}

#[derive(Resource, Default)]
pub struct EditorClipboard(pub String);

fn on_focused_keyboard_input(
    trigger: On<FocusedInput<KeyboardInput>>,
    mut query: Query<&mut TextEditor>,
    mut font_cx: ResMut<FontCx>,
    mut layout_cx: ResMut<LayoutCx>,
    mut modifiers: ResMut<EditorModifiers>,
    mut clipboard: ResMut<EditorClipboard>,
) {
    println!("on_focused_keyboard_input");
    if let Ok(mut editor) = query.get_mut(trigger.focused_entity) {
        println!("got editor");
        let drv = &mut editor.editor.driver(&mut font_cx.0, &mut layout_cx.0);
        let keyboard = &trigger.input;

        match keyboard.logical_key {
            Key::Shift => {
                modifiers.shift = keyboard.state == ButtonState::Pressed;
                return;
            }
            Key::Control => {
                modifiers.command = keyboard.state == ButtonState::Pressed;
                return;
            }
            #[cfg(target_os = "macos")]
            Key::Super => {
                modifiers.command = keyboard.state == ButtonState::Pressed;
                return;
            }
            _ => {}
        };

        if keyboard.state.is_pressed() {
            if modifiers.command {
                match &keyboard.logical_key {
                    Key::Character(str) => {
                        if let Some(char) = str.chars().next() {
                            // convert to lowercase so that the commands work with capslock on
                            match char.to_ascii_lowercase() {
                                'c' => {
                                    // copy
                                    if let Some(text) = drv.editor.selected_text() {
                                        clipboard.0 = text.to_owned();
                                    }
                                }
                                'x' => {
                                    // cut
                                    if let Some(text) = drv.editor.selected_text() {
                                        clipboard.0 = text.to_owned();
                                        drv.delete_selection();
                                    }
                                }
                                'v' => {
                                    // paste
                                    drv.insert_or_replace_selection(&clipboard.0);
                                }
                                'a' => {
                                    // select all
                                    drv.select_all();
                                }
                                _ => {
                                    // not recognised, ignore
                                }
                            }
                        }
                    }
                    Key::ArrowLeft => {
                        drv.move_word_left();
                    }
                    Key::ArrowRight => {
                        drv.move_word_right();
                    }
                    Key::Home => {
                        if modifiers.shift {
                            drv.select_to_text_start();
                        } else {
                            drv.move_to_text_start();
                        }
                    }
                    Key::End => {
                        if modifiers.shift {
                            drv.select_to_text_end();
                        } else {
                            drv.move_to_text_end();
                        }
                    }
                    _ => {
                        // not recognised, ignore
                    }
                }
            }

            match &keyboard.logical_key {
                Key::Space => {
                    drv.insert_or_replace_selection(" ");
                }
                Key::Character(str) => {
                    println!("key: {str}");
                    drv.insert_or_replace_selection(str);
                }
                Key::ArrowLeft => {
                    if modifiers.shift {
                        drv.select_left();
                    } else {
                        drv.move_left();
                    }
                }
                Key::ArrowRight => {
                    if modifiers.shift {
                        drv.select_right();
                    } else {
                        drv.move_right();
                    }
                }
                Key::ArrowUp => {
                    if modifiers.shift {
                        drv.select_up();
                    } else {
                        drv.move_up();
                    }
                }
                Key::ArrowDown => {
                    if modifiers.shift {
                        drv.select_down();
                    } else {
                        drv.move_down();
                    }
                }
                Key::Backspace => {
                    drv.backdelete();
                }
                Key::Delete => {
                    if modifiers.shift {
                        drv.delete_selection();
                    } else {
                        drv.delete();
                    }
                }
                Key::Home => {
                    if modifiers.shift {
                        drv.select_to_line_start();
                    } else {
                        drv.move_to_line_start();
                    }
                }
                Key::End => {
                    if modifiers.shift {
                        drv.select_to_line_end();
                    } else {
                        drv.move_to_line_end();
                    }
                }
                Key::Escape => {
                    drv.collapse_selection();
                }
                _ => {}
            }
        }
    }
}

pub fn update_editor_system(
    mut font_cx: ResMut<FontCx>,
    mut layout_cx: ResMut<LayoutCx>,
    mut scale_cx: ResMut<ScaleCx>,
    mut font_atlas_set: ResMut<FontAtlasSet>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut textures: ResMut<Assets<Image>>,
    mut input_field_query: Query<(
        &TextFont,
        &LineHeight,
        &FontHinting,
        Ref<ComputedUiRenderTargetInfo>,
        &mut TextEditor,
        &mut TextInput,
        &mut TextLayoutInfo,
        Ref<ComputedNode>,
    )>,
) {
    for (
        text_font,
        _line_height,
        hinting,
        target,
        mut editor,
        text_field,
        mut info,
        computed_node,
    ) in input_field_query.iter_mut()
    {
        if text_field.is_changed() {
            editor.editor.set_text(text_field.0.as_str());
        }
        if target.is_changed() {
            editor.editor.set_scale(target.scale_factor());
        }

        if computed_node.is_changed() {
            editor.editor.set_width(Some(computed_node.size().x));
        }

        let mut driver = editor.editor.driver(&mut font_cx.0, &mut layout_cx.0);

        driver.refresh_layout();

        let layout = driver.layout();

        info.scale_factor = layout.scale();
        info.size = (
            layout.width() / layout.scale(),
            layout.height() / layout.scale(),
        )
            .into();

        info.glyphs.clear();
        info.run_geometry.clear();

        // println!("info.size: {}", info.size);
        for line in layout.lines() {
            for (line_index, item) in line.items().enumerate() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyph_run) => {
                        let (span_index, smoothing) = glyph_run.style().brush;

                        let run = glyph_run.run();

                        let font_data = run.font();
                        let font_size = run.font_size();
                        let coords = run.normalized_coords();

                        let font_atlas_key = FontAtlasKey {
                            id: font_data.data.id() as u32,
                            index: font_data.index,
                            font_size_bits: font_size.to_bits(),
                            variations_hash: FixedHasher.hash_one(coords),
                            hinting: *hinting,
                            font_smoothing: smoothing,
                        };

                        for glyph in glyph_run.positioned_glyphs() {
                            let font_atlases = font_atlas_set.entry(font_atlas_key).or_default();
                            let Ok(atlas_info) = get_glyph_atlas_info(
                                font_atlases,
                                GlyphCacheKey {
                                    glyph_id: glyph.id as u16,
                                },
                            )
                            .map(Ok)
                            .unwrap_or_else(|| {
                                let font_ref = FontRef::from_index(
                                    font_data.data.as_ref(),
                                    font_data.index as usize,
                                )
                                .unwrap();
                                let mut scaler = scale_cx
                                    .builder(font_ref)
                                    .size(font_size)
                                    .hint(true)
                                    .normalized_coords(coords)
                                    .build();
                                add_glyph_to_atlas(
                                    font_atlases,
                                    texture_atlases.as_mut(),
                                    textures.as_mut(),
                                    &mut scaler,
                                    text_font.font_smoothing,
                                    glyph.id as u16,
                                )
                            }) else {
                                continue;
                            };

                            let texture_atlas =
                                texture_atlases.get(atlas_info.texture_atlas).unwrap();
                            let location = atlas_info.location;
                            let glyph_rect = texture_atlas.textures[location.glyph_index];
                            let glyph_size = UVec2::new(glyph_rect.width(), glyph_rect.height());
                            let x = glyph_size.x as f32 / 2. + glyph.x + location.offset.x as f32;
                            let y = glyph_size.y as f32 / 2. + glyph.y - location.offset.y as f32;

                            info.glyphs.push(PositionedGlyph {
                                position: (x, y).into(),
                                size: glyph_size.as_vec2(),
                                atlas_info,
                                span_index: span_index as usize,
                                line_index,
                                byte_index: line.text_range().start,
                                byte_length: line.text_range().len(),
                            });
                        }

                        info.run_geometry.push(RunGeometry {
                            span_index: span_index as usize,
                            bounds: Rect {
                                min: Vec2::new(glyph_run.offset(), line.metrics().min_coord),
                                max: Vec2::new(
                                    glyph_run.offset() + glyph_run.advance(),
                                    line.metrics().max_coord,
                                ),
                            },
                            strikethrough_y: glyph_run.baseline()
                                - run.metrics().strikethrough_offset,
                            strikethrough_thickness: run.metrics().strikethrough_size,
                            underline_y: glyph_run.baseline() - run.metrics().underline_offset,
                            underline_thickness: run.metrics().underline_size,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

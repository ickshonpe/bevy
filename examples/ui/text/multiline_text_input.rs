//! Demonstrates a single, minimal multiline [`EditableText`] widget.

use bevy::color::palettes::css::{DARK_SLATE_GRAY, YELLOW};
use bevy::input::keyboard::Key;
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin};
use bevy::input_focus::{AutoFocus, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextCursorStyle};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, TabNavigationPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, submit_font_size_and_visible_lines)
        .run();
}

#[derive(Component)]
struct MultilineInput;

#[derive(Component)]
struct VisibleLinesInput;

#[derive(Component)]
struct FontSizeInput;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands
        .spawn(Node {
            width: percent(100.),
            height: percent(100.),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,

            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::End,
                        row_gap: px(10.),
                        ..default()
                    },
                    TabGroup::default(),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            width: px(450.),
                            border: px(2.).all(),
                            padding: px(8.).all(),
                            ..default()
                        },
                        EditableText {
                            visible_lines: Some(8.),
                            allow_newlines: true,
                            ..default()
                        },
                        TextLayout {
                            linebreak: LineBreak::AnyCharacter,
                            ..default()
                        },
                        TextCursorStyle {
                            selected_text_color: Some(Color::BLACK),
                            ..default()
                        },
                        TextFont {
                            font: asset_server.load("fonts/FiraMono-Medium.ttf").into(),
                            font_size: FontSize::Px(30.),
                            ..default()
                        },
                        BackgroundColor(DARK_SLATE_GRAY.into()),
                        BorderColor::all(YELLOW),
                        MultilineInput,
                        TabIndex(0),
                        AutoFocus,
                    ));

                    parent.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: px(10.),
                            ..default()
                        },
                        children![
                            (
                                Text::new("visible lines:"),
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf").into(),
                                    font_size: FontSize::Px(30.),
                                    ..default()
                                },
                            ),
                            (
                                Node {
                                    width: px(100.),
                                    border: px(2.).all(),
                                    ..default()
                                },
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf").into(),
                                    font_size: FontSize::Px(30.),
                                    ..default()
                                },
                                TextLayout {
                                    justify: Justify::End,
                                    ..default()
                                },
                                BackgroundColor(DARK_SLATE_GRAY.into()),
                                BorderColor::all(YELLOW),
                                EditableText::new("8"),
                                EditableTextFilter::new(|c| c.is_ascii_digit()),
                                TextCursorStyle {
                                    selected_text_color: Some(Color::BLACK),
                                    ..default()
                                },
                                VisibleLinesInput,
                                TabIndex(1),
                            )
                        ],
                    ));

                    parent.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: px(10.),
                            ..default()
                        },
                        children![
                            (
                                Text::new("font size:"),
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf").into(),
                                    font_size: FontSize::Px(30.),
                                    ..default()
                                },
                            ),
                            (
                                Node {
                                    width: px(100.),
                                    border: px(2.).all(),
                                    ..default()
                                },
                                TextFont {
                                    font: asset_server.load("fonts/FiraMono-Medium.ttf").into(),
                                    font_size: FontSize::Px(30.),
                                    ..default()
                                },
                                TextLayout {
                                    justify: Justify::End,
                                    ..default()
                                },
                                BackgroundColor(DARK_SLATE_GRAY.into()),
                                BorderColor::all(YELLOW),
                                EditableText::new("30"),
                                EditableTextFilter::new(|c| c.is_ascii_digit()),
                                TextCursorStyle {
                                    selected_text_color: Some(Color::BLACK),
                                    ..default()
                                },
                                FontSizeInput,
                                TabIndex(3),
                            )
                        ],
                    ));
                });
        });
}

fn submit_font_size_and_visible_lines(
    input_focus: Res<InputFocus>,
    keyboard_input: Res<ButtonInput<Key>>,
    mut query_set: ParamSet<(
        Query<&EditableText, With<VisibleLinesInput>>,
        Query<&EditableText, With<FontSizeInput>>,
        Query<(&mut EditableText, &mut TextFont), With<MultilineInput>>,
    )>,
) {
    if !keyboard_input.just_pressed(Key::Enter) || !keyboard_input.pressed(Key::Control) {
        return;
    }

    let Some(focused_entity) = input_focus.get() else {
        return;
    };

    if let Ok(visible_lines_input) = query_set.p0().get(focused_entity) {
        let Some(submitted_value) = parse_positive_integer(visible_lines_input) else {
            return;
        };
        let mut multiline_query = query_set.p2();
        let Ok((mut multiline_input, _)) = multiline_query.single_mut() else {
            return;
        };
        multiline_input.visible_lines = Some(submitted_value as f32);
        return;
    }

    if let Ok(font_size_input) = query_set.p1().get(focused_entity) {
        let Some(submitted_value) = parse_positive_integer(font_size_input) else {
            return;
        };
        let mut multiline_query = query_set.p2();
        let Ok((_, mut multiline_font)) = multiline_query.single_mut() else {
            return;
        };
        multiline_font.font_size = FontSize::Px(submitted_value as f32);
    }
}

fn parse_positive_integer(input: &EditableText) -> Option<u32> {
    let mut submitted_value = String::new();
    submitted_value.reserve(input.value().into_iter().map(str::len).sum());
    for sub_str in input.value() {
        submitted_value.push_str(sub_str);
    }

    let Ok(submitted_value) = submitted_value.parse::<u32>() else {
        return None;
    };

    if submitted_value == 0 {
        return None;
    }

    Some(submitted_value)
}

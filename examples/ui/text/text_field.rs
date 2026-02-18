//! Text field example

use bevy::{
    color::palettes::css::NAVY,
    input_focus::{InputDispatchPlugin, InputFocus},
    prelude::*,
    text::Underline,
    ui::widget::TextInput,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputDispatchPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // UI camera
    commands.spawn(Camera2d);

    let input_entity = commands
        .spawn((
            Node {
                width: px(300.),
                height: px(500.),
                ..default()
            },
            TextInput::new("editable text"),
            TextFont { ..default() },
            BackgroundColor(NAVY.into()),
        ))
        .id();

    commands.insert_resource(InputFocus(Some(input_entity)));

    // Text with one section
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                margin: px(40.).all(),
                padding: px(10.).all(),
                row_gap: px(10.),
                ..Default::default()
            },
            Outline {
                ..Default::default()
            },
        ))
        .with_child((Text::new("Example text field"), Underline))
        .add_child(input_entity);
}

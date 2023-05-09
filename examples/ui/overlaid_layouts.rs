//! This example demonstrates overlayed layouts

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

// A unit struct to help identify the FPS UI component, since there may be many Text components
#[derive(Component)]
struct FpsText;

// A unit struct to help identify the color-changing Text component
#[derive(Component)]
struct ColorText;

fn setup(mut commands: Commands) {
    // UI camera
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(UiLayoutBundle {
            order: UiLayoutOrder(2),
            ..Default::default()
        })
        .with_children(|builder| {
            builder.spawn(NodeBundle {
                style: Style {
                    size: Size::width(Val::Percent(100.)),
                    margin: UiRect {
                        left: Val::Percent(25.),
                        right: Val::Percent(35.),
                        ..default()
                    },
                    ..Default::default()
                },
                background_color: Color::GREEN.into(),
                ..Default::default()
            });
        });

    commands
        .spawn(UiLayoutBundle {
            order: UiLayoutOrder(1),
            ..Default::default()
        })
        .with_children(|builder| {
            builder.spawn(NodeBundle {
                style: Style {
                    size: Size::all(Val::Percent(75.)),
                    margin: UiRect::all(Val::Px(10.)),
                    ..Default::default()
                },
                background_color: Color::NAVY.into(),
                ..Default::default()
            });
        });

    commands.spawn(NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            right: Val::Px(0.),
            bottom: Val::Px(0.),
            size: Size::all(Val::Percent(75.)),

            ..Default::default()
        },
        background_color: Color::RED.into(),
        ..Default::default()
    });
}

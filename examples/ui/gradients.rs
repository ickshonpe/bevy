//! Example demonstrating gradients

use std::f32::consts::PI;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn(NodeBundle {
        style: Style {
            flex_wrap: FlexWrap::Wrap,
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            row_gap: Val::Px(10.),
            column_gap: Val::Px(10.),
            ..Default::default()
        },
        ..Default::default()
    }).with_children(|commands| {
    for i in 0..4 {
        let angle = 0.5 * PI * i as f32;
        commands.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            ..Default::default()
        }).with_children(|commands| {
            commands.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(100.),
                    height: Val::Px(100.),
                    ..Default::default()
                },
                background_color: BackgroundColor::LinearGradient(LinearGradient { start_color: Color::WHITE, end_color: Color::BLACK, angle }),
                ..Default::default()
            });

            commands.spawn(TextBundle::from_section(angle.to_string(), TextStyle::default()));
        });
    }

    for i in 0..8 {
        let angle = 0.25 * PI * i as f32;
        commands.spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            ..Default::default()
        }).with_children(|commands| {
            commands.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(100.),
                    height: Val::Px(100.),
                    ..Default::default()
                },
                background_color: BackgroundColor::LinearGradient(LinearGradient { start_color: Color::WHITE, end_color: Color::RED, angle }),
                ..Default::default()
            });

            commands.spawn(TextBundle::from_section(angle.to_string(), TextStyle::default()));
        });
    }
});
}
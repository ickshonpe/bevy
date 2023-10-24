//! Example demonstrating gradients

use std::f32::consts::PI;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    let root = commands
        .spawn(NodeBundle {
            style: Style {
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                row_gap: Val::Px(20.),
                column_gap: Val::Px(10.),
                ..Default::default()
            },
            ..Default::default()
        })
        .id();

    let group = spawn_group(&mut commands);

    commands.entity(root).add_child(group);
}

fn spawn_group(commands: &mut Commands) -> Entity {
    commands
        .spawn(NodeBundle {
            style: Style {
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(10.),
                column_gap: Val::Px(10.),
                ..Default::default()
            },
            ..Default::default()
        })
        .with_children(|commands| {
            commands
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .with_children(|commands| {
                    commands.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(500.),
                            height: Val::Px(500.),
                            ..Default::default()
                        },
                        background_color: RadialGradient::new(
                            Default::default(),
                            Default::default(),
                            vec![
                                (Color::RED, Val::Auto).into(),
                                //(Color::GREEN, Val::Auto).into(),
                                (Color::BLUE, Val::Auto).into(),
                            ],
                        )
                        .into(),
                        ..Default::default()
                    });

                    commands.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(500.),
                            height: Val::Px(500.),
                            ..Default::default()
                        },
                        background_color: RadialGradient::new(
                            Default::default(),
                            Default::default(),
                            vec![
                                (Color::RED, Val::Px(10.)).into(),
                                (Color::GREEN, Val::Px(20.)).into(),
                                (Color::GREEN, Val::Px(30.)).into(),
                                (Color::BLUE, Val::Px(50.)).into(),
                                (Color::BLUE, Val::Px(80.)).into(),
                                (Color::YELLOW, Val::Px(300.)).into(),
                                (Color::BLACK, Val::Px(300.)).into(),
                            ],
                        )
                        .into(),
                        ..Default::default()
                    });
                });
        })
        .id()
}

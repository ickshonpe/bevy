//! Example demonstrating bordered UI nodes

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
                border: UiRect::all(Val::Px(20.0)),
                border_radius: BorderRadius::all(Val::Px(40.)),
                padding: UiRect::all(Val::Px(3.)),
                align_self: AlignSelf::Stretch,
                justify_self: JustifySelf::Stretch,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                align_content: AlignContent::FlexStart,
                ..Default::default()
            },
            border_color: BorderColor(Color::GRAY),
            background_color: BackgroundColor(Color::DARK_GRAY),
            ..Default::default()
        })
        .id();

    // all the different combinations of border edges
    let borders = [
        UiRect::default(),
        UiRect::all(Val::Px(10.)),
        UiRect::left(Val::Px(10.)),
        UiRect::right(Val::Px(10.)),
        UiRect::top(Val::Px(10.)),
        UiRect::bottom(Val::Px(10.)),
        UiRect::horizontal(Val::Px(10.)),
        UiRect::vertical(Val::Px(10.)),
        UiRect {
            left: Val::Px(10.),
            top: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            left: Val::Px(10.),
            bottom: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            right: Val::Px(10.),
            top: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            right: Val::Px(10.),
            bottom: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            right: Val::Px(10.),
            top: Val::Px(10.),
            bottom: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            left: Val::Px(10.),
            top: Val::Px(10.),
            bottom: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            left: Val::Px(10.),
            right: Val::Px(10.),
            top: Val::Px(10.),
            ..Default::default()
        },
        UiRect {
            left: Val::Px(10.),
            right: Val::Px(10.),
            bottom: Val::Px(10.),
            ..Default::default()
        },
    ];

    for i in 0..64 {
        let inner_spot = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Px(10.),
                    height: Val::Px(10.),
                    border_radius: BorderRadius::all(Val::Px(100.)),
                    ..Default::default()
                },
                background_color: Color::YELLOW.into(),
                ..Default::default()
            })
            .id();
        let bordered_node = commands
            .spawn((NodeBundle {
                    style: Style {
                        width: Val::Px(50.),
                        height: Val::Px(50.),
                        border: borders[i % borders.len()],
                        margin: UiRect::all(Val::Px(6.)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(15.)),
                        ..Default::default()
                    },
                    background_color: Color::MAROON.into(),
                    border_color: Color::RED.into(),
                    ..Default::default()
                },
                Outline {
                    width: Val::Px(4.),
                    color: Color::ORANGE_RED,
                },
            ))
            .add_child(inner_spot)
            .id();
        commands.entity(root).add_child(bordered_node);
    }
}

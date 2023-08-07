//! Simple example demonstrating overflow behavior.

use bevy::{prelude::*, winit::WinitSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Only run the app when there is user input. This will significantly reduce CPU/GPU use.
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    let image = asset_server.load("branding/icon.png");

    let style = Style {
        min_width: Val::Px(100.),
        min_height: Val::Px(100.),
        max_width: Val::Px(100.),
        max_height: Val::Px(100.),
        overflow: Overflow::clip(),
        ..Default::default()
    };
    
        commands
            .spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    width: Val::Percent(100.),
                    row_gap: Val::Px(10.),
                    column_gap: Val::Px(10.),
                    ..Default::default()
                },
                background_color: Color::ANTIQUE_WHITE.into(),
                ..Default::default()
            })
            .with_children(|parent| {
                for s in 0..4 {
                    let mut content_transform = UiContentTransform::default();
                    for _ in 0..2 {
                        parent.spawn(NodeBundle { style: Style { flex_direction: FlexDirection::Row, align_items: AlignItems::Center,row_gap: Val::Px(10.),
                            column_gap: Val::Px(10.),
                            justify_content: JustifyContent::Center, ..Default::default() }, ..Default::default() })
                        .with_children(|parent| {
                            for _ in 0..4 {
                                let mut inner_style = style.clone();
                                match s {
                                    0 => inner_style.left = Val::Px(50.),
                                    1 => inner_style.right = Val::Px(50.),
                                    2 => inner_style.top = Val::Px(50.),
                                    _ => inner_style.bottom = Val::Px(50.),
                                }
                                parent.spawn(NodeBundle {
                                    style: style.clone(),
                                    background_color: Color::CYAN.into(),
                                    ..Default::default()
                                }).with_children(|parent| {
                                    parent.spawn(ImageBundle {
                                        image: UiImage::new(image.clone()),
                                        style: inner_style,
                                        background_color: Color::WHITE.into(),
                                        content_orientation: content_transform,
                                        ..Default::default()
                                    });
                                });
                                content_transform = content_transform.rotate_left();
                            }
                        });
                        content_transform = content_transform.flip_x();
                    }
                }
            });
}

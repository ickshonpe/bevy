//! Simple example for debugging image nodes

use bevy::{prelude::*, winit::WinitSettings};
use bevy_internal::window::WindowResolution;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                
               // resolution: WindowResolution::new(1000., 500.).with_scale_factor_override(1.0),
                ..Default::default()
            }),
            ..Default::default()
        }))
        // Only run the app when there is user input. This will significantly reduce CPU/GPU use.
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    let image = asset_server.load("branding/300x100.png");

    commands
        .spawn(NodeBundle {
            style: Style {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                size: Size::width(Val::Percent(100.)),
                gap: Size::all(Val::Px(10.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .with_children(|builder| {
            builder.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    size: Size::width(Val::Px(500.)),
                    gap: Size::all(Val::Px(10.)),
                    ..default()
                },
                background_color: Color::YELLOW.into(),
                ..Default::default()
            }).with_children(|builder| {
                builder.spawn(ImageBundle {
                    image: UiImage {
                        texture: image.clone().into(),
                        mode: ImageMode::FillNode,
                        ..Default::default()
                    },
                    style: Style {
                        size: Size::width(Val::Percent(50.)),
                        ..default()
                    },
                    background_color: Color::RED.with_a(0.5).into(),
                    ..Default::default()
                });

                builder.spawn(ImageBundle {
                    image: UiImage {
                        texture:  asset_server.load("branding/100x100.png"),
                        mode: ImageMode::FillNode,
                        ..Default::default()
                    },
                    style: Style {
                        size: Size::width(Val::Percent(50.)),
                        ..default()
                    },
                    background_color: Color::RED.with_a(0.5).into(),
                    ..Default::default()
                });

                builder.spawn(ImageBundle {
                    image: UiImage {
                        texture:  asset_server.load("branding/200x100.png"),
                        mode: ImageMode::FillNode,
                        ..Default::default()
                    },
                    style: Style {
                        size: Size::width(Val::Percent(50.)),
                        ..default()
                    },
                    background_color: Color::RED.with_a(0.5).into(),
                    ..Default::default()
                });

                builder.spawn(ImageBundle {
                    image: UiImage {
                        texture:  asset_server.load("branding/400x100.png"),
                        mode: ImageMode::FillNode,
                        ..Default::default()
                    },
                    style: Style {
                        size: Size::width(Val::Percent(50.)),
                        ..default()
                    },
                    background_color: Color::RED.with_a(0.5).into(),
                    ..Default::default()
                });
                // builder.spawn(ImageBundle {
                //     image: UiImage {
                //         texture: image.clone().into(),
                //         mode: ImageMode::PreserveAspectRatio,
                //         ..Default::default()
                //     },
                //     style: Style {
                //         size: Size::width(Val::Percent(50.)),
                //         ..default()
                //     },
                //     background_color: Color::GREEN.with_a(0.5).into(),
                //     ..Default::default()
                // });
                // builder.spawn(ImageBundle {
                //     image: UiImage {
                //         texture: image.clone().into(),
                //         mode: ImageMode::PreserveAspectRatio,
                //         ..Default::default()
                //     },
                //     style: Style {
                //         size: Size::width(Val::Percent(50.)),
                //         ..default()
                //     },
                //     background_color: Color::BLUE.with_a(0.5).into(),
                //     ..Default::default()
                // });
            });
        });
}

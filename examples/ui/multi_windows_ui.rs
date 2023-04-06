//! This example illustrates ui in multiple windows.

use bevy::{
    prelude::*, render::camera::RenderTarget, window::WindowRef,
};


fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Window 1".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_systems(Startup, spawn)
        .run();
}

fn spawn(mut commands: Commands, asset_server: Res<AssetServer>) {
    let window_2 = commands.spawn(Window {
        title: "Window 2".to_owned(),
        ..Default::default()
    })
    .id();

    commands.spawn(Camera2dBundle::default());
    commands.spawn(Camera2dBundle {
        camera: Camera {
            target: RenderTarget::Window(WindowRef::Entity(window_2)),
            ..Default::default()
        },
        ..Default::default()
    });

    commands.spawn(TextBundle {
        style: Style {
            size: Size::width(Val::Percent(50.)),
            ..Default::default()
        },
        text: Text::from_section(
            "Window 1",
            TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 40.0,
                color: Color::WHITE,
            }
        ),
        ..Default::default()
    });


    commands.spawn(TextBundle {
        style: Style {
            size: Size::width(Val::Percent(50.)),
            ..Default::default()
        },
        text: Text::from_section(
            "Window 2",
            TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 40.0,
                color: Color::WHITE,
            }
        ),
        ..Default::default()
    });

}
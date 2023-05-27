//! Demonstrates multiple windows each with their own UI layout

use bevy::{prelude::*, render::camera::RenderTarget, window::WindowRef};
use bevy_internal::ui::LayoutContext;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_scene)
        .add_systems(Update, bevy::window::close_on_esc)
        .run();
}

fn setup_scene(mut commands: Commands) {
    // Primary window camera
    commands.spawn(Camera3dBundle::default());

    // Spawn a second window
    let second_window = commands
        .spawn(Window {
            title: "Second Window".to_owned(),
            ..default()
        })
        .insert(LayoutContext::default())
        .id();

    // Secondary window camera
    commands.spawn(Camera3dBundle {
        camera: Camera {
            target: RenderTarget::Window(WindowRef::Entity(second_window)),
            ..default()
        },
        ..default()
    });

    commands.spawn(TextBundle::from_section(
        "First Window",
        TextStyle::default(),
    ));
    commands.spawn(TextBundle::from_section(
        "Second Window",
        TextStyle::default(),
    ));
}

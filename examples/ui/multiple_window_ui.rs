//! Demonstrates multiple windows each with their own UI layout

use bevy::{prelude::*, render::camera::RenderTarget, window::WindowRef};
use bevy_internal::{ui::LayoutContext, window::PrimaryWindow};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_scene)
        .add_systems(Update, bevy::window::close_on_esc)
        .run();
}

#[derive(Component, Default)]
struct FirstWindowNode;

#[derive(Component, Default)]
struct SecondWindowNode;

fn setup_scene(mut commands: Commands, query: Query<Entity, With<PrimaryWindow>>) {
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

    spawn_nodes::<FirstWindowNode>(&mut commands, "first window", None);

    spawn_nodes::<SecondWindowNode>(&mut commands, "second window", Some(second_window));
}

fn spawn_nodes<M: Component + Default>(commands: &mut Commands, title: &str, view: Option<Entity>) {
    let mut ec = commands.spawn(NodeBundle {
        style: Style {
            flex_direction: FlexDirection::Column,
            column_gap: Val::Px(30.),
            ..Default::default()
        },
        ..Default::default()
    });
    ec.with_children(|builder| {
        builder.spawn(TextBundle::from_section(title, TextStyle::default()));

        builder.spawn((
            TextBundle::from_section("0", TextStyle::default()),
            M::default(),
        ));

        builder
            .spawn((
                ButtonBundle {
                    button: Button,
                    style: Style {
                        padding: UiRect::all(Val::Px(10.)),
                        ..Default::default()
                    },
                    background_color: Color::BLACK.into(),
                    ..Default::default()
                },
                M::default(),
            ))
            .with_children(|builder| {
                builder.spawn(TextBundle::from_section(
                    format!("{title} button"),
                    TextStyle::default(),
                ));
            });
    });

    if let Some(view) = view {
        ec.insert(UiView { view });
    }
}

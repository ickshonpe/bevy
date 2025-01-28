//! Uses two windows to visualize a 3D model from different angles.

use bevy::{prelude::*, render::camera::RenderTarget, window::WindowRef};

fn main() {
    App::new()
        // By default, a primary window gets spawned by `WindowPlugin`, contained in `DefaultPlugins`
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_scene)
        .add_systems(Update, swap_cameras)
        .run();
}

#[derive(Component)]
pub struct Marker;

fn setup_scene(mut commands: Commands, asset_server: Res<AssetServer>) {
    // add entities to the world
    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/torus/torus.gltf")),
    ));
    // light
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(3.0, 3.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let first_window_camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();

    // Spawn a second window
    let second_window = commands
        .spawn(Window {
            title: "Second window".to_owned(),
            ..default()
        })
        .id();

    let second_window_camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
            Camera {
                target: RenderTarget::Window(WindowRef::Entity(second_window)),
                ..default()
            },
        ))
        .id();

    let node = Node {
        position_type: PositionType::Absolute,
        top: Val::Px(12.0),
        left: Val::Px(12.0),
        ..default()
    };

    commands.spawn((
        Text::new("First window"),
        node.clone(),
        // Since we are using multiple cameras, we need to specify which camera UI should be rendered to
        UiTargetCamera(first_window_camera),
    ));

    commands.spawn((
        Text::new("Second window"),
        node,
        UiTargetCamera(second_window_camera),
    ));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::End,
                justify_content: JustifyContent::End,
                ..Default::default()
            },
            UiTargetCamera(second_window_camera),
            Marker,
        ))
        .with_child(Text::new("Hello"));
}

fn swap_cameras(
    mut t: Local<f32>,
    time: Res<Time>,
    cameras: Query<Entity, With<Camera>>,
    mut target: Query<&mut UiTargetCamera, With<Marker>>,
) {
    *t += time.delta_secs();

    if 1. < *t {
        *t = 0.;
        let mut target_camera = target.single_mut();
        for camera in cameras.iter() {
            if target_camera.0 != camera {
                target_camera.0 = camera;
                break;
            }
        }
    }
}

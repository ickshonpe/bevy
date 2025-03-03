//! Demonstrates how to use transparency in 3D.
//! Shows the effects of different blend modes.
//! The `fade_transparency` system smoothly changes the transparency over time.

use bevy::{math::ops, prelude::*};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, fade_transparency)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Opaque plane, uses `alpha_mode: Opaque` by default
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(6.0, 6.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
    ));

    // Transparent sphere, uses `alpha_mode: Mask(f32)`
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(3).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Alpha channel of the color controls transparency.
            // We set it to 0.0 here, because it will be changed over time in the
            // `fade_transparency` function.
            // Note that the transparency has no effect on the objects shadow.
            base_color: Color::srgba(0.2, 0.7, 0.1, 0.0),
            // Mask sets a cutoff for transparency. Alpha values below are fully transparent,
            // alpha values above are fully opaque.
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        })),
        Transform::from_xyz(1.0, 0.5, -1.5),
    ));

    // Transparent unlit sphere, uses `alpha_mode: Mask(f32)`
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(3).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.7, 0.1, 0.0),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(-1.0, 0.5, -1.5),
    ));

    // Transparent cube, uses `alpha_mode: Blend`
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        // Notice how there is no need to set the `alpha_mode` explicitly here.
        // When converting a color to a material using `into()`, the alpha mode is
        // automatically set to `Blend` if the alpha channel is anything lower than 1.0.
        MeshMaterial3d(materials.add(Color::srgba(0.5, 0.5, 1.0, 0.0))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    // Transparent cube, uses `alpha_mode: AlphaToCoverage`
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.5, 1.0, 0.5, 0.0),
            alpha_mode: AlphaMode::AlphaToCoverage,
            ..default()
        })),
        Transform::from_xyz(-1.5, 0.5, 0.0),
    ));

    // Opaque sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(3).unwrap())),
        MeshMaterial3d(materials.add(Color::srgb(0.7, 0.2, 0.1))),
        Transform::from_xyz(0.0, 0.5, -1.5),
    ));

    // Light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Fades the alpha channel of all materials between 0 and 1 over time.
/// Each blend mode responds differently to this:
/// - [`Opaque`](AlphaMode::Opaque): Ignores alpha channel altogether, these materials stay completely opaque.
/// - [`Mask(f32)`](AlphaMode::Mask): Object appears when the alpha value goes above the mask's threshold, disappears
///   when the alpha value goes back below the threshold.
/// - [`Blend`](AlphaMode::Blend): Object fades in and out smoothly.
/// - [`AlphaToCoverage`](AlphaMode::AlphaToCoverage): Object fades in and out
///   in steps corresponding to the number of multisample antialiasing (MSAA)
///   samples in use. For example, assuming 8xMSAA, the object will be
///   completely opaque, then will be 7/8 opaque (1/8 transparent), then will be
///   6/8 opaque, then 5/8, etc.
pub fn fade_transparency(time: Res<Time>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let alpha = (ops::sin(time.elapsed_secs()) / 2.0) + 0.5;
    for (_, material) in materials.iter_mut() {
        material.base_color.set_alpha(alpha);
    }
}

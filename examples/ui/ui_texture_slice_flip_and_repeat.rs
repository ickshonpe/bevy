//! This example illustrates how to create buttons with their textures sliced
//! and kept in proportion instead of being stretched by the button dimensions

use bevy::{prelude::*, winit::WinitSettings};
use bevy_render::texture::{ImageLoaderSettings, ImageSampler};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(UiScale(2.))
        // Only run the app when there is user input. This will significantly reduce CPU/GPU use.
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let image = asset_server.load_with_settings(
        "textures/fantasy_ui_borders/numbered_slices.png",
        |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::nearest();
        },
    );

    let slicer = TextureSlicer {
        border: BorderRect::square(16.0),
        center_scale_mode: SliceScaleMode::Tile { stretch_value: 1. },
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 1. },
        max_corner_scale: 1.0,
    };
    // ui camera
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_content: AlignContent::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(10.),
                row_gap: Val::Px(10.),
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            for ([w, h], flip_x, flip_y) in [
                ([160.0, 160.0], false, false),
                ([320.0, 160.0], false, true),
                ([320.0, 160.0], true, false),
                ([160.0, 160.0], true, true),
            ] {
                parent.spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Px(w),
                            height: Val::Px(h),
                            ..default()
                        },
                        ..Default::default()
                    },
                    UiImage {
                        texture: image.clone(),
                        flip_x,
                        flip_y,
                        ..Default::default()
                    },
                    ImageScaleMode::Sliced(slicer.clone()),
                ));
            }
        });
}
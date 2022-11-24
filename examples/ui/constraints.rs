//! Demonstrates using min/max size constraints with text.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UiScale { scale: 2.0 } );
    commands
        .spawn(Camera2dBundle::default());
    commands
        .spawn(TextBundle {
            text: Text::from_section(
                 "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Maecenas auctor, nunc ac faucibus fringilla.",
                TextStyle { 
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"), 
                    font_size: 30.0, 
                    color: Color::WHITE
                },
            ).with_alignment(TextAlignment::CENTER),
            style: Style {
                min_size: Size::new(Val::Percent(25.0), Val::Percent(100.0)),
                max_size: Size::new(Val::Percent(50.0), Val::Percent(100.0)),
                ..Default::default()
            },
            ..Default::default()
        });
}

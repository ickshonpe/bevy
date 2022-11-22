//! Demonstrates using min/max size constraints with text.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    let text_style = TextStyle {
        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
        size: 16.0,
        color: Color::WHITE,
    };

    


}
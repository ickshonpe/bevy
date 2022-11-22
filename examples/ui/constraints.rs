//! Demonstrates using min/max size constraints with text.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(
            WindowPlugin {
                window: WindowDescriptor { 
                    width: 800., 
                    height: 800.,
                    ..Default::default()
                },
                ..Default::default()
            }
        ))
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
  //  commands.insert_resource(UiScale { scale: 0.9 });

    commands.spawn(Camera2dBundle::default());
    
    let text_style = TextStyle {
        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
        font_size: 32.0,
        color: Color::WHITE,
    };
    

    commands.spawn(NodeBundle {
        style: Style { 
            position_type: PositionType::Absolute,
            position: UiRect { left: Val::Px(100.), top: Val::Px(100.), ..Default::default() },
            size: Size::new(Val::Px(600.), Val::Px(600.)),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        background_color: BackgroundColor(Color::NAVY),
        ..Default::default()
    }).with_children(|builder| {
        // builder.spawn(TextBundle {
        //     text: Text::from_section("200px: constrained text one two three four five six seven eight nine ten", text_style.clone()),
        //     style: Style {
        //         max_size: Size { width: Val::Px(200.), height: Val::Auto },
        //         ..Default::default()
        //     },
        //     ..Default::default()
        // });
        // builder.spawn(TextBundle {
        //     text: Text::from_section("400px: constrained text one two three four five six seven eight nine ten", text_style.clone()),
        //     style: Style {
        //         max_size: Size { width: Val::Px(400.), height: Val::Auto },
        //         ..Default::default()
        //     },
        //     ..Default::default()
        // });
        // builder.spawn(TextBundle {
        //     style: Style {
        //         max_size: Size { width: Val::Px(600.), height: Val::Auto },
        //         ..Default::default()
        //     },
        //     text: Text::from_section("600px: constrained text one two three four five six seven eight nine ten", text_style.clone()),
        //     ..Default::default()
        // });
        builder.spawn(TextBundle {
            text: Text::from_section(
                //"25%: constrained text one two three four five six seven eight nine ten",
                "x y x y xxy x y x y x y x y x y x y x y x y x y x y x y x y x y x",
                //"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                text_style.clone()),
            style: Style {
                max_size: Size { width: Val::Percent(50.), ..Default::default() },
                //size: Size { height: Val::Px(100.), ..Default::default() },
                ..Default::default()
            },
            ..Default::default()
        });
        builder.spawn(TextBundle {
            text: Text::from_section(
                "a y x y xxy x y x y x y x y x y x y x y x y x y x y x y x y x y x",
                //"50%: constrained text one two three four five six seven eight nine ten", 
                text_style.clone()),
            style: Style {
                max_size: Size { width: Val::Px(300.), ..Default::default() },
                ..Default::default()
            },
            ..Default::default()
        });
        
    });

    
    


}
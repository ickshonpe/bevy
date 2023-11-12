//! This example illustrates how to create a button that changes color and text based on its
//! interaction state.

use bevy::{prelude::*, winit::WinitSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Only run the app when there is user input. This will significantly reduce CPU/GPU use.
        .insert_resource(WinitSettings::desktop_app())
        .add_systems(Startup, setup)
        .add_systems(Update, button_system)
        .run();
}

fn normal_button() -> UiColor {
    Color::hex("0E1012").unwrap().into()
}

fn linear() -> UiColor {
    LinearGradient {
        angle: deg(311.35),
        stops: vec![
            (
                Color::rgb_u8(156, 165, 174).with_a(0.2),
                Val::Percent(14.06),
            )
                .into(),
            (Color::rgb_u8(21, 23, 25).with_a(0.2), Val::Percent(33.95)).into(),
            (Color::rgb_u8(51, 56, 62).with_a(0.2), Val::Percent(95.24)).into(),
        ],
    }
    .into()
}

fn radial() -> UiColor {
    RadialGradient {
        center: (-Val::Percent(36.85), -Val::Percent(75.74)).into(),
        shape: RadialGradientShape::Ellipse(
            Val::Percent(228.15).into(),
            Val::Percent(175.74).into(),
        ),
        stops: vec![
            (Color::hex("BFA6FB").unwrap(), Val::Percent(24.48)).into(),
            (Color::hex("AC158B").unwrap(), Val::Percent(53.12)).into(),
            (Color::rgb_u8(209, 124, 42).with_a(0.69), Val::Percent(90.6)).into(),
            (Color::hex("EBE1D4").unwrap(), Val::Percent(100.)).into(),
        ],
    }
    .into()
}

fn button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text>,
) {
    for (interaction, mut color, mut border_color, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                text.sections[0].value = "Press".to_string();
                *color = linear().into();
                border_color.0 = radial();
            }
            Interaction::Hovered => {
                text.sections[0].value = "Hover".to_string();
                *color = linear().into();
                border_color.0 = radial();
            }
            Interaction::None => {
                text.sections[0].value = "Button".to_string();
                *color = normal_button().into();
                border_color.0 = Color::BLACK.into();
            }
        }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // ui camera
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        border_radius: BorderRadius::all(Val::Px(25.0)),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    border_color: radial().into(),
                    background_color: normal_button().into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        "Button",
                        TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 40.0,
                            color: Color::rgb(0.9, 0.9, 0.9),
                        },
                    ));
                });
        });
}

//! This example illustrates how to create UI text and update it in a system.
//!
//! It displays the current FPS in the top left corner, as well as text that changes color
//! in the bottom right. For text within a scene, please see the text2d example.
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_user_input)
        .run();
}

#[derive(Component)]
struct RootNode;

#[derive(Component)]
struct DescriptionText;

#[derive(Resource)]
struct OverflowSettings(OverflowAxis);

fn setup(mut commands: Commands) {
    // Camera
    commands.spawn(Camera2d);

    let overflow_settings = OverflowSettings(OverflowAxis::Clip);

    commands
        .spawn((
            RootNode,
            Node {
                left: Val::Px(100.0),
                top: Val::Px(100.0),
                width: Val::Px(100.0),
                height: Val::Px(50.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.8, 0.9, 0.6)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Hello World"),
                TextFont::from_font_size(12.0),
                TextColor(Color::srgb(0.4, 0.4, 0.4)),
                TextLayout::new_with_justify(Justify::Center),
            ));
        });

    commands.spawn((
        DescriptionText,
        Text::new(description_node_text(&overflow_settings, 1.0)),
        TextFont::from_font_size(16.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
    ));
    commands.insert_resource(overflow_settings);
}

fn handle_user_input(
    keys: Res<ButtonInput<KeyCode>>,
    root_node_q: Single<(&mut UiTransform, &mut Node), With<RootNode>>,
    // inner_node_q: Single<&mut Node, (With<InnerNode>, Without<RootNode>)>,
    mut overflow_settings: ResMut<OverflowSettings>,
    mut description_text_q: Single<&mut Text, With<DescriptionText>>,
) {
    let (mut root_node_transform, mut root_node) = root_node_q.into_inner();
    // let mut inner_node = inner_node_q.into_inner();

    if keys.just_pressed(KeyCode::KeyA) {
        root_node_transform.scale += Vec2::splat(0.125);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        root_node_transform.scale =
            (root_node_transform.scale - Vec2::splat(0.125)).max(Vec2::splat(0.125));
    }
    if keys.just_pressed(KeyCode::KeyR) {
        overflow_settings.0 = next_overflow_axis(overflow_settings.0);
        root_node.overflow = Overflow {
            x: overflow_settings.0,
            y: overflow_settings.0,
        };
    }
    let mut description_text = description_text_q.into_inner();
    *description_text = Text::new(description_node_text(
        &overflow_settings,
        root_node_transform.scale.x,
    ));
}

fn description_node_text(overflow_settings: &OverflowSettings, scale: f32) -> String {
    format!(
        "Press A/Z to scale the root UI node.               \n\
        R to change root node overflow.\n\
        Current overflow: {:?}.\n\
        Current Scale: {:.3}\n\
        It gets really weird at 1.5x and above.",
        overflow_settings.0, scale,
    )
}

fn next_overflow_axis(current: OverflowAxis) -> OverflowAxis {
    match current {
        OverflowAxis::Clip => OverflowAxis::Hidden,
        OverflowAxis::Hidden => OverflowAxis::Scroll,
        OverflowAxis::Scroll => OverflowAxis::Visible,
        OverflowAxis::Visible => OverflowAxis::Clip,
    }
}

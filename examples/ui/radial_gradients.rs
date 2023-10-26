//! Example demonstrating gradients

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn inner(
    commands: &mut ChildBuilder,
    c: RectPosition,
    stops: &Vec<ColorStop>,
) {
    for s in [
        RadialGradientShape::CircleRadius(Val::Percent(25.)),
        RadialGradientShape::CircleRadius(Val::Percent(40.)),
        RadialGradientShape::CircleSized(RadialGradientSize::ClosestCorner),
        RadialGradientShape::CircleSized(RadialGradientSize::ClosestSide),
        RadialGradientShape::CircleSized(RadialGradientSize::FarthestCorner),
        RadialGradientShape::CircleSized(RadialGradientSize::FarthestSide),
        RadialGradientShape::Ellipse(Val::Percent(40.), Val::Percent(20.)),
        RadialGradientShape::Ellipse(Val::Percent(20.), Val::Percent(40.)),
        RadialGradientShape::EllipseSized(RadialGradientSize::ClosestCorner),
        RadialGradientShape::EllipseSized(RadialGradientSize::ClosestSide),
        RadialGradientShape::EllipseSized(RadialGradientSize::FarthestCorner),
        RadialGradientShape::EllipseSized(RadialGradientSize::FarthestSide),
    ] {
        commands.spawn(NodeBundle {
            style: Style {
                width: Val::Px(50.),
                height: Val::Px(50.),
                ..Default::default()
            },
            background_color: RadialGradient::new(
                c,
                s,
                stops.clone(),
            )
            .into(),
            ..Default::default()
        });
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    let root = commands
        .spawn(NodeBundle {
            style: Style {
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                row_gap: Val::Px(20.),
                column_gap: Val::Px(10.),
                ..Default::default()
            },
            ..Default::default()
        })
        .id();

    let group = spawn_group(&mut commands);

    commands.entity(root).add_child(group);
}

fn spawn_group(commands: &mut Commands) -> Entity {
    commands
        .spawn(NodeBundle {
            style: Style {
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Start,
                row_gap: Val::Px(10.),
                column_gap: Val::Px(10.),
                ..Default::default()
            },
            ..Default::default()
        })
        .with_children(|commands| {
            commands
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.),
                        column_gap: Val::Px(10.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .with_children(|commands| {
                   
                    for c in [
                        RectPosition::CENTER,
                        RectPosition::new(RectPositionAxis::CENTER, RectPositionAxis::Start(Val::Percent(25.))),
                        RectPosition::new(RectPositionAxis::CENTER, RectPositionAxis::End(Val::Percent(25.))),
                        RectPosition::new( RectPositionAxis::Start(Val::Percent(25.)), RectPositionAxis::CENTER),
                        RectPosition::new( RectPositionAxis::End(Val::Percent(25.)), RectPositionAxis::CENTER),
                        RectPosition::new(RectPositionAxis::Start(Val::Percent(25.)), RectPositionAxis::Start(Val::Percent(25.))),
                        RectPosition::new(RectPositionAxis::End(Val::Percent(25.)), RectPositionAxis::Start(Val::Percent(25.))),
                        RectPosition::new(RectPositionAxis::Start(Val::Percent(25.)), RectPositionAxis::End(Val::Percent(25.))),
                        RectPosition::new(RectPositionAxis::End(Val::Percent(25.)), RectPositionAxis::End(Val::Percent(25.))),
                    ] {
                        for stops in [
                            vec![(Color::WHITE, Val::Auto).into(), (Color::BLACK, Val::Auto).into()],
                            vec![
                                (Color::RED, Val::Percent(10.)).into(),
                                (Color::GREEN, Val::Percent(20.)).into(),
                                (Color::GREEN, Val::Percent(30.)).into(),
                                (Color::BLUE, Val::Percent(30.)).into(),
                                (Color::BLUE, Val::Percent(40.)).into(),
                                (Color::YELLOW, Val::Auto).into(),
                            ],
                        ] {
                            commands
                                .spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Row,
                                        row_gap: Val::Px(10.),
                                        column_gap: Val::Px(10.),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .with_children(|commands| {
                                    inner(commands, c, &stops);
                                }); 
                        }
                    }
                });
            }).id()
    
}


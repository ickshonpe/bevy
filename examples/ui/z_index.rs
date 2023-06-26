//! Demonstrates how to use the z-index component on UI nodes to control their relative depth
//!
//! It uses colored boxes with different z-index values to demonstrate how it can affect the order of
//! depth of nodes compared to their siblings, but also compared to the entire UI.

use bevy::prelude::*;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // spawn the container with default z-index.
    // the default z-index value is `ZIndex::Local(0)`.
    // because this is a root UI node, using local or global values will do the same thing.
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(180.0),
                        height: Val::Px(100.0),
                        ..default()
                    },
                    ..Default::default()
                })
                .with_children(|parent| {
                    // The purple node is the first child, but it has the greatest `ZIndex` of the nodes in this stacking context.
                    // It is rendered last, showing on top of the other colored UI nodes.
                    parent.spawn(NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(15.0),
                            bottom: Val::Px(10.0),
                            width: Val::Px(100.),
                            height: Val::Px(60.),
                            ..Default::default()
                        },
                        z_index: ZIndex(1),
                        background_color: Color::PURPLE.into(),
                        ..default()
                    });

                    // The grey node is given the default `ZIndex` which is equal to `0`.
                    // The children of the grey node will be in a new stacking context.
                    parent
                        .spawn(NodeBundle {
                            background_color: Color::GRAY.into(),
                            style: Style {
                                position_type: PositionType::Absolute,
                                width: Val::Px(180.0),
                                height: Val::Px(100.0),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|grey_parent| {
                            // Spawn a red node with default z-index.
                            grey_parent.spawn(NodeBundle {
                                background_color: Color::RED.into(),
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(10.0),
                                    bottom: Val::Px(40.0),
                                    width: Val::Px(100.0),
                                    height: Val::Px(50.0),
                                    ..default()
                                },
                                ..default()
                            });

                            // Spawn a blue  node with a positive local z-index of 2.
                            // it will show above other nodes in the gray container.
                            // Because this node is in a new stacking context, even though it has a `ZIndex` of 2
                            // it will be shown behind the purple node as the the purple node has a higher `ZIndex` in
                            // the stacking context containing the purple node and this node's parent.
                            grey_parent.spawn(NodeBundle {
                                z_index: ZIndex(2),
                                background_color: Color::BLUE.into(),
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(45.0),
                                    bottom: Val::Px(30.0),
                                    width: Val::Px(100.),
                                    height: Val::Px(50.),
                                    ..default()
                                },
                                ..default()
                            });

                            // Spawn a geen node with a negative local z-index.
                            // It will show under other nodes in the gray container.
                            // Because this node is in a new stacking context, even though it has a `ZIndex` of -2
                            // it will be shown in front of the yellow node as the the yellow node has a lower `ZIndex` in
                            // the stacking context containing the yellow node and this node's parent.
                            grey_parent.spawn(NodeBundle {
                                z_index: ZIndex(-2),
                                background_color: Color::GREEN.into(),
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(70.0),
                                    bottom: Val::Px(20.0),
                                    width: Val::Px(100.),
                                    height: Val::Px(75.),
                                    ..default()
                                },
                                ..default()
                            });
                        });

                    // The yellow node is the last child but it has the lowest `ZIndex` in the local stacking context.
                    // It is rendered first and is shown behind the other colored UI nodes.
                    parent.spawn(NodeBundle {
                        z_index: ZIndex(-1),
                        background_color: Color::YELLOW.into(),
                        style: Style {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-15.0),
                            bottom: Val::Px(-15.0),
                            width: Val::Px(100.),
                            height: Val::Px(125.),
                            ..default()
                        },
                        ..default()
                    });
                });
        });
}

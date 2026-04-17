use crate::{
    experimental::UiChildren,
    prelude::{Button, Label},
    ui_transform::UiGlobalTransform,
    widget::{ImageNode, TextUiReader},
    ComputedNode, UiSystems,
};
use bevy_a11y::{AccessibilityNode, AccessibilitySystems};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::{
    hierarchy::ChildOf,
    prelude::Entity,
    query::{Changed, With, Without},
    schedule::IntoScheduleConfigs,
    system::{Commands, Query},
};
use bevy_math::Affine2;

use accesskit::{Affine, Node, Rect, Role};

fn calc_label(
    text_reader: &mut TextUiReader,
    children: impl Iterator<Item = Entity>,
) -> Option<Box<str>> {
    let mut name = None;
    for child in children {
        let values = text_reader
            .iter(child)
            .map(|(_, _, text, _, _, _, _)| text.into())
            .collect::<Vec<String>>();
        if !values.is_empty() {
            name = Some(values.join(" "));
        }
    }
    name.map(String::into_boxed_str)
}

fn sync_bounds_and_transforms(
    mut accessible_nodes_query: Query<(
        &mut AccessibilityNode,
        &ComputedNode,
        &UiGlobalTransform,
        &ChildOf,
    )>,
    accessible_transform_query: Query<&UiGlobalTransform, With<AccessibilityNode>>,
) {
    for (mut accessible, node, ui_transform, child_of) in &mut accessible_nodes_query {
        accessible.set_bounds(Rect::new(
            -0.5 * node.size.x as f64,
            -0.5 * node.size.y as f64,
            0.5 * node.size.x as f64,
            0.5 * node.size.y as f64,
        ));

        // If the node has an accessible parent, its transform in the accessiblity tree needs to be relative to the parent.
        let transform = accessible_transform_query
            .get(child_of.parent())
            .ok()
            .and_then(UiGlobalTransform::try_inverse)
            .map_or(ui_transform.affine(), |inverse| {
                inverse * ui_transform.affine()
            });

        if transform.is_finite() && transform != Affine2::IDENTITY {
            accessible.set_transform(Affine::new(transform.to_cols_array().map(f64::from)));
        } else {
            accessible.clear_transform();
        }
    }
}

fn button_changed(
    mut commands: Commands,
    mut query: Query<(Entity, Option<&mut AccessibilityNode>), Changed<Button>>,
    ui_children: UiChildren,
    mut text_reader: TextUiReader,
) {
    for (entity, accessible) in &mut query {
        let label = calc_label(&mut text_reader, ui_children.iter_ui_children(entity));
        if let Some(mut accessible) = accessible {
            accessible.set_role(Role::Button);
            if let Some(name) = label {
                accessible.set_label(name);
            } else {
                accessible.clear_label();
            }
        } else {
            let mut node = Node::new(Role::Button);
            if let Some(label) = label {
                node.set_label(label);
            }
            commands
                .entity(entity)
                .try_insert(AccessibilityNode::from(node));
        }
    }
}

fn image_changed(
    mut commands: Commands,
    mut query: Query<
        (Entity, Option<&mut AccessibilityNode>),
        (Changed<ImageNode>, Without<Button>),
    >,
    ui_children: UiChildren,
    mut text_reader: TextUiReader,
) {
    for (entity, accessible) in &mut query {
        let label = calc_label(&mut text_reader, ui_children.iter_ui_children(entity));
        if let Some(mut accessible) = accessible {
            accessible.set_role(Role::Image);
            if let Some(label) = label {
                accessible.set_label(label);
            } else {
                accessible.clear_label();
            }
        } else {
            let mut node = Node::new(Role::Image);
            if let Some(label) = label {
                node.set_label(label);
            }
            commands
                .entity(entity)
                .try_insert(AccessibilityNode::from(node));
        }
    }
}

fn label_changed(
    mut commands: Commands,
    mut query: Query<(Entity, Option<&mut AccessibilityNode>), Changed<Label>>,
    mut text_reader: TextUiReader,
) {
    for (entity, accessible) in &mut query {
        let values = text_reader
            .iter(entity)
            .map(|(_, _, text, _, _, _, _)| text.into())
            .collect::<Vec<String>>();
        let label = Some(values.join(" ").into_boxed_str());
        if let Some(mut accessible) = accessible {
            accessible.set_role(Role::Label);
            if let Some(label) = label {
                accessible.set_value(label);
            } else {
                accessible.clear_value();
            }
        } else {
            let mut node = Node::new(Role::Label);
            if let Some(label) = label {
                node.set_value(label);
            }
            commands
                .entity(entity)
                .try_insert(AccessibilityNode::from(node));
        }
    }
}

/// `AccessKit` integration for `bevy_ui`.
pub(crate) struct AccessibilityPlugin;

impl Plugin for AccessibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                button_changed,
                image_changed,
                label_changed,
                sync_bounds_and_transforms
                    .after(button_changed)
                    .after(image_changed)
                    .after(label_changed)
                    // the listed systems do not affect calculated size
                    .ambiguous_with(crate::ui_stack_system),
            )
                .in_set(UiSystems::PostLayout)
                .before(AccessibilitySystems::Update),
        );
    }
}

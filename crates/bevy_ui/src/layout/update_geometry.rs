use crate::NodePosition;
use crate::NodeSize;
use crate::UiContext;
use crate::UiNode;
use crate::helpers::*;
use crate::layout_tree::UiNodeLayouts;
use bevy_ecs::prelude::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_hierarchy::Children;
use bevy_math::Vec2;

pub fn update_ui_node_geometry_recursively(
    layouts: Res<UiNodeLayouts>,
    ui_context: ResMut<UiContext>,
    mut node_geometry_query: Query<(&UiNode, &mut NodeSize, &mut NodePosition)>,
    root_node_query: Query<
        Entity,
        (
            With<UiNode>,            
            With<NodeSize>,
            With<NodePosition>,
        ),
    >,
    children_query: Query<
        &Children,
        (
            With<UiNode>,
            With<NodeSize>,
            With<NodePosition>,
        ),
    >,
) {
    let Some(physical_to_logical_factor) = ui_context
        .0
        .as_ref()
        .map(|context|  context.physical_to_logical_factor)
    else {
        return;
    };

    fn update_node_geometry_recursively(
        layouts: &UiNodeLayouts,
        inherited_position: Vec2,
        entity: Entity,
        node_geometry_query: &mut Query<(&UiNode, &mut NodeSize, &mut NodePosition)>,
        children_query: &Query<
            &Children,
            (
                With<UiNode>,
                With<NodeSize>,
                With<NodePosition>,
            ),
        >,
        physical_to_logical_factor: f32,
    ) {
        if let Ok((node, mut node_size, mut node_position)) = node_geometry_query.get_mut(entity)
        {
            let layout = layouts[node.key];
            let new_size = Vec2::convert_from(layout.size) * physical_to_logical_factor;
            let local_position = Vec2::convert_from(layout.location) * physical_to_logical_factor;
            let new_position = local_position + inherited_position;

            // only trigger change detection when the new value is different
            if node_size.calculated_size != new_size {
                node_size.calculated_size = new_size;
            }

            if node_position.0 != new_position {
                node_position.0 = new_position;
            }

            if let Ok(children) = children_query.get(entity) {
                for child in children.iter() {
                    update_node_geometry_recursively(
                        layouts,
                        new_position,
                        *child,
                        node_geometry_query,
                        children_query,
                        physical_to_logical_factor,
                    );
                }
            }
        }
    }

    for root_entity in root_node_query.iter() {
        update_node_geometry_recursively(
            &layouts,
            Vec2::ZERO,
            root_entity,
            &mut node_geometry_query,
            &children_query,
            physical_to_logical_factor as f32,
        );
    }
}

pub fn update_ui_node_geometry_with_stack(
    ui_context: ResMut<UiContext>,
    layouts: Res<UiNodeLayouts>,
    root_node_query: Query<Entity, (With<UiNode>, With<NodeSize>, With<NodePosition>)>,
    mut node_geometry_query: Query<(&UiNode, &mut NodeSize, &mut NodePosition)>,
    children_query: Query<
        &Children,
        (
            With<UiNode>,
            With<NodeSize>,
            With<NodePosition>,
        ),
    >,
) {
    let Some(physical_to_logical_factor) = ui_context
    .0
    .as_ref()
    .map(|context|  context.physical_to_logical_factor as f32)
    else {
        return;
    };

    for root_entity in root_node_query.iter() {
        let mut stack = vec![(root_entity, Vec2::ZERO)];

        while let Some((entity, inherited_position)) = stack.pop() {
            if let Ok((node, mut node_size, mut node_position)) = node_geometry_query.get_mut(entity) {
                let layout = layouts[node.key];
                let new_size = Vec2::convert_from(layout.size) * physical_to_logical_factor;
                let local_position = Vec2::convert_from(layout.location) * physical_to_logical_factor;
                let new_position = local_position + inherited_position;

                // only trigger change detection when the new value is different
                if node_size.calculated_size != new_size {
                    node_size.calculated_size = new_size;
                }

                if node_position.0 != new_position {
                    node_position.0 = new_position;
                }

                if let Ok(children) = children_query.get(entity) {
                    for child in children.iter() {
                        stack.push((*child, new_position));
                    }
                }
            }
        }
    }
}
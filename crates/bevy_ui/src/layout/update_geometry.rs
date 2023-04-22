use crate::NodePosition;
use crate::NodeSize;
use crate::UiContext;
use crate::UiNode;
use crate::UiNodeLayout;
use bevy_ecs::prelude::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_hierarchy::Children;
use bevy_math::Vec2;

pub fn update_ui_node_geometry(
    ui_context: ResMut<UiContext>,
    mut node_geometry_query: Query<(&UiNodeLayout, &mut NodeSize, &mut NodePosition)>,
    root_node_query: Query<
        Entity,
        (
            With<UiNode>,
            With<UiNodeLayout>,
            With<NodeSize>,
            With<NodePosition>,
        ),
    >,
    children_query: Query<
        &Children,
        (
            With<UiNode>,
            With<UiNodeLayout>,
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
        inherited_position: Vec2,
        entity: Entity,
        node_geometry_query: &mut Query<(&UiNodeLayout, &mut NodeSize, &mut NodePosition)>,
        children_query: &Query<
            &Children,
            (
                With<UiNode>,
                With<UiNodeLayout>,
                With<NodeSize>,
                With<NodePosition>,
            ),
        >,
        physical_to_logical_factor: f32,
    ) {
        if let Ok((layout, mut node_size, mut node_position)) = node_geometry_query.get_mut(entity)
        {
            let new_size = layout.size() * physical_to_logical_factor;
            let local_position = layout.local_position() * physical_to_logical_factor;
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
            Vec2::ZERO,
            root_entity,
            &mut node_geometry_query,
            &children_query,
            physical_to_logical_factor as f32,
        );
    }
}

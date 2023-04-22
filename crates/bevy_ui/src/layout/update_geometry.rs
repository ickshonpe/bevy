use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_math::Vec2;
use bevy_transform::prelude::Transform;
use taffy::tree::LayoutTree;

use crate::layout_tree::UiLayoutTree;
use crate::Node;
use crate::NodeSize;
use crate::UiContext;

pub fn update_ui_node_geometry(
    ui_surface: UiLayoutTree,
    ui_context: ResMut<UiContext>,
    mut node_transform_query: Query<(&Node, &mut NodeSize, &mut Transform)>,
) {
    let Some(physical_to_logical_factor) = ui_context
        .0
        .as_ref()
        .map(|context|  context.physical_to_logical_factor)
    else {
        return;
    };

    let to_logical = |v| (physical_to_logical_factor * v as f64) as f32;

    // PERF: try doing this incrementally
    node_transform_query
        .par_iter_mut()
        .for_each_mut(|(node, mut node_size, mut transform)| {
            let layout = ui_surface.layout(node.key);
            let new_size = Vec2::new(
                to_logical(layout.size.width),
                to_logical(layout.size.height),
            );
            // only trigger change detection when the new value is different

            if node_size.calculated_size != new_size {
                node_size.calculated_size = new_size;
            }

            let mut new_position = transform.translation;
            new_position.x = to_logical(layout.location.x + layout.size.width / 2.0);
            new_position.y = to_logical(layout.location.y + layout.size.height / 2.0);

            let parent_key = ui_surface.parent(node.key).unwrap();
            if parent_key != **ui_surface.window_node {
                let parent_layout = ui_surface.layout(parent_key);
                new_position.x -= to_logical(parent_layout.size.width / 2.0);
                new_position.y -= to_logical(parent_layout.size.height / 2.0);
            }

            // only trigger change detection when the new value is different
            if transform.translation != new_position {
                transform.translation = new_position;
            }
        });
}

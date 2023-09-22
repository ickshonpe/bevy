//! This module contains the systems that update the stored UI nodes stack

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;

use crate::{ComputedLayout, ZIndex};

/// The current UI stack, which contains all UI nodes ordered by their depth (back-to-front).
///
/// The first entry is the furthest node from the camera and is the first one to get rendered
/// while the last entry is the first node to receive interactions.
#[derive(Debug, Resource, Default)]
pub struct UiStack {
    /// List of UI nodes ordered from back-to-front
    pub uinodes: Vec<Entity>,
}

/// Generates the render stack for UI nodes.
pub fn ui_stack_system(
    mut ui_stack: ResMut<UiStack>,
    root_node_query: Query<Entity, (With<ComputedLayout>, Without<Parent>)>,
    mut node_query: Query<(&mut ComputedLayout, Option<&Children>)>,
    zindex_query: Query<&ZIndex>,
) {
    ui_stack.uinodes.clear();
    let uinodes = &mut ui_stack.uinodes;

    fn update_uistack_recursively(
        entity: Entity,
        uinodes: &mut Vec<Entity>,
        node_query: &mut Query<(&mut ComputedLayout, Option<&Children>)>,
        zindex_query: &Query<&ZIndex>,
    ) {
        let Ok((mut computed_layout, children)) = node_query.get_mut(entity) else {
            return;
        };

        computed_layout.stack_index = uinodes.len() as u32;
        uinodes.push(entity);

        if let Some(children) = children {
            let mut z_children: Vec<(Entity, i32)> = children
                .iter()
                .map(|&child_id| {
                    (
                        child_id,
                        match zindex_query.get(child_id) {
                            Ok(ZIndex(z)) => *z,
                            _ => 0,
                        },
                    )
                })
                .collect();
            z_children.sort_by_key(|k| k.1);
            for (child_id, _) in z_children {
                update_uistack_recursively(child_id, uinodes, node_query, zindex_query);
            }
        }
    }

    let mut root_nodes: Vec<_> = root_node_query.iter().collect();
    root_nodes.sort_by_cached_key(|entity| {
        zindex_query
            .get(*entity)
            .map(|zindex| zindex.0)
            .unwrap_or(0)
    });

    for entity in root_nodes {
        update_uistack_recursively(entity, uinodes, &mut node_query, &zindex_query);
    }
}

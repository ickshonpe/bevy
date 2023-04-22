mod algorithm;
mod convert;
mod data;
pub mod helpers;
pub mod layout_tree;
pub mod update_geometry;

use crate::{ContentSize, Style, UiScale};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    prelude::Component,
    query::{Added, Changed, With, Without},
    removal_detection::RemovedComponents,
    system::{Query, Res, ResMut, Resource},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_math::Vec2;
use bevy_reflect::Reflect;
use bevy_utils::HashMap;
use bevy_window::{PrimaryWindow, Window, WindowScaleFactorChanged};

use self::{layout_tree::UiLayoutTree};

#[derive(Component, Debug, Reflect)]
pub struct UiNode {
    #[reflect(ignore)]
    key: taffy::node::Node,
}

impl Default for UiNode {
    fn default() -> Self {
        Self {
            key: Default::default(),
        }
    }
}

#[derive(Resource, Default)]
pub struct UiContext(pub Option<LayoutContext>);

pub struct LayoutContext {
    pub window_entity: Entity,
    pub require_full_update: bool,
    pub scale_factor: f64,
    pub logical_size: Vec2,
    pub physical_size: Vec2,
    pub physical_to_logical_factor: f64,
    pub min_size: f32,
    pub max_size: f32,
}

impl LayoutContext {
    /// create new a [`LayoutContext`] from the window's physical size and scale factor
    fn new(window_entity: Entity, scale_factor: f64, physical_size: Vec2, require_full_update: bool) -> Self {
        let physical_to_logical_factor = 1. / scale_factor;
        Self {
            window_entity,
            require_full_update,
            scale_factor,
            logical_size: physical_size * physical_to_logical_factor as f32,
            physical_size,
            min_size: physical_size.x.min(physical_size.y),
            max_size: physical_size.x.max(physical_size.y),
            physical_to_logical_factor,
        }
    }
}

fn _assert_send_sync_ui_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

/// Remove the corresponding taffy node for any entity that has its `Node` component removed.
pub fn clean_up_removed_ui_nodes(
    mut ui_surface: UiLayoutTree,
    mut removed_nodes: RemovedComponents<UiNode>,
) {
    // clean up removed nodes
    for entity in removed_nodes.iter() {
        if let Some(node) = ui_surface.entity_to_node.remove(&entity) {
            let _ = ui_surface.node_to_entity.remove(node);
            let _ = ui_surface.remove(node);
        }
    }
}

pub fn update_measure_tracking(
    mut ui_surface: UiLayoutTree,
    mut removed_content_sizes: RemovedComponents<ContentSize>,
    added_content_size_query: Query<&UiNode, Added<ContentSize>>,
) {
    for entity in removed_content_sizes.iter() {
        if let Some(key) = ui_surface.entity_to_node.get(&entity) {
            if let Some(data) = ui_surface.nodes.get_mut(*key) {
                data.needs_measure = false;
            }
        }
    }

    for node in added_content_size_query.iter() {
        ui_surface.nodes.get_mut(node.key).unwrap().needs_measure = true;
    }
}

/// Insert a new taffy node into the layout for any entity that had a `Node` component added.
pub fn insert_new_ui_nodes(
    mut ui_surface: UiLayoutTree,
    mut new_node_query: Query<(Entity, &mut UiNode), Added<UiNode>>,
) {
    for (entity, mut node) in new_node_query.iter_mut() {
        node.key = ui_surface.new_leaf(entity, taffy::style::Style::DEFAULT).unwrap();        
        if let Some(old_key) = ui_surface.entity_to_node.insert(entity, node.key) {
            let _ = ui_surface.remove(old_key);
        }
    }
}

/// Synchonise the Bevy and Taffy Parent-Children trees
pub fn synchonise_ui_children(
    mut flex_surface: UiLayoutTree,
    mut removed_children: RemovedComponents<Children>,
    children_query: Query<(&UiNode, &Children), Changed<Children>>,
) {
    // Iterate through all entities with a removed `Children` component and if they have a corresponding Taffy node, remove their children from the Taffy tree.
    for entity in removed_children.iter() {
        flex_surface.try_remove_children(entity);
    }

    // Update the corresponding Taffy children of Bevy entities with changed `Children`
    for (node, children) in &children_query {
        flex_surface.update_children(node.key, children);
    }
}

pub fn update_ui_windows(
    mut resize_events: EventReader<bevy_window::WindowResized>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut ui_context: ResMut<UiContext>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
) {
    // assume one window for time being...
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let (primary_window_entity, logical_to_physical_factor, physical_size) =
        if let Ok((entity, primary_window)) = primary_window.get_single() {
            (
                entity,
                primary_window.resolution.scale_factor(),
                Vec2::new(
                    primary_window.resolution.physical_width() as f32,
                    primary_window.resolution.physical_height() as f32,
                ),
            )
        } else {
            ui_context.0 = None;
            return;
        };

    let require_full_update = ui_context.0.is_none()
        || resize_events
            .iter()
            .any(|resized_window| resized_window.window == primary_window_entity)
        || !scale_factor_events.is_empty()
        || ui_scale.is_changed();
    scale_factor_events.clear();

    let scale_factor = logical_to_physical_factor * ui_scale.scale;
    let context = LayoutContext::new(primary_window_entity, scale_factor, physical_size, require_full_update);
    ui_context.0 = Some(context);
}

#[allow(clippy::too_many_arguments)]
pub fn update_ui_layout(
    ui_context: ResMut<UiContext>,
    mut layout_tree: UiLayoutTree,
    root_node_query: Query<&UiNode, (With<Style>, Without<Parent>)>,
    style_query: Query<(&UiNode, Ref<Style>)>,
    full_style_query: Query<(&UiNode, &Style)>,
) {
    let Some(ref layout_context) = ui_context.0 else {
        return
    };

    if layout_context.require_full_update {
        for (node, style) in full_style_query.iter() {
            layout_tree.update_node(node.key, style, layout_context);
        }
    } else {
        for (node, style) in style_query.iter() {
            if style.is_changed() {
                layout_tree.update_node(node.key, &style, layout_context);
            }
        }
    }

    // update window root nodes
    layout_tree.update_window(layout_context.window_entity, layout_context.physical_size);

    // update window children (for now assuming all Nodes live in the primary window)
    layout_tree.set_window_children(root_node_query.iter().map(|node| node.key));

    // compute layouts
    layout_tree.compute_window_layout();
}

#[cfg(test)]
mod tests {
    use crate::NodePosition;
    use crate::clean_up_removed_ui_nodes;
    use crate::insert_new_ui_nodes;
    use crate::synchonise_ui_children;
    use crate::update_ui_layout;
    use crate::AlignItems;
    use crate::LayoutContext;
    use crate::NodeSize;
    use crate::Style;
    use crate::UiContext;
    use crate::UiNode;
    use bevy_ecs::prelude::*;
    use bevy_ecs::system::SystemState;
    use bevy_math::Vec2;
    use taffy::tree::LayoutTree;

    fn node_bundle() -> (UiNode, NodeSize, NodePosition, Style) {
        (UiNode::default(), NodeSize::default(), NodePosition::default(), Style::default())
    }

    fn ui_schedule() -> Schedule {
        let mut ui_schedule = Schedule::default();
        ui_schedule.add_systems((
            clean_up_removed_ui_nodes.before(insert_new_ui_nodes),
            insert_new_ui_nodes.before(synchonise_ui_children),
            synchonise_ui_children.before(update_ui_layout),
            update_ui_layout,
        ));
        ui_schedule
    }

    use super::layout_tree::*;
    fn init_ui_layout_resources(world: &mut World) {
        world.init_resource::<UiEntityToNodeMap>();
        world.init_resource::<UiNodeToEntityMap>();
        world.init_resource::<UiNodes>();
        world.init_resource::<UiChildNodes>();
        world.init_resource::<UiParentNodes>();
        world.init_resource::<UiWindowNode>();
        world.init_resource::<UiWindowNode>();
        world.init_resource::<UiLayoutConfig>();
    }

    #[test]
    fn test_insert_and_remove_node() {
        let mut world = World::new();
        let window_entity = world.spawn_empty().id();
        world.insert_resource(UiContext(Some(LayoutContext::new(
            window_entity,
            3.0,
            Vec2::new(1000., 500.),
            true,
        ))));
        init_ui_layout_resources(&mut world);

        let mut ui_schedule = ui_schedule();

        // add ui node entity to world
        let entity = world.spawn(node_bundle()).id();

        // ui update
        ui_schedule.run(&mut world);

        let key = world.get::<UiNode>(entity).unwrap().key;

        let mut ui_layout_system_state = SystemState::<UiLayoutTree>::new(&mut world);
        let ui_layout = ui_layout_system_state.get_mut(&mut world);
        // ui node entity should be associated with a taffy node
        assert_eq!(ui_layout.entity_to_node[&entity], key);

        // taffy node should be a child of the window node
        assert_eq!(ui_layout.parent(key).unwrap(), **ui_layout.window_node);

        // despawn the ui node entity
        world.entity_mut(entity).despawn();

        ui_schedule.run(&mut world);

        let ui_layout = ui_layout_system_state.get_mut(&mut world);
        // the despawned entity's associated taffy node should also be removed
        assert!(!ui_layout.entity_to_node.contains_key(&entity));

        // window node should have no children
        assert!(ui_layout.children(**ui_layout.window_node).next().is_none());
    }

    #[test]
    fn test_node_style_update() {
        let mut world = World::new();
        init_ui_layout_resources(&mut world);
        let window_entity = world.spawn_empty().id();
        world.insert_resource(UiContext(Some(LayoutContext::new(
            window_entity,
            3.0,
            Vec2::new(1000., 500.),
            true,
        ))));
        let mut ui_schedule = ui_schedule();

        let mut ui_layout_system_state = SystemState::<UiLayoutTree>::new(&mut world);

        // add a ui node entity to the world and run the ui schedule to add a corresponding node to the taffy layout tree
        let entity = world.spawn(node_bundle()).id();
        ui_schedule.run(&mut world);
        // modify the ui node's style component and rerun the schedule
        world.get_mut::<Style>(entity).unwrap().align_items = AlignItems::Baseline;

        // don't want a full update
        world.insert_resource(UiContext(Some(LayoutContext::new(
            window_entity,
            3.0,
            Vec2::new(1000., 500.),
            false,
        ))));

        ui_schedule.run(&mut world);

        // check the corresponding taffy node's style is also updated
        let ui_layout = ui_layout_system_state.get_mut(&mut world);
        let key = ui_layout.entity_to_node[&entity];
        assert_eq!(
            ui_layout.style(key).align_items,
            Some(taffy::style::AlignItems::Baseline)
        );
    }
}

mod convert;
pub mod debug;

use crate::{ContentSize, Node, Style, UiPosition, UiScale, UiStacks};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    prelude::{Bundle, Component},
    query::{With, Without},
    reflect::ReflectComponent,
    removal_detection::RemovedComponents,
    system::{Query, Res, ResMut, Resource},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::warn;
use bevy_math::Vec2;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::camera::RenderTarget;
use bevy_utils::HashMap;
use bevy_window::Window;
use std::fmt;
use taffy::{prelude::Size, style_helpers::TaffyMaxContent, Taffy};

#[derive(Component, Default)]
pub struct UiLayoutViewportNodeId(taffy::node::Node);

#[derive(Component)]
pub struct UiTarget(pub Entity);

#[derive(Bundle)]
pub struct UiLayoutBundle {
    pub viewport_id: UiLayoutViewportNodeId,
    pub target: UiTarget,
    pub layout_context: LayoutContext,
}

/// Marks an entity as `UI root entity` with an associated root Taffy node and holds the resolution and scale factor information necessary to compute a UI layout.
#[derive(Component, Debug, Reflect, PartialEq)]
#[reflect(Component, Default)]
pub struct LayoutContext {
    /// The size of the root node in the layout tree.
    ///
    /// Should match the size of the output window in physical pixels of the display device.
    pub root_node_size: Vec2,
    /// [`Style`] properties of UI node entites with `Val::Px` values are multiplied by the `combined_scale_factor` before they are copied to the Taffy layout tree.
    ///
    /// `combined_scale_factor` is calculated by multiplying together the `scale_factor` of the output window and [`crate::UiScale`].
    pub combined_scale_factor: f64,
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self {
            root_node_size: Vec2::new(800., 600.),
            combined_scale_factor: 1.0,
        }
    }
}

pub struct UiLayout {}

#[derive(Resource)]
pub struct UiSurface {
    entity_to_taffy: HashMap<Entity, taffy::node::Node>,
    root_nodes: HashMap<Entity, taffy::node::Node>,
    taffy: Taffy,
}

fn _assert_send_sync_ui_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
    _assert_send_sync::<Taffy>();
    _assert_send_sync::<UiSurface>();
}

impl fmt::Debug for UiSurface {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UiSurface")
            .field("entity_to_taffy", &self.entity_to_taffy)
            .field("window_nodes", &self.root_nodes)
            .finish()
    }
}

impl Default for UiSurface {
    fn default() -> Self {
        Self {
            entity_to_taffy: Default::default(),
            root_nodes: Default::default(),
            taffy: Taffy::new(),
        }
    }
}

impl UiSurface {
    /// Retrieves the taffy node corresponding to given entity exists, or inserts a new taffy node into the layout if no corresponding node exists.
    /// Then convert the given [`Style`] and use it update the taffy node's style.
    pub fn upsert_node(&mut self, entity: Entity, style: &Style, context: &LayoutContext) {
        let mut added = false;
        let taffy = &mut self.taffy;
        let taffy_node = self.entity_to_taffy.entry(entity).or_insert_with(|| {
            added = true;
            taffy.new_leaf(convert::from_style(context, style)).unwrap()
        });

        if !added {
            self.taffy
                .set_style(*taffy_node, convert::from_style(context, style))
                .unwrap();
        }
    }

    /// Update the `MeasureFunc` of the taffy node corresponding to the given [`Entity`].
    pub fn update_measure(&mut self, entity: Entity, measure_func: taffy::node::MeasureFunc) {
        let taffy_node = self.entity_to_taffy.get(&entity).unwrap();
        self.taffy.set_measure(*taffy_node, Some(measure_func)).ok();
    }

    /// Update the children of the taffy node corresponding to the given [`Entity`].
    pub fn update_children(&mut self, entity: Entity, children: &Children) {
        let mut taffy_children = Vec::with_capacity(children.len());
        for child in children {
            if let Some(taffy_node) = self.entity_to_taffy.get(child) {
                taffy_children.push(*taffy_node);
            } else {
                warn!(
                    "Unstyled child in a UI entity hierarchy. You are using an entity \
without UI components as a child of an entity with UI components, results may be unexpected."
                );
            }
        }

        let taffy_node = self.entity_to_taffy.get(&entity).unwrap();
        self.taffy
            .set_children(*taffy_node, &taffy_children)
            .unwrap();
    }

    /// Removes children from the entity's taffy node if it exists. Does nothing otherwise.
    pub fn try_remove_children(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_children(*taffy_node, &[]).unwrap();
        }
    }

    /// Removes the measure from the entity's taffy node if it exists. Does nothing otherwise.
    pub fn try_remove_measure(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy.set_measure(*taffy_node, None).unwrap();
        }
    }

    /// Retrieve or insert the root layout node and update its size to match the size of the window.
    pub fn update_root_node(&mut self, taffy_node: taffy::node::Node, physical_size: Vec2) {
        self.taffy
            .set_style(
                taffy_node,
                taffy::style::Style {
                    size: taffy::geometry::Size {
                        width: taffy::style::Dimension::Points(physical_size.x),
                        height: taffy::style::Dimension::Points(physical_size.y),
                    },
                    ..Default::default()
                },
            )
            .unwrap();
    }

    /// Set the ui node entities without a [`Parent`] as children to the root node in the taffy layout.
    pub fn set_root_nodes_children(
        &mut self,
        parent_window: Entity,
        children: impl Iterator<Item = Entity>,
    ) {
        let taffy_node = self.root_nodes.get(&parent_window).unwrap();
        let child_nodes = children
            .map(|e| *self.entity_to_taffy.get(&e).unwrap())
            .collect::<Vec<taffy::node::Node>>();
        self.taffy.set_children(*taffy_node, &child_nodes).unwrap();
    }

    /// Compute the layout for each window entity's corresponding root node in the layout.
    pub fn compute_window_layouts(&mut self) {
        for window_node in self.root_nodes.values() {
            self.taffy
                .compute_layout(*window_node, Size::MAX_CONTENT)
                .unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from Taffy
    pub fn remove_entities(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            if let Some(node) = self.entity_to_taffy.remove(&entity) {
                self.taffy.remove(node).unwrap();
            }
        }
    }

    /// Get the layout geometry for the taffy node corresponding to the ui node [`Entity`].
    /// Does not compute the layout geometry, `compute_window_layouts` should be run before using this function.
    pub fn get_layout(&self, entity: Entity) -> Result<&taffy::layout::Layout, LayoutError> {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            self.taffy
                .layout(*taffy_node)
                .map_err(LayoutError::TaffyError)
        } else {
            warn!(
                "Styled child in a non-UI entity hierarchy. You are using an entity \
with UI components as a child of an entity without UI components, results may be unexpected."
            );
            Err(LayoutError::InvalidHierarchy)
        }
    }
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn ui_layout_system(
    ui_scale: Res<UiScale>,
    ui_stacks: ResMut<UiStacks>,
    mut removed_layouts: RemovedComponents<LayoutContext>,
    windows_query: Query<&Window>,
    mut layout_query: Query<(
        Entity,
        &mut LayoutContext,
        &UiTarget,
        &mut UiLayoutViewportNodeId,
    )>,
    mut ui_surface: ResMut<UiSurface>,
    style_query: Query<Ref<Style>, With<Node>>,
    mut measure_query: Query<(Entity, &mut ContentSize)>,
    ref_children_query: Query<(Entity, Ref<Children>), With<Node>>,
    children_query: Query<&Children>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_content_sizes: RemovedComponents<ContentSize>,
    mut node_geometry_query: Query<(&mut Node, &mut UiPosition)>,
    root_ui_nodes_query: Query<Entity, (With<Node>, With<UiPosition>, Without<Parent>)>,
    mut removed_nodes: RemovedComponents<Node>,
) {
    bevy_log::debug!("ui_layout_system");
    // If a UI root entity is deleted, its associated Taffy root node must also be deleted.
    for entity in removed_layouts.iter() {
        if let Some(taffy_node) = ui_surface.root_nodes.remove(&entity) {
            let _ = ui_surface.taffy.remove(taffy_node);
        }
    }

    for (_entity, mut layout_context, target, _id) in layout_query.iter_mut() {
        if let Ok(window) = windows_query.get(target.0) {
            let new_layout_context = LayoutContext {
                root_node_size: Vec2::new(
                    window.resolution.physical_width() as f32,
                    window.resolution.physical_height() as f32,
                ),
                combined_scale_factor: window.resolution.scale_factor() * ui_scale.scale,
            };
            if *layout_context != new_layout_context {
                *layout_context = new_layout_context;
            }
        }
    }

    for (ui_layout_entity, layout_context, _target, mut id) in layout_query.iter_mut() {
        if id.0 == taffy::node::Node::default() {
            id.0 = ui_surface
                .taffy
                .new_leaf(taffy::style::Style::default())
                .unwrap();
            ui_surface.root_nodes.insert(ui_layout_entity, id.0);
        }
        ui_surface.update_root_node(id.0, layout_context.root_node_size);

        if layout_context.is_changed() {
            // Update all nodes in stack for this context
            //
            // All nodes have to be updated on changes to the `LayoutContext` so any viewport values can be recalculated.
            for &ui_node in ui_stacks.view_to_stacks[&ui_layout_entity].uinodes.iter() {
                if let Ok(style) = style_query.get(ui_node) {
                    ui_surface.upsert_node(ui_node, &style, &layout_context);
                }
            }
        } else {
            for &ui_node in ui_stacks.view_to_stacks[&ui_layout_entity].uinodes.iter() {
                if let Ok(style) = style_query.get(ui_node) {
                    if style.is_changed() {
                        ui_surface.upsert_node(ui_node, &style, &layout_context);
                    }
                }
            }
        }
    }

    // Add new `MeasureFunc`s to the `Taffy` layout tree
    for (entity, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            // The `ContentSize` component only holds a `MeasureFunc` temporarily until it reaches here and is moved into the `Taffy` layout tree.
            ui_surface.update_measure(entity, measure_func);
        }
    }

    // Only entities with a `Node` component are considered UI node entities.
    // When a `Node` component of an entity is removed, the Taffy node associated with that entity must be deleted from the Taffy layout tree.
    ui_surface.remove_entities(removed_nodes.iter());

    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding Taffy node.
    for entity in removed_content_sizes.iter() {
        ui_surface.try_remove_measure(entity);
    }

    // Set the associated Taffy nodes of UI node entities without a `Parent` component to be children of the UI's root Taffy node
    for (ui_root_entity, ui_stack) in ui_stacks.view_to_stacks.iter() {
        bevy_log::debug!("set children for root node: {ui_root_entity:?}");
        ui_surface.set_root_nodes_children(*ui_root_entity, ui_stack.roots.iter().copied());
    }

    // Remove the associated Taffy children of entities which had their `Children` component removed since the last layout update
    //
    // This must be performed before `update_children` to account for cases where a `Children` component has been both removed and then reinserted between layout updates.
    for entity in removed_children.iter() {
        ui_surface.try_remove_children(entity);
    }

    // If the `Children` of a UI node entity have been changed since the last layout update, the children of the associated Taffy node must be updated.
    for (entity, children) in &ref_children_query {
        if children.is_changed() {
            ui_surface.update_children(entity, &children);
        }
    }

    // compute layouts
    ui_surface.compute_window_layouts();

    fn update_ui_nodes_recursively(
        ui_surface: &UiSurface,
        entity: Entity,
        inverse_combined_scale_factor: f32,
        ui_node_query: &mut Query<(&mut Node, &mut UiPosition)>,
        children_query: &Query<&Children>,
        inherited_position: Vec2,
    ) {
        let layout = ui_surface.get_layout(entity).unwrap();
        let new_size =
            Vec2::new(layout.size.width, layout.size.height) * inverse_combined_scale_factor;
        let local_position =
            Vec2::new(layout.location.x, layout.location.y) * inverse_combined_scale_factor;
        let next_position = local_position + inherited_position;
        let new_position = next_position + 0.5 * new_size;

        let (mut node, mut position) = ui_node_query.get_mut(entity).unwrap();
        if node.calculated_size != new_size {
            node.calculated_size = new_size;
        }

        if position.0 != new_position {
            position.0 = new_position;
        }

        if let Ok(children) = children_query.get(entity) {
            for &child_entity in children.iter() {
                update_ui_nodes_recursively(
                    ui_surface,
                    child_entity,
                    inverse_combined_scale_factor,
                    ui_node_query,
                    children_query,
                    next_position,
                );
            }
        }
    }

    for entity in root_ui_nodes_query.iter() {
        // let taffy_id = ui_surface.entity_to_taffy[&entity];
        // let taffy_parent_id = ui_surface.taffy.parent(taffy_id).unwrap();
        // let layout_size = ui_surface.taffy.layout(taffy_parent_id);
        let inverse_combined_scale_factor = 1.5f32.recip();
        update_ui_nodes_recursively(
            &ui_surface,
            entity,
            inverse_combined_scale_factor,
            &mut node_geometry_query,
            &children_query,
            Vec2::ZERO,
        );
    }

    //debug::print_ui_layout_tree(&ui_surface);
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use crate::ui_layout_system;
    use crate::LayoutContext;
    use crate::UiSurface;
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;
    use bevy_math::Vec2;
    use bevy_utils::prelude::default;
    use taffy::tree::LayoutTree;

    #[test]
    fn spawn_and_despawn_ui_node() {
        let mut world = World::new();
        world.init_resource::<UiSurface>();

        let layout_entity = world
            .spawn(LayoutContext {
                root_node_size: Vec2::new(800., 600.),
                ..default()
            })
            .id();

        let ui_node = world
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(25.),
                    ..default()
                },
                ..default()
            })
            .id();

        let mut ui_schedule = Schedule::default();
        ui_schedule.add_systems(ui_layout_system);
        ui_schedule.run(&mut world);

        let ui_surface = world.resource::<UiSurface>();

        // `layout_entity` should have an associated Taffy root node
        let taffy_root = *ui_surface
            .root_nodes
            .get(&layout_entity)
            .expect("Window node not found.");

        // `ui_node` should have an associated Taffy node
        let taffy_node = *ui_surface
            .entity_to_taffy
            .get(&ui_node)
            .expect("UI node entity should have an associated Taffy node after layout update");

        // `window_node` should be the only child of `taffy_root`
        assert_eq!(ui_surface.taffy.child_count(taffy_root).unwrap(), 1);
        assert!(
            ui_surface
                .taffy
                .children(taffy_root)
                .unwrap()
                .contains(&taffy_node),
            "Root UI Node entity's corresponding Taffy node is not a child of the root Taffy node."
        );

        // `taffy_root` should be the parent of `window_node`
        assert_eq!(
            ui_surface.taffy.parent(taffy_node),
            Some(taffy_root),
            "Root UI Node entity's corresponding Taffy node is not a child of the root Taffy node."
        );

        ui_schedule.run(&mut world);

        let derived_size = world.get::<Node>(ui_node).unwrap().calculated_size;
        approx::assert_relative_eq!(derived_size.x, 200.);
        approx::assert_relative_eq!(derived_size.y, 600.);

        world.despawn(ui_node);
        ui_schedule.run(&mut world);
        let ui_surface = world.resource::<UiSurface>();

        // `ui_node`'s associated taffy node should be deleted
        assert!(
            !ui_surface.entity_to_taffy.contains_key(&ui_node),
            "Despawned UI node has an associated Taffy node after layout update"
        );

        // `taffy_root` should have no remaining children
        assert_eq!(
            ui_surface.taffy.child_count(taffy_root).unwrap(),
            0,
            "Taffy root node has children after despawning all root UI nodes."
        );
    }
}

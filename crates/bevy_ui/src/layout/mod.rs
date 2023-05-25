mod convert;
pub mod debug_output;

use crate::{ContentSize, NodeOrder, NodePosition, NodeSize, Style, UiScale, ZIndex};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    prelude::Component,
    prelude::DetectChangesMut,
    query::Added,
    query::Changed,
    query::{With, Without},
    reflect::ReflectComponent,
    removal_detection::RemovedComponents,
    system::{Local, Query, Res, ResMut, Resource, SystemParam},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::{debug, trace, warn};
use bevy_math::{Affine2, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_utils::HashMap;
use bevy_window::{
    NormalizedWindowRef, PrimaryWindow, Window, WindowRef, WindowScaleFactorChanged,
};
use std::marker::PhantomData;
use taffy::{prelude::Size, style_helpers::TaffyMaxContent, Taffy};

/// The render target, Taffy root node, UI root node, and render order for a UI layout.
pub struct UiLayout {
    /// render target (window atm, might change to camera?)
    pub target: UiNormalizedLayoutTarget,
    /// Root node in taffy tree (could be an actual distinct taffy tree)
    /// This root taffy node is the size of the target viewport and has one child corresponding to `ui_root_node`.
    pub taffy_root_node: taffy::node::Node,
    /// root bevy ui node (has a `NodeKey` component corresponding to a taffy node that is a child of `taffy_root_node`).
    pub ui_root_node: Entity,
    /// Ui Layouts are sorted by order.
    /// Layouts with a lower order will be drawn behind layouts with a greater order.
    /// If layouts have the same order, the oldest UI root node Entity will drawn behind.
    pub order: i32,
}

impl std::fmt::Display for UiLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UiRoot [target: {:?} <- taffy_root: {:?} <- ui_root: {:?}]",
            self.target.entity(),
            self.taffy_root_node,
            self.ui_root_node,
        )
    }
}

/// List of the layout trees
#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiLayouts {
    /// List of ui layout trees sorted by their `order` field ascending.
    roots: Vec<UiLayout>,
}

/// Where/How a UI Layout should be rendered
#[derive(Component, Default, Deref, DerefMut)]
pub struct UiLayoutTarget {
    pub target: WindowRef,
}

/// Where/How a UI Layout should be rendered
#[derive(Component, Deref, DerefMut)]
pub struct UiNormalizedLayoutTarget {
    pub target: NormalizedWindowRef,
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiEntityToTaffyMap {
    entity_to_taffy: HashMap<Entity, taffy::node::Node>,
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiRootMap {
    entity_to_taffy_roots: HashMap<Entity, taffy::node::Node>,
}

impl UiRootMap {
    pub fn is_root_ui_node(&self, entity: Entity) -> bool {
        self.entity_to_taffy_roots.contains_key(&entity)
    }
}

/// Default UI Target
#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiDefaultLayoutTarget(pub UiLayoutTarget);

/// Used internally by `ui_layout_system`
#[derive(Component, Default, Debug, Reflect)]
pub struct TaffyKey {
    #[reflect(ignore)]
    /// Identifies the node within the Taffy layout tree corresponding to the entity with this component.
    key: taffy::node::Node,
}

#[derive(Resource, Default)]
pub struct UiContext(pub Option<LayoutContext>);

#[derive(Component, Debug, Clone, Copy)]
pub struct UiTransform(pub Affine2);

#[derive(Component, Default, Copy, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct UiLayoutOrder(pub i32);

pub struct LayoutContext {
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
    fn new(scale_factor: f64, physical_size: Vec2, require_full_update: bool) -> Self {
        let physical_to_logical_factor = 1. / scale_factor;
        Self {
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

#[derive(SystemParam)]
pub struct UiSurface<'w, 's> {
    entity_to_taffy: ResMut<'w, UiEntityToTaffyMap>,
    tree: ResMut<'w, TaffyTree>,
    layouts: Res<'w, UiLayouts>,
    #[system_param(ignore)]
    phantom: PhantomData<fn() -> &'s ()>,
}

#[derive(Resource, Deref, DerefMut)]
pub struct TaffyTree {
    tree: Taffy,
}

impl Default for TaffyTree {
    fn default() -> Self {
        let mut tree = Taffy::default();
        tree.disable_rounding();
        Self { tree }
    }
}

fn _assert_send_sync_ui_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
    _assert_send_sync::<Taffy>();
    _assert_send_sync::<UiSurface>();
}

impl<'w, 's> UiSurface<'w, 's> {
    fn insert_lookup(&mut self, entity: Entity, node: taffy::node::Node) {
        trace!("inserting lookup {entity:?} -> {node:?}");
        if let Some(old_key) = self.entity_to_taffy.insert(entity, node) {
            trace!("\tremoving {old_key:?}");
            self.tree.remove(old_key).ok();
        }
    }

    /// Update the children of the taffy node corresponding to the given [`Entity`].
    fn update_children(&mut self, parent: taffy::node::Node, children: &Children) {
        let mut taffy_children = Vec::with_capacity(children.len());
        debug!("Update children for parent -> {parent:?}");
        for child in children {
            if let Some(taffy_node) = self.entity_to_taffy.get(child) {
                taffy_children.push(*taffy_node);
                debug!("\tpush child {child:?} -> {taffy_node:?}");
            } else {
                warn!(
                    "Unstyled child in a UI entity hierarchy. You are using an entity \
without UI components as a child of an entity with UI components, results may be unexpected."
                );
            }
        }

        self.tree.set_children(parent, &taffy_children).unwrap();
        debug!("Set children.");
    }

    /// Removes children from the entity's taffy node if it exists. Does nothing otherwise.
    fn try_remove_children(&mut self, entity: Entity) {
        trace!("try remove corresponding taffy children for {entity:?}");
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            trace!("\ttaffy_node found: {taffy_node:?}");
            self.tree.set_children(*taffy_node, &[]).unwrap();
            trace!("\tremoved all children");
        }
    }

    /// Removes the measure from the entity's taffy node if it exists. Does nothing otherwise.
    fn try_remove_measure(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.get(&entity) {
            trace!("Try remove measure for {entity:?}");
            self.tree.set_measure(*taffy_node, None).unwrap();
        }
    }

    /// Compute the layout for each window entity's corresponding root node in the layout.
    fn compute_all_layouts(&mut self) {
        debug!("compute layouts");

        for ui_layout in self.layouts.iter() {
            self.tree
                .compute_layout(ui_layout.taffy_root_node, Size::MAX_CONTENT)
                .unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from taffy
    fn remove_nodes(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            if let Some(node) = self.entity_to_taffy.remove(&entity) {
                self.tree.remove(node).unwrap();
            }
        }
    }
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

/// Remove the corresponding taffy node for any entity that has its `Node` component removed.
pub fn clean_up_removed_ui_nodes_system(
    mut ui_surface: UiSurface,
    mut removed_nodes: RemovedComponents<TaffyKey>,
    mut removed_calculated_sizes: RemovedComponents<ContentSize>,
) {
    debug!("clean_up_removed_ui_nodes_system");
    // clean up removed nodes
    ui_surface.remove_nodes(removed_nodes.iter());

    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding taffy node.
    for entity in removed_calculated_sizes.iter() {
        ui_surface.try_remove_measure(entity);
    }
    debug!("clean_up_removed_ui_nodes_system finished");
}

/// Insert a new taffy node into the layout for any entity that had a `Node` component added.
pub fn insert_new_ui_nodes_system(
    mut ui_surface: UiSurface,
    mut new_node_query: Query<(Entity, &mut TaffyKey), Added<TaffyKey>>,
) {
    debug!("insert_new_ui_nodes_system");
    for (entity, mut node) in new_node_query.iter_mut() {
        node.key = ui_surface
            .tree
            .new_leaf(taffy::style::Style::DEFAULT)
            .unwrap();
        trace!("\tInserted new taffy leaf: {:?} => {:?}", entity, node.key);
        // if let Some(old_key) = ui_surface.entity_to_taffy.insert(entity, node.key) {
        //     ui_surface.taffy.remove(old_key).ok();
        // }
        ui_surface.insert_lookup(entity, node.key);
    }
    debug!("\tinsert_new_ui_nodes_system finished");
}

pub fn update_ui_windows_system(
    mut resize_events: EventReader<bevy_window::WindowResized>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut ui_context: ResMut<UiContext>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
) {
    debug!("update_ui_windows_system");
    // assume one window for time being...
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let (primary_window_entity, logical_to_physical_factor, window_physical_size) =
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
    let context = LayoutContext::new(scale_factor, window_physical_size, require_full_update);
    ui_context.0 = Some(context);
    debug!("update_ui_windows_system finished");
}

/// update and insert new ui roots
pub fn update_ui_layouts(
    mut ui_layouts: ResMut<UiLayouts>,
    mut ui_root_map: ResMut<UiRootMap>,
    mut taffy: ResMut<TaffyTree>,
    default_layout_target: Res<UiDefaultLayoutTarget>,
    orphaned_uinode_query: Query<
        (Entity, &TaffyKey, &NodeOrder, Option<&UiLayoutTarget>),
        Without<Parent>,
    >,
    primary_window_query: Query<Entity, With<PrimaryWindow>>,
    windows_query: Query<&Window>,
) {
    debug!("update_ui_layouts syste");
    let primary_window = primary_window_query.get_single().ok();

    // Recreate the list each frame for now, typically it shouldn't be very large.
    ui_layouts.clear();

    // For each UI node without a parent add a `UiLayout` item to the layouts list
    for (ui_root_node, TaffyKey { key }, &NodeOrder(order), maybe_layout_target) in
        orphaned_uinode_query.iter()
    {
        // The `UiLayoutTarget` component is optional for root ui entities.
        // Without a `UiLayoutTarget`, we attempt to use the default layout target (which by default is the primary window).
        // If there is no default layout target, or the target is the primary window and the primary window wasn't found, then this node won't be added to the list.
        if let Some(target) = maybe_layout_target
            .and_then(|layout_target| layout_target.normalize(primary_window))
            .or_else(|| default_layout_target.0.normalize(primary_window))
        {
            // Retrieve the taffy node for the root node from `UiRootMap` or create a new node and add it to `UiRootMap`
            let taffy_root_node = *ui_root_map
                .entry(ui_root_node)
                .or_insert_with(|| {
                    let taffy_root_node = taffy.new_with_children(taffy::style::Style::default(), &[*key]).unwrap();
                    debug!("New taffy root: [ target {target:?} -> Taffy root {taffy_root_node:?} -> UI root [{ui_root_node:?} | {key:?}]");
                    taffy_root_node
                });

            // Set the Taffy root's size to the physical window resolution
            let resolution = &windows_query.single().resolution;
            taffy
                .set_style(
                    taffy_root_node,
                    taffy::style::Style {
                        size: taffy::geometry::Size {
                            width: taffy::style::Dimension::Points(
                                resolution.physical_width() as f32
                            ),
                            height: taffy::style::Dimension::Points(
                                resolution.physical_height() as f32
                            ),
                        },
                        ..Default::default()
                    },
                )
                .unwrap();

            ui_layouts.push(UiLayout {
                target: UiNormalizedLayoutTarget { target },
                taffy_root_node,
                ui_root_node,
                order,
            });
        }
    }

    // Sort ui layouts by their order
    ui_layouts.sort_by(|n, m| n.order.cmp(&m.order));

    debug!("update_ui_layouts complete");
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn update_ui_nodes_system(
    ui_context: ResMut<UiContext>,
    mut ui_surface: UiSurface,
    style_query: Query<(&TaffyKey, Ref<Style>)>,
    full_style_query: Query<(&TaffyKey, &Style)>,
    mut measure_query: Query<(&TaffyKey, &mut ContentSize)>,
    changed_order_query: Query<&Parent, (Changed<NodeOrder>, With<NodeSize>)>,
    node_order_query: Query<&NodeOrder>,
    mut removed_children: RemovedComponents<Children>,
    mut children_query: Query<(&TaffyKey, &mut Children), With<NodeSize>>,
) {
    debug!("update_ui_nodes_system");
    let Some(ref layout_context) = ui_context.0 else {
        return
    };

    if layout_context.require_full_update {
        // update all nodes
        for (node, style) in full_style_query.iter() {
            trace!("Update style -> {:?}", node);
            ui_surface
                .tree
                .set_style(node.key, convert::from_style(layout_context, style))
                .ok();
        }
    } else {
        for (node, style) in style_query.iter() {
            if style.is_changed() {
                trace!("Update style -> {:?}", node);
                ui_surface
                    .tree
                    .set_style(node.key, convert::from_style(layout_context, &style))
                    .ok();
            }
        }
    }

    for (taffy_node, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            trace!("Update measure func -> {taffy_node:?}");
            ui_surface
                .tree
                .set_measure(taffy_node.key, Some(measure_func))
                .ok();
        }
    }

    // update and remove children
    for entity in removed_children.iter() {
        ui_surface.try_remove_children(entity);
    }

    for parent in changed_order_query.iter() {
        let (_, mut children) = children_query.get_mut(parent.get()).unwrap();
        children.set_changed();
    }

    for (node, mut children) in children_query.iter_mut() {
        if children.is_changed() {
            children.sort_by(|c, d| {
                let c_ord = node_order_query
                    .get_component(*c)
                    .map(|n: &NodeOrder| n.0)
                    .unwrap_or(0);
                let d_ord = node_order_query
                    .get_component(*d)
                    .map(|n: &NodeOrder| n.0)
                    .unwrap_or(0);
                c_ord.cmp(&d_ord)
            });
            ui_surface.update_children(node.key, &children);
        }
    }

    // compute layouts
    ui_surface.compute_all_layouts();
    debug!("update_ui_layouts_system finished");
}

pub fn update_node_geometries_iteratively(
    mut stack: Local<Vec<(Entity, Vec2, Vec2)>>,
    ui_surface: UiSurface,
    ui_context: Res<UiContext>,
    mut node_geometry_query: Query<(&TaffyKey, &mut NodeSize, &mut NodePosition, &mut ZIndex)>,
    just_children_query: Query<&Children>,
) {
    debug!("update_nodes_iteratively");
    let Some(physical_to_logical_factor) = ui_context
        .0
        .as_ref()
        .map(|context|  context.physical_to_logical_factor)
    else {
        return;
    };

    stack.clear();

    let mut order: u32 = 0;

    for ui_layout in ui_surface.layouts.iter() {
        stack.push((ui_layout.ui_root_node, Vec2::ZERO, Vec2::ZERO));
        while let Some((node_entity, inherited_position, abs)) = stack.pop() {
            if let Ok((node, mut node_size, mut position, mut z_index)) =
                node_geometry_query.get_mut(node_entity)
            {
                z_index.0 = order;
                order += 1;
                let layout = ui_surface.tree.layout(node.key).unwrap();
                let abs = abs + Vec2::new(layout.location.x, layout.location.y);

                let location = Vec2::new(
                    layout.location.x.round(),
                    layout.location.y.round(),
                );
                let size = Vec2::new(
                    layout.size.width,
                    layout.size.height,
                );
                let size = (abs + size).round() - abs.round();

                let new_size = Vec2::new(
                    (size.x as f64 * physical_to_logical_factor) as f32,
                    (size.y as f64 * physical_to_logical_factor) as f32,
                );
                let half_size = 0.5 * new_size;
                if node_size.calculated_size != new_size {
                    node_size.calculated_size = new_size;
                }
                
                position.0 = inherited_position
                    + half_size
                    + Vec2::new(
                        (location.x as f64 * physical_to_logical_factor) as f32,
                        (location.y as f64 * physical_to_logical_factor) as f32,
                    );

                if let Ok(children) = just_children_query.get(node_entity) {
                    // Push the children nodes onto the stack.
                    for child in children {
                        stack.push((*child, position.0 - half_size, abs));
                    }
                }
            }
        }
    }
    debug!("update_nodes_iteratively finished");
}

pub fn update_nodes_recursively(
    ui_surface: UiSurface,
    ui_context: Res<UiContext>,
    mut node_geometry_query: Query<(&TaffyKey, &mut NodeSize, &mut NodePosition, &mut ZIndex)>,
    just_children_query: Query<&Children>,
) {
    let Some(physical_to_logical_factor) = ui_context
            .0
            .as_ref()
            .map(|context|  context.physical_to_logical_factor)
        else {
            return;
        };

    fn update_node_geometry_recursively(
        ui_surface: &UiSurface,
        inherited_position: Vec2,
        node_entity: Entity,
        node_geometry_query: &mut Query<(&TaffyKey, &mut NodeSize, &mut NodePosition, &mut ZIndex)>,
        children_query: &Query<&Children>,
        physical_to_logical_factor: f64,
        order: &mut u32,
    ) {
        if let Ok((node, mut node_size, mut node_position, mut z_index)) =
            node_geometry_query.get_mut(node_entity)
        {
            z_index.0 = *order;
            *order += 1;
            let layout = ui_surface.tree.layout(node.key).unwrap();
            let new_size = Vec2::new(
                (layout.size.width as f64 * physical_to_logical_factor) as f32,
                (layout.size.height as f64 * physical_to_logical_factor) as f32,
            );
            let half_size = 0.5 * new_size;
            if node_size.calculated_size != new_size {
                node_size.calculated_size = new_size;
            }
            node_position.0 = inherited_position
                + half_size
                + Vec2::new(
                    (layout.location.x as f64 * physical_to_logical_factor) as f32,
                    (layout.location.y as f64 * physical_to_logical_factor) as f32,
                );

            if let Ok(children) = children_query.get(node_entity) {
                let next_position = node_position.0 - half_size;
                for child in children {
                    update_node_geometry_recursively(
                        ui_surface,
                        next_position,
                        *child,
                        node_geometry_query,
                        children_query,
                        physical_to_logical_factor,
                        order,
                    );
                }
            }
        }
    }

    let mut order: u32 = 0;

    for ui_layout in ui_surface.layouts.iter() {
        update_node_geometry_recursively(
            &ui_surface,
            Vec2::ZERO,
            ui_layout.ui_root_node,
            &mut node_geometry_query,
            &just_children_query,
            physical_to_logical_factor,
            &mut order,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::clean_up_removed_ui_nodes_system;
    use crate::insert_new_ui_nodes_system;
    use crate::update_ui_nodes_system;
    use crate::AlignItems;
    use crate::LayoutContext;
    use crate::NodeSize;
    use crate::Style;
    use crate::TaffyKey;
    use crate::UiContext;
    use crate::UiSurface;
    use bevy_ecs::prelude::*;
    use bevy_math::Vec2;
    use taffy::tree::LayoutTree;

    fn node_bundle() -> (TaffyKey, NodeSize, Style) {
        (TaffyKey::default(), NodeSize::default(), Style::default())
    }

    fn ui_schedule() -> Schedule {
        let mut ui_schedule = Schedule::default();
        ui_schedule.add_systems((
            clean_up_removed_ui_nodes_system.before(insert_new_ui_nodes_system),
            insert_new_ui_nodes_system.before(synchonise_ui_children_system),
            synchonise_ui_children_system.before(update_ui_nodes_system),
            update_ui_nodes_system,
        ));
        ui_schedule
    }

    #[test]
    fn test_insert_and_remove_node() {
        let mut world = World::new();
        world.init_resource::<UiSurface>();
        world.insert_resource(UiContext(Some(LayoutContext::new(
            3.0,
            Vec2::new(1000., 500.),
            true,
        ))));
        let mut ui_schedule = ui_schedule();

        // add ui node entity to world
        let entity = world.spawn(node_bundle()).id();

        // ui update
        ui_schedule.run(&mut world);

        let key = world.get::<TaffyKey>(entity).unwrap().key;
        let surface = world.resource::<UiSurface>();

        // ui node entity should be associated with a taffy node
        assert_eq!(surface.entity_to_taffy[&entity], key);

        // taffy node should be a child of the window node
        assert_eq!(surface.tree.parent(key), surface.default_layout);

        // despawn the ui node entity
        world.entity_mut(entity).despawn();

        ui_schedule.run(&mut world);

        let surface = world.resource::<UiSurface>();

        // the despawned entity's associated taffy node should also be removed
        assert!(!surface.entity_to_taffy.contains_key(&entity));

        // window node should have no children
        assert!(surface
            .tree
            .children(surface.default_layout.unwrap())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_node_style_update() {
        let mut world = World::new();
        world.init_resource::<UiSurface>();
        world.insert_resource(UiContext(Some(LayoutContext::new(
            3.0,
            Vec2::new(1000., 500.),
            true,
        ))));
        let mut ui_schedule = ui_schedule();

        // add a ui node entity to the world and run the ui schedule to add a corresponding node to the taffy layout tree
        let entity = world.spawn(node_bundle()).id();
        ui_schedule.run(&mut world);

        // modify the ui node's style component and rerun the schedule
        world.get_mut::<Style>(entity).unwrap().align_items = AlignItems::Baseline;

        // don't want a full update
        world.insert_resource(UiContext(Some(LayoutContext::new(
            3.0,
            Vec2::new(1000., 500.),
            false,
        ))));

        ui_schedule.run(&mut world);

        // check the corresponding taffy node's style is also updated
        let ui_surface = world.resource::<UiSurface>();
        let key = ui_surface.entity_to_taffy[&entity];
        assert_eq!(
            ui_surface.tree.style(key).unwrap().align_items,
            Some(taffy::style::AlignItems::Baseline)
        );
    }
}

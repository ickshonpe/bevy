mod convert;
pub mod debug_output;

use crate::{ContentSize, NodeOrder, NodeSize, Style, UiScale, UiTransform, ZIndex};
use bevy_derive::{DerefMut, Deref};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    prelude::{Bundle, Component},
    query::Added,
    query::{With, Without},
    reflect::ReflectComponent,
    removal_detection::RemovedComponents,
    system::{Local, Query, Res, ResMut, Resource, SystemParam, Commands},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::{debug, warn};
use bevy_math::{Affine3A, Vec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::view::{ComputedVisibility, Visibility};
use bevy_transform::{components::Transform, prelude::GlobalTransform};
use bevy_utils::{HashMap, HashSet};
use bevy_window::{PrimaryWindow, Window, WindowScaleFactorChanged};
use std::{marker::PhantomData};
use taffy::{prelude::Size, style_helpers::TaffyMaxContent, Taffy};

/// Used internally by `ui_layout_system`
#[derive(Component, Default, Debug, Reflect)]
pub struct NodeKey {
    #[reflect(ignore)]
    /// Identifies the node within the Taffy layout tree corresponding to the entity with this component.
    key: taffy::node::Node,
}

#[derive(Resource, Default)]
pub struct UiContext(pub Option<LayoutContext>);

/// Used internally by `ui_layout_system`
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
pub struct UiLayoutData {
    #[reflect(ignore)]
    node_key: taffy::node::Node,
}

#[derive(Component, Default, Copy, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct UiLayoutOrder(pub i32);

#[derive(Bundle, Default)]
pub struct UiLayoutBundle {
    pub order: UiLayoutOrder,
    pub data: UiLayoutData,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visbility: ComputedVisibility,
}

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

#[derive(Debug)]
pub struct UiLayout {
    /// entity representing the layout
    /// children of this node are added to the layout as children of the root taffy node
    pub layout_entity: Entity,
    /// root taffy node for the layout, same size as the window resolution
    pub taffy_root: taffy::node::Node,
    /// sort order, lower drawn first
    pub order: i32,
}

#[derive(Resource)]
pub struct UiData {
    /// Ui Node entity to taffy layout tree node lookup map
    entity_to_taffy: HashMap<Entity, taffy::node::Node>,
    taffy_to_entity: HashMap<taffy::node::Node, Entity>,
    /// default layout node, orphaned Ui entities attach to here
    default_layout: Option<taffy::node::Node>,
    /// contains data for each distinct ui layout
    ui_layouts: Vec<UiLayout>,
    /// contains data for each layout child
    layout_children: HashMap<taffy::node::Node, Vec<Entity>>,
}


#[derive(SystemParam)]
pub struct UiSurface<'w, 's> {
    data: ResMut<'w, UiData>,
    taffy: ResMut<'w, UiLayoutTree>,
    #[system_param(ignore)]
    phantom: PhantomData<fn() -> &'s ()>
}

#[derive(Resource, Deref, DerefMut)]
pub struct UiLayoutTree {
    taffy: Taffy,
}

fn _assert_send_sync_ui_surface_impl_safe() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<HashMap<Entity, taffy::node::Node>>();
    _assert_send_sync::<Taffy>();
    _assert_send_sync::<UiSurface>();
}

pub fn ui_setup_system(
    mut commands: Commands,
) {
    let mut taffy = Taffy::new();
        let default_layout = taffy.new_leaf(taffy::prelude::Style::default()).unwrap();
        let default_layout_entity = commands
            .spawn(UiLayoutBundle {
                order: UiLayoutOrder(0),
                data: UiLayoutData {
                    node_key: default_layout,
                },
                ..Default::default()
            })
            .id();
        
        commands.insert_resource(UiData {
            entity_to_taffy: Default::default(),
            taffy_to_entity: Default::default(),
            default_layout: Some(default_layout),
            ui_layouts: vec![UiLayout {
                layout_entity: default_layout_entity,
                taffy_root: default_layout,
                order: 0,
            }],
            layout_children: [(default_layout, vec![])].into_iter().collect(),
        });

        commands.insert_resource(UiLayoutTree { taffy });
}

impl <'w, 's> UiSurface<'w, 's> {
    fn insert_lookup(&mut self, entity: Entity, node: taffy::node::Node) {
        debug!("inserting lookup {entity:?} -> {node:?}");
        if let Some(old_key) = self.data.entity_to_taffy.insert(entity, node) {
            debug!("\tremoving {old_key:?}");
            self.taffy.remove(old_key).ok();
            self.data.taffy_to_entity.remove(&old_key);
            self.data.taffy_to_entity.insert(node, entity);
        }
    }

    fn insert_ui_layout(&mut self, layout_entity: Entity, order: i32) -> taffy::node::Node {
        debug!("insert ui layout {layout_entity:?}");
        let layout_node = self.taffy.new_leaf(taffy::style::Style::default()).unwrap();
        self.data.ui_layouts.push(UiLayout {
            layout_entity,
            taffy_root: layout_node,
            order,
        });
        debug!("Inserted layout: {layout_entity:?} -> {layout_node:?}, order: {order}");
        self.insert_lookup(layout_entity, layout_node);
        layout_node
    }

    fn update_style(
        &mut self,
        taffy_node: taffy::node::Node,
        style: &Style,
        context: &LayoutContext,
    ) {
        debug!("Update style -> {taffy_node:?}");
        self.taffy
            .set_style(taffy_node, convert::from_style(context, style))
            .ok();
    }

    /// Update the `MeasureFunc` of the taffy node corresponding to the given [`Entity`].
    fn update_measure(
        &mut self,
        taffy_node: taffy::node::Node,
        measure_func: taffy::node::MeasureFunc,
    ) {
        debug!("Update measure func -> {taffy_node:?}");
        self.taffy.set_measure(taffy_node, Some(measure_func)).ok();
    }

    /// Update the children of the taffy node corresponding to the given [`Entity`].
    fn update_children(&mut self, parent: taffy::node::Node, children: &Children) {
        let mut taffy_children = Vec::with_capacity(children.len());
        debug!("Update children for parent -> {parent:?}");
        for child in children {
            if let Some(taffy_node) = self.data.entity_to_taffy.get(child) {
                taffy_children.push(*taffy_node);
                debug!("\tpush child {child:?} -> {taffy_node:?}");
            } else {
                warn!(
                    "Unstyled child in a UI entity hierarchy. You are using an entity \
without UI components as a child of an entity with UI components, results may be unexpected."
                );
            }
        }

        self.taffy.set_children(parent, &taffy_children).unwrap();
        debug!("Set children.");
    }

    /// Removes children from the entity's taffy node if it exists. Does nothing otherwise.
    fn try_remove_children(&mut self, entity: Entity) {
        debug!("try remove corresponding taffy children for {entity:?}");
        if let Some(taffy_node) = self.data.entity_to_taffy.get(&entity) {
            debug!("\ttaffy_node found: {taffy_node:?}");
            self.taffy.set_children(*taffy_node, &[]).unwrap();
            debug!("\tremoved all children");
        }
    }

    /// Removes the measure from the entity's taffy node if it exists. Does nothing otherwise.
    fn try_remove_measure(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.data.entity_to_taffy.get(&entity) {
            debug!("Try remove measure for {entity:?}");
            self.taffy.set_measure(*taffy_node, None).unwrap();
        }
    }

    /// Update the size of each layout node to match the size of the window.
    fn update_layout_nodes(&mut self, size: Vec2) {
        let taffy = &mut self.taffy;
        for UiLayout { taffy_root, .. } in &self.data.ui_layouts {
            debug!("Update layout node: {taffy_root:?}, res: {size}");
            taffy
                .set_style(
                    *taffy_root,
                    taffy::style::Style {
                        size: taffy::geometry::Size {
                            width: taffy::style::Dimension::Points(size.x),
                            height: taffy::style::Dimension::Points(size.y),
                        },
                        ..Default::default()
                    },
                )
                .unwrap();
        }
    }

    /// Set the ui node entities without a [`Parent`] as children to the default root node in the taffy layout.
    fn set_default_layout_children(&mut self, children: impl Iterator<Item = Entity>) {
        debug!("set default layout children");
        if let Some(node) = self.data.default_layout {
            debug!("\tdefault_layout: {node:?}");
            let data = self.data.as_mut();
            let UiData { layout_children, entity_to_taffy, .. }  = data;
            
            let childs = layout_children.get_mut(&node).unwrap();
            
            *childs = children.collect();
            let child_nodes = childs
                .iter()
                .map(|e| *entity_to_taffy.get(e).unwrap())
                .collect::<Vec<taffy::node::Node>>();
            for (e, n) in childs.iter().zip(child_nodes.iter()) {
                debug!("\t{e:?} -> {n:?}");
            }
            self.taffy.set_children(node, &child_nodes).unwrap();
        }
        debug!("default layout children set");
    }

    fn set_layout_children(&mut self, layout: taffy::node::Node, children: &Children) {
        debug!("set layout children for {layout:?}");
        self.data.layout_children
            .insert(layout, children.iter().copied().collect());
        let child_nodes = children
            .iter()
            .map(|e| *self.data.entity_to_taffy.get(e).unwrap())
            .collect::<Vec<taffy::node::Node>>();
        self.taffy.set_children(layout, &child_nodes).unwrap();
    }

    /// Compute the layout for each window entity's corresponding root node in the layout.
    fn compute_all_layouts(&mut self) {
        debug!("compute layouts");
        for UiLayout { taffy_root, .. } in &self.data.ui_layouts {
            self.taffy
                .compute_layout(*taffy_root, Size::MAX_CONTENT)
                .unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from taffy
    fn remove_nodes(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for entity in entities {
            if let Some(node) = self.data.entity_to_taffy.remove(&entity) {
                self.taffy.remove(node).unwrap();
            }
        }
    }

    /// Removes layouts and set a new default if necessary
    fn remove_layouts(
        &mut self,
        removed_entities: impl IntoIterator<Item = Entity>,
        maybe_next_default_layout_entity: Option<Entity>,
    ) {
        debug!("remove_layouts, next default: {maybe_next_default_layout_entity:?}");
        for entity in removed_entities {
            debug!("\tremoving {entity:?}");
            if let Some(node_to_delete) = self.data.entity_to_taffy.get(&entity).copied() {
                self.taffy.remove(node_to_delete).unwrap();
                self.data.layout_children.remove(&node_to_delete);
                if self.data.default_layout == Some(node_to_delete) {
                    self.data.default_layout =
                        maybe_next_default_layout_entity.and_then(|next_default_layout_entity| {
                            self.data.ui_layouts
                                .iter()
                                .find(|UiLayout { layout_entity, .. }| {
                                    *layout_entity == next_default_layout_entity
                                })
                                .map(|UiLayout { taffy_root, .. }| *taffy_root)
                        });
                }
                self.data.ui_layouts
                    .retain(|UiLayout { layout_entity, .. }| *layout_entity != entity);
            }
        }
    }
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

pub fn sort_children_by_node_order_system(
    mut sorted: Local<HashSet<Entity>>,
    order_query: Query<(Ref<NodeOrder>, &Parent)>,
    mut children_query: Query<&mut Children>,
) {
    sorted.clear();
    for (order, parent) in order_query.iter() {
        if order.is_changed() && !sorted.contains(&parent.get()) {
            children_query
                .get_mut(parent.get())
                .unwrap()
                .sort_by(|c, d| {
                    let c_ord = order_query.get_component(*c).unwrap_or(&NodeOrder(0)).0;
                    let d_ord = order_query.get_component(*d).unwrap_or(&NodeOrder(0)).0;
                    c_ord.cmp(&d_ord)
                });
        }
    }
}

/// Remove the corresponding taffy node for any entity that has its `Node` component removed.
pub fn clean_up_removed_ui_nodes_system(
    mut ui_surface: UiSurface,
    mut removed_nodes: RemovedComponents<NodeKey>,
    mut removed_calculated_sizes: RemovedComponents<ContentSize>,
) {
    // clean up removed nodes
    ui_surface.remove_nodes(removed_nodes.iter());

    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding taffy node.
    for entity in removed_calculated_sizes.iter() {
        ui_surface.try_remove_measure(entity);
    }
}

/// Insert a new taffy node into the layout for any entity that had a `Node` component added.
pub fn insert_new_ui_nodes_system(
    mut ui_surface: UiSurface,
    mut new_node_query: Query<(Entity, &mut NodeKey), Added<NodeKey>>,
) {
    for (entity, mut node) in new_node_query.iter_mut() {
        node.key = ui_surface
            .taffy
            .new_leaf(taffy::style::Style::DEFAULT)
            .unwrap();
        // if let Some(old_key) = ui_surface.entity_to_taffy.insert(entity, node.key) {
        //     ui_surface.taffy.remove(old_key).ok();
        // }
        ui_surface.insert_lookup(entity, node.key);
    }
}

/// Synchonise the Bevy and Taffy Parent-Children trees
pub fn synchonise_ui_children_system(
    mut ui_surface: UiSurface,
    mut removed_children: RemovedComponents<Children>,
    children_query: Query<(&NodeKey, Ref<Children>)>,
) {
    // Iterate through all entities with a removed `Children` component and if they have a corresponding Taffy node, remove their children from the Taffy tree.
    for entity in removed_children.iter() {
        ui_surface.try_remove_children(entity);
    }

    // Update the corresponding Taffy children of Bevy entities with changed `Children`
    for (node, children) in &children_query {
        if children.is_changed() {
            ui_surface.update_children(node.key, &children);
        }
    }
}

pub fn update_ui_windows_system(
    mut resize_events: EventReader<bevy_window::WindowResized>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut ui_context: ResMut<UiContext>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
) {
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
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn update_ui_layouts_system(
    ui_context: ResMut<UiContext>,
    mut ui_surface: UiSurface,
    orphaned_node_query: Query<Entity, (With<NodeSize>, With<NodeKey>, Without<Parent>)>,
    style_query: Query<(&NodeKey, Ref<Style>)>,
    full_style_query: Query<(&NodeKey, &Style)>,
    mut measure_query: Query<(&NodeKey, &mut ContentSize)>,
    mut removed_layouts: RemovedComponents<UiLayoutOrder>,
    mut layouts_query: Query<
        (Entity, &mut UiLayoutData, &UiLayoutOrder, Option<&Children>),
        Without<NodeSize>,
    >,
) {
    let Some(ref layout_context) = ui_context.0 else {
        return
    };

    if layout_context.require_full_update {
        // update all nodes
        for (node, style) in full_style_query.iter() {
            ui_surface.update_style(node.key, style, layout_context);
        }
    } else {
        for (node, style) in style_query.iter() {
            if style.is_changed() {
                ui_surface.update_style(node.key, &style, layout_context);
            }
        }
    }

    // insert new ui layouts
    for (layout_entity, mut layout_data, &UiLayoutOrder(order), _) in layouts_query.iter_mut() {
        if layout_data.node_key == taffy::node::Node::default() {
            layout_data.node_key = ui_surface.insert_ui_layout(layout_entity, order);
        }
    }

    // clean up removed layouts
    ui_surface.remove_layouts(
        removed_layouts.iter(),
        layouts_query.iter().next().map(|(entity, ..)| entity),
    );

    for (node, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            ui_surface.update_measure(node.key, measure_func);
        }
    }

    // update layout nodes so their size matches the size of the primary window
    ui_surface.update_layout_nodes(layout_context.physical_size);

    // sort layouts by order
    ui_surface
        .data.ui_layouts
        .sort_by_key(|UiLayout { order, .. }| *order);

    // update orphaned nodes as children of the default layout (for now assuming all Nodes live in the primary window)
    ui_surface.set_default_layout_children(orphaned_node_query.iter());

    for (_, data, _, children) in layouts_query.iter() {
        if let Some(children) = children {
            ui_surface.set_layout_children(data.node_key, children);
        }
    }

    // compute layouts
    ui_surface.compute_all_layouts();
}

pub fn update_nodes(
    mut ui_surface: UiSurface,
    ui_context: Res<UiContext>,
    mut node_geometry_query: Query<(&NodeKey, &mut NodeSize, &mut UiTransform, &mut ZIndex)>,
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
        inherited_transform: Affine3A,
        node_entity: Entity,
        node_geometry_query: &mut Query<(&NodeKey, &mut NodeSize, &mut UiTransform, &mut ZIndex)>,
        children_query: &Query<&Children>,
        physical_to_logical_factor: f64,
        order: &mut u32,
    ) {
        if let Ok((node, mut node_size, mut transform, mut z_index)) =
            node_geometry_query.get_mut(node_entity)
        {
            z_index.0 = *order;
            *order += 1;
            let layout = ui_surface.taffy.layout(node.key).unwrap();
            let new_size = Vec2::new(
                (layout.size.width as f64 * physical_to_logical_factor) as f32,
                (layout.size.height as f64 * physical_to_logical_factor) as f32,
            );
            let half_size = (0.5 * new_size).extend(0.);
            if node_size.calculated_size != new_size {
                node_size.calculated_size = new_size;
            }
            let new_position = Vec2::new(
                (layout.location.x as f64 * physical_to_logical_factor) as f32,
                (layout.location.y as f64 * physical_to_logical_factor) as f32,
            );

            transform.0 = inherited_transform
                * Affine3A::from_translation(new_position.extend(0.) + half_size);

            if let Ok(children) = children_query.get(node_entity) {
                let next_transform = transform.0 * Affine3A::from_translation(-half_size);
                for child in children {
                    update_node_geometry_recursively(
                        ui_surface,
                        next_transform,
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

    for UiLayout { taffy_root, .. } in ui_surface.data.ui_layouts.iter() {
        for child in ui_surface.data.layout_children.get(taffy_root).unwrap() {
            update_node_geometry_recursively(
                &ui_surface,
                Affine3A::default(),
                *child,
                &mut node_geometry_query,
                &just_children_query,
                physical_to_logical_factor,
                &mut order,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::clean_up_removed_ui_nodes_system;
    use crate::insert_new_ui_nodes_system;
    use crate::synchonise_ui_children_system;
    use crate::update_ui_layouts_system;
    use crate::AlignItems;
    use crate::LayoutContext;
    use crate::NodeKey;
    use crate::NodeSize;
    use crate::Style;
    use crate::UiContext;
    use crate::UiSurface;
    use bevy_ecs::prelude::*;
    use bevy_math::Vec2;
    use taffy::tree::LayoutTree;

    fn node_bundle() -> (NodeKey, NodeSize, Style) {
        (NodeKey::default(), NodeSize::default(), Style::default())
    }

    fn ui_schedule() -> Schedule {
        let mut ui_schedule = Schedule::default();
        ui_schedule.add_systems((
            clean_up_removed_ui_nodes_system.before(insert_new_ui_nodes_system),
            insert_new_ui_nodes_system.before(synchonise_ui_children_system),
            synchonise_ui_children_system.before(update_ui_layouts_system),
            update_ui_layouts_system,
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

        let key = world.get::<NodeKey>(entity).unwrap().key;
        let surface = world.resource::<UiSurface>();

        // ui node entity should be associated with a taffy node
        assert_eq!(surface.entity_to_taffy[&entity], key);

        // taffy node should be a child of the window node
        assert_eq!(surface.taffy.parent(key), surface.default_layout);

        // despawn the ui node entity
        world.entity_mut(entity).despawn();

        ui_schedule.run(&mut world);

        let surface = world.resource::<UiSurface>();

        // the despawned entity's associated taffy node should also be removed
        assert!(!surface.entity_to_taffy.contains_key(&entity));

        // window node should have no children
        assert!(surface
            .taffy
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
            ui_surface.taffy.style(key).unwrap().align_items,
            Some(taffy::style::AlignItems::Baseline)
        );
    }
}

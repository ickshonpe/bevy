mod convert;
pub mod debug;

use crate::{ContentSize, Node, Style, UiScale, TreeNode};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    query::{With, Without, Added},
    removal_detection::RemovedComponents,
    system::{Query, Res, ResMut, Resource},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_log::warn;
use bevy_math::Vec2;
use bevy_transform::components::Transform;
use bevy_utils::HashMap;
use bevy_window::{PrimaryWindow, Window, WindowResolution, WindowScaleFactorChanged};
use std::fmt;
use taffy::{prelude::Size, style_helpers::TaffyMaxContent, Taffy, tree::LayoutTree};

pub struct LayoutContext {
    pub scale_factor: f64,
    pub physical_size: Vec2,
    pub min_size: f32,
    pub max_size: f32,
}

impl LayoutContext {
    /// create new a [`LayoutContext`] from the window's physical size and scale factor
    fn new(scale_factor: f64, physical_size: Vec2) -> Self {
        Self {
            scale_factor,
            physical_size,
            min_size: physical_size.x.min(physical_size.y),
            max_size: physical_size.x.max(physical_size.y),
        }
    }
}

#[derive(Resource)]
pub struct UiSurface {
    entity_to_taffy: HashMap<Entity, taffy::node::Node>,
    window_nodes: HashMap<Entity, taffy::node::Node>,
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
            .field("window_nodes", &self.window_nodes)
            .finish()
    }
}

impl Default for UiSurface {
    fn default() -> Self {
        Self {
            entity_to_taffy: Default::default(),
            window_nodes: Default::default(),
            taffy: Taffy::new(),
        }
    }
}

impl UiSurface {
    /// Update the `Style` of the taffy node corresponding to the given [`Entity`].
    pub fn update_style(
        &mut self,
        taffy_node: taffy::node::Node,
        style: &Style,
        context: &LayoutContext,
    ) {
        self.taffy
            .set_style(taffy_node, convert::from_style(context, style))
            .ok();
    }

    /// Update the `MeasureFunc` of the taffy node corresponding to the given [`Entity`].
    pub fn update_measure(
        &mut self,
        taffy_node: taffy::node::Node,
        measure_func: taffy::node::MeasureFunc,
    ) {
        self.taffy.set_measure(taffy_node, Some(measure_func)).ok();
    }

    // Update the children of the taffy node corresponding to the given [`Entity`].
    pub fn update_children(&mut self, parent: taffy::node::Node, children: &Children) {
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

        self.taffy.set_children(parent, &taffy_children).unwrap();
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
    pub fn update_window(&mut self, window: Entity, window_resolution: &WindowResolution) {
        let taffy = &mut self.taffy;
        let node = self
            .window_nodes
            .entry(window)
            .or_insert_with(|| taffy.new_leaf(taffy::style::Style::default()).unwrap());

        taffy
            .set_style(
                *node,
                taffy::style::Style {
                    size: taffy::geometry::Size {
                        width: taffy::style::Dimension::Points(
                            window_resolution.physical_width() as f32
                        ),
                        height: taffy::style::Dimension::Points(
                            window_resolution.physical_height() as f32,
                        ),
                    },
                    ..Default::default()
                },
            )
            .unwrap();
    }

    /// Set the ui node entities without a [`Parent`] as children to the root node in the taffy layout.
    pub fn set_window_children(
        &mut self,
        parent_window: Entity,
        children: impl Iterator<Item = taffy::node::Node>,
    ) {
        let taffy_node = self.window_nodes.get(&parent_window).unwrap();
        let child_nodes = children.collect::<Vec<taffy::node::Node>>();
        self.taffy.set_children(*taffy_node, &child_nodes).unwrap();
    }

    /// Compute the layout for each window entity's corresponding root node in the layout.
    pub fn compute_window_layouts(&mut self) {
        for window_node in self.window_nodes.values() {
            self.taffy
                .compute_layout(*window_node, Size::MAX_CONTENT)
                .unwrap();
        }
    }

    /// Removes each entity from the internal map and then removes their associated node from taffy
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

/// Insert a new taffy node into the layout for any entity that had a `Node` component added.
pub fn insert_new_ui_nodes(
    mut ui_surface: ResMut<UiSurface>,
    mut new_node_query: Query<(Entity, &mut TreeNode), Added<TreeNode>>,
) {
    for (entity, mut node) in new_node_query.iter_mut() {
        node.key = ui_surface
            .taffy
            .new_leaf(taffy::style::Style::DEFAULT)
            .unwrap();
        if let Some(old_key) = ui_surface.entity_to_taffy.insert(entity, node.key) {
            ui_surface.taffy.remove(old_key).ok();
        }
    }
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn ui_layout_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    ui_scale: Res<UiScale>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
    mut resize_events: EventReader<bevy_window::WindowResized>,
    mut ui_surface: ResMut<UiSurface>,
    root_node_query: Query<&TreeNode, Without<Parent>>,
    style_query: Query<(&TreeNode, Ref<Style>)>,
    mut measure_query: Query<(&TreeNode, &mut ContentSize)>,
    children_query: Query<(&TreeNode, Ref<Children>), With<Node>>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_content_sizes: RemovedComponents<ContentSize>,
    mut node_transform_query: Query<(&TreeNode, &mut Node, &mut Transform)>,
    mut removed_nodes: RemovedComponents<Node>,
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
            return;
        };

    let resized = resize_events
        .iter()
        .any(|resized_window| resized_window.window == primary_window_entity);

    // update window root nodes
    for (entity, window) in windows.iter() {
        ui_surface.update_window(entity, &window.resolution);
    }

    let scale_factor = logical_to_physical_factor * ui_scale.scale;

    let layout_context = LayoutContext::new(scale_factor, physical_size);

    if !scale_factor_events.is_empty() || ui_scale.is_changed() || resized {
        scale_factor_events.clear();
        // update all nodes
        for (tree_node, style) in style_query.iter() {
            ui_surface.update_style(tree_node.key, &style, &layout_context);
        }
    } else {
        for (tree_node, style) in style_query.iter() {
            if style.is_changed() {
                ui_surface.update_style(tree_node.key, &style, &layout_context);
            }
        }
    }

    for (tree_node, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            ui_surface.update_measure(tree_node.key, measure_func);
        }
    }

    // clean up removed nodes
    ui_surface.remove_entities(removed_nodes.iter());

    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding taffy node.
    for entity in removed_content_sizes.iter() {
        ui_surface.try_remove_measure(entity);
    }

    // update window children (for now assuming all Nodes live in the primary window)
    ui_surface.set_window_children(primary_window_entity, root_node_query.iter().map(|tree_node| tree_node.key));

    // update and remove children
    for entity in removed_children.iter() {
        ui_surface.try_remove_children(entity);
    }
    for (tree_node, children) in &children_query {
        if children.is_changed() {
            ui_surface.update_children(tree_node.key, &children);
        }
    }

    // compute layouts
    ui_surface.compute_window_layouts();

    let physical_to_logical_factor = 1. / logical_to_physical_factor;

    let to_logical = |v| (physical_to_logical_factor * v as f64) as f32;

    // PERF: try doing this incrementally
    for (tree_node, mut node, mut transform) in &mut node_transform_query {
        let layout = ui_surface.taffy.layout(tree_node.key).unwrap();
        let new_size = Vec2::new(
            to_logical(layout.size.width),
            to_logical(layout.size.height),
        );
        // only trigger change detection when the new value is different
        if node.calculated_size != new_size {
            node.calculated_size = new_size;
        }
        let mut new_position = transform.translation;
        new_position.x = to_logical(layout.location.x + layout.size.width / 2.0);
        new_position.y = to_logical(layout.location.y + layout.size.height / 2.0);
        if let Some(parent) = ui_surface.taffy.parent(tree_node.key) {
            if let Ok(parent_layout) = ui_surface.taffy.layout(parent) {
                new_position.x -= to_logical(parent_layout.size.width / 2.0);
                new_position.y -= to_logical(parent_layout.size.height / 2.0);
            }
        }
        // only trigger change detection when the new value is different
        if transform.translation != new_position {
            transform.translation = new_position;
        }
    }
}

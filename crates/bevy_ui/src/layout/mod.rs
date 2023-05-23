mod convert;
pub mod debug;

use crate::{ContentSize, Node, NodeSize, Style, UiScale};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    change_detection::DetectChanges,
    entity::Entity,
    event::EventReader,
    query::{With, Without},
    removal_detection::RemovedComponents,
    system::{Query, Res, ResMut, Resource, SystemParam},
    world::Ref,
};
use bevy_hierarchy::{Children, Parent};
use bevy_math::Vec2;
use bevy_transform::components::Transform;
use bevy_utils::HashMap;
use bevy_window::{PrimaryWindow, Window, WindowScaleFactorChanged};
use slotmap::{SlotMap, SparseSecondaryMap};
use std::marker::PhantomData;
use taffy::{
    node::MeasureFunc,
    prelude::{Layout, Size},
    style_helpers::TaffyMaxContent,
    tree::LayoutTree,
};

type TaffyNode = taffy::node::Node;
type TaffyStyle = taffy::style::Style;

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

pub(crate) const CACHE_SIZE: usize = 7;

struct TaffyNodeData {
    style: TaffyStyle,
    layout: Layout,
    needs_measure: bool,
    size_cache: [Option<taffy::layout::Cache>; CACHE_SIZE],
}

impl TaffyNodeData {
    /// Create the data for a new node
    #[must_use]
    pub const fn new(style: TaffyStyle) -> Self {
        Self {
            style,
            size_cache: [None; CACHE_SIZE],
            layout: Layout::new(),
            needs_measure: false,
        }
    }

    /// Marks a node and all of its parents (recursively) as dirty
    ///
    /// This clears any cached data and signals that the data must be recomputed.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.size_cache = [None; CACHE_SIZE];
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
struct EntityToTaffyMap(HashMap<Entity, TaffyNode>);

#[derive(Resource, Default, Deref, DerefMut)]
struct TaffyNodes(SlotMap<TaffyNode, TaffyNodeData>);

#[derive(Resource, Default, Deref, DerefMut)]
struct TaffyChildren(SlotMap<TaffyNode, Vec<TaffyNode>>);

#[derive(Resource, Default, Deref, DerefMut)]
struct TaffyParents(SlotMap<TaffyNode, Option<TaffyNode>>);

#[derive(Resource, Default, Deref, DerefMut)]
struct MeasureFuncs(SparseSecondaryMap<TaffyNode, taffy::node::MeasureFunc>);

#[derive(Resource, Default)]
pub struct WindowTaffyNode {
    taffy_node: TaffyNode,
    previous_physical_size: Vec2,
}

pub fn ui_setup(app: &mut bevy_app::App) {
    app
        .init_resource::<EntityToTaffyMap>()    
        .init_resource::<MeasureFuncs>()
        .init_resource::<TaffyNodes>()
        .init_resource::<TaffyChildren>()
        .init_resource::<TaffyParents>()
        .init_resource::<WindowTaffyNode>();
}

#[derive(Debug)]
pub enum LayoutError {
    InvalidHierarchy,
    TaffyError(taffy::error::TaffyError),
}

// Insert new UI nodes into the UI layout
pub fn insert_ui_nodes_system(
    mut tree: UiLayoutTree,
    mut removed_nodes: RemovedComponents<Node>,
    mut node_keys_query: Query<(Entity, &mut Node)>,
) {
    // clean up removed nodes first
    for entity in removed_nodes.iter() {
        tree.remove_entity(entity);
    }

    for (entity, mut node) in node_keys_query.iter_mut() {
        // Users can only instantiate `Node` components containing a null key
        if node.is_null() {
            node.taffy_node = tree.insert(entity);
        }
    }
}

#[derive(SystemParam)]
pub struct UiLayoutTree<'w, 's> {
    nodes: ResMut<'w, TaffyNodes>,
    entity_to_taffy: ResMut<'w, EntityToTaffyMap>,
    children: ResMut<'w, TaffyChildren>,
    parents: ResMut<'w, TaffyParents>,
    measure_funcs: ResMut<'w, MeasureFuncs>,
    #[system_param(ignore)]
    phantom: PhantomData<fn() -> &'s ()>,
}

impl<'w, 's> taffy::tree::LayoutTree for UiLayoutTree<'w, 's> {
    type ChildIter<'a> = core::slice::Iter<'a, TaffyNode> where Self: 'a;

    fn children(&self, node: TaffyNode) -> Self::ChildIter<'_> {
        self.children[node].iter()
    }

    fn child_count(&self, node: TaffyNode) -> usize {
        self.children[node].len()
    }

    fn is_childless(&self, node: TaffyNode) -> bool {
        self.children[node].is_empty()
    }

    fn parent(&self, node: TaffyNode) -> Option<TaffyNode> {
        self.parents.get(node).copied().flatten()
    }

    fn style(&self, node: TaffyNode) -> &TaffyStyle {
        &self.nodes[node].style
    }

    fn layout(&self, node: TaffyNode) -> &taffy::prelude::Layout {
        &self.nodes[node].layout
    }

    fn layout_mut(&mut self, node: TaffyNode) -> &mut taffy::prelude::Layout {
        &mut self.nodes[node].layout
    }

    #[inline(always)]
    fn mark_dirty(&mut self, node: TaffyNode) -> taffy::error::TaffyResult<()> {
        Ok(self.mark_dirty_internal(node))
    }

    fn measure_node(
        &self,
        node: TaffyNode,
        known_dimensions: Size<Option<f32>>,
        available_space: Size<taffy::style::AvailableSpace>,
    ) -> Size<f32> {
        match &self.measure_funcs[node] {
            MeasureFunc::Raw(measure) => measure(known_dimensions, available_space),
            MeasureFunc::Boxed(measure) => {
                (measure as &dyn Fn(_, _) -> _)(known_dimensions, available_space)
            }
        }
    }

    fn needs_measure(&self, node: TaffyNode) -> bool {
        self.nodes[node].needs_measure && self.measure_funcs.get(node).is_some()
    }

    fn cache_mut(&mut self, node: TaffyNode, index: usize) -> &mut Option<taffy::layout::Cache> {
        &mut self.nodes[node].size_cache[index]
    }

    fn child(&self, node: TaffyNode, id: usize) -> TaffyNode {
        self.children[node][id]
    }
}

impl<'w, 's> UiLayoutTree<'w, 's> {
    fn mark_dirty_internal(&mut self, node: TaffyNode) {
        /// WARNING: this will stack-overflow if the tree contains a cycle
        fn mark_dirty_recursive(
            nodes: &mut TaffyNodes,
            parents: &TaffyParents,
            node_id: TaffyNode,
        ) {
            nodes[node_id].mark_dirty();

            if let Some(Some(node)) = parents.get(node_id) {
                mark_dirty_recursive(nodes, parents, *node);
            }
        }

        mark_dirty_recursive(&mut self.nodes, &self.parents, node);
    }

    fn remove_entity(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_taffy.remove(&entity) {
            self.remove(taffy_node);
        }
    }

    #[inline]
    fn remove(&mut self, taffy_node: TaffyNode) {
        if let Some(parent) = self.parents[taffy_node] {
            if let Some(children) = self.children.get_mut(parent) {
                children.retain(|f| *f != taffy_node);
            }
        }

        let _ = self.children.remove(taffy_node);
        let _ = self.parents.remove(taffy_node);
        let _ = self.nodes.remove(taffy_node);
    }

    fn insert(&mut self, entity: Entity) -> TaffyNode {
        let taffy_node = self.nodes.insert(TaffyNodeData::new(TaffyStyle::default()));
        if let Some(old_taffy_node) = self.entity_to_taffy.insert(entity, taffy_node) {
            self.remove(old_taffy_node);
        } 
        let _ = self.children.insert(Vec::with_capacity(0));
        let _ = self.parents.insert(None);
        taffy_node
    }
}

/// Updates the UI's layout tree, computes the new layout geometry and then updates the sizes and transforms of all the UI nodes.
#[allow(clippy::too_many_arguments)]
pub fn ui_layout_system(
    mut tree: UiLayoutTree,
    mut window_node: ResMut<WindowTaffyNode>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut scale_factor_events: EventReader<WindowScaleFactorChanged>,
    mut window_resized_events: EventReader<bevy_window::WindowResized>,
    mut window_created_events: EventReader<bevy_window::WindowCreated>,
    root_node_query: Query<&Node, Without<Parent>>,
    style_query: Query<(&Node, Ref<Style>)>,
    mut measure_query: Query<(&Node, &mut ContentSize)>,
    children_query: Query<(&Node, Ref<Children>)>,
    mut removed_children: RemovedComponents<Children>,
    mut removed_content_sizes: RemovedComponents<ContentSize>,
    mut node_transform_query: Query<(&Node, &mut NodeSize, &mut Transform)>,
) {
    // When a `ContentSize` component is removed from an entity, we need to remove the measure from the corresponding taffy node.
    for entity in removed_content_sizes.iter() {
        if let Some(taffy_node) = tree.entity_to_taffy.get(&entity).copied() {
            tree.nodes[taffy_node].needs_measure = false;
            tree.measure_funcs.remove(taffy_node);
            tree.mark_dirty_internal(taffy_node);
        }
    }

    // remove children
    for entity in removed_children.iter() {
        if let Some(parent_node) = tree.entity_to_taffy.get(&entity).copied() {
            for child in &tree.children[parent_node] {
                tree.parents[*child] = None;
            }

            tree.children[parent_node] = vec![];
            tree.mark_dirty_internal(parent_node);
        }
    }

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

    let window_changed = window_resized_events
        .iter()
        .any(|resized_window| resized_window.window == primary_window_entity)
        ||
        window_created_events
        .iter()
        .any(|created_window| created_window.window == primary_window_entity);


    // update window root nodes
    if window_node.taffy_node == TaffyNode::default() {
        window_node.taffy_node = tree.nodes.insert(TaffyNodeData::new(TaffyStyle::default()));
    }

    if window_node.previous_physical_size != physical_size
        || window_node.taffy_node == TaffyNode::default()
    {
        tree.nodes[window_node.taffy_node].style = TaffyStyle {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::Points(physical_size.x as f32),
                height: taffy::style::Dimension::Points(physical_size.y as f32),
            },
            ..Default::default()
        };
        tree.mark_dirty_internal(window_node.taffy_node);
    }

    let scale_factor = logical_to_physical_factor * ui_scale.scale;

    let layout_context = LayoutContext::new(scale_factor, physical_size);

    if !scale_factor_events.is_empty() || ui_scale.is_changed() || window_changed {
        scale_factor_events.clear();
        // update all nodes
        for (node, style) in style_query.iter() {
            tree.nodes[node.taffy_node].style = convert::from_style(&layout_context, &style);
            tree.mark_dirty_internal(node.taffy_node);
        }
    } else {
        for (node, style) in style_query.iter() {
            if style.is_changed() {
                tree.nodes[node.taffy_node].style = convert::from_style(&layout_context, &style);
                tree.mark_dirty_internal(node.taffy_node);
            }
        }
    }

    for (&Node { taffy_node }, mut content_size) in measure_query.iter_mut() {
        if let Some(measure_func) = content_size.measure_func.take() {
            tree.measure_funcs.insert(taffy_node, measure_func);
            tree.nodes[taffy_node].needs_measure = true;
            tree.mark_dirty_internal(taffy_node);
        }
    }

    // update window children (for now assuming all Nodes live in the primary window)
    for old_child in &tree.children[window_node.taffy_node] {
        tree.parents[*old_child] = None;
    }

    for &Node { taffy_node } in root_node_query.iter() {
        tree.parents[taffy_node] = Some(window_node.taffy_node);
    }

    let window_children = &mut tree.children[window_node.taffy_node];
    window_children.clear();
    window_children.extend(
        root_node_query
            .iter()
            .map(|&Node { taffy_node }| taffy_node),
    );
    tree.mark_dirty_internal(window_node.taffy_node);

    // update children
    for (&Node { taffy_node: parent }, children) in &children_query {
        if children.is_changed() {
            for child in &tree.children[parent] {
                tree.parents[*child] = None;
            }

            tree.children[parent].clear();

            for child_entity in children.iter() {
                if let Some(taffy_child) = tree.entity_to_taffy.get(child_entity) {
                    tree.parents[*taffy_child] = Some(parent);
                    tree.children[parent].push(*taffy_child);
                }
            }

            tree.mark_dirty_internal(parent);
        }
    }

    // compute layout
    // ui_surface.compute_window_layouts();

    let size_and_baselines = taffy::prelude::layout_flexbox(
        &mut tree,
        window_node.taffy_node,
        taffy::prelude::Size::NONE,
        Size::MAX_CONTENT.into_options(),
        Size::MAX_CONTENT,
        taffy::layout::SizingMode::InherentSize,
    );

    let layout = taffy::prelude::Layout {
        order: 0,
        size: size_and_baselines.size,
        location: taffy::geometry::Point::ZERO,
    };

    tree.nodes[window_node.taffy_node].layout = layout;

    round_layout(&mut tree, window_node.taffy_node, 0., 0.);

    let physical_to_logical_factor = 1. / logical_to_physical_factor;

    let to_logical = |v| (physical_to_logical_factor * v as f64) as f32;

    // PERF: try doing this incrementally
    for (node, mut node_size, mut transform) in &mut node_transform_query {
        let layout = tree.nodes[node.taffy_node].layout;
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

        let taffy_parent = tree.parents[node.taffy_node].unwrap();
        if taffy_parent != window_node.taffy_node {
            let parent_size = tree.nodes[taffy_parent].layout.size;
            new_position.x -= to_logical(parent_size.width / 2.0);
            new_position.y -= to_logical(parent_size.height / 2.0);
        }
        // only trigger change detection when the new value is different
        if transform.translation != new_position {
            transform.translation = new_position;
        }
    }
}

fn round_layout(tree: &mut impl LayoutTree, node: TaffyNode, abs_x: f32, abs_y: f32) {
    let layout = tree.layout_mut(node);
    let abs_x = abs_x + layout.location.x;
    let abs_y = abs_y + layout.location.y;

    layout.location.x = layout.location.x.round();
    layout.location.y = layout.location.y.round();
    layout.size.width = (abs_x + layout.size.width).round() - abs_x.round();
    layout.size.height = (abs_y + layout.size.height).round() - abs_y.round();

    let child_count = tree.child_count(node);
    for index in 0..child_count {
        let child = tree.child(node, index);
        round_layout(tree, child, abs_x, abs_y);
    }
}

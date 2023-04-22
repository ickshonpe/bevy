use super::algorithm;
use super::data::UiNodeData;
use crate::ContentSize;
use crate::UiNodeLayout;
use bevy_derive::Deref;
use bevy_derive::DerefMut;
use bevy_ecs::prelude::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Resource;
use bevy_ecs::system::SystemParam;
use bevy_utils::HashMap;
use slotmap::SlotMap;
use taffy::error::TaffyResult;
use taffy::prelude::Node;
use taffy::style::AvailableSpace;
use taffy::style_helpers::TaffyMaxContent;
use taffy::tree::LayoutTree;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiEntityToNodeMap(HashMap<Entity, Node>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiNodeToEntityMap(SlotMap<Node, Entity>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiNodes(SlotMap<Node, UiNodeData>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiChildNodes(SlotMap<Node, Vec<Node>>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiParentNodes(SlotMap<Node, Option<Node>>);

#[derive(Resource, Default, Deref, DerefMut)]
pub struct UiWindowNode(Node);

#[derive(Resource)]
pub struct UiLayoutConfig {
    pub use_rounding: bool,
}

impl Default for UiLayoutConfig {
    fn default() -> Self {
        Self { use_rounding: true }
    }
}

#[derive(SystemParam)]
pub struct UiLayoutTree<'w, 's> {
    pub config: ResMut<'w, UiLayoutConfig>,
    pub nodes: ResMut<'w, UiNodes>,
    pub children: ResMut<'w, UiChildNodes>,
    pub parents: ResMut<'w, UiParentNodes>,
    pub entity_to_node: ResMut<'w, UiEntityToNodeMap>,
    pub node_to_entity: ResMut<'w, UiNodeToEntityMap>,
    pub window_node: ResMut<'w, UiWindowNode>,
    pub measure_funcs: Query<'w, 's, &'static ContentSize>,
    pub layout: Query<'w, 's, &'static mut UiNodeLayout>,
}

impl<'w, 's> LayoutTree for UiLayoutTree<'w, 's> {
    type ChildIter<'a> =  core::slice::Iter<'a, Node>
    where
        Self: 'a;

    fn children(&self, node: Node) -> Self::ChildIter<'_> {
        self.children[node].iter()
    }

    fn child_count(&self, node: Node) -> usize {
        self.children[node].len()
    }

    fn is_childless(&self, node: Node) -> bool {
        self.children[node].is_empty()
    }

    fn child(&self, node: Node, index: usize) -> Node {
        self.children[node][index]
    }

    fn parent(&self, node: Node) -> Option<Node> {
        self.parents.get(node).copied().flatten()
    }

    fn style(&self, node: Node) -> &taffy::style::Style {
        &self.nodes[node].style
    }

    fn layout(&self, node: Node) -> &taffy::prelude::Layout {
        let entity = self.node_to_entity[node];
        let layout = self.layout.get(entity).unwrap();
        &layout.layout
    }

    fn layout_mut(&mut self, node: Node) -> &mut taffy::prelude::Layout {
        let entity = self.node_to_entity[node];
        let layout = self.layout.get_mut(entity).unwrap();
        &mut layout.into_inner().layout
    }

    fn mark_dirty(&mut self, node: Node) -> taffy::error::TaffyResult<()> {
        self.mark_dirty_internal(node)
    }

    fn measure_node(
        &self,
        node: Node,
        known_dimensions: taffy::prelude::Size<Option<f32>>,
        available_space: taffy::prelude::Size<taffy::style::AvailableSpace>,
    ) -> taffy::prelude::Size<f32> {
        let entity = self.node_to_entity[node];
        let measure_func = &self.measure_funcs.get(entity).unwrap().measure_func;
        match measure_func {
            taffy::node::MeasureFunc::Raw(measure) => measure(known_dimensions, available_space),
            taffy::node::MeasureFunc::Boxed(measure) => {
                (measure as &dyn Fn(_, _) -> _)(known_dimensions, available_space)
            }
        }
    }

    fn needs_measure(&self, node: Node) -> bool {
        self.nodes[node].needs_measure
    }

    fn cache_mut(&mut self, node: Node, index: usize) -> &mut Option<taffy::layout::Cache> {
        &mut self.nodes[node].size_cache[index]
    }
}

impl<'w, 's> UiLayoutTree<'w, 's> {
    fn mark_dirty_internal(&mut self, node: Node) -> TaffyResult<()> {
        /// WARNING: this will stack-overflow if the tree contains a cycle
        fn mark_dirty_recursive(
            nodes: &mut SlotMap<Node, UiNodeData>,
            parents: &SlotMap<Node, Option<Node>>,
            node_id: Node,
        ) {
            nodes[node_id].mark_dirty();

            if let Some(Some(node)) = parents.get(node_id) {
                mark_dirty_recursive(nodes, parents, *node);
            }
        }

        mark_dirty_recursive(&mut self.nodes, &self.parents, node);

        Ok(())
    }

    pub fn compute_layout(
        &mut self,
        node: Node,
        available_space: taffy::prelude::Size<AvailableSpace>,
    ) -> Result<(), taffy::error::TaffyError> {
        algorithm::compute_layout(self, node, available_space)
    }

    pub fn update_node(
        &mut self,
        taffy_node: taffy::node::Node,
        style: &crate::Style,
        context: &crate::LayoutContext,
    ) {
        self.nodes.get_mut(taffy_node).unwrap().style = super::convert::from_style(context, style);
    }

    /// Directly sets the `children` of the supplied `parent`
    pub fn set_children(&mut self, parent: Node, children: &[Node]) -> TaffyResult<()> {
        // Remove node as parent from all its current children.
        for child in &self.children[parent] {
            self.parents[*child] = None;
        }

        // Build up relation node <-> child
        for child in children {
            self.parents[*child] = Some(parent);
        }

        self.children[parent] = children.iter().copied().collect::<_>();

        self.mark_dirty_internal(parent)?;

        Ok(())
    }

    pub fn update_children(
        &mut self,
        parent: taffy::node::Node,
        children: &bevy_hierarchy::Children,
    ) {
        let mut taffy_children = Vec::with_capacity(children.len());
        for child in children {
            if let Some(taffy_node) = self.entity_to_node.get(child) {
                taffy_children.push(*taffy_node);
            } else {
                bevy_log::warn!(
                    "Unstyled child in a UI entity hierarchy. You are using an entity \
without UI components as a child of an entity with UI components, results may be unexpected."
                );
            }
        }

        self.set_children(parent, &taffy_children).unwrap();
    }

    /// Removes children from the entity's taffy node if it exists. Does nothing otherwise.
    pub fn try_remove_children(&mut self, entity: Entity) {
        if let Some(taffy_node) = self.entity_to_node.get(&entity) {
            self.set_children(*taffy_node, &[]).unwrap();
        }
    }

    /// Remove a specific [`Node`] from the tree and drops it
    ///
    /// Returns the id of the node removed.
    pub fn remove(&mut self, node: Node) -> TaffyResult<Node> {
        if let Some(parent) = self.parents[node] {
            if let Some(children) = self.children.get_mut(parent) {
                children.retain(|f| *f != node);
            }
        }

        let _ = self.children.remove(node);
        let _ = self.parents.remove(node);
        let _ = self.nodes.remove(node);

        Ok(node)
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the [`Node`] of the new node
    pub fn new_leaf(&mut self, entity: Entity, layout: taffy::style::Style) -> TaffyResult<Node> {
        let id = self.nodes.insert(UiNodeData::new(layout));
        let _ = self.children.insert(Vec::with_capacity(0));
        let _ = self.parents.insert(None);
        let _ = self.node_to_entity.insert(entity);
        Ok(id)
    }

    /// Sets the [`Style`] of the provided `node`
    pub fn set_style(&mut self, node: Node, style: taffy::style::Style) -> TaffyResult<()> {
        self.nodes[node].style = style;
        self.mark_dirty_internal(node)?;
        Ok(())
    }

    pub fn update_window(&mut self, window_entity: Entity, window_resolution: bevy_math::Vec2) {
        if self.window_node.0 == taffy::node::Node::default() {
            self.window_node.0 = self.new_leaf(window_entity, taffy::style::Style::default()).unwrap();
        }
        self.set_style(
            self.window_node.0,
            taffy::style::Style {
                size: taffy::geometry::Size {
                    width: taffy::style::Dimension::Points(window_resolution.x),
                    height: taffy::style::Dimension::Points(window_resolution.y),
                },
                ..Default::default()
            },
        )
        .unwrap();
    }

    pub fn set_window_children(&mut self, children: impl Iterator<Item = taffy::node::Node>) {
        let child_nodes = children.collect::<Vec<taffy::node::Node>>();
        self.set_children(self.window_node.0, &child_nodes).unwrap();
    }

    pub fn compute_window_layout(&mut self) {
        self.compute_layout(self.window_node.0, taffy::prelude::Size::MAX_CONTENT)
            .unwrap();
    }
}

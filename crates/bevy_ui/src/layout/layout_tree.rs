use bevy_ecs::prelude::Entity;
use bevy_ecs::system::{ResMut, Query, Resource};
use slotmap::SlotMap;
use taffy::error::{self, TaffyError};
use taffy::error::TaffyResult;
use taffy::layout::Cache;
use taffy::prelude::*;

use crate::CalculatedSize;

/// The number of cache entries for each node in the tree
pub(crate) const CACHE_SIZE: usize = 7;

/// Layout information for a given [`Node`](crate::node::Node)
///
/// Stored in a [`Taffy`].
pub(crate) struct NodeData {
    /// The layout strategy used by this node
    pub(crate) style: Style,
    /// The results of the layout computation
    pub(crate) layout: Layout,

    /// Should we try and measure this node?
    pub(crate) needs_measure: bool,

    /// The primary cached results of the layout computation
    pub(crate) size_cache: [Option<Cache>; CACHE_SIZE],
}


pub(crate) struct LayoutConfig {
    /// Whether to round layout values
    pub(crate) use_rounding: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self { use_rounding: true }
    }
}

impl NodeData {
    /// Create the data for a new node
    #[must_use]
    pub const fn new(style: Style) -> Self {
        Self { style, size_cache: [None; CACHE_SIZE], layout: Layout::new(), needs_measure: false }
    }

    /// Marks a node and all of its parents (recursively) as dirty
    ///
    /// This clears any cached data and signals that the data must be recomputed.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.size_cache = [None; CACHE_SIZE];
    }
}

#[derive(Resource)]
pub struct TaffyNodes {
    /// The [`NodeData`] for each node stored in this tree
    pub nodes: SlotMap<Node, NodeData>,

    /// The children of each node
    ///
    /// The indexes in the outer vector correspond to the position of the parent [`NodeData`]
    pub children: SlotMap<Node, ChildrenVec<Node>>,

    /// The parents of each node
    ///
    /// The indexes in the outer vector correspond to the position of the child [`NodeData`]
    pub parents: SlotMap<Node, Option<Node>>,

    pub config: LayoutConfig,

    pub entities: SlotMap<Node, Entity>,
}

#[derive(SystemParam)]
pub struct UiLayout<'w, 's> {
    tree: ResMut<'s, TaffyNodes>,
    measure_query: Query<'w, 's, &'static CalculatedSize>,
}

impl <'w, 's> taffy::tree::LayoutTree for UiLayout<'w, 's> {
    type ChildIter<'a> = core::slice::Iter<'a, DefaultKey>;

    fn children(&self, node: Node) -> Self::ChildIter<'_> {
        self.tree.children[node].iter()
    }

    fn child_count(&self, node: Node) -> usize {
        self.tree.children[node].len()
    }

    fn is_childless(&self, node: Node) -> bool {
        self.tree.children[node].is_empty()
    }

    fn parent(&self, node: Node) -> Option<Node> {
        self.tree.parents.get(node).copied().flatten()
    }

    fn style(&self, node: Node) -> &Style {
        &self.tree.nodes[node].style
    }

    fn layout(&self, node: Node) -> &Layout {
        &self.tree.nodes[node].layout
    }

    fn layout_mut(&mut self, node: Node) -> &mut Layout {
        &mut self.tree.nodes[node].layout
    }

    #[inline(always)]
    fn mark_dirty(&mut self, node: Node) -> TaffyResult<()> {
        self.mark_dirty_internal(node)
    }

    fn measure_node(
        &self,
        node: Node,
        known_dimensions: Size<Option<f32>>,
        available_space: Size<AvailableSpace>,
    ) -> Size<f32> {
        let entity = self.tree.entities[node];
        let calculated_size: &CalculatedSize = self.measure_query.get(entity).unwrap();
        let size = calculated_size.measure.measure(
            known_dimensions.width,
            known_dimensions.height,
            available_space.width,
            available_space.height
        );
        Size {
            width: size.x,
            height: size.y,
        }
    }

    fn needs_measure(&self, node: Node) -> bool {
        self.tree.nodes[node].needs_measure 
        && 
        self.measure_query.contains(self.tree.entities[node])
    }

    fn cache_mut(&mut self, node: Node, index: usize) -> &mut Option<Cache> {
        &mut self.tree.nodes[node].size_cache[index]
    }

    fn child(&self, node: Node, id: usize) -> Node {
        self.tree.children[node][id]
    }
}

impl UiLayout {
    /// Enable rounding of layout values. Rounding is enabled by default.
    pub fn enable_rounding(&mut self) {
        self.tree.config.use_rounding = true;
    }

    /// Disable rounding of layout values. Rounding is enabled by default.
    pub fn disable_rounding(&mut self) {
        self.tree.config.use_rounding = false;
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the [`Node`] of the new node
    pub fn new_leaf(&mut self, layout: Style) -> TaffyResult<Node> {
        let id = self.tree.nodes.insert(NodeData::new(layout));
        let _ = self.tree.children.insert(Vec::with_capacity(0));
        let _ = self.tree.parents.insert(None);

        Ok(id)
    }

    /// Creates and adds a new node, which may have any number of `children`
    pub fn new_with_children(&mut self, layout: Style, children: &[Node]) -> TaffyResult<Node> {
        let id = self.tree.nodes.insert(NodeData::new(layout));

        for child in children {
            self.tree.parents[*child] = Some(id);
        }

        let _ = self.tree.children.insert(children.iter().copied().collect::<_>());
        let _ = self.tree.parents.insert(None);

        Ok(id)
    }

    /// Drops all nodes in the tree
    pub fn clear(&mut self) {
        self.tree.nodes.clear();
        self.tree.children.clear();
        self.tree.parents.clear();
    }

    /// Remove a specific [`Node`] from the tree and drops it
    ///
    /// Returns the id of the node removed.
    pub fn remove(&mut self, node: Node) -> TaffyResult<Node> {
        if let Some(parent) = self.tree.parents[node] {
            if let Some(children) = self.tree.children.get_mut(parent) {
                children.retain(|f| *f != node);
            }
        }

        let _ = self.tree.children.remove(node);
        let _ = self.tree.parents.remove(node);
        let _ = self.tree.nodes.remove(node);

        Ok(node)
    }

    /// Sets the [`MeasureFunc`] of the associated node
    pub fn set_measure(&mut self, node: Node) -> TaffyResult<()> {
        // if let Some(measure) = measure {
        //     self.tree.nodes[node].needs_measure = true;
        //     self.measure_funcs.insert(node, measure);
        // } else {
        //     self.tree.nodes[node].needs_measure = false;
        //     self.measure_funcs.remove(node);
        // }
        self.tree.nodes[node].needs_measure = true;
        self.mark_dirty_internal(node)?;

        Ok(())
    }

    /// Adds a `child` [`Node`] under the supplied `parent`
    pub fn add_child(&mut self, parent: Node, child: Node) -> TaffyResult<()> {
        self.tree.parents[child] = Some(parent);
        self.tree.children[parent].push(child);
        self.mark_dirty_internal(parent)?;

        Ok(())
    }

    /// Directly sets the `children` of the supplied `parent`
    pub fn set_children(&mut self, parent: Node, children: &[Node]) -> TaffyResult<()> {
        // Remove node as parent from all its current children.
        for child in &self.tree.children[parent] {
            self.tree.parents[*child] = None;
        }

        // Build up relation node <-> child
        for child in children {
            self.tree.parents[*child] = Some(parent);
        }

        self.tree.children[parent] = children.iter().copied().collect::<_>();

        self.mark_dirty_internal(parent)?;

        Ok(())
    }

    /// Removes the `child` of the parent `node`
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn remove_child(&mut self, parent: Node, child: Node) -> TaffyResult<Node> {
        let index = self.tree.children[parent].iter().position(|n| *n == child).unwrap();
        self.remove_child_at_index(parent, index)
    }

    /// Removes the child at the given `index` from the `parent`
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn remove_child_at_index(&mut self, parent: Node, child_index: usize) -> TaffyResult<Node> {
        let child_count = self.tree.children[parent].len();
        if child_index >= child_count {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count });
        }

        let child = self.tree.children[parent].remove(child_index);
        self.tree.parents[child] = None;

        self.mark_dirty_internal(parent)?;

        Ok(child)
    }

    /// Replaces the child at the given `child_index` from the `parent` node with the new `child` node
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    pub fn replace_child_at_index(&mut self, parent: Node, child_index: usize, new_child: Node) -> TaffyResult<Node> {
        let child_count = self.tree.children[parent].len();
        if child_index >= child_count {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count });
        }

        self.tree.parents[new_child] = Some(parent);
        let old_child = core::mem::replace(&mut self.tree.children[parent][child_index], new_child);
        self.tree.parents[old_child] = None;

        self.mark_dirty_internal(parent)?;

        Ok(old_child)
    }

    /// Returns the child [`Node`] of the parent `node` at the provided `child_index`
    pub fn child_at_index(&self, parent: Node, child_index: usize) -> TaffyResult<Node> {
        let child_count = self.tree.children[parent].len();
        if child_index >= child_count {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count });
        }

        Ok(self.tree.children[parent][child_index])
    }

    /// Returns the number of children of the `parent` [`Node`]
    pub fn child_count(&self, parent: Node) -> TaffyResult<usize> {
        Ok(self.tree.children[parent].len())
    }

    /// Returns a list of children that belong to the parent [`Node`]
    pub fn children(&self, parent: Node) -> TaffyResult<Vec<Node>> {
        Ok(self.tree.children[parent].iter().copied().collect::<_>())
    }

    /// Sets the [`Style`] of the provided `node`
    pub fn set_style(&mut self, node: Node, style: Style) -> TaffyResult<()> {
        self.tree.nodes[node].style = style;
        self.mark_dirty_internal(node)?;
        Ok(())
    }

    /// Gets the [`Style`] of the provided `node`
    pub fn style(&self, node: Node) -> TaffyResult<&Style> {
        Ok(&self.tree.nodes[node].style)
    }

    /// Return this node layout relative to its parent
    pub fn layout(&self, node: Node) -> TaffyResult<&Layout> {
        Ok(&self.tree.nodes[node].layout)
    }

    /// Marks the layout computation of this node and its children as outdated
    ///
    /// Performs a recursive depth-first search up the tree until the root node is reached
    ///
    /// WARNING: this will stack-overflow if the tree contains a cycle
    fn mark_dirty_internal(&mut self, node: Node) -> TaffyResult<()> {
        /// WARNING: this will stack-overflow if the tree contains a cycle
        fn mark_dirty_recursive(
            nodes: &mut SlotMap<Node, NodeData>,
            parents: &SlotMap<Node, Option<Node>>,
            node_id: Node,
        ) {
            nodes[node_id].mark_dirty();

            if let Some(Some(node)) = parents.get(node_id) {
                mark_dirty_recursive(nodes, parents, *node);
            }
        }

        mark_dirty_recursive(&mut self.tree.nodes, &self.tree.parents, node);

        Ok(())
    }

    /// Indicates whether the layout of this node (and its children) need to be recomputed
    pub fn dirty(&self, node: Node) -> TaffyResult<bool> {
        Ok(self.tree.nodes[node].size_cache.iter().all(|entry| entry.is_none()))
    }

    /// Updates the stored layout of the provided `node` and its children
    pub fn compute_layout(&mut self, node: Node, available_space: Size<AvailableSpace>) -> Result<(), TaffyError> {
        
        let size_and_baselines = taffy::compute::GenericAlgorithm::perform_layout(
            self,
            root,
            Size::NONE,
            available_space.into_options(),
            available_space,
            SizingMode::InherentSize,
        );
    
        let layout = Layout { order: 0, size: size_and_baselines.size, location: taffy::geometry::Point::ZERO };
        *self.layout_mut(root) = layout;
    
        // If rounding is enabled, recursively round the layout's of this node and all children
        if self.tree.config.use_rounding {
            round_layout(taffy, root, 0.0, 0.0);
        }
    
        Ok(())
    }
}
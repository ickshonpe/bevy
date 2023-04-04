//! UI [`Node`] types and related data structures.
//!
//! Layouts are composed of multiple nodes, which live in a tree-like data structure.
use core::iter::Empty;
use std::process::Child;


use bevy_derive::{Deref, DerefMut};
use bevy_ecs::component::StorageType;
use bevy_ecs::storage::Table;
use bevy_ecs::system::CommandQueue;
use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;

/// A node in a layout.
pub type Node = Entity;

pub type Taffy = World;

use crate::data::CACHE_SIZE;
use crate::error::{TaffyError, TaffyResult};
use crate::geometry::Size;
use crate::layout::{Cache, Layout};

use crate::style::{AvailableSpace, Style};
#[cfg(any(feature = "std", feature = "alloc"))]
use crate::sys::Box;
use crate::sys::{new_vec_with_capacity, ChildrenVec, Vec};
use crate::error;
use crate::tree::LayoutTree;

/// A function type that can be used in a [`MeasureFunc`]
///
/// This trait is automatically implemented for all types (including closures) that define a function with the appropriate type signature.
pub trait Measurable: Send + Sync + Fn(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32> {}
impl<F: Send + Sync + Fn(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>> Measurable for F {}

/// A function that can be used to compute the intrinsic size of a node
#[derive(Component)]
pub enum MeasureFunc {
    /// Stores an unboxed function
    Raw(fn(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>),

    /// Stores a boxed function
    #[cfg(any(feature = "std", feature = "alloc"))]
    Boxed(Box<dyn Measurable>),
}

/// Global configuration values for a Taffy instance
#[derive(Resource)]
pub(crate) struct TaffyConfig {
    /// Whether to round layout values
    pub(crate) use_rounding: bool,
}

impl Default for TaffyConfig {
    fn default() -> Self {
        Self { use_rounding: true }
    }
}

/// needs a measure?
#[derive(Component)]
pub struct NeedsMeasure(pub bool);

const EMPTY_SLICE: &'static [Entity] = &[];


/// cached size layout info something
#[derive(Component, Default, Deref, DerefMut)]
pub struct SizeCache(pub [Option<Cache>; CACHE_SIZE]);

impl LayoutTree for World {
    type ChildIter<'a> = core::slice::Iter<'a, Entity>;

    fn children(&self, node: Entity) -> Self::ChildIter<'_> {
        self.get::<Children>(node)
            .map(|children| children.iter())
            .unwrap_or_else(|| { EMPTY_SLICE.iter() })
    }

    fn child_count(&self, node: Node) -> usize {
        self.get::<Children>(node).map(|children| children.iter().count()).unwrap_or(0)
    }

    fn is_childless(&self, node: Node) -> bool {
        if let Some(children) = self.get::<Children>(node) {
            children.is_empty()
        } else {
            return true;
        }
    }

    fn parent(&self, node: Node) -> Option<Node> {
        self.get::<Parent>(node).map(|parent| parent.get())
    }

    fn style(&self, node: Node) -> &Style {
        self.get::<Style>(node).unwrap()
    }

    fn layout(&self, node: Node) -> &Layout {
        self.get::<Layout>(node).unwrap()
    }

    fn layout_mut(&mut self, node: Node) -> Mut<'_, Layout> {
        self.get_mut::<Layout>(node).unwrap()
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
        match self.get::<MeasureFunc>(node).unwrap()
        {
            MeasureFunc::Raw(measure) => measure(known_dimensions, available_space),

            #[cfg(any(feature = "std", feature = "alloc"))]
            MeasureFunc::Boxed(measure) => (measure as &dyn Fn(_, _) -> _)(known_dimensions, available_space),
        }
    }

    fn needs_measure(&self, node: Node) -> bool {
        self.get::<NeedsMeasure>(node).unwrap().0 
        && self.entity(node).contains::<MeasureFunc>()
    }

    fn cache_mut(&mut self, node: Node) -> Mut<'_, SizeCache> {
        self.get_mut::<SizeCache>(node).unwrap()
    }

    fn child(&self, node: Node, id: usize) -> Node {
        self.get::<Children>(node).unwrap()[id]
    }
}

pub fn init_ui(world: &mut World) {
    world.init_resource::<TaffyConfig>();
}

pub trait TaffyWorld {
    fn world(&self) -> &World;
    fn world_mut(&mut self) -> &mut World;

    fn enable_rounding(&mut self) {
        self.world_mut().get_resource_mut::<TaffyConfig>().unwrap().use_rounding = true;
    }

    fn style(&self, node: Node) -> TaffyResult<&Style> {
        Ok(self.world().get::<Style>(node).unwrap())
    }

    fn disable_rounding(&mut self) {
        self.world_mut().get_resource_mut::<TaffyConfig>().unwrap().use_rounding = false;
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the [`Node`] of the new node
    fn new_leaf(&mut self, style: Style) -> TaffyResult<Node> {
        Ok(self.world_mut().spawn((
            style,
            Layout::new(),
            NeedsMeasure(false),
            SizeCache::default(),
        )).id())
    }

    /// Creates and adds a new unattached leaf node to the tree, and returns the [`Node`] of the new node
    ///
    /// Creates and adds a new leaf node with a supplied [`MeasureFunc`]
    fn new_leaf_with_measure(&mut self, style: Style, measure: MeasureFunc) -> TaffyResult<Node> {
        Ok(self.world_mut().spawn((
            style,
            Layout::new(),
            NeedsMeasure(true),
            SizeCache::default(),
            measure,
        )).id())
    }

    /// Creates and adds a new node, which may have any number of `children`
    fn new_with_children(&mut self, style: Style, children: &[Node]) -> TaffyResult<Node> {
        Ok(self.world_mut().spawn((
            style,
            Layout::new(),
            NeedsMeasure(false),
            SizeCache::default(),
        )).push_children(children).id())
    }
        
    /// Drops all nodes in the tree
    fn clear_nodes(&mut self) {
        let mut query = self.world_mut().query_filtered::<Entity, With<Layout>>();
        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &self.world());
        for entity in query.iter(&self.world()) {
            commands.entity(entity)
            .remove::<(
                Style,
                Layout,
                SizeCache,
                MeasureFunc,
                NeedsMeasure,
            )>();
            commands.entity(entity).remove_parent().clear_children();
        }
    
        command_queue.apply(self.world_mut());        
    }

    /// Remove a specific [`Node`] from the tree and drops it
    ///
    /// Returns the id of the node removed.
    fn remove(&mut self, node: Node) -> TaffyResult<Node> {
        let mut entity_mut = self.world_mut()
            .entity_mut(node);
        entity_mut
            .remove::<(
                Style,
                Layout,
                NeedsMeasure,
                SizeCache,
                MeasureFunc,
            )>();
        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &self.world());
        commands.entity(node).remove_parent().clear_children();
        command_queue.apply(self.world_mut());
        Ok(node)
    }

    /// Sets the [`MeasureFunc`] of the associated node
    fn set_measure(&mut self, node: Node, measure: Option<MeasureFunc>) -> TaffyResult<()> {
        let mut entity_mut = self.world_mut().entity_mut(node);
        if let Some(measure) = measure {
            entity_mut.insert(measure);
            entity_mut.get_mut::<NeedsMeasure>().unwrap().0 = true;
        } else {
            entity_mut.remove::<MeasureFunc>();
            entity_mut.get_mut::<NeedsMeasure>().unwrap().0 = false;
        }

        self.mark_dirty_internal(node)?;

        Ok(())
    }

    /// Adds a `child` [`Node`] under the supplied `parent`
    fn add_child(&mut self, parent: Node, child: Node) -> TaffyResult<()> {
        self.world_mut().entity_mut(parent).add_child(child);
        self.mark_dirty_internal(parent)?;

        Ok(())
    }

    /// Directly sets the `children` of the supplied `parent`
    fn set_children(&mut self, parent: Node, children: &[Node]) -> TaffyResult<()> {
        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &self.world());
        commands.entity(parent).replace_children(children);
        command_queue.apply(self.world_mut());
        self.mark_dirty_internal(parent)?;

        Ok(())
    }

    /// Removes the `child` of the parent `node`
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    fn remove_child(&mut self, parent: Node, child: Node) -> TaffyResult<Node> {
        self.world_mut().entity_mut(parent).remove_children(&[child]);
        self.mark_dirty_internal(parent)?;
        Ok(child)
    }

    /// Replaces the child at the given `child_index` from the `parent` node with the new `child` node
    ///
    /// The child is not removed from the tree entirely, it is simply no longer attached to its previous parent.
    fn replace_child_at_index(&mut self, parent: Node, child_index: usize, new_child: Node) -> TaffyResult<Node> {
        let mut command_queue = CommandQueue::default();
        let mut commands = Commands::new(&mut command_queue, &self.world());
        let mut children = self.world().entity(parent).get::<Children>().unwrap().iter().copied().collect::<Vec<_>>();
        let child_count = children.len();
        if child_index >= child_count {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count });
        }
        let old_child = children[child_index];
        children[child_index] = new_child;
        commands.entity(parent).replace_children(&children);
        command_queue.apply(self.world_mut());

        self.mark_dirty_internal(parent)?;

        Ok(old_child)
    }

    fn remove_child_at_index(&mut self, parent: Node, child_index: usize) -> TaffyResult<Node> {
        let Ok(children) = self.children(parent) else {
            return Err(error::TaffyError::ChildIndexOutOfBounds  { parent, child_index, child_count: 0 });
        };
        if child_index >= children.len() {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count: children.len() });
        }

        let child = children[child_index];
        self.world_mut().entity_mut(parent).remove_children(&[child]);
        self.mark_dirty_internal(parent)?;
        Ok(child)
    }

    /// Returns the child [`Node`] of the parent `node` at the provided `child_index`
    fn child_at_index(&self, parent: Node, child_index: usize) -> TaffyResult<Node> {
        let children = self.world().entity(parent).get::<Children>().unwrap();
        let child_count = children.len();
        if child_index >= child_count {
            return Err(error::TaffyError::ChildIndexOutOfBounds { parent, child_index, child_count });
        }

        Ok(children.iter().nth(child_index).unwrap().clone())
    }

    /// Returns the number of children of the `parent` [`Node`]
    fn child_count(&self, parent: Node) -> TaffyResult<usize> {
        Ok(
            if let Some(children) = self.world().entity(parent).get::<Children>() {
                children.len()
            } else {
                0
            }
        )
    }

    /// Returns a list of children that belong to the parent [`Node`]
    fn children_list(&self, parent: Node) -> TaffyResult<Vec<Node>> {
        Ok(self.world().get::<Children>(parent).unwrap().iter().copied().collect::<Vec<_>>())        
    }

    /// Sets the [`Style`] of the provided `node`
    fn set_style(&mut self, node: Node, style: Style) -> TaffyResult<()> {
        self.world_mut().entity_mut(node).insert(style);
        self.mark_dirty_internal(node)?;
        Ok(())
    }

    /// Returns the [`Style`] of the provided `node`
    fn layout(&self, node: Node) -> TaffyResult<&Layout> {
        Ok(self.world().entity(node).get::<Layout>().unwrap())
    }

    /// Returns the kids of the provided `node`
    fn children(&self, parent: Node) -> TaffyResult<Vec<Node>> {
        if let Some(children) = self.world().entity(parent).get::<Children>() {
            Ok(children.iter().copied().collect::<Vec<_>>())
        } else {
            Ok(vec![])
        }
    }

    /// Marks the layout computation of this node and its children as outdated
    ///
    /// Performs a recursive depth-first search up the tree until the root node is reached
    ///
    /// WARNING: this will stack-overflow if the tree contains a cycle
    fn mark_dirty_internal(&mut self, node: Node) -> TaffyResult<()> {
        // WARNING: this will stack-overflow if the tree contains a cycle
        let query = self.world_mut().query::<(&mut SizeCache, Option<&Parent>)>();
        fn mark_dirty_recursive(
            world: &mut World,
            mut dirty_query: QueryState<(&mut SizeCache, Option<&Parent>)>,
            node_id: Node,
        ) {
           if let Ok((mut cache, parent)) = dirty_query.get_mut(world, node_id) {
                *cache = SizeCache::default();
                if let Some(parent) = parent {
                    let parent_id = parent.get();
                    mark_dirty_recursive(world, dirty_query, parent_id);
                }
            }
        }

        mark_dirty_recursive(&mut self.world_mut(), query, node);

        Ok(())
    }

    /// Indicates whether the layout of this node (and its children) need to be recomputed
    fn dirty(&self, node: Node) -> TaffyResult<bool> {
        Ok(self.world().get::<SizeCache>(node).unwrap().0.iter().all(|entry| entry.is_none()))
    }

    /// Updates the stored layout of the provided `node` and its children
    fn compute_layout(&mut self, node: Node, available_space: Size<AvailableSpace>) -> Result<(), TaffyError> {
        crate::compute::compute_layout(self.world_mut(), node, available_space)
    }
}

impl TaffyWorld for World {
    #[inline(always)]
    fn world(&self) -> &World {
        self
    }

    #[inline(always)]
    fn world_mut(&mut self) -> &mut World {
        self
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::bool_assert_comparison)]

    use super::TaffyWorld;
    use crate::node::{Taffy, MeasureFunc, Node, init_ui};
    use crate::prelude::Size;
    use crate::style::{Dimension, Display, FlexDirection, Style, AvailableSpace};
    use crate::style_helpers::*;
    use crate::sys;
    

    #[test]
    fn test_new_leaf() {
        let mut taffy = Taffy::new();

        let res = taffy.new_leaf(Style::default());
        assert!(res.is_ok());
        let node = res.unwrap();

        // node should be in the taffy tree and have no children
        assert!(taffy.child_count(node).unwrap() == 0);
    }

    #[test]
    fn new_leaf_with_measure() {
        let mut taffy = Taffy::new();

        let res = taffy.new_leaf_with_measure(Style::default(), MeasureFunc::Raw(|_, _| Size::ZERO));
        assert!(res.is_ok());
        let node = res.unwrap();

        // node should be in the taffy tree and have no children
        assert!(taffy.child_count(node).unwrap() == 0);
    }

    /// Test that new_with_children works as expected
    #[test]
    fn test_new_with_children() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        // node should have two children
        assert_eq!(taffy.child_count(node).unwrap(), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child0);
        assert_eq!(taffy.children(node).unwrap()[1], child1);
    }

    #[test]
    fn remove_node_should_remove() {
        let mut taffy = Taffy::new();

        let node = taffy.new_leaf(Style::default()).unwrap();

        let _ = taffy.remove(node).unwrap();
    }

    #[test]
    fn remove_node_should_detach_herarchy() {
        let mut taffy = Taffy::new();

        // Build a linear tree layout: <0> <- <1> <- <2>
        let node2 = taffy.new_leaf(Style::default()).unwrap();
        let node1 = taffy.new_with_children(Style::default(), &[node2]).unwrap();
        let node0 = taffy.new_with_children(Style::default(), &[node1]).unwrap();

        // Both node0 and node1 should have 1 child nodes
        assert_eq!(taffy.children(node0).unwrap().as_slice(), &[node1]);
        assert_eq!(taffy.children(node1).unwrap().as_slice(), &[node2]);

        // Disconnect the tree: <0> <2>
        let _ = taffy.remove(node1).unwrap();

        // Both remaining nodes should have no child nodes
        assert!(taffy.children(node0).unwrap().is_empty());
        assert!(taffy.children(node2).unwrap().is_empty());
    }

    #[test]
    fn remove_last_node() {
        let mut taffy = Taffy::new();

        let parent = taffy.new_leaf(Style::default()).unwrap();
        let child = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(parent, child).unwrap();

        taffy.remove(child).unwrap();
        taffy.remove(parent).unwrap();
    }

    #[test]
    fn set_measure_x() {
        let mut taffy = Taffy::new();
        init_ui(&mut taffy);
        let node = taffy
            .new_leaf_with_measure(Style::default(), MeasureFunc::Raw(|_, _| {
                Size { width: 200.0, height: 200.0 }
            }))
            .unwrap();

        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 200.0);

        taffy.set_measure(node, Some(MeasureFunc::Raw(|_, _| Size { width: 100.0, height: 100.0 }))).unwrap();
        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 100.0);
    }

    #[test]
    fn set_measure_of_previously_unmeasured_node() {
        let mut taffy = Taffy::new();
        init_ui(&mut taffy);
        let node = taffy.new_leaf(Style::default()).unwrap();
        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 0.0);

        taffy.set_measure(node, Some(MeasureFunc::Raw(|_, _| Size { width: 100.0, height: 100.0 }))).unwrap();
        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        assert_eq!(taffy.layout(node).unwrap().size.width, 100.0);
    }

    /// Test that adding `add_child()` works
    #[test]
    fn add_child() {
        let mut taffy = Taffy::new();
        let node = taffy.new_leaf(Style::default()).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 0);

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(node, child0).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 1);

        let child1 = taffy.new_leaf(Style::default()).unwrap();
        taffy.add_child(node, child1).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 2);
    }

    #[test]
    fn set_children() {
        let mut taffy = Taffy::new();

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        assert_eq!(taffy.child_count(node).unwrap(), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child0);
        assert_eq!(taffy.children(node).unwrap()[1], child1);

        let child2 = taffy.new_leaf(Style::default()).unwrap();
        let child3 = taffy.new_leaf(Style::default()).unwrap();
        taffy.set_children(node, &[child2, child3]).unwrap();

        assert_eq!(taffy.child_count(node).unwrap(), 2);
        assert_eq!(taffy.children(node).unwrap()[0], child2);
        assert_eq!(taffy.children(node).unwrap()[1], child3);
    }

    /// Test that removing a child works
    #[test]
    fn remove_child() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        assert_eq!(taffy.child_count(node).unwrap(), 2);

        taffy.remove_child(node, child0).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);

        taffy.remove_child(node, child1).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 0);
    }

    #[test]
    fn remove_child_at_index() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        assert_eq!(taffy.child_count(node).unwrap(), 2);

        taffy.remove_child_at_index(node, 0).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);

        taffy.remove_child_at_index(node, 0).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 0);
    }

    #[test]
    fn replace_child_at_index() {
        let mut taffy = Taffy::new();

        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();

        let node = taffy.new_with_children(Style::default(), &[child0]).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child0);

        taffy.replace_child_at_index(node, 0, child1).unwrap();
        assert_eq!(taffy.child_count(node).unwrap(), 1);
        assert_eq!(taffy.children(node).unwrap()[0], child1);
    }
    #[test]
    fn test_child_at_index() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let child2 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1, child2]).unwrap();

        assert!(if let Ok(result) = taffy.child_at_index(node, 0) { result == child0 } else { false });
        assert!(if let Ok(result) = taffy.child_at_index(node, 1) { result == child1 } else { false });
        assert!(if let Ok(result) = taffy.child_at_index(node, 2) { result == child2 } else { false });
    }
    #[test]
    fn test_child_count() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        assert!(if let Ok(count) = taffy.child_count(node) { count == 2 } else { false });
        assert!(if let Ok(count) = taffy.child_count(child0) { count == 0 } else { false });
        assert!(if let Ok(count) = taffy.child_count(child1) { count == 0 } else { false });
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn test_children() {
        let mut taffy = Taffy::new();
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        let mut children: sys::Vec<Node> = sys::Vec::new();
        children.push(child0);
        children.push(child1);

        let children_result = taffy.children(node).unwrap();
        assert_eq!(children_result, children);

        assert!(taffy.children(child0).unwrap().is_empty());
    }
    #[test]
    fn test_set_style() {
        let mut taffy = Taffy::new();

        let node = taffy.new_leaf(Style::default()).unwrap();
        assert_eq!(taffy.style(node).unwrap().display, Display::Flex);

        taffy.set_style(node, Style { display: Display::None, ..Style::default() }).unwrap();
        assert_eq!(taffy.style(node).unwrap().display, Display::None);
    }
    #[test]
    fn test_style() {
        let mut taffy = Taffy::new();

        let style = Style { display: Display::None, flex_direction: FlexDirection::RowReverse, ..Default::default() };

        let node = taffy.new_leaf(style.clone()).unwrap();

        let res = taffy.style(node);
        assert!(res.is_ok());
        assert!(res.unwrap() == &style);
    }
    #[test]
    fn test_layout() {
        let mut taffy = Taffy::new();
        let node = taffy.new_leaf(Style::default()).unwrap();

        // TODO: Improve this test?
        let res = taffy.layout(node);
        assert!(res.is_ok());
    }

    #[test]
    fn test_mark_dirty() {        
        let mut taffy = Taffy::new();
        init_ui(&mut taffy);
        let child0 = taffy.new_leaf(Style::default()).unwrap();
        let child1 = taffy.new_leaf(Style::default()).unwrap();
        let node = taffy.new_with_children(Style::default(), &[child0, child1]).unwrap();

        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();

        assert_eq!(taffy.dirty(child0).unwrap(), false);
        assert_eq!(taffy.dirty(child1).unwrap(), false);
        assert_eq!(taffy.dirty(node).unwrap(), false);

        crate::tree::LayoutTree::mark_dirty(&mut taffy, node).unwrap();
        assert_eq!(taffy.dirty(child0).unwrap(), false);
        assert_eq!(taffy.dirty(child1).unwrap(), false);
        assert_eq!(taffy.dirty(node).unwrap(), true);

        taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
        crate::tree::LayoutTree::mark_dirty(&mut taffy, child0).unwrap();
        assert_eq!(taffy.dirty(child0).unwrap(), true);
        assert_eq!(taffy.dirty(child1).unwrap(), false);
        assert_eq!(taffy.dirty(node).unwrap(), true);
    }

    #[test]
    fn compute_layout_should_produce_valid_result() {
        let mut taffy = Taffy::new();
        init_ui(&mut taffy);
        let node_result = taffy.new_leaf(Style {
            size: Size { width: Dimension::Points(10f32), height: Dimension::Points(10f32) },
            ..Default::default()
        });
        assert!(node_result.is_ok());
        let node = node_result.unwrap();
        let layout_result = taffy.compute_layout(
            node,
            Size { width: AvailableSpace::Definite(100.), height: AvailableSpace::Definite(100.) },
        );
        assert!(layout_result.is_ok());
    }

    #[test]
    fn measure_func_is_send_and_sync() {
        fn is_send_and_sync<T: Send + Sync>() {}
        is_send_and_sync::<MeasureFunc>();
    }
}

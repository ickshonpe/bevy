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
use taffy::prelude::Size;
use taffy::style::AvailableSpace;
use taffy::tree::LayoutTree;

use crate::ContentSize;

use super::data::UiNodeData;

#[derive(Resource, Deref, DerefMut)]
pub struct UiEntityToNodeMap(HashMap<Entity, Node>);

#[derive(Resource, Deref, DerefMut)]
pub struct UiNodeToEntityMap(HashMap<Node, Entity>);

#[derive(Resource, Deref, DerefMut)]
pub struct UiNodes(SlotMap<Node, UiNodeData>);

#[derive(Resource, Deref, DerefMut)]
pub struct UiChildNodes(SlotMap<Node, Vec<Node>>);

#[derive(Resource, Deref, DerefMut)]
pub struct UiParentNodes(SlotMap<Node, Option<Node>>);

#[derive(Resource, Deref, DerefMut)]
pub struct UiWindowNodes(SlotMap<Node, Option<Node>>);

#[derive(Resource)]
pub struct UiLayoutConfig {
    pub use_rounding: bool,
}

#[derive(SystemParam)]
pub struct UiLayoutTree<'w, 's> {
    pub config: ResMut<'w, UiLayoutConfig>,
    pub nodes: ResMut<'w, UiNodes>,
    pub children: ResMut<'w, UiChildNodes>,
    pub parents: ResMut<'w, UiParentNodes>,
    pub entity_to_node: ResMut<'w, UiEntityToNodeMap>,
    pub node_to_entity: ResMut<'w, UiNodeToEntityMap>,
    pub measure_funcs: Query<'w, 's, &'static ContentSize>,
    
}

impl <'w, 's> LayoutTree for UiLayoutTree<'w, 's> {
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
        &self.nodes[node].layout
    }

    fn layout_mut(&mut self, node: Node) -> &mut taffy::prelude::Layout {
        &mut self.nodes[node].layout
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
        let entity = self.node_to_entity.get(&node).unwrap();
        let measure_func = self.measure_funcs.get(*entity).unwrap().measure_func.as_ref().unwrap();
        match measure_func {
            taffy::node::MeasureFunc::Raw(measure) => measure(known_dimensions, available_space),
            taffy::node::MeasureFunc::Boxed(measure) => (measure as &dyn Fn(_, _) -> _)(known_dimensions, available_space),
        }            
    }

    fn needs_measure(&self, node: Node) -> bool {
        self.nodes[node].needs_measure
    }

    fn cache_mut(&mut self, node: Node, index: usize) -> &mut Option<taffy::layout::Cache> {
        &mut self.nodes[node].size_cache[index]
    }
}

impl <'w, 's> UiLayoutTree<'w, 's> {
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

    fn compute_layout(&mut self, node: Node, available_space: Size<AvailableSpace>) -> Result<(), taffy::error::TaffyError> {
        Ok(())
    }
}
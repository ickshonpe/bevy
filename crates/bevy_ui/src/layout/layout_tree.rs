use bevy_ecs::prelude::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Resource;
use bevy_ecs::system::SystemParam;
use bevy_utils::HashMap;
use slotmap::SlotMap;
use taffy::prelude::Node;
use taffy::tree::LayoutTree;

use crate::CalculatedSize;

use super::data::UiNodeData;

#[derive(Resource)]
pub struct UiEntityToNodeMap(HashMap<Entity, Node>);

#[derive(Resource)]
pub struct UiChildNodes(SlotMap<Node, Vec<Node>>);

#[derive(Resource)]
pub struct UiParentNodes(SlotMap<Node, Vec<Node>>);

#[derive(Resource)]
pub struct UiNodes(SlotMap<Node, UiNodeData>);

#[derive(SystemParam)]
pub struct UiSurface<'w, 's> {
    children: ResMut<'w, UiChildNodes>,
    parents: ResMut<'w, UiParentNodes>,
    measure_funcs: Query<'w, 's, &'static CalculatedSize>,
}

impl <'w, 's> LayoutTree for UiSurface<'w, 's> {
    type ChildIter<'a> =  core::slice::Iter<'a, Node>
    where
        Self: 'a;

    fn children(&self, node: Node) -> Self::ChildIter<'_> {
        todo!()
    }

    fn child_count(&self, node: Node) -> usize {
        todo!()
    }

    fn is_childless(&self, node: Node) -> bool {
        todo!()
    }

    fn child(&self, node: Node, index: usize) -> Node {
        todo!()
    }

    fn parent(&self, node: Node) -> Option<Node> {
        todo!()
    }

    fn style(&self, node: Node) -> &taffy::style::Style {
        todo!()
    }

    fn layout(&self, node: Node) -> &taffy::prelude::Layout {
        todo!()
    }

    fn layout_mut(&mut self, node: Node) -> &mut taffy::prelude::Layout {
        todo!()
    }

    fn mark_dirty(&mut self, node: Node) -> taffy::error::TaffyResult<()> {
        todo!()
    }

    fn measure_node(
        &self,
        node: Node,
        known_dimensions: taffy::prelude::Size<Option<f32>>,
        available_space: taffy::prelude::Size<taffy::style::AvailableSpace>,
    ) -> taffy::prelude::Size<f32> {
        todo!()
    }

    fn needs_measure(&self, node: Node) -> bool {
        todo!()
    }

    fn cache_mut(&mut self, node: Node, index: usize) -> &mut Option<taffy::layout::Cache> {
        todo!()
    }
}
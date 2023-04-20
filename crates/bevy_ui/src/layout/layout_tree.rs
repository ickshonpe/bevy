use bevy_ecs::system::ResMut;
use bevy_ecs::system::Resource;
use bevy_ecs::system::SystemParam;
use bevy_render::render_resource::encase::CalculateSizeFor;
use slotmap::SlotMap;
use taffy::prelude::Node;

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
    measure_funcs: Query<'w, 's, &CalculatedSize>,
}


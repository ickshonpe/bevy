use bevy_ecs::prelude::DetectChanges;
use bevy_ecs::system::Query;
use bevy_ecs::world::Ref;
use bevy_transform::prelude::GlobalTransform;
use crate::NodePosition;

pub fn update_node_positions(
    mut node_query: Query<(&mut NodePosition, Ref<GlobalTransform>)>,
) {
    for (mut node_position, global_transform) in node_query.iter_mut() {
        if global_transform.is_changed() {
            node_position.calculated_position = global_transform.translation().truncate();
        }        
    }
}
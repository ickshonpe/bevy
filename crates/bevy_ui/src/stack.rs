//! This module contains the systems that update the stored UI nodes stack

use crate::{GlobalZIndex, ZIndex};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    entity::{EntityHashMap, EntityHashSet},
    prelude::*,
};
use bevy_reflect::std_traits::ReflectDefault;
use bevy_reflect::Reflect;

/// The order of the node in the UI layout.
/// Nodes with a higher stack index are drawn on top of and receive interactions before nodes with lower stack indices.
///
/// Automatically calculated in [`UiSystems::Stack`](`super::UiSystems::Stack`).
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Reflect)]
#[reflect(Component, Default)]
pub struct ComputedStackIndex {
    /// order of this node's root ancestor
    pub root: u32,
    /// order of the node in the local UI stack
    pub local: u32,
}

/// Local UI stack added to each UI root, contains all UI nodes descending from this root, ordered by their depth (back-to-front).
#[derive(Component, Default, PartialEq, Eq, Deref, DerefMut, Reflect)]
#[reflect(Component, Default)]
pub struct LocalUiStack(pub Vec<Entity>);

/// List of all root UI node entities in order of their depth (back-to-front).
#[derive(Debug, Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct UiStack(pub Vec<Entity>);

/// A `StackRoot` can be either a root UI node, or a parented UI node with a `GlobalZIndex` component.
/// The stack root and its descendants, up to any nested `StackRoots`, occupy a contiguous range in the render stack.
#[derive(Ord, PartialOrd, PartialEq, Eq)]
pub(crate) struct StackRoot {
    global_z: i32,
    local_z: i32,
    new_or_changed: bool,
    previous_index: usize,
}

/// Generates the render stack for UI nodes.
///
/// Create a list of root nodes from parentless entities and entities with a `GlobalZIndex` component.
/// Then build the `LocalUiStack`s from a walk of the existing layout trees starting from each root node,
/// filtering branches by `Without<GlobalZIndex>`so that we don't revisit nodes.
pub fn ui_stack_system(
    mut commands: Commands,
    mut cache: Local<Vec<(Entity, i32)>>,
    mut stack_roots: Local<Vec<(Entity, StackRoot)>>,
    mut stack_root_order: Local<EntityHashMap<usize>>,
    mut visited_stack_roots: Local<EntityHashSet>,
    mut new_local_stack: Local<Vec<Entity>>,
    ui_root_nodes: Query<Entity, (With<ComputedStackIndex>, Without<ChildOf>)>,
    root_node_query: Query<(Entity, Option<Ref<GlobalZIndex>>, Option<Ref<ZIndex>>)>,
    zindex_global_node_query: Query<
        (Entity, Ref<GlobalZIndex>, Option<Ref<ZIndex>>),
        With<ComputedStackIndex>,
    >,
    ui_children: Query<&Children, With<ComputedStackIndex>>,
    zindex_query: Query<Option<&ZIndex>, (With<ComputedStackIndex>, Without<GlobalZIndex>)>,
    mut update_query: Query<&mut ComputedStackIndex>,
    mut local_ui_stack_query: Query<(Entity, &mut LocalUiStack)>,
    mut ui_stack: ResMut<UiStack>,
) {
    stack_root_order.clear();
    for (order, entity) in ui_stack.0.iter().enumerate() {
        stack_root_order.insert(*entity, order);
    }
    ui_stack.0.clear();
    visited_stack_roots.clear();

    for (id, maybe_global_zindex, maybe_zindex) in
        root_node_query.iter_many(ui_root_nodes.iter()).matched()
    {
        let previous = stack_root_order.get(&id).copied();
        stack_roots.push((
            id,
            StackRoot {
                global_z: maybe_global_zindex.map(|z| z.0).unwrap_or(0),
                local_z: maybe_zindex.map(|z| z.0).unwrap_or(0),
                new_or_changed: previous.is_none()
                    || maybe_global_zindex.as_ref().is_some_and(Ref::is_changed)
                    || maybe_zindex.as_ref().is_some_and(Ref::is_changed),
                previous_index: previous.unwrap_or(usize::MAX),
            },
        ));
        visited_stack_roots.insert(id);
    }

    for (id, global_zindex, maybe_zindex) in zindex_global_node_query.iter() {
        if visited_stack_roots.contains(&id) {
            continue;
        }

        let previous = stack_root_order.get(&id).copied();
        stack_roots.push((
            id,
            StackRoot {
                global_z: global_zindex.0,
                local_z: maybe_zindex.map(|z| z.0).unwrap_or(0),
                new_or_changed: previous.is_none()
                    || global_zindex.is_changed()
                    || maybe_zindex.as_ref().is_some_and(Ref::is_changed),
                previous_index: previous.unwrap_or(usize::MAX),
            },
        ));
        visited_stack_roots.insert(id);
    }

    // An unstable sort is sufficient here. Roots that are equal must be new, and we
    // only care about maintaining stability across frames.
    stack_roots.sort_unstable_by(|(_, a), (_, b)| a.cmp(b));

    for (root_index, (root_entity, _)) in stack_roots.drain(..).enumerate() {
        ui_stack.0.push(root_entity);
        new_local_stack.clear();
        update_uistack_recursive(
            &mut cache,
            root_entity,
            &ui_children,
            &zindex_query,
            &mut new_local_stack,
        );

        for (local_index, entity) in new_local_stack.iter().enumerate() {
            if let Ok(mut computed_stack_index) = update_query.get_mut(*entity) {
                computed_stack_index.set_if_neq(ComputedStackIndex {
                    root: root_index as u32,
                    local: local_index as u32,
                });
            }
        }

        if let Ok((_, mut local_ui_stack)) = local_ui_stack_query.get_mut(root_entity) {
            if local_ui_stack.0 != *new_local_stack {
                core::mem::swap(&mut local_ui_stack.0, &mut new_local_stack);
            }
        } else {
            commands
                .entity(root_entity)
                .try_insert(LocalUiStack(core::mem::take(&mut new_local_stack)));
        }
    }

    for (entity, _) in local_ui_stack_query {
        if !visited_stack_roots.contains(&entity) {
            commands.entity(entity).remove::<LocalUiStack>();
        }
    }
}

fn update_uistack_recursive(
    child_buffer: &mut Vec<(Entity, i32)>,
    node_entity: Entity,
    ui_children: &Query<&Children, With<ComputedStackIndex>>,
    zindex_query: &Query<Option<&ZIndex>, (With<ComputedStackIndex>, Without<GlobalZIndex>)>,
    ui_stack: &mut Vec<Entity>,
) {
    ui_stack.push(node_entity);

    let start = child_buffer.len();
    child_buffer.extend(
        ui_children
            .get(node_entity)
            .into_iter()
            .flatten()
            .filter_map(|&child_entity| {
                zindex_query
                    .get(child_entity)
                    .ok()
                    .map(|zindex| (child_entity, zindex.map(|zindex| zindex.0).unwrap_or(0)))
            }),
    );
    let end = child_buffer.len();

    child_buffer[start..end].sort_by_key(|child| child.1);

    for index in start..end {
        let child_entity = child_buffer[index].0;
        update_uistack_recursive(
            child_buffer,
            child_entity,
            ui_children,
            zindex_query,
            ui_stack,
        );
    }

    child_buffer.truncate(start);
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        query::Changed,
        schedule::Schedule,
        system::Commands,
        world::{CommandQueue, World},
    };

    use crate::{ComputedStackIndex, GlobalZIndex, Node, ZIndex};

    use super::{ui_stack_system, LocalUiStack, UiStack};

    #[derive(Component, PartialEq, Debug, Clone)]
    struct Label(&'static str);

    fn node_with_global_and_local_zindex(
        name: &'static str,
        global_zindex: i32,
        local_zindex: i32,
    ) -> (Label, Node, GlobalZIndex, ZIndex) {
        (
            Label(name),
            Node::default(),
            GlobalZIndex(global_zindex),
            ZIndex(local_zindex),
        )
    }

    fn node_with_global_zindex(
        name: &'static str,
        global_zindex: i32,
    ) -> (Label, Node, GlobalZIndex) {
        (Label(name), Node::default(), GlobalZIndex(global_zindex))
    }

    fn node_with_zindex(name: &'static str, zindex: i32) -> (Label, Node, ZIndex) {
        (Label(name), Node::default(), ZIndex(zindex))
    }

    fn node_without_zindex(name: &'static str) -> (Label, Node) {
        (Label(name), Node::default())
    }

    #[test]
    fn test_update_computed_ui_stacks_system() {
        let mut world = World::default();
        world.init_resource::<UiStack>();

        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn(node_with_global_zindex("0", 2));

        commands
            .spawn(node_with_zindex("1", 1))
            .with_children(|parent| {
                parent
                    .spawn(node_without_zindex("1-0"))
                    .with_children(|parent| {
                        parent.spawn(node_without_zindex("1-0-0"));
                        parent.spawn(node_without_zindex("1-0-1"));
                        parent.spawn(node_with_zindex("1-0-2", -1));
                    });
                parent.spawn(node_without_zindex("1-1"));
                parent
                    .spawn(node_with_global_zindex("1-2", -1))
                    .with_children(|parent| {
                        parent.spawn(node_without_zindex("1-2-0"));
                        parent.spawn(node_with_global_zindex("1-2-1", -3));
                        parent
                            .spawn(node_without_zindex("1-2-2"))
                            .with_children(|_| ());
                        parent.spawn(node_without_zindex("1-2-3"));
                    });
                parent.spawn(node_without_zindex("1-3"));
            });

        commands
            .spawn(node_without_zindex("2"))
            .with_children(|parent| {
                parent
                    .spawn(node_without_zindex("2-0"))
                    .with_children(|_parent| ());
                parent
                    .spawn(node_without_zindex("2-1"))
                    .with_children(|parent| {
                        parent.spawn(node_without_zindex("2-1-0"));
                    });
            });

        commands.spawn(node_with_global_zindex("3", -2));

        queue.apply(&mut world);

        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);
        schedule.run(&mut world);
        let mut changed_local_stacks = world.query_filtered::<Entity, Changed<LocalUiStack>>();
        world.clear_trackers();
        schedule.run(&mut world);
        assert!(changed_local_stacks.iter(&world).next().is_none());

        let mut query = world.query::<&Label>();
        let ui_stack_roots = world.resource::<UiStack>();
        let actual_result = ui_stack_roots
            .0
            .iter()
            .flat_map(|entity| world.get::<LocalUiStack>(*entity).unwrap().iter())
            .map(|entity| query.get(&world, *entity).unwrap().clone())
            .collect::<Vec<_>>();
        let expected_result = vec![
            (Label("1-2-1")), // GlobalZIndex(-3)
            (Label("3")),     // GlobalZIndex(-2)
            (Label("1-2")),   // GlobalZIndex(-1)
            (Label("1-2-0")),
            (Label("1-2-2")),
            (Label("1-2-3")),
            (Label("2")),
            (Label("2-0")),
            (Label("2-1")),
            (Label("2-1-0")),
            (Label("1")), // ZIndex(1)
            (Label("1-0")),
            (Label("1-0-2")), // ZIndex(-1)
            (Label("1-0-0")),
            (Label("1-0-1")),
            (Label("1-1")),
            (Label("1-3")),
            (Label("0")), // GlobalZIndex(2)
        ];
        assert_eq!(actual_result, expected_result);

        let last_root = *ui_stack_roots.0.last().unwrap();
        let last_stack = world.get::<LocalUiStack>(last_root).unwrap();
        assert_eq!(last_stack.len(), 1);
        let last_entity = last_stack[0];
        assert_eq!(*query.get(&world, last_entity).unwrap(), Label("0"));

        let actual_result = world
            .get::<LocalUiStack>(ui_stack_roots.0[4])
            .unwrap()
            .iter()
            .map(|entity| query.get(&world, *entity).unwrap().clone())
            .collect::<Vec<_>>();
        let expected_result = vec![
            (Label("1")), // ZIndex(1)
            (Label("1-0")),
            (Label("1-0-2")), // ZIndex(-1)
            (Label("1-0-0")),
            (Label("1-0-1")),
            (Label("1-1")),
            (Label("1-3")),
        ];
        assert_eq!(actual_result, expected_result);

        let expected_result = ui_stack_roots
            .0
            .iter()
            .flat_map(|entity| world.get::<LocalUiStack>(*entity).unwrap().iter())
            .copied()
            .collect::<Vec<_>>();
        let mut computed_ui_stacks_query = world.query::<(&ComputedStackIndex, &LocalUiStack)>();
        let mut computed_ui_stacks = computed_ui_stacks_query.iter(&world).collect::<Vec<_>>();
        computed_ui_stacks.sort_by_key(|(stack_index, _)| *stack_index);
        let actual_result = computed_ui_stacks
            .into_iter()
            .flat_map(|(_, ui_stack)| ui_stack.iter())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(actual_result, expected_result);
    }

    #[test]
    fn test_with_equal_global_zindex_zindex_decides_order() {
        let mut world = World::default();
        world.init_resource::<UiStack>();

        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        commands.spawn(node_with_global_and_local_zindex("0", -1, 1));
        commands.spawn(node_with_global_and_local_zindex("1", -1, 2));
        commands.spawn(node_with_global_and_local_zindex("2", 1, 3));
        commands.spawn(node_with_global_and_local_zindex("3", 1, -3));
        commands
            .spawn(node_without_zindex("4"))
            .with_children(|builder| {
                builder.spawn(node_with_global_and_local_zindex("5", 0, -1));
                builder.spawn(node_with_global_and_local_zindex("6", 0, 1));
                builder.spawn(node_with_global_and_local_zindex("7", -1, -1));
                builder.spawn(node_with_global_zindex("8", 1));
            });

        queue.apply(&mut world);

        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);
        schedule.run(&mut world);

        let mut query = world.query::<&Label>();
        let ui_stack_roots = world.resource::<UiStack>();
        let actual_result = ui_stack_roots
            .0
            .iter()
            .flat_map(|entity| world.get::<LocalUiStack>(*entity).unwrap().iter())
            .map(|entity| query.get(&world, *entity).unwrap().clone())
            .collect::<Vec<_>>();

        let expected_result = vec![
            (Label("7")),
            (Label("0")),
            (Label("1")),
            (Label("5")),
            (Label("4")),
            (Label("6")),
            (Label("3")),
            (Label("8")),
            (Label("2")),
        ];

        assert_eq!(actual_result, expected_result);

        assert_eq!(ui_stack_roots.0.len(), expected_result.len());
        for entity in &ui_stack_roots.0 {
            assert_eq!(world.get::<LocalUiStack>(*entity).unwrap().len(), 1);
        }
    }

    #[test]
    fn order_of_stack_roots_should_be_preserved_between_frames() {
        #[derive(Component)]
        struct Marker;
        let mut world = World::default();
        world.init_resource::<UiStack>();

        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);

        for _ in 0..10 {
            world.spawn((Node::default(), GlobalZIndex(0)));
        }

        schedule.run(&mut world);

        let uinodes = world.resource::<UiStack>().0.clone();

        for marked_entity in uinodes.iter().take(3) {
            world.entity_mut(*marked_entity).insert(Marker);
        }

        schedule.run(&mut world);

        assert_eq!(uinodes, world.resource::<UiStack>().0);
    }

    #[test]
    fn last_updated_stack_root_should_be_on_top() {
        let mut world = World::default();
        world.init_resource::<UiStack>();

        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);

        for _ in 0..10 {
            world.spawn((Node::default(), GlobalZIndex(0)));
        }

        schedule.run(&mut world);

        let first = world.resource::<UiStack>().0[0];

        world.entity_mut(first).insert(GlobalZIndex(0));

        schedule.run(&mut world);

        assert_eq!(first, *world.resource::<UiStack>().0.last().unwrap());

        let first = world.resource::<UiStack>().0[0];

        world.entity_mut(first).insert(ZIndex(0));

        schedule.run(&mut world);

        assert_eq!(first, *world.resource::<UiStack>().0.last().unwrap());
    }

    #[test]
    fn order_of_parented_stack_roots_should_be_preserved_between_frames() {
        #[derive(Component)]
        struct Marker;
        let mut world = World::default();
        world.init_resource::<UiStack>();

        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);

        let parent = world.spawn(Node::default()).id();
        for _ in 0..10 {
            world.spawn((Node::default(), GlobalZIndex(0), ChildOf(parent)));
        }

        schedule.run(&mut world);

        let uinodes = world.resource::<UiStack>().0.clone();

        for marked_entity in uinodes.iter().filter(|entity| **entity != parent).take(3) {
            world.entity_mut(*marked_entity).insert(Marker);
        }

        schedule.run(&mut world);

        assert_eq!(uinodes, world.resource::<UiStack>().0);
    }

    #[test]
    fn growing_local_stack_preserves_other_stack_indices() {
        let mut world = World::default();
        world.init_resource::<UiStack>();
        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);

        let first = world.spawn((Node::default(), GlobalZIndex(-1))).id();
        let second = world.spawn(Node::default()).id();
        let child = world.spawn((Node::default(), ChildOf(second))).id();
        schedule.run(&mut world);
        world.clear_trackers();

        let added = world.spawn((Node::default(), ChildOf(first))).id();
        schedule.run(&mut world);

        let mut changed = world.query_filtered::<Entity, Changed<ComputedStackIndex>>();
        assert!(!changed
            .iter(&world)
            .any(|entity| entity == second || entity == child));
        assert_eq!(world.get::<LocalUiStack>(first).unwrap().0, [first, added]);
        assert_eq!(
            world.get::<LocalUiStack>(second).unwrap().0,
            [second, child]
        );
    }

    #[test]
    fn local_stack_removed_when_node_stops_being_a_stack_root() {
        let mut world = World::default();
        world.init_resource::<UiStack>();
        let mut schedule = Schedule::default();
        schedule.add_systems(ui_stack_system);

        let parent = world.spawn(Node::default()).id();
        let child = world
            .spawn((Node::default(), GlobalZIndex(1), ChildOf(parent)))
            .id();
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(world.get::<LocalUiStack>(child).unwrap().0, [child]);

        world.entity_mut(child).remove::<GlobalZIndex>();
        schedule.run(&mut world);
        assert!(world.get::<LocalUiStack>(child).is_none());
        assert_eq!(
            world.get::<LocalUiStack>(parent).unwrap().0,
            [parent, child]
        );
        assert_eq!(world.resource::<UiStack>().0, [parent]);

        world.entity_mut(child).remove::<ChildOf>();
        schedule.run(&mut world);
        assert_eq!(world.get::<LocalUiStack>(child).unwrap().0, [child]);
        assert_eq!(world.get::<LocalUiStack>(parent).unwrap().0, [parent]);
    }
}

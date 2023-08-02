use crate::{
    archetype::{ArchetypeEntity, ArchetypeId, Archetypes},
    component::Tick,
    entity::{Entities, Entity},
    prelude::World,
    query::{ArchetypeFilter, DebugCheckedUnwrap, QueryState, WorldQuery},
    storage::{TableId, TableRow, Tables},
};
use std::{borrow::Borrow, iter::FusedIterator, marker::PhantomData, mem::MaybeUninit};

use super::ReadOnlyWorldQuery;

/// enumerated!
pub struct QueryManyEnumeratedIter<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery, I: Iterator>
where
    I::Item: Borrow<Entity>,
{
    entity_iter: I,
    entities: &'w Entities,
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    fetch: Q::Fetch<'w>,
    filter: F::Fetch<'w>,
    query_state: &'s QueryState<Q, F>,
    index: usize,
}

impl<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery, I: Iterator>
    QueryManyEnumeratedIter<'w, 's, Q, F, I>
where
    I::Item: Borrow<Entity>,
{
    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    /// This does not validate that `world.id()` matches `query_state.world_id`. Calling this on a `world`
    /// with a mismatched [`WorldId`](crate::world::WorldId) is unsound.
    pub(crate) unsafe fn new<EntityList: IntoIterator<IntoIter = I>>(
        world: &'w World,
        query_state: &'s QueryState<Q, F>,
        entity_list: EntityList,
        last_run: Tick,
        this_run: Tick,
    ) -> QueryManyEnumeratedIter<'w, 's, Q, F, I> {
        let fetch = Q::init_fetch(world, &query_state.fetch_state, last_run, this_run);
        let filter = F::init_fetch(world, &query_state.filter_state, last_run, this_run);
        QueryManyEnumeratedIter {
            query_state,
            entities: &world.entities,
            archetypes: &world.archetypes,
            tables: &world.storages.tables,
            fetch,
            filter,
            entity_iter: entity_list.into_iter(),
            index: 0,
        }
    }

    /// Safety:
    /// The lifetime here is not restrictive enough for Fetch with &mut access,
    /// as calling `fetch_next_aliased_unchecked` multiple times can produce multiple
    /// references to the same component, leading to unique reference aliasing.
    ///
    /// It is always safe for shared access.
    #[inline(always)]
    unsafe fn fetch_next_aliased_unchecked(&mut self) -> Option<(usize, Q::Item<'w>)> {
        for entity in self.entity_iter.by_ref() {
            self.index += 1;
            let entity = *entity.borrow();
            let location = match self.entities.get(entity) {
                Some(location) => location,
                None => continue,
            };

            if !self
                .query_state
                .matched_archetypes
                .contains(location.archetype_id.index())
            {
                continue;
            }

            let archetype = self
                .archetypes
                .get(location.archetype_id)
                .debug_checked_unwrap();
            let table = self.tables.get(location.table_id).debug_checked_unwrap();

            // SAFETY: `archetype` is from the world that `fetch/filter` were created for,
            // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
            Q::set_archetype(
                &mut self.fetch,
                &self.query_state.fetch_state,
                archetype,
                table,
            );
            // SAFETY: `table` is from the world that `fetch/filter` were created for,
            // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
            F::set_archetype(
                &mut self.filter,
                &self.query_state.filter_state,
                archetype,
                table,
            );

            // SAFETY: set_archetype was called prior.
            // `location.archetype_row` is an archetype index row in range of the current archetype, because if it was not, the match above would have `continue`d
            if F::filter_fetch(&mut self.filter, entity, location.table_row) {
                // SAFETY: set_archetype was called prior, `location.archetype_row` is an archetype index in range of the current archetype
                return Some((
                    self.index - 1,
                    Q::fetch(&mut self.fetch, entity, location.table_row),
                ));
            }
        }
        None
    }

    /// Get next result from the query
    #[inline(always)]
    pub fn fetch_next(&mut self) -> Option<(usize, Q::Item<'_>)> {
        // SAFETY: we are limiting the returned reference to self,
        // making sure this method cannot be called multiple times without getting rid
        // of any previously returned unique references first, thus preventing aliasing.
        unsafe {
            self.fetch_next_aliased_unchecked()
                .map(|(index, item)| (index, Q::shrink(item)))
        }
    }
}

impl<'w, 's, Q: ReadOnlyWorldQuery, F: ReadOnlyWorldQuery, I: Iterator> Iterator
    for QueryManyEnumeratedIter<'w, 's, Q, F, I>
where
    I::Item: Borrow<Entity>,
{
    type Item = (usize, Q::Item<'w>);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: It is safe to alias for ReadOnlyWorldQuery.
        unsafe { self.fetch_next_aliased_unchecked() }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, max_size) = self.entity_iter.size_hint();
        (0, max_size)
    }
}

// This is correct as [`QueryManyIter`] always returns `None` once exhausted.
impl<'w, 's, Q: ReadOnlyWorldQuery, F: ReadOnlyWorldQuery, I: Iterator> FusedIterator
    for QueryManyEnumeratedIter<'w, 's, Q, F, I>
where
    I::Item: Borrow<Entity>,
{
}

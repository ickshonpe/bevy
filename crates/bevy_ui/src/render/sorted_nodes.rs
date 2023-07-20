use std::{iter::Peekable, vec::Drain};

use bevy_ecs::system::Resource;

use super::ExtractedUiNode;

pub const UI_NODES_BUFFERS: usize = 4;

#[derive(Resource, Default)]
pub struct ExtractedUiNodes {
    pub uinodes: [Vec<ExtractedUiNode>; UI_NODES_BUFFERS],
}
pub struct ExtractedUiNodesIter<'a> {
    nodes: [Peekable<Drain<'a, ExtractedUiNode>>; UI_NODES_BUFFERS],
    firsts_stack: [usize; UI_NODES_BUFFERS],
}

impl<'a> IntoIterator for &'a mut ExtractedUiNodes {
    type Item = ExtractedUiNode;
    type IntoIter = ExtractedUiNodesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ExtractedUiNodesIter::new(&mut self.uinodes)
    }
}

// From <https://github.com/rust-lang/rust/issues/76118#issuecomment-1075138065>
// FIXME: replace with `array::each_mut` once it is stabilized
fn array_of_mut<T, const N: usize>(arr: &mut [T; N]) -> [&mut T; N] {
    use core::mem::MaybeUninit;
    let mut out: MaybeUninit<[&mut T; N]> = MaybeUninit::uninit();

    let buf = out.as_mut_ptr() as *mut &mut T;
    let mut refs = arr.iter_mut();

    for i in 0..N as isize {
        // SAFETY:
        // - Limited to 0..N, therefore within bounds of `out`
        // - Rust already allocated `buf` (it is `out`), therefore its addres
        //   should not overflow an isize
        // - MaybeUninit<[&mut T; N]> has the same alignment as [&mut T; N]
        unsafe { buf.offset(i).write(refs.next().unwrap()) }
    }

    // SAFETY: we initialized every element from 0 to N.
    unsafe { out.assume_init() }
}
impl<'a> ExtractedUiNodesIter<'a> {
    pub fn new(nodes: &'a mut [Vec<ExtractedUiNode>; UI_NODES_BUFFERS]) -> Self {
        let firsts = array_of_mut(nodes).map(|n| n.first().map_or(usize::MAX, |n| n.stack_index));
        Self {
            nodes: array_of_mut(nodes).map(|n| n.drain(..).peekable()),
            firsts_stack: firsts,
        }
    }
}
fn index_min((min_index, min): (usize, usize), (i, value): (usize, &usize)) -> (usize, usize) {
    if *value < min {
        (i, *value)
    } else {
        (min_index, min)
    }
}
impl Iterator for ExtractedUiNodesIter<'_> {
    type Item = ExtractedUiNode;

    fn next(&mut self) -> Option<Self::Item> {
        let iter = self.firsts_stack.iter().enumerate();
        let (n, _) = iter.fold((usize::MAX, usize::MAX), index_min);
        let next = self.nodes.get_mut(n)?.next()?;
        self.firsts_stack[n] = self.nodes[n].peek().map_or(usize::MAX, |n| n.stack_index);

        Some(next)
    }
}
impl ExtractedUiNodes {
    /// Retrieves the next empty `ExtractedUiNode` buffer. If none exists, creates one before returning it.
    pub fn next_buffer(&mut self) -> &mut Vec<ExtractedUiNode> {
        let mut iter = self.uinodes.iter_mut();
        iter.find(|uinodes| uinodes.is_empty()).expect(
            "Too many calls to ExtractedUiNodes::next_buffer, \
             sorted_nodes::UI_NODES_BUFFERS needs to be incremented",
        )
    }
}

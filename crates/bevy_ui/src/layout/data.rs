use taffy::layout::Cache;
use taffy::style::Style;

/// The number of cache entries for each node in the tree
pub const CACHE_SIZE: usize = 7;

pub struct UiNodeData {
    /// The layout strategy used by this node
    pub style: Style,

    /// Should we try and measure this node?
    pub needs_measure: bool,

    /// The primary cached results of the layout computation
    pub size_cache: [Option<Cache>; CACHE_SIZE],
}

impl UiNodeData {
    /// Create the data for a new node
    #[must_use]
    pub const fn new(style: Style) -> Self {
        Self {
            style,
            size_cache: [None; CACHE_SIZE],
            needs_measure: false,
        }
    }

    /// Marks a node and all of its parents (recursively) as dirty
    ///
    /// This clears any cached data and signals that the data must be recomputed.
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.size_cache = [None; CACHE_SIZE];
    }
}

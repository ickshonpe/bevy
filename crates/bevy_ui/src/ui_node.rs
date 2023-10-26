use crate::{UiRect, Val};
use bevy_asset::Handle;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{prelude::Component, reflect::ReflectComponent};
use bevy_math::{vec2, Rect, Vec2};
use bevy_reflect::prelude::*;
use bevy_render::{color::Color, texture::Image};
use bevy_transform::prelude::GlobalTransform;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::{
    f32::consts::{FRAC_PI_2, PI},
    num::{NonZeroI16, NonZeroU16}
};
use thiserror::Error;

/// Describes the size of a UI node
#[derive(Component, Debug, Copy, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct Node {
    /// The size of the node as width and height in logical pixels
    /// automatically calculated by [`super::layout::ui_layout_system`]
    pub(crate) calculated_size: Vec2,
    /// The width of this node's outline
    /// If this value is `Auto`, negative or `0.` then no outline will be rendered
    /// automatically calculated by [`super::layout::resolve_outlines_system`]
    pub(crate) outline_width: f32,
    // The amount of space between the outline and the edge of the node
    pub(crate) outline_offset: f32,
    /// The unrounded size of the node as width and height in logical pixels
    /// automatically calculated by [`super::layout::ui_layout_system`]
    pub(crate) unrounded_size: Vec2,
    pub(crate) border: [f32; 4],
    pub(crate) border_radius: [f32; 4],
    pub(crate) position: Vec2,
}

impl Node {
    /// The calculated node size as width and height in logical pixels
    /// automatically calculated by [`super::layout::ui_layout_system`]
    pub const fn size(&self) -> Vec2 {
        self.calculated_size
    }

    /// The calculated node size as width and height in logical pixels before rounding
    /// automatically calculated by [`super::layout::ui_layout_system`]
    pub const fn unrounded_size(&self) -> Vec2 {
        self.unrounded_size
    }

    /// Returns the size of the node in physical pixels based on the given scale factor and `UiScale`.
    #[inline]
    pub fn physical_size(&self, scale_factor: f64, ui_scale: f64) -> Vec2 {
        Vec2::new(
            (self.calculated_size.x as f64 * scale_factor * ui_scale) as f32,
            (self.calculated_size.y as f64 * scale_factor * ui_scale) as f32,
        )
    }

    /// Returns the logical pixel coordinates of the UI node, based on its [`GlobalTransform`].
    #[inline]
    pub fn logical_rect(&self, transform: &GlobalTransform) -> Rect {
        Rect::from_center_size(transform.translation().truncate(), self.size())
    }

    /// Returns the logical pixel coordinates of the UI node, based on its [`GlobalTransform`].
    #[inline]
    pub fn rect(&self) -> Rect {
        Rect { min: self.position, max: self.position + self.size() }
    }

    /// Returns the physical pixel coordinates of the UI node, based on its [`GlobalTransform`] and the scale factor.
    #[inline]
    pub fn physical_rect(
        &self,
        transform: &GlobalTransform,
        scale_factor: f64,
        ui_scale: f64,
    ) -> Rect {
        let rect = self.logical_rect(transform);
        Rect {
            min: Vec2::new(
                (rect.min.x as f64 * scale_factor * ui_scale) as f32,
                (rect.min.y as f64 * scale_factor * ui_scale) as f32,
            ),
            max: Vec2::new(
                (rect.max.x as f64 * scale_factor * ui_scale) as f32,
                (rect.max.y as f64 * scale_factor * ui_scale) as f32,
            ),
        }
    }

    #[inline]
    /// Returns the thickness of the UI node's outline.
    /// If this value is negative or `0.` then no outline will be rendered.
    pub fn outline_width(&self) -> f32 {
        self.outline_width
    }

    #[inline]
    pub fn position(&self) -> Vec2 {
        self.position
    }
}

impl Node {
    pub const DEFAULT: Self = Self {
        calculated_size: Vec2::ZERO,
        outline_width: 0.,
        outline_offset: 0.,
        unrounded_size: Vec2::ZERO,
        border: [0.; 4],
        border_radius: [0.; 4],
        position: Vec2::ZERO,
    };
}

impl Default for Node {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Position relative to an axis-aligned rectangle along one of its axis
/// * Negative values move the origin left or up on the respective axis, positive values down and to the right.
/// * `Val::Auto` is equivalent to `Val::ZERO`
/// * `Val::Percent` is based on the length of the axis of the node.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Default, PartialEq, Serialize, Deserialize)]
pub enum RectPositionAxis {
    /// Position is relative to the rectangle's start (left or top edge) on this axes
    Start(Val),
    /// Position is relative to the rectangle's center on this axes
    Center(Val),
    /// Position is relative to the rectangle's start (right or bottom edge) on this axes
    End(Val),
}

impl Default for RectPositionAxis {
    fn default() -> Self {
        RectPositionAxis::Center(Val::Auto)
    }
}

impl RectPositionAxis {
    pub const START: Self = Self::Start(Val::Auto);
    pub const CENTER: Self = Self::Center(Val::Auto);
    pub const END: Self = Self::End(Val::Auto);
    pub const DEFAULT: Self = Self::CENTER;

    /// Resolve a `RectPositionAxis` to a value in logical pixels
    /// Assumes min <= max
    pub fn resolve(self, min: f32, max: f32, viewport_size: Vec2) -> f32 {
        let length = max - min;
        let (val, point) = match self {
            RectPositionAxis::Start(val) => (val, min),
            RectPositionAxis::Center(val) => (val, min + 0.5 * length),
            RectPositionAxis::End(val) => (val, max),
        };
        point + val.resolve(length, viewport_size).unwrap_or(0.)
    }
}

/// Position relative to an axis aligned rectangle
/// Position outside of a rectangle's bounds are valid.
#[derive(Default, Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Default, PartialEq, Serialize, Deserialize)]
pub struct RectPosition {
    /// Horizontal position
    pub x: RectPositionAxis,
    /// Vertical position
    pub y: RectPositionAxis,
}

impl RectPosition {
    /// A new `RectPosition` with the given axis values
    pub const fn new(x: RectPositionAxis, y: RectPositionAxis) -> Self {
        Self { x, y }
    }

    /// A `RectPosition`with both axis set to the same value
    pub const fn all(value: RectPositionAxis) -> Self {
        Self { x: value, y: value }
    }

    /// An `RectPosition` relative to the center of the node.
    pub const fn center(x: Val, y: Val) -> Self {
        Self {
            x: RectPositionAxis::Center(x),
            y: RectPositionAxis::Center(y),
        }
    }

    /// A `RectPosition` at the center.
    pub const CENTER: Self = Self::all(RectPositionAxis::CENTER);
    /// A `RectPosition` at the top left corner.
    pub const TOP_LEFT: Self = Self::all(RectPositionAxis::START);
    /// A `RectPosition` at the top right corner.
    pub const TOP_RIGHT: Self = Self::new(RectPositionAxis::END, RectPositionAxis::START);
    /// A `RectPosition` at the bottom right corner.
    pub const BOTTOM_RIGHT: Self = Self::new(RectPositionAxis::END, RectPositionAxis::END);
    /// A `RectPosition` at the bottom left corner.
    pub const BOTTOM_LEFT: Self = Self::all(RectPositionAxis::END);
    /// A `RectPosition` at the center of the top edge.
    pub const TOP_CENTER: Self = Self::new(RectPositionAxis::CENTER, RectPositionAxis::START);
    /// A `RectPosition` at the center of the bottom edge.
    pub const BOTTOM_CENTER: Self = Self::new(RectPositionAxis::CENTER, RectPositionAxis::END);
    /// A `RectPosition` at the center of the left edge.
    pub const LEFT_CENTER: Self = Self::new(RectPositionAxis::CENTER, RectPositionAxis::START);
    /// A `RectPosition` at the center of the right edge.
    pub const RIGHT_CENTER: Self = Self::new(RectPositionAxis::CENTER, RectPositionAxis::END);

    pub fn resolve(self, rect: Rect, viewport_size: Vec2) -> Vec2 {
        Vec2 {
            x: self.x.resolve(rect.min.x, rect.max.x, viewport_size),
            y: self.y.resolve(rect.min.y, rect.max.y, viewport_size),
        }
    }
}

/// Describes the style of a UI container node
///
/// Node's can be laid out using either Flexbox or CSS Grid Layout.<br />
/// See below for general learning resources and for documentation on the individual style properties.
///
/// ### Flexbox
///
/// - [MDN: Basic Concepts of Grid Layout](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Grid_Layout/Basic_Concepts_of_Grid_Layout)
/// - [A Complete Guide To Flexbox](https://css-tricks.com/snippets/css/a-guide-to-flexbox/) by CSS Tricks. This is detailed guide with illustrations and comphrehensive written explanation of the different Flexbox properties and how they work.
/// - [Flexbox Froggy](https://flexboxfroggy.com/). An interactive tutorial/game that teaches the essential parts of Flebox in a fun engaging way.
///
/// ### CSS Grid
///
/// - [MDN: Basic Concepts of Flexbox](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Flexible_Box_Layout/Basic_Concepts_of_Flexbox)
/// - [A Complete Guide To CSS Grid](https://css-tricks.com/snippets/css/complete-guide-grid/) by CSS Tricks. This is detailed guide with illustrations and comphrehensive written explanation of the different CSS Grid properties and how they work.
/// - [CSS Grid Garden](https://cssgridgarden.com/). An interactive tutorial/game that teaches the essential parts of CSS Grid in a fun engaging way.

#[derive(Component, Clone, PartialEq, Debug, Reflect)]
#[reflect(Component, Default, PartialEq)]
pub struct Style {
    /// Which layout algorithm to use when laying out this node's contents:
    ///   - [`Display::Flex`]: Use the Flexbox layout algorithm
    ///   - [`Display::Grid`]: Use the CSS Grid layout algorithm
    ///   - [`Display::None`]: Hide this node and perform layout as if it does not exist.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/display>
    pub display: Display,

    /// Whether a node should be laid out in-flow with, or independently of it's siblings:
    ///  - [`PositionType::Relative`]: Layout this node in-flow with other nodes using the usual (flexbox/grid) layout algorithm.
    ///  - [`PositionType::Absolute`]: Layout this node on top and independently of other nodes.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/position>
    pub position_type: PositionType,

    /// Whether overflowing content should be displayed or clipped.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/overflow>
    pub overflow: Overflow,

    /// Defines the text direction. For example English is written LTR (left-to-right) while Arabic is written RTL (right-to-left).
    ///
    /// Note: the corresponding CSS property also affects box layout order, but this isn't yet implemented in bevy.
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/direction>
    pub direction: Direction,

    /// The horizontal position of the left edge of the node.
    ///  - For relatively positioned nodes, this is relative to the node's position as computed during regular layout.
    ///  - For absolutely positioned nodes, this is relative to the *parent* node's bounding box.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/left>
    pub left: Val,

    /// The horizontal position of the right edge of the node.
    ///  - For relatively positioned nodes, this is relative to the node's position as computed during regular layout.
    ///  - For absolutely positioned nodes, this is relative to the *parent* node's bounding box.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/right>
    pub right: Val,

    /// The vertical position of the top edge of the node.
    ///  - For relatively positioned nodes, this is relative to the node's position as computed during regular layout.
    ///  - For absolutely positioned nodes, this is relative to the *parent* node's bounding box.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/top>
    pub top: Val,

    /// The vertical position of the bottom edge of the node.
    ///  - For relatively positioned nodes, this is relative to the node's position as computed during regular layout.
    ///  - For absolutely positioned nodes, this is relative to the *parent* node's bounding box.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/bottom>
    pub bottom: Val,

    /// The ideal width of the node. `width` is used when it is within the bounds defined by `min_width` and `max_width`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/width>
    pub width: Val,

    /// The ideal height of the node. `height` is used when it is within the bounds defined by `min_height` and `max_height`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/height>
    pub height: Val,

    /// The minimum width of the node. `min_width` is used if it is greater than either `width` and/or `max_width`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/min-width>
    pub min_width: Val,

    /// The minimum height of the node. `min_height` is used if it is greater than either `height` and/or `max_height`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/min-height>
    pub min_height: Val,

    /// The maximum width of the node. `max_width` is used if it is within the bounds defined by `min_width` and `width`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/max-width>
    pub max_width: Val,

    /// The maximum height of the node. `max_height` is used if it is within the bounds defined by `min_height` and `height`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/max-height>
    pub max_height: Val,

    /// The aspect ratio of the node (defined as `width / height`)
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/aspect-ratio>
    pub aspect_ratio: Option<f32>,

    /// For Flexbox containers:
    ///   - Sets default cross-axis alignment of the child items.
    /// For CSS Grid containers:
    ///   - Controls block (vertical) axis alignment of children of this grid container within their grid areas
    ///
    /// This value is overriden [`JustifySelf`] on the child node is set.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/align-items>
    pub align_items: AlignItems,

    /// For Flexbox containers:
    ///   - This property has no effect. See `justify_content` for main-axis alignment of flex items.
    /// For CSS Grid containers:
    ///   - Sets default inline (horizontal) axis alignment of child items within their grid areas
    ///
    /// This value is overriden [`JustifySelf`] on the child node is set.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/justify-items>
    pub justify_items: JustifyItems,

    /// For Flexbox items:
    ///   - Controls cross-axis alignment of the item.
    /// For CSS Grid items:
    ///   - Controls block (vertical) axis alignment of a grid item within it's grid area
    ///
    /// If set to `Auto`, alignment is inherited from the value of [`AlignItems`] set on the parent node.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/align-self>
    pub align_self: AlignSelf,

    /// For Flexbox items:
    ///   - This property has no effect. See `justify_content` for main-axis alignment of flex items.
    /// For CSS Grid items:
    ///   - Controls inline (horizontal) axis alignment of a grid item within it's grid area.
    ///
    /// If set to `Auto`, alignment is inherited from the value of [`JustifyItems`] set on the parent node.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/justify-items>
    pub justify_self: JustifySelf,

    /// For Flexbox containers:
    ///   - Controls alignment of lines if flex_wrap is set to [`FlexWrap::Wrap`] and there are multiple lines of items
    /// For CSS Grid container:
    ///   - Controls alignment of grid rows
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/align-content>
    pub align_content: AlignContent,

    /// For Flexbox containers:
    ///   - Controls alignment of items in the main axis
    /// For CSS Grid containers:
    ///   - Controls alignment of grid columns
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/justify-content>
    pub justify_content: JustifyContent,

    /// The amount of space around a node outside its border.
    ///
    /// If a percentage value is used, the percentage is calculated based on the width of the parent node.
    ///
    /// # Example
    /// ```
    /// # use bevy_ui::{Style, UiRect, Val};
    /// let style = Style {
    ///     margin: UiRect {
    ///         left: Val::Percent(10.),
    ///         right: Val::Percent(10.),
    ///         top: Val::Percent(15.),
    ///         bottom: Val::Percent(15.)
    ///     },
    ///     ..Default::default()
    /// };
    /// ```
    /// A node with this style and a parent with dimensions of 100px by 300px, will have calculated margins of 10px on both left and right edges, and 15px on both top and bottom edges.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/margin>
    pub margin: UiRect,

    /// The amount of space between the edges of a node and its contents.
    ///
    /// If a percentage value is used, the percentage is calculated based on the width of the parent node.
    ///
    /// # Example
    /// ```
    /// # use bevy_ui::{Style, UiRect, Val};
    /// let style = Style {
    ///     padding: UiRect {
    ///         left: Val::Percent(1.),
    ///         right: Val::Percent(2.),
    ///         top: Val::Percent(3.),
    ///         bottom: Val::Percent(4.)
    ///     },
    ///     ..Default::default()
    /// };
    /// ```
    /// A node with this style and a parent with dimensions of 300px by 100px, will have calculated padding of 3px on the left, 6px on the right, 9px on the top and 12px on the bottom.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/padding>
    pub padding: UiRect,

    /// The amount of space between the margins of a node and its padding.
    ///
    /// If a percentage value is used, the percentage is calculated based on the width of the parent node.
    ///
    /// The size of the node will be expanded if there are constraints that prevent the layout algorithm from placing the border within the existing node boundary.
    ///
    /// Rendering for borders is not yet implemented.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/border-width>
    pub border: UiRect,

    /// Whether a Flexbox container should be a row or a column. This property has no effect of Grid nodes.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/flex-direction>
    pub flex_direction: FlexDirection,

    /// Whether a Flexbox container should wrap it's contents onto multiple line wrap if they overflow. This property has no effect of Grid nodes.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/flex-wrap>
    pub flex_wrap: FlexWrap,

    /// Defines how much a flexbox item should grow if there's space available. Defaults to 0 (don't grow at all).
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/flex-grow>
    pub flex_grow: f32,

    /// Defines how much a flexbox item should shrink if there's not enough space available. Defaults to 1.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/flex-shrink>
    pub flex_shrink: f32,

    /// The initial length of a flexbox in the main axis, before flex growing/shrinking properties are applied.
    ///
    /// `flex_basis` overrides `size` on the main axis if both are set,  but it obeys the bounds defined by `min_size` and `max_size`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/flex-basis>
    pub flex_basis: Val,

    /// The size of the gutters between items in a vertical flexbox layout or between rows in a grid layout
    ///
    /// Note: Values of `Val::Auto` are not valid and are treated as zero.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/row-gap>
    pub row_gap: Val,

    /// The size of the gutters between items in a horizontal flexbox layout or between column in a grid layout
    ///
    /// Note: Values of `Val::Auto` are not valid and are treated as zero.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/column-gap>
    pub column_gap: Val,

    /// Controls whether automatically placed grid items are placed row-wise or column-wise. And whether the sparse or dense packing algorithm is used.
    /// Only affect Grid layouts
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-auto-flow>
    pub grid_auto_flow: GridAutoFlow,

    /// Defines the number of rows a grid has and the sizes of those rows. If grid items are given explicit placements then more rows may
    /// be implicitly generated by items that are placed out of bounds. The sizes of those rows are controlled by `grid_auto_rows` property.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-template-rows>
    pub grid_template_rows: Vec<RepeatedGridTrack>,

    /// Defines the number of columns a grid has and the sizes of those columns. If grid items are given explicit placements then more columns may
    /// be implicitly generated by items that are placed out of bounds. The sizes of those columns are controlled by `grid_auto_columns` property.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-template-columns>
    pub grid_template_columns: Vec<RepeatedGridTrack>,

    /// Defines the size of implicitly created rows. Rows are created implicitly when grid items are given explicit placements that are out of bounds
    /// of the rows explicitly created using `grid_template_rows`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-auto-rows>
    pub grid_auto_rows: Vec<GridTrack>,
    /// Defines the size of implicitly created columns. Columns are created implicitly when grid items are given explicit placements that are out of bounds
    /// of the columns explicitly created using `grid_template_columms`.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-template-columns>
    pub grid_auto_columns: Vec<GridTrack>,

    /// The row in which a grid item starts and how many rows it spans.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-row>
    pub grid_row: GridPlacement,

    /// The column in which a grid item starts and how many columns it spans.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-column>
    pub grid_column: GridPlacement,

    /// Used to add rounded corners to a UI node. You can set a UI node to have uniformly rounded corners
    /// or specify different radii for each corner. If a given radius exceeds half the length of the smallest dimension between the node's height or width,
    /// the radius will calculated as half the smallest dimension.
    ///
    /// Elliptical nodes are not supported yet. Percentage values are based on the node's smallest dimension, either width or height.
    ///
    /// # Example
    /// ```
    /// # use bevy_ui::{Style, UiRect, UiBorderRadius, Val};
    /// let style = Style {
    ///     // Set a uniform border radius of 10 logical pixels
    ///     border_radius: UiBorderRadius::all(Val::Px(10.)),
    ///     ..Default::default()
    /// };
    /// let style = Style {
    ///     border_radius: UiBorderRadius {
    ///         // The top left corner will be rounded with a radius of 10 logical pixels.
    ///         top_left: Val::Px(10.),
    ///         // Percentage values are based on the node's smallest dimension, either width or height.
    ///         top_right: Val::Percent(20.),
    ///         // Viewport coordinates can also be used.
    ///         bottom_left: Val::Vw(10.),
    ///         // The bottom right corner will be unrounded.
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// };
    /// ```
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/border-radius>
    pub border_radius: BorderRadius,
}

impl Style {
    pub const DEFAULT: Self = Self {
        display: Display::DEFAULT,
        position_type: PositionType::DEFAULT,
        left: Val::Auto,
        right: Val::Auto,
        top: Val::Auto,
        bottom: Val::Auto,
        direction: Direction::DEFAULT,
        flex_direction: FlexDirection::DEFAULT,
        flex_wrap: FlexWrap::DEFAULT,
        align_items: AlignItems::DEFAULT,
        justify_items: JustifyItems::DEFAULT,
        align_self: AlignSelf::DEFAULT,
        justify_self: JustifySelf::DEFAULT,
        align_content: AlignContent::DEFAULT,
        justify_content: JustifyContent::DEFAULT,
        margin: UiRect::DEFAULT,
        padding: UiRect::DEFAULT,
        border: UiRect::DEFAULT,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_basis: Val::Auto,
        width: Val::Auto,
        height: Val::Auto,
        min_width: Val::Auto,
        min_height: Val::Auto,
        max_width: Val::Auto,
        max_height: Val::Auto,
        aspect_ratio: None,
        overflow: Overflow::DEFAULT,
        row_gap: Val::Px(0.0),
        column_gap: Val::Px(0.0),
        grid_auto_flow: GridAutoFlow::DEFAULT,
        grid_template_rows: Vec::new(),
        grid_template_columns: Vec::new(),
        grid_auto_rows: Vec::new(),
        grid_auto_columns: Vec::new(),
        grid_column: GridPlacement::DEFAULT,
        grid_row: GridPlacement::DEFAULT,
        border_radius: BorderRadius::DEFAULT,
    };
}

impl Default for Style {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How items are aligned according to the cross axis
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum AlignItems {
    /// The items are packed in their default position as if no alignment was applied
    Default,
    /// Items are packed towards the start of the axis.
    Start,
    /// Items are packed towards the end of the axis.
    End,
    /// Items are packed towards the start of the axis, unless the flex direction is reversed;
    /// then they are packed towards the end of the axis.
    FlexStart,
    /// Items are packed towards the end of the axis, unless the flex direction is reversed;
    /// then they are packed towards the start of the axis.
    FlexEnd,
    /// Items are aligned at the center.
    Center,
    /// Items are aligned at the baseline.
    Baseline,
    /// Items are stretched across the whole cross axis.
    Stretch,
}

impl AlignItems {
    pub const DEFAULT: Self = Self::Default;
}

impl Default for AlignItems {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How items are aligned according to the cross axis
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum JustifyItems {
    /// The items are packed in their default position as if no alignment was applied
    Default,
    /// Items are packed towards the start of the axis.
    Start,
    /// Items are packed towards the end of the axis.
    End,
    /// Items are aligned at the center.
    Center,
    /// Items are aligned at the baseline.
    Baseline,
    /// Items are stretched across the whole cross axis.
    Stretch,
}

impl JustifyItems {
    pub const DEFAULT: Self = Self::Default;
}

impl Default for JustifyItems {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How this item is aligned according to the cross axis.
/// Overrides [`AlignItems`].
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum AlignSelf {
    /// Use the parent node's [`AlignItems`] value to determine how this item should be aligned.
    Auto,
    /// This item will be aligned with the start of the axis.
    Start,
    /// This item will be aligned with the end of the axis.
    End,
    /// This item will be aligned with the start of the axis, unless the flex direction is reversed;
    /// then it will be aligned with the end of the axis.
    FlexStart,
    /// This item will be aligned with the end of the axis, unless the flex direction is reversed;
    /// then it will be aligned with the start of the axis.
    FlexEnd,
    /// This item will be aligned at the center.
    Center,
    /// This item will be aligned at the baseline.
    Baseline,
    /// This item will be stretched across the whole cross axis.
    Stretch,
}

impl AlignSelf {
    pub const DEFAULT: Self = Self::Auto;
}

impl Default for AlignSelf {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How this item is aligned according to the main axis.
/// Overrides [`JustifyItems`].
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum JustifySelf {
    /// Use the parent node's [`JustifyItems`] value to determine how this item should be aligned.
    Auto,
    /// This item will be aligned with the start of the axis.
    Start,
    /// This item will be aligned with the end of the axis.
    End,
    /// This item will be aligned at the center.
    Center,
    /// This item will be aligned at the baseline.
    Baseline,
    /// This item will be stretched across the whole main axis.
    Stretch,
}

impl JustifySelf {
    pub const DEFAULT: Self = Self::Auto;
}

impl Default for JustifySelf {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defines how each line is aligned within the flexbox.
///
/// It only applies if [`FlexWrap::Wrap`] is present and if there are multiple lines of items.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum AlignContent {
    /// The items are packed in their default position as if no alignment was applied
    Default,
    /// Each line moves towards the start of the cross axis.
    Start,
    /// Each line moves towards the end of the cross axis.
    End,
    /// Each line moves towards the start of the cross axis, unless the flex direction is reversed; then the line moves towards the end of the cross axis.
    FlexStart,
    /// Each line moves towards the end of the cross axis, unless the flex direction is reversed; then the line moves towards the start of the cross axis.
    FlexEnd,
    /// Each line moves towards the center of the cross axis.
    Center,
    /// Each line will stretch to fill the remaining space.
    Stretch,
    /// Each line fills the space it needs, putting the remaining space, if any
    /// inbetween the lines.
    SpaceBetween,
    /// The gap between the first and last items is exactly THE SAME as the gap between items.
    /// The gaps are distributed evenly.
    SpaceEvenly,
    /// Each line fills the space it needs, putting the remaining space, if any
    /// around the lines.
    SpaceAround,
}

impl AlignContent {
    pub const DEFAULT: Self = Self::Default;
}

impl Default for AlignContent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defines how items are aligned according to the main axis
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum JustifyContent {
    /// The items are packed in their default position as if no alignment was applied
    Default,
    /// Items are packed toward the start of the axis.
    Start,
    /// Items are packed toward the end of the axis.
    End,
    /// Pushed towards the start, unless the flex direction is reversed; then pushed towards the end.
    FlexStart,
    /// Pushed towards the end, unless the flex direction is reversed; then pushed towards the start.
    FlexEnd,
    /// Centered along the main axis.
    Center,
    /// Remaining space is distributed between the items.
    SpaceBetween,
    /// Remaining space is distributed around the items.
    SpaceAround,
    /// Like [`JustifyContent::SpaceAround`] but with even spacing between items.
    SpaceEvenly,
}

impl JustifyContent {
    pub const DEFAULT: Self = Self::Default;
}

impl Default for JustifyContent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defines the text direction
///
/// For example English is written LTR (left-to-right) while Arabic is written RTL (right-to-left).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum Direction {
    /// Inherit from parent node.
    Inherit,
    /// Text is written left to right.
    LeftToRight,
    /// Text is written right to left.
    RightToLeft,
}

impl Direction {
    pub const DEFAULT: Self = Self::Inherit;
}

impl Default for Direction {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Whether to use a Flexbox layout model.
///
/// Part of the [`Style`] component.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum Display {
    /// Use Flexbox layout model to determine the position of this [`Node`].
    Flex,
    /// Use CSS Grid layout model to determine the position of this [`Node`].
    Grid,
    /// Use no layout, don't render this node and its children.
    ///
    /// If you want to hide a node and its children,
    /// but keep its layout in place, set its [`Visibility`](bevy_render::view::Visibility) component instead.
    None,
}

impl Display {
    pub const DEFAULT: Self = Self::Flex;
}

impl Default for Display {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defines how flexbox items are ordered within a flexbox
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum FlexDirection {
    /// Same way as text direction along the main axis.
    Row,
    /// Flex from top to bottom.
    Column,
    /// Opposite way as text direction along the main axis.
    RowReverse,
    /// Flex from bottom to top.
    ColumnReverse,
}

impl FlexDirection {
    pub const DEFAULT: Self = Self::Row;
}

impl Default for FlexDirection {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Whether to show or hide overflowing items
#[derive(Copy, Clone, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct Overflow {
    /// Whether to show or clip overflowing items on the x axis        
    pub x: OverflowAxis,
    /// Whether to show or clip overflowing items on the y axis
    pub y: OverflowAxis,
}

impl Overflow {
    pub const DEFAULT: Self = Self {
        x: OverflowAxis::DEFAULT,
        y: OverflowAxis::DEFAULT,
    };

    /// Show overflowing items on both axes
    pub const fn visible() -> Self {
        Self {
            x: OverflowAxis::Visible,
            y: OverflowAxis::Visible,
        }
    }

    /// Clip overflowing items on both axes
    pub const fn clip() -> Self {
        Self {
            x: OverflowAxis::Clip,
            y: OverflowAxis::Clip,
        }
    }

    /// Clip overflowing items on the x axis
    pub const fn clip_x() -> Self {
        Self {
            x: OverflowAxis::Clip,
            y: OverflowAxis::Visible,
        }
    }

    /// Clip overflowing items on the y axis
    pub const fn clip_y() -> Self {
        Self {
            x: OverflowAxis::Visible,
            y: OverflowAxis::Clip,
        }
    }

    /// Overflow is visible on both axes
    pub const fn is_visible(&self) -> bool {
        self.x.is_visible() && self.y.is_visible()
    }
}

impl Default for Overflow {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Whether to show or hide overflowing items
#[derive(Copy, Clone, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum OverflowAxis {
    /// Show overflowing items.
    Visible,
    /// Hide overflowing items.
    Clip,
}

impl OverflowAxis {
    pub const DEFAULT: Self = Self::Visible;

    /// Overflow is visible on this axis
    pub const fn is_visible(&self) -> bool {
        matches!(self, Self::Visible)
    }
}

impl Default for OverflowAxis {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The strategy used to position this node
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum PositionType {
    /// Relative to all other nodes with the [`PositionType::Relative`] value.
    Relative,
    /// Independent of all other nodes.
    ///
    /// As usual, the `Style.position` field of this node is specified relative to its parent node.
    Absolute,
}

impl PositionType {
    const DEFAULT: Self = Self::Relative;
}

impl Default for PositionType {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Defines if flexbox items appear on a single line or on multiple lines
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum FlexWrap {
    /// Single line, will overflow if needed.
    NoWrap,
    /// Multiple lines, if needed.
    Wrap,
    /// Same as [`FlexWrap::Wrap`] but new lines will appear before the previous one.
    WrapReverse,
}

impl FlexWrap {
    const DEFAULT: Self = Self::NoWrap;
}

impl Default for FlexWrap {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Controls whether grid items are placed row-wise or column-wise. And whether the sparse or dense packing algorithm is used.
///
/// The "dense" packing algorithm attempts to fill in holes earlier in the grid, if smaller items come up later. This may cause items to appear out-of-order, when doing so would fill in holes left by larger items.
///
/// Defaults to [`GridAutoFlow::Row`]
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/grid-auto-flow>
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum GridAutoFlow {
    /// Items are placed by filling each row in turn, adding new rows as necessary
    Row,
    /// Items are placed by filling each column in turn, adding new columns as necessary.
    Column,
    /// Combines `Row` with the dense packing algorithm.
    RowDense,
    /// Combines `Column` with the dense packing algorithm.
    ColumnDense,
}

impl GridAutoFlow {
    const DEFAULT: Self = Self::Row;
}

impl Default for GridAutoFlow {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect_value(PartialEq, Serialize, Deserialize)]
pub enum MinTrackSizingFunction {
    /// Track minimum size should be a fixed pixel value
    Px(f32),
    /// Track minimum size should be a percentage value
    Percent(f32),
    /// Track minimum size should be content sized under a min-content constraint
    MinContent,
    /// Track minimum size should be content sized under a max-content constraint
    MaxContent,
    /// Track minimum size should be automatically sized
    Auto,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect_value(PartialEq, Serialize, Deserialize)]
pub enum MaxTrackSizingFunction {
    /// Track maximum size should be a fixed pixel value
    Px(f32),
    /// Track maximum size should be a percentage value
    Percent(f32),
    /// Track maximum size should be content sized under a min-content constraint
    MinContent,
    /// Track maximum size should be content sized under a max-content constraint
    MaxContent,
    /// Track maximum size should be sized according to the fit-content formula with a fixed pixel limit
    FitContentPx(f32),
    /// Track maximum size should be sized according to the fit-content formula with a percentage limit
    FitContentPercent(f32),
    /// Track maximum size should be automatically sized
    Auto,
    /// The dimension as a fraction of the total available grid space (`fr` units in CSS)
    /// Specified value is the numerator of the fraction. Denominator is the sum of all fractions specified in that grid dimension
    /// Spec: <https://www.w3.org/TR/css3-grid-layout/#fr-unit>
    Fraction(f32),
}

/// A [`GridTrack`] is a Row or Column of a CSS Grid. This struct specifies what size the track should be.
/// See below for the different "track sizing functions" you can specify.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct GridTrack {
    pub(crate) min_sizing_function: MinTrackSizingFunction,
    pub(crate) max_sizing_function: MaxTrackSizingFunction,
}

impl GridTrack {
    const DEFAULT: Self = Self {
        min_sizing_function: MinTrackSizingFunction::Auto,
        max_sizing_function: MaxTrackSizingFunction::Auto,
    };

    /// Create a grid track with a fixed pixel size
    pub fn px<T: From<Self>>(value: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Px(value),
            max_sizing_function: MaxTrackSizingFunction::Px(value),
        }
        .into()
    }

    /// Create a grid track with a percentage size
    pub fn percent<T: From<Self>>(value: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Percent(value),
            max_sizing_function: MaxTrackSizingFunction::Percent(value),
        }
        .into()
    }

    /// Create a grid track with an `fr` size.
    /// Note that this will give the track a content-based minimum size.
    /// Usually you are best off using `GridTrack::flex` instead which uses a zero minimum size
    pub fn fr<T: From<Self>>(value: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Auto,
            max_sizing_function: MaxTrackSizingFunction::Fraction(value),
        }
        .into()
    }

    /// Create a grid track with an `minmax(0, Nfr)` size.
    pub fn flex<T: From<Self>>(value: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Px(0.0),
            max_sizing_function: MaxTrackSizingFunction::Fraction(value),
        }
        .into()
    }

    /// Create a grid track which is automatically sized to fit it's contents, and then
    pub fn auto<T: From<Self>>() -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Auto,
            max_sizing_function: MaxTrackSizingFunction::Auto,
        }
        .into()
    }

    /// Create a grid track which is automatically sized to fit it's contents when sized at their "min-content" sizes
    pub fn min_content<T: From<Self>>() -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::MinContent,
            max_sizing_function: MaxTrackSizingFunction::MinContent,
        }
        .into()
    }

    /// Create a grid track which is automatically sized to fit it's contents when sized at their "max-content" sizes
    pub fn max_content<T: From<Self>>() -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::MaxContent,
            max_sizing_function: MaxTrackSizingFunction::MaxContent,
        }
        .into()
    }

    /// Create a fit-content() grid track with fixed pixel limit
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/fit-content_function>
    pub fn fit_content_px<T: From<Self>>(limit: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Auto,
            max_sizing_function: MaxTrackSizingFunction::FitContentPx(limit),
        }
        .into()
    }

    /// Create a fit-content() grid track with percentage limit
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/fit-content_function>
    pub fn fit_content_percent<T: From<Self>>(limit: f32) -> T {
        Self {
            min_sizing_function: MinTrackSizingFunction::Auto,
            max_sizing_function: MaxTrackSizingFunction::FitContentPercent(limit),
        }
        .into()
    }

    /// Create a minmax() grid track
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/minmax>
    pub fn minmax<T: From<Self>>(min: MinTrackSizingFunction, max: MaxTrackSizingFunction) -> T {
        Self {
            min_sizing_function: min,
            max_sizing_function: max,
        }
        .into()
    }
}

impl Default for GridTrack {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
/// How many times to repeat a repeated grid track
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/repeat>
pub enum GridTrackRepetition {
    /// Repeat the track fixed number of times
    Count(u16),
    /// Repeat the track to fill available space
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/repeat#auto-fill>
    AutoFill,
    /// Repeat the track to fill available space but collapse any tracks that do not end up with
    /// an item placed in them.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/CSS/repeat#auto-fit>
    AutoFit,
}

impl From<u16> for GridTrackRepetition {
    fn from(count: u16) -> Self {
        Self::Count(count)
    }
}

impl From<i32> for GridTrackRepetition {
    fn from(count: i32) -> Self {
        Self::Count(count as u16)
    }
}

impl From<usize> for GridTrackRepetition {
    fn from(count: usize) -> Self {
        Self::Count(count as u16)
    }
}

/// Represents a *possibly* repeated [`GridTrack`].
///
/// The repetition parameter can either be:
///   - The integer `1`, in which case the track is non-repeated.
///   - a `u16` count to repeat the track N times
///   - A `GridTrackRepetition::AutoFit` or `GridTrackRepetition::AutoFill`
///
/// Note: that in the common case you want a non-repeating track (repetition count 1), you may use the constructor methods on [`GridTrack`]
/// to create a `RepeatedGridTrack`. i.e. `GridTrack::px(10.0)` is equivalent to `RepeatedGridTrack::px(1, 10.0)`.
///
/// You may only use one auto-repetition per track list. And if your track list contains an auto repetition
/// then all track (in and outside of the repetition) must be fixed size (px or percent). Integer repetitions are just shorthand for writing out
/// N tracks longhand and are not subject to the same limitations.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct RepeatedGridTrack {
    pub(crate) repetition: GridTrackRepetition,
    pub(crate) tracks: SmallVec<[GridTrack; 1]>,
}

impl RepeatedGridTrack {
    /// Create a repeating set of grid tracks with a fixed pixel size
    pub fn px<T: From<Self>>(repetition: impl Into<GridTrackRepetition>, value: f32) -> T {
        Self {
            repetition: repetition.into(),
            tracks: SmallVec::from_buf([GridTrack::px(value)]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with a percentage size
    pub fn percent<T: From<Self>>(repetition: impl Into<GridTrackRepetition>, value: f32) -> T {
        Self {
            repetition: repetition.into(),
            tracks: SmallVec::from_buf([GridTrack::percent(value)]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with automatic size
    pub fn auto<T: From<Self>>(repetition: u16) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::auto()]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with an `fr` size.
    /// Note that this will give the track a content-based minimum size.
    /// Usually you are best off using `GridTrack::flex` instead which uses a zero minimum size
    pub fn fr<T: From<Self>>(repetition: u16, value: f32) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::fr(value)]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with an `minmax(0, Nfr)` size.
    pub fn flex<T: From<Self>>(repetition: u16, value: f32) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::flex(value)]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with min-content size
    pub fn min_content<T: From<Self>>(repetition: u16) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::min_content()]),
        }
        .into()
    }

    /// Create a repeating set of grid tracks with max-content size
    pub fn max_content<T: From<Self>>(repetition: u16) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::max_content()]),
        }
        .into()
    }

    /// Create a repeating set of fit-content() grid tracks with fixed pixel limit
    pub fn fit_content_px<T: From<Self>>(repetition: u16, limit: f32) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::fit_content_px(limit)]),
        }
        .into()
    }

    /// Create a repeating set of fit-content() grid tracks with percentage limit
    pub fn fit_content_percent<T: From<Self>>(repetition: u16, limit: f32) -> T {
        Self {
            repetition: GridTrackRepetition::Count(repetition),
            tracks: SmallVec::from_buf([GridTrack::fit_content_percent(limit)]),
        }
        .into()
    }

    /// Create a repeating set of minmax() grid track
    pub fn minmax<T: From<Self>>(
        repetition: impl Into<GridTrackRepetition>,
        min: MinTrackSizingFunction,
        max: MaxTrackSizingFunction,
    ) -> T {
        Self {
            repetition: repetition.into(),
            tracks: SmallVec::from_buf([GridTrack::minmax(min, max)]),
        }
        .into()
    }

    /// Create a repetition of a set of tracks
    pub fn repeat_many<T: From<Self>>(
        repetition: impl Into<GridTrackRepetition>,
        tracks: impl Into<Vec<GridTrack>>,
    ) -> T {
        Self {
            repetition: repetition.into(),
            tracks: SmallVec::from_vec(tracks.into()),
        }
        .into()
    }
}

impl From<GridTrack> for RepeatedGridTrack {
    fn from(track: GridTrack) -> Self {
        Self {
            repetition: GridTrackRepetition::Count(1),
            tracks: SmallVec::from_buf([track]),
        }
    }
}

impl From<GridTrack> for Vec<GridTrack> {
    fn from(track: GridTrack) -> Self {
        vec![GridTrack {
            min_sizing_function: track.min_sizing_function,
            max_sizing_function: track.max_sizing_function,
        }]
    }
}

impl From<GridTrack> for Vec<RepeatedGridTrack> {
    fn from(track: GridTrack) -> Self {
        vec![RepeatedGridTrack {
            repetition: GridTrackRepetition::Count(1),
            tracks: SmallVec::from_buf([track]),
        }]
    }
}

impl From<RepeatedGridTrack> for Vec<RepeatedGridTrack> {
    fn from(track: RepeatedGridTrack) -> Self {
        vec![track]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
/// Represents the position of a grid item in a single axis.
///
/// There are 3 fields which may be set:
///   - `start`: which grid line the item should start at
///   - `end`: which grid line the item should end at
///   - `span`: how many tracks the item should span
///
/// The default `span` is 1. If neither `start` or `end` is set then the item will be placed automatically.
///
/// Generally, at most two fields should be set. If all three fields are specified then `span` will be ignored. If `end` specifies an earlier
/// grid line than `start` then `end` will be ignored and the item will have a span of 1.
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Grid_Layout/Line-based_Placement_with_CSS_Grid>
pub struct GridPlacement {
    /// The grid line at which the item should start. Lines are 1-indexed. Negative indexes count backwards from the end of the grid. Zero is not a valid index.
    pub(crate) start: Option<NonZeroI16>,
    /// How many grid tracks the item should span. Defaults to 1.
    pub(crate) span: Option<NonZeroU16>,
    /// The grid line at which the item should end. Lines are 1-indexed. Negative indexes count backwards from the end of the grid. Zero is not a valid index.
    pub(crate) end: Option<NonZeroI16>,
}

impl GridPlacement {
    pub const DEFAULT: Self = Self {
        start: None,
        span: Some(unsafe { NonZeroU16::new_unchecked(1) }),
        end: None,
    };

    /// Place the grid item automatically (letting the `span` default to `1`).
    pub fn auto() -> Self {
        Self::DEFAULT
    }

    /// Place the grid item automatically, specifying how many tracks it should `span`.
    ///
    /// # Panics
    ///
    /// Panics if `span` is `0`
    pub fn span(span: u16) -> Self {
        Self {
            start: None,
            end: None,
            span: try_into_grid_span(span).expect("Invalid span value of 0."),
        }
    }

    /// Place the grid item specifying the `start` grid line (letting the `span` default to `1`).
    ///
    /// # Panics
    ///
    /// Panics if `start` is `0`
    pub fn start(start: i16) -> Self {
        Self {
            start: try_into_grid_index(start).expect("Invalid start value of 0."),
            ..Self::DEFAULT
        }
    }

    /// Place the grid item specifying the `end` grid line (letting the `span` default to `1`).
    ///
    /// # Panics
    ///
    /// Panics if `end` is `0`
    pub fn end(end: i16) -> Self {
        Self {
            end: try_into_grid_index(end).expect("Invalid end value of 0."),
            ..Self::DEFAULT
        }
    }

    /// Place the grid item specifying the `start` grid line and how many tracks it should `span`.
    ///
    /// # Panics
    ///
    /// Panics if `start` or `span` is `0`
    pub fn start_span(start: i16, span: u16) -> Self {
        Self {
            start: try_into_grid_index(start).expect("Invalid start value of 0."),
            end: None,
            span: try_into_grid_span(span).expect("Invalid span value of 0."),
        }
    }

    /// Place the grid item specifying `start` and `end` grid lines (`span` will be inferred)
    ///
    /// # Panics
    ///
    /// Panics if `start` or `end` is `0`
    pub fn start_end(start: i16, end: i16) -> Self {
        Self {
            start: try_into_grid_index(start).expect("Invalid start value of 0."),
            end: try_into_grid_index(end).expect("Invalid end value of 0."),
            span: None,
        }
    }

    /// Place the grid item specifying the `end` grid line and how many tracks it should `span`.
    ///
    /// # Panics
    ///
    /// Panics if `end` or `span` is `0`
    pub fn end_span(end: i16, span: u16) -> Self {
        Self {
            start: None,
            end: try_into_grid_index(end).expect("Invalid end value of 0."),
            span: try_into_grid_span(span).expect("Invalid span value of 0."),
        }
    }

    /// Mutate the item, setting the `start` grid line
    ///
    /// # Panics
    ///
    /// Panics if `start` is `0`
    pub fn set_start(mut self, start: i16) -> Self {
        self.start = try_into_grid_index(start).expect("Invalid start value of 0.");
        self
    }

    /// Mutate the item, setting the `end` grid line
    ///
    /// # Panics
    ///
    /// Panics if `end` is `0`
    pub fn set_end(mut self, end: i16) -> Self {
        self.end = try_into_grid_index(end).expect("Invalid end value of 0.");
        self
    }

    /// Mutate the item, setting the number of tracks the item should `span`
    ///
    /// # Panics
    ///
    /// Panics if `span` is `0`
    pub fn set_span(mut self, span: u16) -> Self {
        self.span = try_into_grid_span(span).expect("Invalid span value of 0.");
        self
    }

    /// Returns the grid line at which the item should start, or `None` if not set.
    pub fn get_start(self) -> Option<i16> {
        self.start.map(NonZeroI16::get)
    }

    /// Returns the grid line at which the item should end, or `None` if not set.
    pub fn get_end(self) -> Option<i16> {
        self.end.map(NonZeroI16::get)
    }

    /// Returns span for this grid item, or `None` if not set.
    pub fn get_span(self) -> Option<u16> {
        self.span.map(NonZeroU16::get)
    }
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Convert an `i16` to `NonZeroI16`, fails on `0` and returns the `InvalidZeroIndex` error.
fn try_into_grid_index(index: i16) -> Result<Option<NonZeroI16>, GridPlacementError> {
    Ok(Some(
        NonZeroI16::new(index).ok_or(GridPlacementError::InvalidZeroIndex)?,
    ))
}

/// Convert a `u16` to `NonZeroU16`, fails on `0` and returns the `InvalidZeroSpan` error.
fn try_into_grid_span(span: u16) -> Result<Option<NonZeroU16>, GridPlacementError> {
    Ok(Some(
        NonZeroU16::new(span).ok_or(GridPlacementError::InvalidZeroSpan)?,
    ))
}

/// Errors that occur when setting contraints for a `GridPlacement`
#[derive(Debug, Eq, PartialEq, Clone, Copy, Error)]
pub enum GridPlacementError {
    #[error("Zero is not a valid grid position")]
    InvalidZeroIndex,
    #[error("Spans cannot be zero length")]
    InvalidZeroSpan,
}

/// The background color of the node
///
/// This serves as the "fill" color.
/// When combined with [`UiImage`], tints the provided texture.
#[derive(Component, Clone, Debug, Reflect, Deref, DerefMut, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct BackgroundColor(pub UiColor);

impl<T> From<T> for BackgroundColor
where
    T: Into<UiColor>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl BackgroundColor {
    pub const DEFAULT: Self = Self(UiColor::Color(Color::WHITE));
}

impl Default for BackgroundColor {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The atlas sprite to be used in a UI Texture Atlas Node
#[derive(Component, Clone, Debug, Reflect, Default)]
#[reflect(Component, Default)]
pub struct UiTextureAtlasImage {
    /// Texture index in the TextureAtlas
    pub index: usize,
    /// Whether to flip the sprite in the X axis
    pub flip_x: bool,
    /// Whether to flip the sprite in the Y axis
    pub flip_y: bool,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub enum UiColor {
    Color(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
}

impl From<Color> for UiColor {
    fn from(value: Color) -> Self {
        Self::Color(value)
    }
}

impl From<LinearGradient> for UiColor {
    fn from(value: LinearGradient) -> Self {
        Self::LinearGradient(value)
    }
}

impl From<RadialGradient> for UiColor {
    fn from(value: RadialGradient) -> Self {
        Self::RadialGradient(value)
    }
}

impl UiColor {
    /// Is this UiColor visible?
    /// Always returns true for gradient values.
    pub fn is_visible(&self) -> bool {
        match self {
            Self::Color(color) => color.a() != 0.,
            _ => true,
        }
    }
}

/// The border color of the UI node.
#[derive(Component, Clone, Debug, Reflect, Deref, DerefMut)]
#[reflect(Component, Default)]
pub struct BorderColor(pub UiColor);

impl<T> From<T> for BorderColor
where
    T: Into<UiColor>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl BorderColor {
    pub const DEFAULT: Self = BorderColor(UiColor::Color(Color::WHITE));
}

impl Default for BorderColor {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Component, Copy, Clone, Default, Debug, Reflect)]
#[reflect(Component, Default)]
/// The [`Outline`] component adds an outline outside the edge of a UI node.
/// Outlines do not take up space in the layout
///
/// To add an [`Outline`] to a ui node you can spawn a `(NodeBundle, Outline)` tuple bundle:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ui::prelude::*;
/// # use bevy_render::prelude::Color;
/// fn setup_ui(mut commands: Commands) {
///     commands.spawn((
///         NodeBundle {
///             style: Style {
///                 width: Val::Px(100.),
///                 height: Val::Px(100.),
///                 ..Default::default()
///             },
///             background_color: Color::BLUE.into(),
///             ..Default::default()
///         },
///         Outline::new(Val::Px(10.), Val::ZERO, Color::RED)
///     ));
/// }
/// ```
///
/// [`Outline`] components can also be added later to existing UI nodes:
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ui::prelude::*;
/// # use bevy_render::prelude::Color;
/// fn outline_hovered_button_system(
///     mut commands: Commands,
///     mut node_query: Query<(Entity, &Interaction, Option<&mut Outline>), Changed<Interaction>>,
/// ) {
///     for (entity, interaction, mut maybe_outline) in node_query.iter_mut() {
///         let outline_color =
///             if matches!(*interaction, Interaction::Hovered) {
///                 Color::WHITE    
///             } else {
///                 Color::NONE
///             };
///         if let Some(mut outline) = maybe_outline {
///             outline.color = outline_color;
///         } else {
///             commands.entity(entity).insert(Outline::new(Val::Px(10.), Val::ZERO, outline_color));
///         }
///     }
/// }
/// ```
/// Inserting and removing an [`Outline`] component repeatedly will result in table moves, so it is generally preferable to
/// set `Outline::color` to `Color::NONE` to hide an outline.
pub struct Outline {
    /// The width of the outline.
    ///
    /// Percentage `Val` values are resolved based on the width of the outlined [`Node`]
    pub width: Val,
    /// The amount of space between a node's outline the edge of the node
    ///
    /// Percentage `Val` values are resolved based on the width of the outlined [`Node`]
    pub offset: Val,
    /// Color of the outline
    ///
    /// If you are frequently toggling outlines for a UI node on and off it is recommended to set `Color::None` to hide the outline.
    /// This avoids the table moves that would occcur from the repeated insertion and removal of the `Outline` component.
    pub color: Color,
}

impl Outline {
    /// Create a new outline
    pub const fn new(width: Val, offset: Val, color: Color) -> Self {
        Self {
            width,
            offset,
            color,
        }
    }
}

/// The 2D texture displayed for this UI node
#[derive(Component, Clone, Debug, Reflect, Default)]
#[reflect(Component, Default)]
pub struct UiImage {
    /// Handle to the texture
    pub texture: Handle<Image>,
    /// Whether the image should be flipped along its x-axis
    pub flip_x: bool,
    /// Whether the image should be flipped along its y-axis
    pub flip_y: bool,
}

impl UiImage {
    pub fn new(texture: Handle<Image>) -> Self {
        Self {
            texture,
            ..Default::default()
        }
    }

    /// flip the image along its x-axis
    #[must_use]
    pub const fn with_flip_x(mut self) -> Self {
        self.flip_x = true;
        self
    }

    /// flip the image along its y-axis
    #[must_use]
    pub const fn with_flip_y(mut self) -> Self {
        self.flip_y = true;
        self
    }
}

impl From<Handle<Image>> for UiImage {
    fn from(texture: Handle<Image>) -> Self {
        Self::new(texture)
    }
}

/// The calculated clip of the node
#[derive(Component, Default, Copy, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct CalculatedClip {
    /// The rect of the clip
    pub clip: Rect,
}

/// Indicates that this [`Node`] entity's front-to-back ordering is not controlled solely
/// by its location in the UI hierarchy. A node with a higher z-index will appear on top
/// of other nodes with a lower z-index.
///
/// UI nodes that have the same z-index will appear according to the order in which they
/// appear in the UI hierarchy. In such a case, the last node to be added to its parent
/// will appear in front of this parent's other children.
///
/// Nodes without this component will be treated as if they had a value of [`ZIndex(0)`].
#[derive(Component, Copy, Clone, Debug, Reflect)]
#[reflect(Component)]
pub enum ZIndex {
    /// Indicates the order in which this node should be rendered relative to its siblings.
    Local(i32),
    /// Indicates the order in which this node should be rendered relative to root nodes and
    /// all other nodes that have a global z-index.
    Global(i32),
}

impl Default for ZIndex {
    fn default() -> Self {
        Self::Local(0)
    }
}

/// Radii for rounded corner edges.
/// * A corner set to a 0 value will be right angled.
/// * The value is clamped to between 0 and half the length of the shortest side of the node before being used.
/// * `Val::AUTO` is resolved to `Val::Px(0.)`.
#[derive(Copy, Clone, Debug, PartialEq, Reflect)]
#[reflect(PartialEq)]
pub struct BorderRadius {
    pub top_left: Val,
    pub top_right: Val,
    pub bottom_left: Val,
    pub bottom_right: Val,
}

impl Default for BorderRadius {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl BorderRadius {
    pub const DEFAULT: Self = Self::ZERO;

    /// Zero curvature. All the corners will be right angled.
    pub const ZERO: Self = Self {
        top_left: Val::Px(0.),
        top_right: Val::Px(0.),
        bottom_right: Val::Px(0.),
        bottom_left: Val::Px(0.),
    };

    /// Maximum curvature. The Ui Node will take a capsule shape or circular if width and height are equal.
    pub const MAX: Self = Self {
        top_left: Val::Px(f32::MAX),
        top_right: Val::Px(f32::MAX),
        bottom_right: Val::Px(f32::MAX),
        bottom_left: Val::Px(f32::MAX),
    };

    #[inline]
    /// Set all four corners to the same curvature.
    pub const fn all(radius: Val) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_left: radius,
            bottom_right: radius,
        }
    }

    #[inline]
    pub fn new(top_left: Val, top_right: Val, bottom_right: Val, bottom_left: Val) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[inline]
    /// Sets the radii to logical pixel values.
    pub fn px(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left: Val::Px(top_left),
            top_right: Val::Px(top_right),
            bottom_right: Val::Px(bottom_right),
            bottom_left: Val::Px(bottom_left),
        }
    }

    #[inline]
    /// Sets the radii to percentage values.
    pub fn percent(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left: Val::Px(top_left),
            top_right: Val::Px(top_right),
            bottom_right: Val::Px(bottom_right),
            bottom_left: Val::Px(bottom_left),
        }
    }

    #[inline]
    /// Sets the radius for the top left corner.
    /// Remaining corners will be right-angled.
    pub fn top_left(radius: Val) -> Self {
        Self {
            top_left: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radius for the top right corner.
    /// Remaining corners will be right-angled.
    pub fn top_right(radius: Val) -> Self {
        Self {
            top_right: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radius for the bottom right corner.
    /// Remaining corners will be right-angled.
    pub fn bottom_right(radius: Val) -> Self {
        Self {
            bottom_right: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radius for the bottom left corner.
    /// Remaining corners will be right-angled.
    pub fn bottom_left(radius: Val) -> Self {
        Self {
            bottom_left: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radii for the top left and bottom left corners.
    /// Remaining corners will be right-angled.
    pub fn left(radius: Val) -> Self {
        Self {
            top_left: radius,
            bottom_left: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radii for the top right and bottom right corners.
    /// Remaining corners will be right-angled.
    pub fn right(radius: Val) -> Self {
        Self {
            top_right: radius,
            bottom_right: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radii for the top left and top right corners.
    /// Remaining corners will be right-angled.
    pub fn top(radius: Val) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Sets the radii for the bottom left and bottom right corners.
    /// Remaining corners will be right-angled.
    pub fn bottom(radius: Val) -> Self {
        Self {
            bottom_left: radius,
            bottom_right: radius,
            ..Default::default()
        }
    }
}

impl From<BorderRadius> for [Val; 4] {
    fn from(value: BorderRadius) -> Self {
        [
            value.top_left,
            value.top_right,
            value.bottom_right,
            value.bottom_left,
        ]
    }
}

/// Converts an angle from degrees into radians
///
/// formula: `angle * PI / 180.`
pub fn deg(angle: f32) -> f32 {
    angle * PI / 180.
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect, Default)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct ColorStop {
    pub color: Color,
    pub point: Val,
}

impl From<Color> for ColorStop {
    fn from(color: Color) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }
}

impl From<(Color, Val)> for ColorStop {
    fn from((color, val): (Color, Val)) -> Self {
        Self { color, point: val }
    }
}

pub fn resolve_color_stops(
    stops: &[ColorStop],
    len: f32,
    viewport_size: Vec2,
) -> Vec<(Color, f32)> {
    if stops.is_empty() {
        return vec![];
    }

    let mut out = stops
        .iter()
        .map(|ColorStop { color, point }| {
            (*color, point.resolve(len, viewport_size).unwrap_or(-1.))
        })
        .collect::<Vec<_>>();
    if out[0].1 < 0.0 {
        out[0].1 = 0.0;
    }

    if stops.len() == 1 {
        out.push(out[0]);
        return out;
    }

    let last = out.last_mut().unwrap();
    if last.1 < 0.0 {
        last.1 = len;
    }

    let mut current = 0.;
    for (_, point) in &mut out {
        if 0.0 <= *point {
            if *point < current {
                *point = current;
            }
            current = *point;
        }
    }

    let mut i = 1;
    while i < out.len() - 1 {
        if out[i].1 < 0.0 {
            let mut j = i + 1;
            while out[j].1 < 0.0 {
                dbg!(j);
                j += 1;
            }
            let n = 1 + j - i;
            dbg!(n);
            let mut s = out[i - 1].1;
            let d = (out[j].1 - s) / n as f32;
            while i < j {
                s += d;
                out[i].1 = s;
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Reflect, Component, Default)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct LinearGradient {
    pub angle: f32,
    pub stops: Vec<ColorStop>,
}

impl LinearGradient {
    /// Angle for a gradient from bottom to top
    pub const BOTTOM_TO_TOP: f32 = 0.;
    /// Angle for a gradient from left to right
    pub const LEFT_TO_RIGHT: f32 = FRAC_PI_2;
    /// Angle for a gradient from top to bottom
    pub const TOP_TO_BOTTOM: f32 = PI;
    /// Angle for a gradient from right to left
    pub const RIGHT_TO_LEFT: f32 = 1.5 * PI;

    pub fn simple(angle: f32, start_color: Color, end_color: Color) -> Self {
        Self {
            angle,
            stops: vec![start_color.into(), end_color.into()],
        }
    }

    pub fn new(angle: f32, stops: Vec<ColorStop>) -> Self {
        Self {
            angle,
            stops,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.stops.iter().all(|stop| stop.color.a() == 0.)
    }

    /// find start point and total length of gradient
    pub fn resolve_geometry(&self, node_rect: Rect) -> (Vec2, f32) {
        let x = self.angle.cos();
        let y = self.angle.sin();
        let dir = Vec2::new(x, y);

        // return the distance of point `p` from the line defined by point `o` and direction `dir`
        fn df_line(o: Vec2, dir: Vec2, p: Vec2) -> f32 {
            // project p onto the the o-dir line and then return the distance between p and the projection.
            return p.distance(o + dir * (p - o).dot(dir));
        }

        fn modulo(x: f32, m: f32) -> f32 {
            return x - m * (x / m).floor();
        }

        let reduced = modulo(self.angle, 2.0 * PI);
        let q = (reduced * 2.0 / PI) as i32;
        let start_point = match q {
            0 => vec2(-1., 1.) * node_rect.size(),
            1 => vec2(-1., -1.) * node_rect.size(),
            2 => vec2(1., -1.) * node_rect.size(),
            _ => vec2(1., 1.) * node_rect.size(),
        } * 0.5f32;

        let length = 2.0 * df_line(start_point, dir, Vec2::ZERO);
        (
            start_point,
            length
        )
    }

    pub fn bottom_to_top(stops: Vec<ColorStop>) -> LinearGradient {
        LinearGradient { angle: Self::BOTTOM_TO_TOP, stops }
    }

    pub fn left_to_right(stops: Vec<ColorStop>) -> LinearGradient {
        LinearGradient { angle: Self::LEFT_TO_RIGHT, stops }
    }

    pub fn top_to_bottom(stops: Vec<ColorStop>) -> LinearGradient {
        LinearGradient { angle: Self::TOP_TO_BOTTOM, stops }
    }

    pub fn right_to_left(stops: Vec<ColorStop>) -> LinearGradient {
        LinearGradient { angle: Self::RIGHT_TO_LEFT, stops }
    }
}

impl From<Color> for LinearGradient {
    fn from(color: Color) -> Self {
        Self::new(0., vec![color.into(), color.into()])
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(PartialEq, Serialize, Deserialize)]
/// The gradient's ending shape
pub enum RadialGradientShape {
    /// The shape is a circle with a `Val` radius
    /// Percentage values are based on the node's width.
    /// `Val::Auto` resolves to 50% of the width.
    CircleRadius(Val),
    /// The size of the circle is determined automatically from the given `RadialGradientSize`.
    CircleSized(RadialGradientSize),
    /// The shape is an axis aligned ellipse.
    /// The first `Val` sets the distance from the center of the ellipse to its edge along its horiontal axis.
    /// The second `Val`sets the distance from the center of the ellipse to its edge along its vertical axis.
    /// * Percentage lengths are based on the width of the node
    Ellipse(Val, Val),
    /// The size of the ellipse is determined automatically from the given `RadialGradientSize`.
    EllipseSized(RadialGradientSize),
}

impl Default for RadialGradientShape {
    fn default() -> Self {
        Self::CircleSized(RadialGradientSize::FarthestCorner)
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect, Default)]
#[reflect(PartialEq, Serialize, Deserialize)]
/// Determines the size of the gradient's ending shape.
pub enum RadialGradientSize {
    /// The gradient's ending shape meets the side of the node closest to its center.
    ClosestSide,
    /// The gradient's ending shape is sized so that it exactly meets the closest corner of the node from its center.
    ClosestCorner,
    /// Similar to `ClosestSide``, except the ending shape is sized to meet the side of the node farthest from its center (or vertical and horizontal sides).
    FarthestSide,
    /// The default value, the gradient's ending shape is sized so that it exactly meets the farthest corner of the node from its center.
    #[default]
    FarthestCorner,
}

/// Representation of an axis-aligned ellipse.
#[derive(Default, Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Reflect)]
#[reflect(Default, PartialEq, Serialize, Deserialize)]
pub struct Ellipse {
    /// The center of the ellipse
    pub center: Vec2,
    /// The distances from the center of the ellipse to its edge, along its horizontal and vertical axes respectively.
    pub extents: Vec2,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Reflect, Component, Default)]
#[reflect(PartialEq, Serialize, Deserialize)]
pub struct RadialGradient {
    pub center: RectPosition,
    pub shape: RadialGradientShape,
    pub stops: Vec<ColorStop>,
}

impl RadialGradient {
    /// A circular gradient from `start_color` to `end_color` sized using `FarthestCorner``.
    pub fn simple(start_color: Color, end_color: Color) -> Self {
        Self {
            center: RectPosition::CENTER,
            shape: RadialGradientShape::default(),
            stops: vec![start_color.into(), end_color.into()],
        }
    }

    pub fn is_visible(&self) -> bool {
        self.stops.iter().all(|stop| stop.color.a() == 0.)
    }

    pub fn new(center: RectPosition, shape: RadialGradientShape, stops: Vec<ColorStop>) -> Self {
        Self {
            center,
            shape,
            stops,
        }
    }

    /// Resolve the shape and position of the gradient
    pub fn resolve_geometry(&self, node_rect: Rect, viewport_size: Vec2) -> Ellipse {
        let center = self.center.resolve(node_rect, viewport_size);

        fn shortest(p: Vec2, r: Rect) -> Vec2 {
            let d_min = (p - r.min).abs();
            let d_max = (p - r.max).abs();
            d_min.min(d_max)
        }

        fn longest(p: Vec2, r: Rect) -> Vec2 {
            let d_min = (p - r.min).abs();
            let d_max = (p - r.max).abs();
            d_min.max(d_max)
        }

        fn closest(p: f32, a: f32, b: f32) -> f32 {
            if (p - a).abs() < (p - b).abs() {
                a
            } else {
                b
            }
        }

        fn farthest(p: f32, a: f32, b: f32) -> f32 {
            if (p - a).abs() < (p - b).abs() {
                b
            } else {
                a
            }
        }

        fn closest_corner(p: Vec2, r: Rect) -> Vec2 {
            vec2(
                closest(p.x, r.min.x, r.max.x),
                closest(p.y, r.min.y, r.max.x),
            )
        }

        fn farthest_corner(p: Vec2, r: Rect) -> Vec2 {
            vec2(
                farthest(p.x, r.min.x, r.max.x),
                farthest(p.y, r.min.y, r.max.y),
            )
        }

        let extents = match self.shape {
            RadialGradientShape::CircleRadius(r) => Vec2::splat(
                r.resolve(node_rect.width(), viewport_size)
                    .unwrap_or(farthest_corner(center, node_rect).distance(center)),
            ),
            RadialGradientShape::CircleSized(shape) => match shape {
                RadialGradientSize::ClosestSide => {
                    Vec2::splat(shortest(center, node_rect).min_element())
                }
                RadialGradientSize::ClosestCorner => {
                    Vec2::splat(closest_corner(center, node_rect).distance(center))
                }
                RadialGradientSize::FarthestSide => {
                    Vec2::splat(longest(center, node_rect).max_element())
                }
                RadialGradientSize::FarthestCorner => {
                    Vec2::splat(farthest_corner(center, node_rect).distance(center))
                }
            },
            RadialGradientShape::Ellipse(w, h) => {
                let w = w.resolve(node_rect.width(), viewport_size).ok();
                let h = h.resolve(node_rect.width(), viewport_size).ok();
                match (w, h) {
                    (None, None) => (center - farthest_corner(center, node_rect)).abs(),
                    (Some(w), None) => Vec2::splat(w),
                    (None, Some(h)) => Vec2::splat(h),
                    (Some(w), Some(h)) => vec2(w, h),
                }
            }
            RadialGradientShape::EllipseSized(shape) => match shape {
                RadialGradientSize::ClosestSide => shortest(center, node_rect),
                RadialGradientSize::ClosestCorner => closest_corner(center, node_rect),
                RadialGradientSize::FarthestSide => longest(center, node_rect),
                RadialGradientSize::FarthestCorner => farthest_corner(center, node_rect),
            },
        };
        Ellipse { center, extents }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_two_stops() {
        let stops = vec![
            ColorStop {
                color: Color::WHITE,
                point: Val::Auto,
            },
            ColorStop {
                color: Color::BLACK,
                point: Val::Auto,
            },
        ];

        let r = resolve_color_stops(&stops, 1., Vec2::ZERO);

        assert_eq!(r.len(), 2);
        assert_eq!(r[0].1, 0.0);
        assert_eq!(r[1].1, 1.0);

        let stops = vec![
            ColorStop {
                color: Color::WHITE,
                point: Val::Auto,
            },
            ColorStop {
                color: Color::RED,
                point: Val::Auto,
            },
            ColorStop {
                color: Color::GREEN,
                point: Val::Auto,
            },
            ColorStop {
                color: Color::YELLOW,
                point: Val::Auto,
            },
            ColorStop {
                color: Color::BLACK,
                point: Val::Auto,
            },
        ];

        let r = resolve_color_stops(&stops, 1., Vec2::ZERO);

        assert_eq!(r.len(), 5);
        assert_eq!(r[0].1, 0.0);
        assert_eq!(r[1].1, 0.25);
        assert_eq!(r[2].1, 0.5);
        assert_eq!(r[3].1, 0.75);
        assert_eq!(r[4].1, 1.0);
    }
}

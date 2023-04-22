use taffy::layout::SizingMode;
use taffy::prelude::*;

use super::layout_tree::UiLayoutTree;

/// Updates the stored layout of the provided `node` and its children
pub fn compute_layout(
    ui_layout_tree: &mut UiLayoutTree,
    root: Node,
    available_space: Size<AvailableSpace>,
) -> Result<(), taffy::error::TaffyError> {
    // Recursively compute node layout
    let size_and_baselines = layout_flexbox(
        ui_layout_tree,
        root,
        Size::NONE,
        available_space.into_options(),
        available_space,
        SizingMode::InherentSize,
    );

    let layout = Layout {
        order: 0,
        size: size_and_baselines.size,
        location: taffy::geometry::Point::ZERO,
    };
    *ui_layout_tree.layout_mut(root) = layout;

    // If rounding is enabled, recursively round the layout's of this node and all children
    if ui_layout_tree.config.use_rounding {
        round_layout(ui_layout_tree, root, 0.0, 0.0);
    }

    Ok(())
}

fn round_layout(tree: &mut impl LayoutTree, node: Node, abs_x: f32, abs_y: f32) {
    let layout = tree.layout_mut(node);
    let abs_x = abs_x + layout.location.x;
    let abs_y = abs_y + layout.location.y;

    layout.location.x = layout.location.x.round();
    layout.location.y = layout.location.y.round();
    layout.size.width = (abs_x + layout.size.width).round() - abs_x.round();
    layout.size.height = (abs_y + layout.size.height).round() - abs_y.round();

    let child_count = tree.child_count(node);
    for index in 0..child_count {
        let child = tree.child(node, index);
        round_layout(tree, child, abs_x, abs_y);
    }
}

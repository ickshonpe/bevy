use taffy::prelude::AvailableSpace;
use taffy::prelude::Size;

pub fn measure_text(
    constraints: Size<Option<f32>>,
    mut size: Size<f32>,
    min_size: Size<f32>,
    max_size: Size<f32>,
    ideal_height: f32,
    _available: Size<AvailableSpace>,
) -> Size<f32> {
    match (constraints.width, constraints.height) {
        (None, None) => {
            // with no constraints
            // ask for maximum width space for text with no wrapping
            size.width = max_size.width;
            size.height = min_size.height;
        }
        (Some(width), None) => {
            size.width = width;
            size.height = ideal_height;
        }
        (None, Some(height)) => {
            size.height = height;
            size.width = max_size.width;
        }
        (Some(width), Some(height)) => {
            size.width = width;
            size.height = height;
        }
    }
    size.width = size.width.ceil();
    size.height = size.height.ceil();
    size
}


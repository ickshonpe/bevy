use taffy::prelude::AvailableSpace;
use taffy::prelude::Size;

pub fn measure_text(
    constraints: Size<Option<f32>>,
    mut size: Size<f32>,
    min_size: Size<f32>,
    max_size: Size<f32>,
    ideal_height: f32,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    println!("Text2: width: {:?}, height: {:?}", constraints.width, constraints.height);
    match (constraints.width, constraints.height) {
        (None, None) => {
            size.width = max_size.width;
            size.height = ideal_height;
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
    size
}
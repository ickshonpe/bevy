use crate::{Image, TextureAccessError, TextureAtlasLayout, TextureFormatPixelInfo as _};
use bevy_asset::RenderAssetUsages;
use bevy_math::{URect, UVec2};
use guillotiere::{size2, Allocation, AtlasAllocator};
use thiserror::Error;

/// An error produced by [`DynamicTextureAtlasBuilder`] when trying to add a new
/// texture to a [`TextureAtlasLayout`].
#[derive(Debug, Error)]
pub enum DynamicTextureAtlasBuilderError {
    /// Unable to allocate space within the atlas for the new texture
    #[error("Couldn't allocate space to add the image requested")]
    FailedToAllocateSpace,
    /// Attempted to add a texture to an uninitialized atlas
    #[error("cannot add texture to uninitialized atlas texture")]
    UninitializedAtlas,
    /// Attempted to add an uninitialized texture to an atlas
    #[error("cannot add uninitialized texture to atlas")]
    UninitializedSourceTexture,
    /// A texture access error occurred
    #[error("texture access error: {0}")]
    TextureAccess(#[from] TextureAccessError),
}

/// Helper utility to update [`TextureAtlasLayout`] on the fly.
///
/// Helpful in cases when texture is created procedurally,
/// e.g: in a font glyph [`TextureAtlasLayout`], only add the [`Image`] texture for letters to be rendered.
pub struct DynamicTextureAtlasBuilder {
    atlas_allocator: AtlasAllocator,
    padding: u32,
    extrude_textures: bool,
}

impl DynamicTextureAtlasBuilder {
    /// Create a new [`DynamicTextureAtlasBuilder`]
    ///
    /// # Arguments
    ///
    /// * `size` - total size for the atlas
    /// * `padding` - gap added between textures in the atlas (and the atlas edge), both in x axis
    ///   and y axis
    /// * `extrude_images` - if true, the border pixels of the each texture in the atlas will be duplicated
    /// (extruded) outward into the padding area.
    /// If false, the padding area is transparent.
    pub fn new(mut size: UVec2, padding: u32, extrude_textures: bool) -> Self {
        if !extrude_textures {
            // This doesn't need to be >= since `AtlasAllocator` requires non-zero size.
            debug_assert!(size.x > padding && size.y > padding);

            size -= padding;
        }

        Self {
            // Leave out padding at the right and bottom, so we don't put textures on the edge of
            // atlas.
            atlas_allocator: AtlasAllocator::new(to_size2(size)),
            padding,
            extrude_textures,
        }
    }

    /// Add a new texture to `atlas_layout`.
    ///
    /// It is the user's responsibility to pass in the correct [`TextureAtlasLayout`].
    /// Also, the asset that `atlas_texture_handle` points to must have a usage matching
    /// [`RenderAssetUsages::MAIN_WORLD`].
    ///
    /// # Arguments
    ///
    /// * `atlas_layout` - The atlas layout to add the texture to.
    /// * `texture` - The source texture to add to the atlas.
    /// * `atlas_texture` - The destination atlas texture to copy the source texture to.
    pub fn add_texture(
        &mut self,
        atlas_layout: &mut TextureAtlasLayout,
        texture: &Image,
        atlas_texture: &mut Image,
    ) -> Result<usize, DynamicTextureAtlasBuilderError> {
        let mut padding = self.padding;
        if self.extrude_textures {
            padding *= 2;
        }

        // Allocate enough space for the texture and the padding to the top and left (bottom and
        // right padding are taken care off since the allocator size omits it on creation).
        let allocation = self.atlas_allocator.allocate(size2(
            (texture.width() + padding).try_into().unwrap(),
            (texture.height() + padding).try_into().unwrap(),
        ));
        if let Some(mut allocation) = allocation {
            assert!(
                atlas_texture.asset_usage.contains(RenderAssetUsages::MAIN_WORLD),
                "The atlas_texture image must have the RenderAssetUsages::MAIN_WORLD usage flag set"
            );
            let atlas_rect = if self.extrude_textures {
                self.place_texture_with_extrusion(atlas_texture, allocation, texture)?;

                let mut rect = allocation.rectangle;
                rect.min.x += self.padding as i32;
                rect.min.y += self.padding as i32;
                rect.max.x -= self.padding as i32;
                rect.max.y -= self.padding as i32;
                rect
            } else {
                let rect = &mut allocation.rectangle;
                // Remove the padding from the top and left (bottom and right padding is taken care of
                // by the "next" allocation and the border restriction).
                rect.min.x += self.padding as i32;
                rect.min.y += self.padding as i32;

                self.place_texture(atlas_texture, allocation, texture)?;
                allocation.rectangle
            };
            Ok(atlas_layout.add_texture(to_rect(atlas_rect)))
        } else {
            Err(DynamicTextureAtlasBuilderError::FailedToAllocateSpace)
        }
    }

    fn place_texture(
        &mut self,
        atlas_texture: &mut Image,
        allocation: Allocation,
        texture: &Image,
    ) -> Result<(), DynamicTextureAtlasBuilderError> {
        let rect = &allocation.rectangle;
        let atlas_width = atlas_texture.width() as usize;
        let rect_width = rect.width() as usize;
        let format_size = atlas_texture.texture_descriptor.format.pixel_size()?;

        let Some(ref mut atlas_data) = atlas_texture.data else {
            return Err(DynamicTextureAtlasBuilderError::UninitializedAtlas);
        };
        let Some(ref data) = texture.data else {
            return Err(DynamicTextureAtlasBuilderError::UninitializedSourceTexture);
        };
        for (texture_y, bound_y) in (rect.min.y..rect.max.y).map(|i| i as usize).enumerate() {
            let begin = (bound_y * atlas_width + rect.min.x as usize) * format_size;
            let end = begin + rect_width * format_size;
            let texture_begin = texture_y * rect_width * format_size;
            let texture_end = texture_begin + rect_width * format_size;
            atlas_data[begin..end].copy_from_slice(&data[texture_begin..texture_end]);
        }
        Ok(())
    }

    fn place_texture_with_extrusion(
        &mut self,
        atlas_texture: &mut Image,
        allocation: Allocation,
        texture: &Image,
    ) -> Result<(), DynamicTextureAtlasBuilderError> {
        let rect = &allocation.rectangle;
        let atlas_width = atlas_texture.width() as usize;
        let texture_width = texture.width() as usize;
        let texture_height = texture.height() as usize;
        let padding = self.padding as usize;
        let format_size = atlas_texture.texture_descriptor.format.pixel_size()?;

        let Some(ref mut atlas_data) = atlas_texture.data else {
            return Err(DynamicTextureAtlasBuilderError::UninitializedAtlas);
        };
        let Some(ref data) = texture.data else {
            return Err(DynamicTextureAtlasBuilderError::UninitializedSourceTexture);
        };

        if texture_width == 0 || texture_height == 0 {
            return Ok(());
        }

        let rect_min_x = rect.min.x as usize;
        let rect_min_y = rect.min.y as usize;
        let content_min_x = rect_min_x + padding;
        let content_min_y = rect_min_y + padding;

        for texture_y in 0..texture_height {
            let atlas_y = content_min_y + texture_y;
            let texture_row_start = texture_y * texture_width * format_size;
            let texture_row_end = texture_row_start + texture_width * format_size;

            let atlas_texture_start = (atlas_y * atlas_width + content_min_x) * format_size;
            let atlas_texture_end = atlas_texture_start + texture_width * format_size;
            atlas_data[atlas_texture_start..atlas_texture_end]
                .copy_from_slice(&data[texture_row_start..texture_row_end]);

            if padding > 0 {
                let left_pixel = &data[texture_row_start..(texture_row_start + format_size)];
                for x in 0..padding {
                    let dst_start = (atlas_y * atlas_width + rect_min_x + x) * format_size;
                    let dst_end = dst_start + format_size;
                    atlas_data[dst_start..dst_end].copy_from_slice(left_pixel);
                }

                let right_pixel_start =
                    texture_row_start + (texture_width.saturating_sub(1)) * format_size;
                let right_pixel = &data[right_pixel_start..(right_pixel_start + format_size)];
                for x in 0..padding {
                    let dst_start =
                        (atlas_y * atlas_width + content_min_x + texture_width + x) * format_size;
                    let dst_end = dst_start + format_size;
                    atlas_data[dst_start..dst_end].copy_from_slice(right_pixel);
                }
            }
        }

        if padding > 0 {
            let row_width = (texture_width + 2 * padding) * format_size;
            let first_row_start = (content_min_y * atlas_width + rect_min_x) * format_size;
            let last_row_start =
                ((content_min_y + texture_height - 1) * atlas_width + rect_min_x) * format_size;

            for y in 0..padding {
                let dst_start = ((rect_min_y + y) * atlas_width + rect_min_x) * format_size;
                atlas_data.copy_within(first_row_start..(first_row_start + row_width), dst_start);
            }

            for y in 0..padding {
                let dst_start =
                    ((content_min_y + texture_height + y) * atlas_width + rect_min_x) * format_size;
                atlas_data.copy_within(last_row_start..(last_row_start + row_width), dst_start);
            }
        }

        Ok(())
    }
}

fn to_rect(rectangle: guillotiere::Rectangle) -> URect {
    URect {
        min: UVec2::new(
            rectangle.min.x.try_into().unwrap(),
            rectangle.min.y.try_into().unwrap(),
        ),
        max: UVec2::new(
            rectangle.max.x.try_into().unwrap(),
            rectangle.max.y.try_into().unwrap(),
        ),
    }
}

fn to_size2(vec2: UVec2) -> guillotiere::Size {
    guillotiere::Size::new(vec2.x as i32, vec2.y as i32)
}

#[cfg(test)]
mod tests {
    use bevy_asset::RenderAssetUsages;
    use bevy_math::{URect, UVec2};

    use crate::{DynamicTextureAtlasBuilder, Image, TextureAtlasLayout};

    fn make_filled_image(size: UVec2, pixel_rgba_bytes: [u8; 4]) -> Image {
        Image::new_fill(
            wgpu_types::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            wgpu_types::TextureDimension::D2,
            &pixel_rgba_bytes,
            wgpu_types::TextureFormat::Rgba8Unorm,
            RenderAssetUsages::all(),
        )
    }

    fn make_image_from_data(size: UVec2, data: Vec<u8>) -> Image {
        Image::new(
            wgpu_types::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            wgpu_types::TextureDimension::D2,
            data,
            wgpu_types::TextureFormat::Rgba8Unorm,
            RenderAssetUsages::all(),
        )
    }

    fn rect_contains_value(image: &Image, rect: URect, pixel_rgba_bytes: [u8; 4]) -> bool {
        let image_data = image.data.as_ref().unwrap();
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let byte_start = ((x + y * image.width()) * 4) as usize;
                if image_data[byte_start..(byte_start + 4)] != pixel_rgba_bytes {
                    return false;
                }
            }
        }
        true
    }

    fn pixel_value_at(image: &Image, x: u32, y: u32) -> [u8; 4] {
        let image_data = image.data.as_ref().unwrap();
        let byte_start = ((x + y * image.width()) * 4) as usize;
        [
            image_data[byte_start],
            image_data[byte_start + 1],
            image_data[byte_start + 2],
            image_data[byte_start + 3],
        ]
    }

    #[test]
    fn allocate_textures() {
        let size = UVec2::new(30, 30);

        let mut atlas_texture = make_filled_image(size, [0, 0, 0, 0]);
        let mut layout = TextureAtlasLayout::new_empty(size);
        let mut builder = DynamicTextureAtlasBuilder::new(size, 0, false);

        let square = UVec2::new(10, 10);
        let colors = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 0, 255, 255],
            [0, 255, 255, 255],
            [0, 255, 255, 255],
        ];
        let texture_0 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[0]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_1 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[1]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_2 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[2]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_3 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[3]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_4 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[4]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_5 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[5]),
                &mut atlas_texture,
            )
            .unwrap();

        let expected_rects = [
            URect::from_corners(UVec2::new(0, 0), UVec2::new(10, 10)),
            URect::from_corners(UVec2::new(10, 0), UVec2::new(20, 10)),
            URect::from_corners(UVec2::new(20, 0), UVec2::new(30, 10)),
            URect::from_corners(UVec2::new(0, 10), UVec2::new(10, 20)),
            URect::from_corners(UVec2::new(0, 20), UVec2::new(10, 30)),
            URect::from_corners(UVec2::new(10, 10), UVec2::new(20, 20)),
        ];
        assert_eq!(layout.textures[texture_0], expected_rects[0]);
        assert_eq!(layout.textures[texture_1], expected_rects[1]);
        assert_eq!(layout.textures[texture_2], expected_rects[2]);
        assert_eq!(layout.textures[texture_3], expected_rects[3]);
        assert_eq!(layout.textures[texture_4], expected_rects[4]);
        assert_eq!(layout.textures[texture_5], expected_rects[5]);

        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[0],
            colors[0]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[1],
            colors[1]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[2],
            colors[2]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[3],
            colors[3]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[4],
            colors[4]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[5],
            colors[5]
        ));
    }

    #[test]
    fn allocate_textures_with_padding() {
        let size = UVec2::new(12, 12);

        let mut atlas_texture = make_filled_image(size, [0, 0, 0, 0]);
        let mut layout = TextureAtlasLayout::new_empty(size);
        let mut builder = DynamicTextureAtlasBuilder::new(size, 1, false);

        let square = UVec2::new(3, 3);
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        let texture_0 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[0]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_1 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[1]),
                &mut atlas_texture,
            )
            .unwrap();
        let texture_2 = builder
            .add_texture(
                &mut layout,
                &make_filled_image(square, colors[2]),
                &mut atlas_texture,
            )
            .unwrap();

        let expected_rects = [
            URect::from_corners(UVec2::new(1, 1), UVec2::new(4, 4)),
            URect::from_corners(UVec2::new(5, 1), UVec2::new(8, 4)),
            // If we didn't pad the right of the texture atlas, there would be just enough space to
            // fit this in the first row, but since we do pad the right, this gets pushed to the
            // next row.
            URect::from_corners(UVec2::new(1, 5), UVec2::new(4, 8)),
        ];
        assert_eq!(layout.textures[texture_0], expected_rects[0]);
        assert_eq!(layout.textures[texture_1], expected_rects[1]);
        assert_eq!(layout.textures[texture_2], expected_rects[2]);

        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[0],
            colors[0]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[1],
            colors[1]
        ));
        assert!(rect_contains_value(
            &atlas_texture,
            expected_rects[2],
            colors[2]
        ));
    }

    #[test]
    fn allocate_texture_with_extrusion() {
        let size = UVec2::new(4, 4);

        let mut atlas_texture = make_filled_image(size, [0, 0, 0, 0]);
        let mut layout = TextureAtlasLayout::new_empty(size);
        let mut builder = DynamicTextureAtlasBuilder::new(size, 1, true);

        let texture = make_image_from_data(
            UVec2::new(2, 2),
            vec![
                1, 0, 0, 255, 2, 0, 0, 255, //
                3, 0, 0, 255, 4, 0, 0, 255,
            ],
        );
        let texture_index = builder
            .add_texture(&mut layout, &texture, &mut atlas_texture)
            .unwrap();

        let expected_rect = URect::from_corners(UVec2::new(1, 1), UVec2::new(3, 3));
        assert_eq!(layout.textures[texture_index], expected_rect);

        let expected = [
            [
                [1, 0, 0, 255],
                [1, 0, 0, 255],
                [2, 0, 0, 255],
                [2, 0, 0, 255],
            ],
            [
                [1, 0, 0, 255],
                [1, 0, 0, 255],
                [2, 0, 0, 255],
                [2, 0, 0, 255],
            ],
            [
                [3, 0, 0, 255],
                [3, 0, 0, 255],
                [4, 0, 0, 255],
                [4, 0, 0, 255],
            ],
            [
                [3, 0, 0, 255],
                [3, 0, 0, 255],
                [4, 0, 0, 255],
                [4, 0, 0, 255],
            ],
        ];
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(
                    pixel_value_at(&atlas_texture, x as u32, y as u32),
                    expected[y][x]
                );
            }
        }
    }
}

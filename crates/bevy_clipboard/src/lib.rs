//! This crate provides a platform-agnostic interface for accessing the clipboard

extern crate alloc;

use alloc::borrow::Cow;
#[cfg(all(feature = "image", any(windows, unix)))]
use bevy_asset::RenderAssetUsages;
use bevy_ecs::resource::Resource;
#[cfg(all(feature = "image", any(windows, unix)))]
use bevy_image::Image;
#[cfg(all(feature = "image", any(windows, unix)))]
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

#[cfg(target_arch = "wasm32")]
use {alloc::sync::Arc, bevy_platform::sync::Mutex, wasm_bindgen_futures::JsFuture};

/// The clipboard prelude
pub mod prelude {
    pub use crate::{Clipboard, ClipboardRead};
}

/// Clipboard plugin
#[derive(Default)]
pub struct ClipboardPlugin;

impl bevy_app::Plugin for ClipboardPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Clipboard>();
    }
}

/// Represents an attempt to read from the clipboard.
///
/// On desktop targets the result is available immediately.
/// On wasm32 the result is fetched asynchronously.
#[derive(Debug)]
pub enum ClipboardRead<T = String> {
    /// The clipboard contents are ready to be accessed.
    Ready(Result<T, ClipboardError>),
    #[cfg(target_arch = "wasm32")]
    /// The clipboard contents are being fetched asynchronously.
    Pending(Arc<Mutex<Option<Result<T, ClipboardError>>>>),
}

impl<T> ClipboardRead<T> {
    /// The result of an attempt to read from the clipboard, once ready.
    /// If the result is still pending, returns `None`.
    pub fn poll_result(&mut self) -> Option<Result<T, ClipboardError>> {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::Pending(shared) => {
                if let Some(contents) = shared.lock().ok().and_then(|mut inner| inner.take()) {
                    *self = Self::Ready(Err(ClipboardError::ContentTaken));
                    Some(contents)
                } else {
                    None
                }
            }
            Self::Ready(inner) => {
                Some(core::mem::replace(inner, Err(ClipboardError::ContentTaken)))
            }
        }
    }
}

#[cfg(all(feature = "image", any(windows, unix)))]
fn try_image_from_imagedata(image: arboard::ImageData<'static>) -> Result<Image, ClipboardError> {
    let size = Extent3d {
        width: u32::try_from(image.width).map_err(|_| ClipboardError::ConversionFailure)?,
        height: u32::try_from(image.height).map_err(|_| ClipboardError::ConversionFailure)?,
        depth_or_array_layers: 1,
    };
    Ok(Image::new(
        size,
        TextureDimension::D2,
        image.bytes.into_owned(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

#[cfg(all(feature = "image", any(windows, unix)))]
fn try_imagedata_from_image(image: &Image) -> Result<arboard::ImageData<'_>, ClipboardError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let data = image
        .data
        .as_ref()
        .ok_or(ClipboardError::ConversionFailure)?;
    if data.len()
        != width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(ClipboardError::ConversionFailure)?
    {
        return Err(ClipboardError::ConversionFailure);
    }

    Ok(arboard::ImageData {
        width,
        height,
        bytes: Cow::Borrowed(data.as_slice()),
    })
}

/// Resource providing access to the clipboard
#[cfg(unix)]
#[derive(Resource)]
pub struct Clipboard(Option<arboard::Clipboard>);

#[cfg(unix)]
impl Default for Clipboard {
    fn default() -> Self {
        {
            Self(arboard::Clipboard::new().ok())
        }
    }
}

/// Resource providing access to the clipboard
#[cfg(not(unix))]
#[derive(Resource, Default)]
pub struct Clipboard;

impl Clipboard {
    /// Fetches UTF-8 text from the clipboard and returns it via a `ClipboardRead`.
    ///
    /// On Windows and Unix `ClipboardRead`s are completed instantly, on wasm32 the result is fetched asynchronously.
    pub fn fetch_text(&mut self) -> ClipboardRead {
        #[cfg(unix)]
        {
            ClipboardRead::Ready(if let Some(clipboard) = self.0.as_mut() {
                clipboard.get_text().map_err(ClipboardError::from)
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            })
        }

        #[cfg(windows)]
        {
            ClipboardRead::Ready(
                arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.get_text())
                    .map_err(ClipboardError::from),
            )
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
                let shared = Arc::new(Mutex::new(None));
                let shared_clone = shared.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let text = JsFuture::from(clipboard.read_text()).await;
                    let text = match text {
                        Ok(text) => text.as_string().ok_or(ClipboardError::ConversionFailure),
                        Err(_) => Err(ClipboardError::ContentNotAvailable),
                    };
                    shared.lock().unwrap().replace(text);
                });
                ClipboardRead::Pending(shared_clone)
            } else {
                ClipboardRead::Ready(Err(ClipboardError::ClipboardNotSupported))
            }
        }

        #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
        {
            ClipboardRead::Ready(Err(ClipboardError::ClipboardNotSupported))
        }
    }

    /// Fetches image data from the clipboard and returns it via a [`ClipboardRead`].
    ///
    /// Only supported on Windows and Unix platforms with the `image` feature enabled.
    #[cfg(all(feature = "image", any(windows, unix)))]
    pub fn fetch_image(&mut self) -> ClipboardRead<Image> {
        #[cfg(unix)]
        {
            ClipboardRead::Ready(if let Some(clipboard) = self.0.as_mut() {
                clipboard
                    .get_image()
                    .map_err(ClipboardError::from)
                    .and_then(try_image_from_imagedata)
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            })
        }

        #[cfg(windows)]
        {
            ClipboardRead::Ready(
                arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.get_image())
                    .map_err(ClipboardError::from)
                    .and_then(try_image_from_imagedata),
            )
        }
    }

    /// Asynchronously retrieves UTF-8 text from the system clipboard.
    pub async fn fetch_text_async(&mut self) -> Result<String, ClipboardError> {
        #[cfg(unix)]
        {
            if let Some(clipboard) = self.0.as_mut() {
                clipboard.get_text().map_err(ClipboardError::from)
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            }
        }

        #[cfg(windows)]
        {
            arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.get_text())
                .map_err(ClipboardError::from)
        }

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use wasm_bindgen_futures::JsFuture;

            let clipboard = web_sys::window()
                .and_then(|w| w.navigator().clipboard())
                .ok_or(ClipboardError::ClipboardNotSupported)?;

            let result = JsFuture::from(clipboard.read_text()).await;
            match result {
                Ok(val) => val.as_string().ok_or(ClipboardError::ConversionFailure),
                Err(_) => Err(ClipboardError::ContentNotAvailable),
            }
        }

        #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
        {
            Err(ClipboardError::ClipboardNotSupported)
        }
    }

    /// Places the text onto the clipboard. Any valid UTF-8 string is accepted.
    ///
    /// # Errors
    ///
    /// Returns error if `text` failed to be stored on the clipboard.
    pub fn set_text<'a, T: Into<Cow<'a, str>>>(&mut self, text: T) -> Result<(), ClipboardError> {
        #[cfg(unix)]
        {
            if let Some(clipboard) = self.0.as_mut() {
                clipboard.set_text(text).map_err(ClipboardError::from)
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            }
        }

        #[cfg(windows)]
        {
            arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_text(text))
                .map_err(ClipboardError::from)
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
                let text = text.into().to_string();
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = JsFuture::from(clipboard.write_text(&text)).await;
                });
                Ok(())
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            }
        }

        #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
        {
            Err(ClipboardError::ClipboardNotSupported)
        }
    }

    /// Places image data onto the clipboard.
    ///
    /// The image must contain initialized 2D pixel data in packed RGBA8 row-major order.
    /// Only supported on Windows and Unix platforms with the `image` feature enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the image data is invalid or the clipboard write fails.
    #[cfg(all(feature = "image", any(windows, unix)))]
    pub fn set_image(&mut self, image: &Image) -> Result<(), ClipboardError> {
        #[cfg(unix)]
        {
            if let Some(clipboard) = self.0.as_mut() {
                let image_data = try_imagedata_from_image(image)?;
                clipboard
                    .set_image(image_data)
                    .map_err(ClipboardError::from)
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            }
        }

        #[cfg(windows)]
        {
            let image_data = try_imagedata_from_image(image)?;
            arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_image(image_data))
                .map_err(ClipboardError::from)
        }
    }
}

/// An error that might happen during a clipboard operation.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ClipboardError {
    /// Clipboard contents were unavailable or not in the expected format.
    ContentNotAvailable,

    /// No suitable clipboard backend was available
    ClipboardNotSupported,

    /// Clipboard access is temporarily locked by another process or thread.
    ClipboardOccupied,

    /// The data could not be converted to or from the required format.
    ConversionFailure,

    /// The clipboard content was already taken from the `ClipboardRead`.
    ContentTaken,

    /// An unknown error
    Unknown {
        /// String describing the error
        description: String,
    },
}

#[cfg(any(windows, unix))]
impl From<arboard::Error> for ClipboardError {
    fn from(value: arboard::Error) -> Self {
        match value {
            arboard::Error::ContentNotAvailable => ClipboardError::ContentNotAvailable,
            arboard::Error::ClipboardNotSupported => ClipboardError::ClipboardNotSupported,
            arboard::Error::ClipboardOccupied => ClipboardError::ClipboardOccupied,
            arboard::Error::ConversionFailure => ClipboardError::ConversionFailure,
            arboard::Error::Unknown { description } => ClipboardError::Unknown { description },
            _ => ClipboardError::Unknown {
                description: "Unknown arboard error variant".to_owned(),
            },
        }
    }
}

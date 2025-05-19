//! This crate provides a platform-agnostic interface for accessing the clipboard

extern crate alloc;

use bevy_asset::RenderAssetUsages;
use bevy_ecs::resource::Resource;
use bevy_image::Image;
use wgpu_types::Extent3d;
use wgpu_types::TextureDimension;
use wgpu_types::TextureFormat;

use {alloc::sync::Arc, bevy_platform::sync::Mutex};

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
pub enum ClipboardRead<T> {
    /// The clipboard contents are ready to be accessed.
    Ready(Result<T, ClipboardError>),
    /// The clipboard contents are being fetched asynchronously.
    Pending(Arc<Mutex<Option<Result<T, ClipboardError>>>>),
}

impl<T> ClipboardRead<T> {
    /// The result of an attempt to read from the clipboard, if it is ready.
    /// If the result is still pending, returns `None`.
    pub fn poll_result(&mut self) -> Option<Result<T, ClipboardError>> {
        match self {
            Self::Pending(shared) => {
                if let Some(contents) = shared.lock().ok().and_then(|mut inner| inner.take()) {
                    *self = Self::Ready(Err(ClipboardError::ContentTaken));
                    Some(contents)
                } else {
                    None
                }
            }
            Self::Ready(inner) => Some(std::mem::replace(inner, Err(ClipboardError::ContentTaken))),
        }
    }
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
    pub fn fetch_text(&mut self) -> ClipboardRead<String> {
        #[cfg(any(windows, unix))]
        {
            #[cfg(windows)]
            let clipboard = arboard::Clipboard::new().map_err(ClipboardError::from);

            #[cfg(unix)]
            let clipboard = self
                .0
                .as_mut()
                .map_err(Err(ClipboardError::ClipboardNotSupported));

            ClipboardRead::Ready(
                clipboard
                    .and_then(|mut clipboard| clipboard.get_text().map_err(ClipboardError::from)),
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
    }

    /// Places the text onto the clipboard. Any valid UTF-8 string is accepted.
    ///
    /// # Errors
    ///
    /// Returns error if `text` failed to be stored on the clipboard.
    pub fn set_text<'a, T: Into<alloc::borrow::Cow<'a, str>>>(
        &mut self,
        text: T,
    ) -> Result<(), ClipboardError> {
        #[cfg(any(windows, unix))]
        {
            #[cfg(windows)]
            let clipboard = arboard::Clipboard::new().map_err(ClipboardError::from);

            #[cfg(unix)]
            let clipboard = self
                .0
                .as_mut()
                .map_err(Err(ClipboardError::ClipboardNotSupported));

            clipboard
                .and_then(|mut clipboard| clipboard.set_text(text).map_err(ClipboardError::from))
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = JsFuture::from(clipboard.write_text(&text)).await;
                });
                Ok(())
            } else {
                Err(ClipboardError::ClipboardNotSupported)
            }
        }
    }

    /// Fetches an image from the clipboard and returns it via a `ClipboardRead`.
    ///
    /// On Windows and Unix `ClipboardRead`s are completed instantly, on wasm32 the result is fetched asynchronously.
    #[cfg(feature = "image")]
    pub fn fetch_image(&mut self) -> ClipboardRead<Image> {
        #[cfg(target_arch = "wasm32")]
        {
            ClipboardRead::Ready(Err::ContentNotAvailable)
        }

        #[cfg(any(windows, unix))]
        {
            #[cfg(windows)]
            let clipboard = arboard::Clipboard::new().map_err(ClipboardError::from);

            #[cfg(unix)]
            let clipboard = self
                .0
                .as_mut()
                .map_err(Err(ClipboardError::ClipboardNotSupported));

            println!("fetch image");

            ClipboardRead::Ready(
                clipboard
                    .and_then(|mut clipboard| clipboard.get_image().map_err(ClipboardError::from))
                    .map(|image_data| {
                        println!("make image");
                        Image::new(
                            Extent3d {
                                width: image_data.width as u32,
                                height: image_data.height as u32,
                                depth_or_array_layers: 1,
                            },
                            TextureDimension::D2,
                            image_data.bytes.into_owned(),
                            TextureFormat::Rgba8Unorm,
                            RenderAssetUsages::default(),
                        )
                    }),
            )
        }
    }

    #[cfg(feature = "image")]
    /// Place an image onto the clipboard.
    ///
    /// # Errors
    ///
    /// Returns error if the image fails to be stored on the clipboard.
    pub fn set_image(width: u32, height: u32, bytes: &[u8]) -> Result<(), ClipboardError> {
        use arboard::ImageData;

        #[cfg(target_arch = "wasm32")]
        {
            Err::ClipboardNotSupported
        }

        #[cfg(any(windows, unix))]
        {
            #[cfg(windows)]
            let clipboard = arboard::Clipboard::new().map_err(ClipboardError::from);

            #[cfg(unix)]
            let clipboard = self
                .0
                .as_mut()
                .map_err(Err(ClipboardError::ClipboardNotSupported));

            clipboard.and_then(|mut clipboard: arboard::Clipboard| {
                clipboard
                    .set_image(ImageData {
                        width: width as usize,
                        height: height as usize,
                        bytes: bytes.into(),
                    })
                    .map_err(ClipboardError::from)
            })
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

    /// An unkown error
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

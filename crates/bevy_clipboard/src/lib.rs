//! This crate provides a platform-agnostic interface for accessing the clipboard

extern crate alloc;

use alloc::sync::Arc;
use bevy_app::Plugin;
use bevy_ecs::resource::Resource;
use bevy_platform::sync::Mutex;
use bevy_tasks::{block_on, IoTaskPool, Task};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

/// The clipboard prelude
pub mod prelude {
    pub use crate::{Clipboard, ClipboardPlugin};
}

/// Clipboard plugin
#[derive(Default)]
pub struct ClipboardPlugin;

impl Plugin for ClipboardPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Clipboard>();
    }
}

#[cfg(all(unix, not(target_os = "android")))]
/// Resource providing access to the clipboard
#[derive(Resource, Clone)]
pub struct Clipboard(Result<Arc<Mutex<arboard::Clipboard>>, ClipboardError>);

#[cfg(all(unix, not(target_os = "android")))]
impl Default for Clipboard {
    fn default() -> Self {
        {
            Self(
                arboard::Clipboard::new()
                    .map(|clipboard| Arc::new(Mutex::new(clipboard)))
                    .map_err(|_| ClipboardError::ClipboardNotSupported),
            )
        }
    }
}

#[cfg(not(all(unix, not(target_os = "android"))))]
/// Resource providing access to the clipboard
#[derive(Resource, Default)]
pub struct Clipboard;

impl Clipboard {
    /// Fetches UTF-8 text from the clipboard and returns it via a `ClipboardRead`.
    /// This performs blocking IO, which may take considerable time (e.g., timeout on X11 is 4s).
    /// For non-blocking clipboard read consider using `fetch_text_task`.
    pub fn fetch_text(&mut self) -> Result<String, ClipboardError> {
        block_on(self.fetch_text_task())
    }

    /// Schedules and returns `Task` on `IoTaskPool` that retrieves UTF-8 text from the clipboard.
    pub fn fetch_text_task(&mut self) -> Task<Result<String, ClipboardError>> {
        let clipboard_res = self.clone();
        IoTaskPool::get().spawn(async move {
            #[cfg(unix)]
            {
                let clipboard_mut = clipboard_res.0?;
                let mut clipboard = clipboard_mut.lock().unwrap();
                clipboard.get_text().map_err(ClipboardError::from)
            }

            #[cfg(windows)]
            {
                arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.get_text())
                    .map_err(ClipboardError::from)
            }

            #[cfg(target_arch = "wasm32")]
            {
                let clipboard = web_sys::window()
                    .map(|w| w.navigator().clipboard())
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
        })
    }

    /// Places the text onto the clipboard. Any valid UTF-8 string is accepted.
    /// This performs blocking IO, which may take considerable time.
    /// For non-blocking clipboard write consider `set_text_task`.
    ///
    /// # Errors
    ///
    /// Returns error if `text` failed to be stored on the clipboard.
    pub fn set_text<T: Into<String>>(&mut self, text: T) -> Result<(), ClipboardError> {
        block_on(self.set_text_task(text))
    }

    /// Places the text onto the clipboard. Any valid UTF-8 string is accepted.
    ///
    /// # Errors
    ///
    /// Task may result in error if `text` failed to be stored on the clipboard.
    pub fn set_text_task<T: Into<String>>(&mut self, text: T) -> Task<Result<(), ClipboardError>> {
        let clipboard_res = self.clone();
        let text_string: String = text.into();

        IoTaskPool::get().spawn(async move {
            #[cfg(unix)]
            {
                let clipboard_mut = clipboard_res.0?;
                clipboard_mut
                    .lock()
                    .unwrap()
                    .set_text(text_string)
                    .map_err(ClipboardError::from)
            }

            #[cfg(windows)]
            {
                arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(text_string))
                    .map_err(ClipboardError::from)
            }

            #[cfg(target_arch = "wasm32")]
            {
                if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
                    let _ = JsFuture::from(clipboard.write_text(&text_string)).await;
                    Ok(())
                } else {
                    Err(ClipboardError::ClipboardNotSupported)
                }
            }

            #[cfg(any(target_os = "android", not(any(unix, windows, target_arch = "wasm32"))))]
            {
                Err(ClipboardError::ClipboardNotSupported)
            }
        })
    }
}

/// An error that might happen during a clipboard operation.
#[non_exhaustive]
#[derive(Debug, displaydoc::Display, Clone)]
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

    /// An unknown clipboard error
    Unknown {
        /// String describing the error
        description: String,
    },
}

impl core::error::Error for ClipboardError {}

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

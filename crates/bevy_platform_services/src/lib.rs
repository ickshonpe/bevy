//! Implementations of high-level OS-specific services.
//!
//! Currently, only the clipboard is implemented, but this crate can serve to later contain other
//! OS independent services such as an on-screen keyboard for mobile and consoles, or desktop notifications.

/// Container for standard features
pub mod prelude {
    #[cfg(feature = "clipboard")]
    pub use crate::clipboard::{Clipboard, ClipboardPlugin, ClipboardRead};
}

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(feature = "clipboard")]
pub use clipboard::ClipboardPlugin;

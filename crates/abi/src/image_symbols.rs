//! Compiler-defined image entrypoints, separate from native runtime exports.
//!
//! Producers and hot-reload consumers share these names. Legacy IR is read
//! using its original initialization entry; new images use the `df.` namespace.
//!
//! ```
//! use dartforge_abi::image_symbols::{CALL_DART, LEGACY_RTI_START, RTI_START};
//! assert!(CALL_DART.starts_with("df."));
//! assert!(RTI_START.starts_with("df."));
//! assert_ne!(RTI_START, LEGACY_RTI_START);
//! ```

/// Zero-argument, void initialization of the program's runtime type universe.
pub const RTI_START: &str = "df.prepare_rti";

/// The program callback passed by address to the native event loop.
pub const CALL_DART: &str = "df.call_dart0";

/// Previous compiler spelling, accepted when publishing legacy IR generations.
pub const LEGACY_RTI_START: &str = "dartforge_rti_init";

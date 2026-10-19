//! Initial target and C FFI contracts for LLVM; it neither loads nor calls libraries.
//! Reference: Dart SDK 3.6.2 sdk/lib/ffi/{native_type,abi,dynamic_library}.dart.
//! Managed handles are not C addresses; WebAssembly requires its own imports.
pub mod image_symbols;

use std::fmt;

/// Explicit profiles; they do not represent support already implemented by the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    WindowsX64,
    LinuxX64,
    Wasm32,
}
impl Target {
    /// Returns the profile's LLVM triple, without inferring the target from the Dart int size.
    pub fn triple(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::Wasm32 => "wasm32-unknown-unknown",
        }
    }
    /// Address width; internal i64 handles do not change this property.
    pub fn pointer_bits(self) -> u8 {
        match self {
            Self::Wasm32 => 32,
            _ => 64,
        }
    }
    /// Tells whether the profile admits native dynamic libraries in the planned model.
    pub fn supports_dynamic_libraries(self) -> bool {
        self != Self::Wasm32
    }
}

/// Subset of native markers with unambiguous scalar lowering.
/// Structs, unions, varargs, callbacks and narrow integers require additional rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {
    Void,
    Int32,
    Int64,
    Double,
    Pointer,
}
impl NativeType {
    /// LLVM representation; ptr stays opaque and uses the target width.
    pub fn llvm(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Int32 => "i32",
            Self::Int64 => "i64",
            Self::Double => "double",
            Self::Pointer => "ptr",
        }
    }
    /// Size and alignment of scalars in the supported profiles; void has no layout.
    pub fn layout(self, target: Target) -> Option<(u8, u8)> {
        match self {
            Self::Void => None,
            Self::Int32 => Some((4, 4)),
            Self::Int64 | Self::Double => Some((8, 8)),
            Self::Pointer => {
                let size = target.pointer_bits() / 8;
                Some((size, size))
            }
        }
    }
}
/// Dart categories expected in FFI signatures; they are not the types of the current AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartType {
    Void,
    Int,
    Double,
    Pointer,
    ManagedObject,
}

/// Explicit C signature; pointers do not carry references to the GC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub result: NativeType,
    pub parameters: Vec<NativeType>,
}
/// Contract diagnostic before any linking attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiError(pub &'static str);
impl fmt::Display for AbiError {
    /// Presents a stable error without depending on an external toolchain.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AbiError {}
impl Signature {
    /// Checks that native markers and the Dart signature correspond.
    ///
    /// # Errors
    /// Rejects arity, incompatible types, managed objects and void in parameters.
    pub fn validate_binding(
        &self,
        result: DartType,
        parameters: &[DartType],
    ) -> Result<(), AbiError> {
        if self.parameters.len() != parameters.len() {
            return Err(AbiError("incompatible FFI arity"));
        }
        if !compatible(self.result, result) {
            return Err(AbiError("incompatible FFI return"));
        }
        for (&native, &dart) in self.parameters.iter().zip(parameters) {
            if native == NativeType::Void || !compatible(native, dart) {
                return Err(AbiError("incompatible FFI parameter"));
            }
        }
        Ok(())
    }
    /// Emits only the LLVM C declaration of a structurally validated signature.
    /// Does not implement conversion of Dart arguments nor resolution/loading of the symbol.
    ///
    /// # Errors
    /// Rejects symbols outside the C subset, reserved names and void parameters.
    pub fn llvm_declaration(&self, symbol: &str) -> Result<String, AbiError> {
        let bytes = symbol.as_bytes();
        if bytes.is_empty()
            || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
            || !bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
            || symbol.starts_with("dartforge_")
            || symbol.starts_with("df_")
        {
            return Err(AbiError("invalid or reserved FFI symbol"));
        }
        if self.parameters.contains(&NativeType::Void) {
            return Err(AbiError("void is not a C parameter"));
        }
        Ok(format!(
            "declare {} @{}({})\n",
            self.result.llvm(),
            symbol,
            self.parameters
                .iter()
                .map(|ty| ty.llvm())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
    /// Validates dynamic library capability without promising a WebAssembly backend.
    ///
    /// # Errors
    /// WebAssembly requires imports/exports and linear memory, not dlopen/LoadLibrary.
    pub fn validate_dynamic_target(&self, target: Target) -> Result<(), AbiError> {
        if !target.supports_dynamic_libraries() {
            return Err(AbiError("dynamic FFI unavailable in the WebAssembly profile"));
        }
        Ok(())
    }
}
/// Maps FFI markers per SDK 3.6.2, without converting handles into pointers.
fn compatible(native: NativeType, dart: DartType) -> bool {
    matches!(
        (native, dart),
        (NativeType::Void, DartType::Void)
            | (NativeType::Int32 | NativeType::Int64, DartType::Int)
            | (NativeType::Double, DartType::Double)
            | (NativeType::Pointer, DartType::Pointer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Pointers depend on the target, but int64 stays eight bytes on wasm32.
    #[test]
    fn target_layouts_do_not_confuse_handles_and_pointers() {
        assert_eq!(NativeType::Pointer.layout(Target::Wasm32), Some((4, 4)));
        assert_eq!(NativeType::Pointer.layout(Target::WindowsX64), Some((8, 8)));
        assert_eq!(NativeType::Int64.layout(Target::Wasm32), Some((8, 8)));
        assert_eq!(NativeType::Void.layout(Target::LinuxX64), None);
    }
    /// Signatures preserve arity/types and reject a managed handle as ptr.
    #[test]
    fn scalar_contracts_and_target_rejections() {
        let sig = Signature {
            result: NativeType::Int64,
            parameters: vec![NativeType::Pointer, NativeType::Int32],
        };
        assert!(
            sig.validate_binding(DartType::Int, &[DartType::Pointer, DartType::Int])
                .is_ok()
        );
        assert!(
            sig.validate_binding(DartType::Int, &[DartType::ManagedObject, DartType::Int])
                .is_err()
        );
        assert!(
            sig.validate_binding(DartType::Double, &[DartType::Pointer, DartType::Int])
                .is_err()
        );
        assert!(sig.validate_binding(DartType::Int, &[]).is_err());
        assert!(sig.validate_dynamic_target(Target::Wasm32).is_err());
        assert!(sig.validate_dynamic_target(Target::LinuxX64).is_ok());
        assert_eq!(
            sig.llvm_declaration("native_sum").unwrap(),
            "declare i64 @native_sum(ptr, i32)\n"
        );
        for name in ["", "1bad", "a\nret void", "dartforge_entry", "df_fn_0"] {
            assert!(sig.llvm_declaration(name).is_err());
        }
        let invalid = Signature {
            result: NativeType::Void,
            parameters: vec![NativeType::Void],
        };
        assert!(invalid.llvm_declaration("f").is_err());
        assert!(
            invalid
                .validate_binding(DartType::Void, &[DartType::Void])
                .is_err()
        );
    }
}

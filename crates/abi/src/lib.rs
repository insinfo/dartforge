//! Contratos iniciais de alvos e FFI C para LLVM; não carrega nem chama bibliotecas.
//! Referência: Dart SDK 3.6.2 sdk/lib/ffi/{native_type,abi,dynamic_library}.dart.
//! Handles gerenciados não são endereços C; WebAssembly exige imports próprios.
use std::fmt;

/// Perfis explícitos; não representam suporte já implementado pelo driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    WindowsX64,
    LinuxX64,
    Wasm32,
}
impl Target {
    /// Retorna o triple LLVM do perfil, sem inferir o alvo pelo tamanho de int Dart.
    pub fn triple(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::Wasm32 => "wasm32-unknown-unknown",
        }
    }
    /// Largura de endereços; handles internos i64 não mudam esta propriedade.
    pub fn pointer_bits(self) -> u8 {
        match self {
            Self::Wasm32 => 32,
            _ => 64,
        }
    }
    /// Informa se o perfil admite bibliotecas dinâmicas nativas no modelo planejado.
    pub fn supports_dynamic_libraries(self) -> bool {
        self != Self::Wasm32
    }
}

/// Subconjunto de marcadores nativos com lowering escalar não ambíguo.
/// Structs, unions, varargs, callbacks e inteiros estreitos exigem regras adicionais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {
    Void,
    Int32,
    Int64,
    Double,
    Pointer,
}
impl NativeType {
    /// Representação LLVM; ptr permanece opaco e usa largura do alvo.
    pub fn llvm(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Int32 => "i32",
            Self::Int64 => "i64",
            Self::Double => "double",
            Self::Pointer => "ptr",
        }
    }
    /// Tamanho e alinhamento dos escalares nos perfis suportados; void não tem layout.
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
/// Categorias Dart esperadas em assinaturas FFI; não são os tipos da AST atual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartType {
    Void,
    Int,
    Double,
    Pointer,
    ManagedObject,
}

/// Assinatura C explícita; ponteiros não carregam referências ao GC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub result: NativeType,
    pub parameters: Vec<NativeType>,
}
/// Diagnóstico de contrato antes de qualquer tentativa de ligação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiError(pub &'static str);
impl fmt::Display for AbiError {
    /// Apresenta erro estável sem depender de toolchain externa.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AbiError {}
impl Signature {
    /// Verifica correspondência de marcadores nativos e assinatura Dart.
    ///
    /// # Erros
    /// Rejeita aridade, tipos incompatíveis, objetos gerenciados e void em parâmetros.
    pub fn validate_binding(
        &self,
        result: DartType,
        parameters: &[DartType],
    ) -> Result<(), AbiError> {
        if self.parameters.len() != parameters.len() {
            return Err(AbiError("aridade FFI incompatível"));
        }
        if !compatible(self.result, result) {
            return Err(AbiError("retorno FFI incompatível"));
        }
        for (&native, &dart) in self.parameters.iter().zip(parameters) {
            if native == NativeType::Void || !compatible(native, dart) {
                return Err(AbiError("parâmetro FFI incompatível"));
            }
        }
        Ok(())
    }
    /// Emite apenas declaração LLVM C de uma assinatura validada estruturalmente.
    /// Não implementa conversão de argumentos Dart nem resolução/carregamento do símbolo.
    ///
    /// # Erros
    /// Rejeita símbolos fora do subconjunto C, nomes reservados e parâmetros void.
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
            return Err(AbiError("símbolo FFI inválido ou reservado"));
        }
        if self.parameters.contains(&NativeType::Void) {
            return Err(AbiError("void não é parâmetro C"));
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
    /// Valida capacidade de biblioteca dinâmica sem prometer backend WebAssembly.
    ///
    /// # Erros
    /// WebAssembly requer imports/exportações e memória linear, não dlopen/LoadLibrary.
    pub fn validate_dynamic_target(&self, target: Target) -> Result<(), AbiError> {
        if !target.supports_dynamic_libraries() {
            return Err(AbiError("FFI dinâmica indisponível no perfil WebAssembly"));
        }
        Ok(())
    }
}
/// Mapeia marcadores FFI conforme SDK 3.6.2, sem converter handles em ponteiros.
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
    /// Ponteiros dependem do alvo, mas int64 permanece de oito bytes em wasm32.
    #[test]
    fn target_layouts_do_not_confuse_handles_and_pointers() {
        assert_eq!(NativeType::Pointer.layout(Target::Wasm32), Some((4, 4)));
        assert_eq!(NativeType::Pointer.layout(Target::WindowsX64), Some((8, 8)));
        assert_eq!(NativeType::Int64.layout(Target::Wasm32), Some((8, 8)));
        assert_eq!(NativeType::Void.layout(Target::LinuxX64), None);
    }
    /// Assinaturas preservam aridade/tipos e rejeitam handle gerenciado como ptr.
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

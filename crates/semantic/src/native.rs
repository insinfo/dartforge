//! Validação do subconjunto FFI escalar: anotações não fornecem corpos Dart.
use super::*;
use dartforge_syntax::NativeType;
impl<'a> Validator<'a> {
    /// Exige correspondência exata entre a assinatura Dart e os escalares da ABI nativa.
    pub(super) fn validate_native(
        &self,
        function: &dartforge_syntax::Function<'a>,
    ) -> Result<(), Diagnostic> {
        let binding = function.native_binding.as_ref().expect("binding presente");
        let error = |message| Diagnostic::new(message, binding.span);
        if self.current_class.is_some()
            || self.current_extension.is_some()
            || function.is_getter
            || !function.type_parameters.is_empty()
        {
            return Err(error(
                "@Native supports only non-generic external top-level functions",
            ));
        }
        if !function.body.is_empty() {
            return Err(error(
                "An external @Native declaration cannot have a Dart body",
            ));
        }
        let mut chars = binding.symbol.bytes();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(error(
                "Native symbol must be an ASCII C identifier in this subset",
            ));
        }
        if binding.parameters.len() != function.parameters.len() {
            return Err(error("Native and Dart parameter counts must match"));
        }
        for (native, dart) in binding.parameters.iter().zip(&function.parameters) {
            if !matches!(native, NativeType::Int32 | NativeType::Int64) || dart.ty != Type::Int {
                return Err(Diagnostic::new(
                    "Native Int32/Int64 parameters require non-null Dart int; other ABI types are unsupported",
                    dart.span,
                ));
            }
        }
        let expected = match binding.result {
            NativeType::Void => Type::Void,
            NativeType::Int32 | NativeType::Int64 => Type::Int,
        };
        if function.return_type != expected {
            return Err(error("Native result requires matching Dart int or void"));
        }
        Ok(())
    }
}

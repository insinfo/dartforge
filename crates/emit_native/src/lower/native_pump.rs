//! Versioned package declarations with valid VM bodies and native replacements.

use super::fn_builder::FnBuilder;
use crate::hir::{Instruction, Operand, Terminator, Type};

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub(super) fn lower_native_pump_contract(&mut self, fid: usize) -> bool {
        let function = &self.ctx.program.functions[fid];
        if self.ctx.program.library(function.library).uri
            != "package:dartforge/src/native_pump.dart"
            || function.kind != dartforge_elements::model::FunctionKind::Function
            || function.class.is_some()
            || function.extension.is_some()
            || !self.func.params.is_empty()
            || !crate::source::has_pragma(
                self.ctx.program,
                self.ctx.interner,
                fid,
                "dartforge:native-pump-v1",
            )
        {
            return false;
        }
        let (native, abi, expected, callback) = match self.ctx.symbol_name(function.name) {
            "_supportsNativePumpV1" => ("supported", Type::I8, Type::I1, false),
            "_enableNativePumpV1" => ("enable", Type::I64, Type::I64, false),
            "_disableNativePumpV1" => ("disable", Type::I64, Type::I64, false),
            "_supportsNativeEventDispatcherV1" => {
                ("dispatcher_supported", Type::I8, Type::I1, false)
            }
            "_setNativeEventDispatcherV1" => ("set_dispatcher", Type::I64, Type::I64, true),
            "_clearNativeEventDispatcherV1" => ("clear_dispatcher", Type::I64, Type::I64, false),
            _ => return false,
        };
        if self.func.return_ty != expected {
            return false;
        }
        let Some(signature) = self.ctx.outline.functions.get(fid) else {
            return false;
        };
        if callback {
            if signature.parameters.len() != 1 {
                return false;
            }
            let parameter = &signature.parameters[0];
            if parameter.kind != dartforge_frontend::ast::ParameterKind::Required {
                return false;
            }
            let dartforge_types::table::Type::Function {
                type_params,
                ret,
                positional,
                optional,
                named,
                nullable,
            } = self.ctx.table.get(parameter.ty)
            else {
                return false;
            };
            if !type_params.is_empty()
                || !positional.is_empty()
                || !optional.is_empty()
                || !named.is_empty()
                || *nullable
                || !matches!(self.ctx.table.get(*ret), dartforge_types::table::Type::Void)
            {
                return false;
            }
        } else if !signature.parameters.is_empty() {
            return false;
        }
        let mut args = Vec::new();
        if callback {
            self.declare_parameters(fid, false);
            let (value, _, ty) = &self.func.params[0];
            debug_assert_eq!(*ty, Type::Ref);
            args.push((Operand::Val(*value), Type::Ref));
        }
        let value = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: format!("dartforge_native_DartForge_native_pump_{native}"),
                args,
                ret_ty: abi,
            },
            abi,
        );
        let value = self.coerce(value, expected);
        self.terminate(Terminator::Return(Some(value)));
        true
    }
}

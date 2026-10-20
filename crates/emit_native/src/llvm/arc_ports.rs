//! Registration of the certified ports of the AOT image before running Dart.

use super::LlvmEmitter;
use std::fmt::Write;

impl LlvmEmitter<'_> {
    pub(super) fn arc_ports_registration_symbol(&self) -> String {
        if self.arc_ports_at_link {
            format!(
                "df.arc.ports.{}",
                self.module.record.as_deref().unwrap_or("program")
            )
        } else {
            "df.arc.register_ports".into()
        }
    }

    pub(super) fn emit_arc_aot_ports(&mut self) {
        let Some(certificate) = &self.module.arc_aot_ports else {
            return;
        };
        certificate
            .check(self.module)
            .expect("compiler bug: stale ARC ports");
        assert!(
            !self.maps_in_jit,
            "AOT ARC ports require a fixed image; the JIT needs pins and retirement"
        );
        if self.arc_ports_at_link {
            let registration = self.arc_ports_registration_symbol();
            writeln!(
                self.out,
                "declare void @{registration}()\n; df.arc-ports {registration}"
            )
            .unwrap();
            let mut port = |symbol: &str, kind: &str, contract: i64| {
                let function = self
                    .module
                    .functions
                    .iter()
                    .find(|f| f.symbol == symbol)
                    .expect("certified ARC port has a body");
                let params = function
                    .params
                    .iter()
                    .map(|(_, _, ty)| ty.llvm_ir())
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(self.out, "; df.arc-port {registration}\t{kind}\t{contract}\tdeclare {} @{symbol}({params})", function.return_ty.llvm_ir()).unwrap();
            };
            for symbol in certificate.uniforms() {
                port(symbol, "uniform", 0);
            }
            for (symbol, arity) in certificate.callbacks() {
                port(symbol, "word", *arity as i64);
            }
            for (symbol, abi) in certificate.typed_set() {
                port(symbol, "typed", *abi);
            }
            return;
        }
        self.out
            .push_str("define private void @df.arc.register_ports() {\n");
        for (i, s) in certificate.uniforms().iter().enumerate() {
            writeln!(self.out, "  %u{i} = ptrtoint ptr @{s} to i64").unwrap();
            writeln!(
                self.out,
                "  %uv{i} = call i64 @dartforge_arc_register_uniform_owned_v1(i64 %u{i})"
            )
            .unwrap();
        }
        for (i, (s, n)) in certificate.callbacks().iter().enumerate() {
            writeln!(self.out, "  %c{i} = ptrtoint ptr @{s} to i64").unwrap();
            writeln!(
                self.out,
                "  %cv{i} = call i64 @dartforge_arc_register_return_owned_v1(i64 %c{i}, i64 {n})"
            )
            .unwrap();
        }
        for (i, (s, abi)) in certificate.typed_set().iter().enumerate() {
            writeln!(self.out, "  %t{i} = ptrtoint ptr @{s} to i64").unwrap();
            writeln!(
                self.out,
                "  %tv{i} = call i64 @dartforge_arc_register_typed_v1(i64 %t{i}, i64 {abi})"
            )
            .unwrap();
        }
        self.out.push_str("  ret void\n}\n");
    }

    pub(super) fn register_arc_aot_ports(&mut self) {
        if self.module.arc_aot_ports.is_some() {
            writeln!(
                self.out,
                "  call void @{}()",
                self.arc_ports_registration_symbol()
            )
            .unwrap();
        }
    }
}

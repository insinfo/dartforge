//! Registration of the certified ports of the AOT image before running Dart.

use super::LlvmEmitter;
use std::fmt::Write;

impl LlvmEmitter<'_> {
    pub(super) fn emit_arc_aot_ports(&mut self) {
        let Some(certificate) = &self.module.arc_aot_ports else {
            return;
        };
        certificate
            .check(self.module)
            .expect("bug do compilador: portas ARC obsoletas");
        assert!(
            !self.maps_in_jit,
            "AOT ARC ports require a fixed image; the JIT needs pins and retirement"
        );
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
            self.out
                .push_str("  call void @df.arc.register_ports()\n");
        }
    }
}

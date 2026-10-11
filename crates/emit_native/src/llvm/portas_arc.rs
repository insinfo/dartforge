//! Registro das portas certificadas da imagem AOT antes de executar Dart.

use super::LlvmEmitter;
use std::fmt::Write;

impl LlvmEmitter<'_> {
    pub(super) fn emitir_portas_arc_aot(&mut self) {
        let Some(certificado) = &self.module.portas_arc_aot else {
            return;
        };
        certificado
            .conferir(self.module)
            .expect("bug do compilador: portas ARC obsoletas");
        assert!(
            !self.mapas_no_jit,
            "portas ARC AOT exigem imagem fixa; JIT precisa de pins e retirada"
        );
        self.out
            .push_str("define private void @df.arc.registrar_portas() {\n");
        for (i, s) in certificado.uniformes().iter().enumerate() {
            writeln!(self.out, "  %u{i} = ptrtoint ptr @{s} to i64").unwrap();
            writeln!(
                self.out,
                "  %uv{i} = call i64 @dartforge_arc_registrar_uniforme_owned_v1(i64 %u{i})"
            )
            .unwrap();
        }
        for (i, (s, n)) in certificado.callbacks().iter().enumerate() {
            writeln!(self.out, "  %c{i} = ptrtoint ptr @{s} to i64").unwrap();
            writeln!(
                self.out,
                "  %cv{i} = call i64 @dartforge_arc_registrar_retorno_owned_v1(i64 %c{i}, i64 {n})"
            )
            .unwrap();
        }
        self.out.push_str("  ret void\n}\n");
    }

    pub(super) fn registrar_portas_arc_aot(&mut self) {
        if self.module.portas_arc_aot.is_some() {
            self.out
                .push_str("  call void @df.arc.registrar_portas()\n");
        }
    }
}

//! `sass_builder:sass_builder` through the Sass of `ng_generator`
//! (`sass::compilar_ativo`, public API), which is dart-sass 1.102.0 byte for
//! byte (the `dartforge-sass` crate) or, when `pubspec.lock` brings `sass`
//! 1.66.0, the compatibility mode for it (`VersaoDartSass::V1_66_0`).
//!
//! Both styles (`compressed` and `expanded`, the `sass_builder` default) and
//! the `.css.map` when `sourceMaps` is on (`dev` turns it on by default), with
//! the `sourceMappingURL` comment in the `.css`, as `SassBuilder` writes them.
//! The stylesheets of `@use`/`@forward`/`@import` resolve as in
//! `BuildImporter` (relative to the file, and `package:` through the package
//! roots); every file read or probed is a dependency of the action.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use crate::valor::Valor;
use dartforge_ng_generator::sass::{Estilo, Leitor, VersaoDartSass, compilar_ativo};
use std::path::Path;
use std::sync::Arc;

/// The `BuildStep` for Sass: what the engine has in memory (outputs of
/// previous builders) or the disk.
struct StepReader<'a> {
    memory: &'a dyn Fn(&Path) -> Option<Arc<[u8]>>,
}

impl Leitor for StepReader<'_> {
    fn existe(&self, path: &Path) -> bool {
        (self.memory)(&dartforge_elements::gerado::chave(path)).is_some() || path.is_file()
    }

    fn ler(&self, path: &Path) -> std::io::Result<String> {
        match (self.memory)(&dartforge_elements::gerado::chave(path)) {
            Some(b) => String::from_utf8(b.to_vec())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            None => std::fs::read_to_string(path),
        }
    }
}

pub struct SassNative;

impl GeradorNativo for SassNative {
    fn chave(&self) -> &'static str {
        "sass_builder:sass_builder"
    }

    fn cobre(&self, _factory: &str) -> bool {
        true
    }

    fn por_pacote(&self) -> bool {
        false
    }

    fn verificado(&self) -> bool {
        true
    }

    fn gerar(
        &self,
        ctx: &mut CtxGerador<'_>,
        request: &PedidoNativo,
    ) -> Result<SaidaNativa, String> {
        let mut s = SaidaNativa::default();
        // The dart-sass from the lock (the engine only calls with an imitated
        // version).
        let version = request.versoes.get("sass").map(String::as_str);
        let Some(version) = version.and_then(VersaoDartSass::do_lock) else {
            return Err(format!(
                "sass: dart-sass version in the lock is not imitated: {}",
                version.unwrap_or("missing")
            ));
        };
        for a in &request.acoes {
            let name = a.entrada.caminho.rsplit('/').next().unwrap_or_default();
            if name.starts_with('_') {
                // Partial: the official builder generates nothing
                // (`sass_builder.dart:43`).
                continue;
            }
            // A missing `outputStyle` is `sass_builder`'s `expanded`; for an
            // unknown value it warns and uses `expanded` — we do not have the
            // warning.
            let style = match a.opcoes.obter("outputStyle") {
                None | Some(Valor::Nulo) => Estilo::Expandido,
                Some(Valor::Texto(v)) if v == "expanded" => Estilo::Expandido,
                Some(Valor::Texto(v)) if v == "compressed" => Estilo::Comprimido,
                Some(_) => {
                    s.recusas.insert(
                        a.entrada_natural.clone(),
                        "sass: unknown outputStyle (the official builder warns)".into(),
                    );
                    continue;
                }
            };
            let Some(source) = ctx.ler(&a.entrada_natural) else {
                s.recusas
                    .insert(a.entrada_natural.clone(), "sass: unreadable input".into());
                continue;
            };
            let text = String::from_utf8_lossy(&source);
            let source_map = a.opcoes.obter("sourceMaps") == Some(&Valor::Bool(true));
            let reader = StepReader {
                memory: ctx.memoria,
            };
            let sheet = match compilar_ativo(
                &text,
                &a.entrada.pacote,
                &a.entrada.caminho,
                &request.raizes,
                &reader,
                style,
                source_map,
                version,
            ) {
                Ok(f) => f,
                Err(m) => {
                    let first = m.lines().next().unwrap_or_default().to_owned();
                    s.recusas.insert(
                        a.entrada_natural.clone(),
                        format!("sass: dart-sass rejects it: {first}"),
                    );
                    continue;
                }
            };
            for file in sheet.lidos.iter().chain(&sheet.sondados) {
                ctx.registrar(Consulta::Arquivo(dartforge_elements::gerado::chave(
                    file,
                )));
            }
            if let Some((_, n)) = a.saidas.iter().find(|(id, _)| id.caminho.ends_with(".css")) {
                s.saidas.insert(n.clone(), sheet.css.into_bytes());
            }
            if let Some(m) = sheet.mapa
                && let Some((_, n)) = a
                    .saidas
                    .iter()
                    .find(|(id, _)| id.caminho.ends_with(".css.map"))
            {
                s.saidas.insert(n.clone(), m.into_bytes());
            }
        }
        Ok(s)
    }
}

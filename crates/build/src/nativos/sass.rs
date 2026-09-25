//! `sass_builder:sass_builder` pelo Sass do `gerador_ng`
//! (`sass::compilar_com`, API pública).
//!
//! **Porta de igualdade**: só o estilo `compressed` sem mapas, medido byte a
//! byte, pode ser publicado. O estilo `expanded` e o `.css.map` continuam no
//! apoio. Os módulos lidos por `@use`/`@import` são dependências da ação.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use crate::valor::Valor;
use dartforge_gerador_ng::sass::{compilar_com, Estilo};

pub struct SassNativo;

impl GeradorNativo for SassNativo {
    fn chave(&self) -> &'static str {
        "sass_builder:sass_builder"
    }

    fn cobre(&self, _fabrica: &str) -> bool {
        true
    }

    fn por_pacote(&self) -> bool {
        false
    }

    fn verificado(&self) -> bool {
        true
    }

    fn gerar(&self, ctx: &mut CtxGerador<'_>, pedido: &PedidoNativo) -> Result<SaidaNativa, String> {
        let mut s = SaidaNativa::default();
        for a in &pedido.acoes {
            let nome = a.entrada.caminho.rsplit('/').next().unwrap_or_default();
            if nome.starts_with('_') {
                continue; // parcial: o oficial não gera nada (`sass_builder.dart:43`)
            }
            if a.entrada.caminho.ends_with(".sass") {
                s.recusas.insert(a.entrada_natural.clone(), "sass: sintaxe indentada (.sass) não suportada".into());
                continue;
            }
            if !matches!(a.opcoes.obter("outputStyle"), Some(Valor::Texto(v)) if v == "compressed") {
                s.recusas.insert(a.entrada_natural.clone(), "sass: outputStyle expanded ainda não verificado".into());
                continue;
            }
            let Some(fonte) = ctx.ler(&a.entrada_natural) else {
                s.recusas.insert(a.entrada_natural.clone(), "sass: entrada ilegível".into());
                continue;
            };
            let texto = String::from_utf8_lossy(&fonte);
            let (mut saida, modulos) = match compilar_com(&texto, a.entrada_natural.parent(), Estilo::Comprimido) {
                Ok(c) => c,
                Err(m) => {
                    s.recusas.insert(a.entrada_natural.clone(), format!("sass: recusa {m:?}"));
                    continue;
                }
            };
            for modulo in modulos {
                ctx.registrar(Consulta::Arquivo(dartforge_elements::gerado::chave(&modulo)));
            }
            if a.opcoes.obter("sourceMaps") == Some(&Valor::Bool(true)) {
                let base = nome.rsplit_once('.').map(|(b, _)| b).unwrap_or(nome);
                saida.push_str(&format!("\n/*# sourceMappingURL={base}.css.map */\n"));
                s.recusas.insert(a.entrada_natural.clone(), "sass: .css.map não gerado".into());
            }
            if let Some((_, n)) = a.saidas.iter().find(|(id, _)| id.caminho.ends_with(".css")) {
                s.saidas.insert(n.clone(), saida.into_bytes());
            }
        }
        Ok(s)
    }
}

//! `sass_builder:sass_builder` pelo Sass do `gerador_ng`
//! (`sass::compilar_em`, API pública).
//!
//! **Porta de igualdade**: a regra governante exige byte a byte, e o Sass do
//! `gerador_ng` só é verificado depois do shim do ngdart (`sass.rs:14-17`),
//! não contra o CSS do `sass_builder` (estilos `expanded`/`compressed`, e o
//! `.css.map` de desenvolvimento). Então este gerador é **não verificado**:
//! o motor publica o apoio e só o executa para medir (`--comparar`), o que
//! diz quantos `.css` já sairiam iguais. O `.css` de componente nem é pedido
//! (o ngdart lê o `.scss` direto); o que chega ao navegador é o servido.
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};

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
        false
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
            let Some(fonte) = ctx.ler(&a.entrada_natural) else {
                s.recusas.insert(a.entrada_natural.clone(), "sass: entrada ilegível".into());
                continue;
            };
            let texto = String::from_utf8_lossy(&fonte);
            let css = match dartforge_gerador_ng::sass::compilar_em(&texto, a.entrada_natural.parent()) {
                Ok(c) => c,
                Err(m) => {
                    s.recusas.insert(a.entrada_natural.clone(), format!("sass: recusa {m:?}"));
                    continue;
                }
            };
            let mapas = a.opcoes.obter("sourceMaps") == Some(&crate::valor::Valor::Bool(true));
            let mut saida = css;
            if mapas {
                let base = nome.rsplit_once('.').map(|(b, _)| b).unwrap_or(nome);
                saida.push_str(&format!("\n\n/*# sourceMappingURL={base}.css.map */"));
                s.recusas.insert(a.entrada_natural.clone(), "sass: .css.map não gerado".into());
            }
            saida.push('\n');
            if let Some((_, n)) = a.saidas.iter().find(|(id, _)| id.caminho.ends_with(".css")) {
                s.saidas.insert(n.clone(), saida.into_bytes());
            }
        }
        Ok(s)
    }
}

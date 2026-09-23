//! `ngdart:ngdart` pelo `gerador_ng`, estágio A (`docs/BUILD-MOTOR.md`;
//! plano do motor §6.2): **uma ação de pacote**, sobre o `Program` da
//! sessão (o `Resolvedor` é o `BuildStep.resolver` sem carga extra), com
//! consultas conservadoras — qualquer `.dart`/`.html`/`.scss`/`.css` do
//! pacote, a lista de arquivos de `lib/`, `web/` e `test/`, e o texto de toda
//! biblioteca de fora do pacote. O motor faz o corte pela saída: só os
//! `.template.dart`/`.css.shim.dart` com texto novo invalidam unidades.
//!
//! O estágio B (uma ação por componente, consultas finas) depende de
//! `gerar_arquivo`, do índice incremental e das consultas feitas, pedidos ao
//! agente do `gerador_ng`.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use std::path::PathBuf;

pub struct NgEstagioA;

const EXTENSOES: &[&str] = &["dart", "html", "scss", "sass", "css"];

fn arquivos(dir: &std::path::Path, v: &mut Vec<PathBuf>) {
    let Ok(ls) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<_> = ls.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, v);
        } else if p.extension().is_some_and(|x| EXTENSOES.iter().any(|e| x == *e)) {
            v.push(p);
        }
    }
}

impl GeradorNativo for NgEstagioA {
    fn chave(&self) -> &'static str {
        "ngdart:ngdart"
    }

    fn cobre(&self, fabrica: &str) -> bool {
        fabrica == "templateCompiler" || fabrica == "stylesheetCompiler"
    }

    fn por_pacote(&self) -> bool {
        true
    }

    fn verificado(&self) -> bool {
        // Verificado byte a byte: 0 diferentes contra o oráculo do corpus
        // ngdart e do new_sali (ESTADO §1.7); o que não sabe, recusa.
        true
    }

    fn gerar(&self, ctx: &mut CtxGerador<'_>, pedido: &PedidoNativo) -> Result<SaidaNativa, String> {
        let raiz = &pedido.raiz_do_pacote;
        // Estágio A cobre o pacote raiz (como o `DARTFORGE_GERADOS=ng`); os
        // outros vêm do apoio.
        let Some((programa, nomes_programa)) = ctx.programa else {
            return Err("ngdart (estágio A): sem programa carregado".into());
        };
        let e_raiz = programa.entry.is_some_and(|l| {
            programa.library(l).units.first().and_then(|&u| programa.unit(u).path.as_ref()).is_some_and(|p| p.starts_with(raiz))
        });
        if !e_raiz {
            return Err("ngdart (estágio A): só o pacote da entrada".into());
        }
        let mut v = Vec::new();
        for d in ["lib", "web", "test"] {
            let dir = raiz.join(d);
            ctx.registrar(Consulta::Glob { dir: dartforge_elements::gerado::chave(&dir), padrao: "**".into() });
            arquivos(&dir, &mut v);
        }
        for p in &v {
            ctx.registrar(Consulta::Arquivo(dartforge_elements::gerado::chave(p)));
        }
        // O que o `Resolvedor` pode perguntar de fora do pacote.
        for l in &programa.libraries {
            if l.is_sdk {
                continue;
            }
            let de_fora = l
                .units
                .first()
                .and_then(|&u| programa.unit(u).path.as_ref())
                .is_none_or(|p| !p.starts_with(raiz));
            if de_fora {
                ctx.registrar(Consulta::FonteBiblioteca(l.uri.clone()));
            }
        }
        let pacote = dartforge_gerador_ng::Pacote { nome: pedido.pacote.clone(), raiz: raiz.clone() };
        let resolvedor = dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
        let mut nomes = dartforge_intern::Interner::new();
        let (g, placar) = dartforge_gerador_ng::gerar_com_apoio(&pacote, &mut nomes, None, Some(&resolvedor));
        let mut s = SaidaNativa::default();
        for (p, f) in g.iter() {
            if f.gerador == "ngdart" {
                s.saidas.insert(p.clone(), f.conteudo.as_bytes().to_vec());
            }
        }
        for (i, p) in placar.pendentes.iter().enumerate() {
            let m = placar
                .conjuntos
                .get(i)
                .and_then(|c| c.iter().next())
                .map(|m| format!("ngdart (estágio A) recusa: {m:?}"))
                .unwrap_or_else(|| "ngdart (estágio A) recusa".into());
            s.recusas.insert(p.clone(), m);
        }
        Ok(s)
    }
}

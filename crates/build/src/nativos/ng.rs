//! `ngdart:ngdart` pelo `gerador_ng`, estágio A (`docs/BUILD-MOTOR.md`;
//! plano do motor §6.2): **uma ação de pacote**, sobre o `Program` da
//! sessão (o `Resolvedor` é o `BuildStep.resolver` sem carga extra), com
//! consultas conservadoras — qualquer `.dart`/`.html`/`.scss`/`.css` do
//! pacote, a lista de arquivos de `lib/`, `web/` e `test/`, e o texto de toda
//! biblioteca de fora do pacote. O motor faz o corte pela saída: só os
//! `.template.dart`/`.css.shim.dart` com texto novo invalidam unidades.
//!
//! Primeiro corte do estágio B: uma edição de HTML já conhecido regenera
//! apenas os componentes que o leram. As demais edições seguem o estágio A;
//! o pacote ainda é a unidade de revalidação do motor.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct NgEstagioA {
    cache: Mutex<Option<CacheNg>>,
}

struct CacheNg {
    saida: SaidaNativa,
    /// Template HTML -> fontes Dart que o leram.
    fontes_do_html: BTreeMap<PathBuf, Vec<PathBuf>>,
    indice: Option<dartforge_gerador_ng::Indice>,
}

/// Algo no texto que pode ser Angular (conservador: qualquer menção ao
/// ngdart, às anotações ou a injetor).
fn marca_angular(texto: &str) -> bool {
    const MARCAS: &[&str] = &[
        "ngdart", "angular", "@Component", "@Directive", "@Pipe", "@Injectable", "GenerateInjector", "Injector",
        "@Input", "@Output", "@HostBinding", "@HostListener", "@View", "@Content", "OpaqueToken", "Provider",
    ];
    MARCAS.iter().any(|m| texto.contains(m))
}

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
        let t = std::time::Instant::now();
        let mut v = Vec::new();
        for d in ["lib", "web", "test"] {
            let dir = raiz.join(d);
            ctx.registrar(Consulta::Glob { dir: dartforge_elements::gerado::chave(&dir), padrao: "**".into() });
            arquivos(&dir, &mut v);
        }
        // Um `.dart` sem nenhuma marca de Angular só entra na geração pela
        // API (o `Resolvedor` pergunta onde um tipo é declarado e o tipo de
        // um membro público; o template dele é o trivial, que só depende do
        // nome). Para ele a consulta é a API da biblioteca: editar um corpo
        // ali não reexecuta o ngdart. O resto — componente, diretiva, pipe,
        // injetor, `.html`, `.scss`, `.css` — é o texto inteiro.
        let biblioteca_de: std::collections::HashMap<std::path::PathBuf, &str> = programa
            .units
            .iter()
            .filter_map(|u| {
                let p = u.path.as_ref()?;
                Some((dartforge_elements::gerado::chave(p), programa.library(u.library).uri.as_str()))
            })
            .collect();
        for p in &v {
            let k = dartforge_elements::gerado::chave(p);
            let dart = p.extension().is_some_and(|x| x == "dart");
            let uri = biblioteca_de.get(&k).copied();
            match (dart, uri) {
                (true, Some(uri)) if !marca_angular(&std::fs::read_to_string(p).unwrap_or_default()) => {
                    ctx.registrar(Consulta::ApiBiblioteca(uri.to_string()));
                    // O nome do arquivo decide o template trivial: a
                    // existência dele também é consulta.
                    ctx.registrar(Consulta::Existe(k));
                }
                _ => ctx.registrar(Consulta::Arquivo(k)),
            }
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
        let t_consultas = t.elapsed();
        // A rodada continua registrando as mesmas consultas conservadoras
        // do estágio A; a aceleração, por ora, só corta a geração.
        if let Some(saida) = self.tentar_html(ctx, pedido) {
            if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
                eprintln!(
                    "ngdart (estágio B): consultas {:.1} ms ({} arquivos), regeneração {:.1} ms",
                    t_consultas.as_secs_f64() * 1000.0,
                    v.len(),
                    (t.elapsed() - t_consultas).as_secs_f64() * 1000.0,
                );
            }
            return Ok(saida);
        }
        let pacote = dartforge_gerador_ng::Pacote { nome: pedido.pacote.clone(), raiz: raiz.clone() };
        let resolvedor = dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
        let mut nomes = dartforge_intern::Interner::new();
        let (g, placar) = dartforge_gerador_ng::gerar_com_apoio(&pacote, &mut nomes, None, Some(&resolvedor));
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (estágio A): consultas {:.1} ms ({} arquivos), gerar_com_apoio {:.1} ms",
                t_consultas.as_secs_f64() * 1000.0,
                v.len(),
                (t.elapsed() - t_consultas).as_secs_f64() * 1000.0
            );
        }
        let mut s = SaidaNativa::default();
        let mut fontes_do_html: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for (p, f) in g.iter() {
            if f.gerador == "ngdart" {
                s.saidas.insert(p.clone(), f.conteudo.as_bytes().to_vec());
                if p.to_string_lossy().ends_with(".template.dart") {
                    if let Some(fonte) = f.entradas.first() {
                        for html in f.entradas.iter().filter(|e| e.extension().is_some_and(|x| x == "html")) {
                            fontes_do_html.entry(dartforge_elements::gerado::chave(html)).or_default().push(fonte.clone());
                        }
                    }
                }
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
        s.unidades_geradas = placar.gerados;
        *self.cache.lock().map_err(|_| "ngdart: cache envenenado")? = Some(CacheNg {
            saida: clone_saida(&s),
            fontes_do_html,
            indice: None,
        });
        Ok(s)
    }
}

fn clone_saida(s: &SaidaNativa) -> SaidaNativa {
    SaidaNativa { saidas: s.saidas.clone(), recusas: s.recusas.clone(), unidades_geradas: s.unidades_geradas }
}

impl NgEstagioA {
    fn tentar_html(&self, ctx: &CtxGerador<'_>, pedido: &PedidoNativo) -> Option<SaidaNativa> {
        let mut mudados = ctx.mudados.iter().filter(|p| p.starts_with(&pedido.raiz_do_pacote));
        let html = mudados.next()?;
        if mudados.next().is_some() || html.extension().is_none_or(|e| e != "html") {
            return None;
        }
        let mut cache = self.cache.lock().ok()?;
        let cache = cache.as_mut()?;
        let fontes = cache.fontes_do_html.get(html)?.clone();
        let (programa, nomes_programa) = ctx.programa?;
        let pacote = dartforge_gerador_ng::Pacote { nome: pedido.pacote.clone(), raiz: pedido.raiz_do_pacote.clone() };
        let resolvedor = dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
        if cache.indice.is_none() {
            cache.indice = Some(indice_do_pacote(&pacote, &resolvedor));
        }
        let indice = cache.indice.as_ref()?;
        let mut novas = Vec::new();
        let componentes = fontes.len();
        for fonte in fontes {
            let texto = std::fs::read_to_string(&fonte).ok()?;
            let mut nomes = dartforge_intern::Interner::new();
            let achados = dartforge_gerador_ng::analisar_arquivo(&fonte, &texto, &mut nomes);
            let saida = dartforge_gerador_ng::gerar_arquivo(
                &pacote, &fonte, &achados, Some(&resolvedor), &mut nomes, indice,
            ).ok()?;
            let destino = dartforge_gerador_ng::caminho_do_template(&fonte);
            if !cache.saida.saidas.contains_key(&destino) {
                return None;
            }
            novas.push((destino, saida.template.into_bytes()));
            for (destino, texto) in saida.extras {
                if !cache.saida.saidas.contains_key(&destino) {
                    return None;
                }
                novas.push((destino, texto.into_bytes()));
            }
        }
        for (destino, bytes) in novas {
            cache.saida.saidas.insert(destino, bytes);
        }
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!("ngdart (estágio B): {} componente(s) para {}", cache.fontes_do_html[html].len(), html.display());
        }
        let mut saida = clone_saida(&cache.saida);
        saida.unidades_geradas = componentes;
        Some(saida)
    }
}

fn indice_do_pacote(
    pacote: &dartforge_gerador_ng::Pacote,
    resolvedor: &dartforge_gerador_ng::resolucao::Resolvedor<'_>,
) -> dartforge_gerador_ng::Indice {
    let mut indice = dartforge_gerador_ng::Indice::do_programa(resolvedor);
    let mut arquivos_dart = Vec::new();
    for dir in ["lib", "web", "test"] {
        arquivos(&pacote.raiz.join(dir), &mut arquivos_dart);
    }
    let mut nomes = dartforge_intern::Interner::new();
    for fonte in arquivos_dart {
        if fonte.extension().is_none_or(|e| e != "dart") || fonte.to_string_lossy().ends_with(".template.dart") {
            continue;
        }
        if let Ok(texto) = std::fs::read_to_string(&fonte) {
            let achados = dartforge_gerador_ng::analisar_arquivo(Path::new(&fonte), &texto, &mut nomes);
            indice.atualizar(pacote, &fonte, &achados, Some(resolvedor));
        }
    }
    indice
}

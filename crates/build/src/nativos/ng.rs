//! `ngdart:ngdart` pelo `gerador_ng`, estágio A (`docs/BUILD-MOTOR.md`;
//! plano do motor §6.2): **uma ação de pacote**, sobre o `Program` da
//! sessão (o `Resolvedor` é o `BuildStep.resolver` sem carga extra), com
//! consultas conservadoras — qualquer `.dart`/`.html`/`.scss`/`.css` do
//! pacote, a lista de arquivos de `lib/`, `web/` e `test/`, e o texto de toda
//! biblioteca de fora do pacote que as dele alcançam. O motor faz o corte pela
//! saída: só os `.template.dart`/`.css.shim.dart` com texto novo invalidam
//! unidades. Vale para o pacote da entrada e para cada dependência a que o
//! ngdart se aplica, cada um com a sua rodada.
//!
//! Estágio B, por dentro da ação de pacote: uma edição de HTML ou folha de
//! estilo já conhecida regenera apenas os componentes que a leram; uma edição
//! de `.dart` do pacote regenera o arquivo e quem o alcança por
//! `import`/`export` (`tentar_dart`). O resto (arquivo novo ou apagado,
//! parte, recusa, outro pacote) segue o estágio A; o pacote ainda é a unidade
//! de revalidação do motor.
use crate::consulta::Consulta;
use crate::executor::{CtxGerador, GeradorNativo, PedidoNativo, SaidaNativa};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct NgEstagioA {
    /// A chave do builder: `ngdart:ngdart` (ngdart 8) ou `ngx_dart:ngx_dart`
    /// (o fork 9 com `package:web`); o `gerador_ng` escolhe o dialeto pelo
    /// programa (`docs/NGDART-COMPILADOR-DE-VISOES.md` parte B).
    chave: &'static str,
    /// Por pacote: a entrada e cada dependência com componentes têm a sua
    /// rodada.
    cache: Mutex<HashMap<String, CacheNg>>,
}

impl Default for NgEstagioA {
    fn default() -> Self {
        Self::com_chave("ngdart:ngdart")
    }
}

impl NgEstagioA {
    /// O gerador para o builder `chave` (`ngdart:ngdart` ou
    /// `ngx_dart:ngx_dart`).
    pub fn com_chave(chave: &'static str) -> Self {
        NgEstagioA {
            chave,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

struct CacheNg {
    saida: SaidaNativa,
    /// HTML/CSS/SCSS -> fontes Dart que leram o recurso.
    fontes_do_recurso: BTreeMap<PathBuf, Vec<PathBuf>>,
    indice: Option<dartforge_gerador_ng::Indice>,
}

/// Algo no texto que pode ser Angular (conservador: qualquer menção ao
/// ngdart, às anotações ou a injetor).
fn marca_angular(texto: &str) -> bool {
    const MARCAS: &[&str] = &[
        "ngdart",
        "angular",
        "@Component",
        "@Directive",
        "@Pipe",
        "@Injectable",
        "GenerateInjector",
        "Injector",
        "@Input",
        "@Output",
        "@HostBinding",
        "@HostListener",
        "@View",
        "@Content",
        "OpaqueToken",
        "Provider",
    ];
    MARCAS.iter().any(|m| texto.contains(m))
}

const EXTENSOES: &[&str] = &["dart", "html", "scss", "sass", "css"];

/// As folhas `.css` que o `sass_builder` (ou outro `SassBuilder`) desta
/// build gerou para os `.scss`/`.sass` do pacote (na memória do motor): o shim do ngdart parte delas, como no
/// oficial, no `outputStyle` do projeto. Uma folha `.css` no disco fica com
/// o disco.
fn folhas_geradas(ctx: &mut CtxGerador<'_>, raiz: &Path) -> HashMap<PathBuf, String> {
    let mut fontes = Vec::new();
    for d in ["lib", "web", "test"] {
        arquivos(&raiz.join(d), &mut fontes);
    }
    let mut folhas = HashMap::new();
    for scss in fontes {
        let parcial = scss
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('_'));
        if parcial || scss.extension().is_none_or(|e| e != "scss" && e != "sass") {
            continue;
        }
        // `x.css` do `sass_builder`, ou `x.scss.css` de um `SassBuilder` com
        // `outputExtension: '.scss.css'` (o `scss_builder` do ngcomponents).
        let mut nome_scss_css = scss.clone().into_os_string();
        nome_scss_css.push(".css");
        for css in [scss.with_extension("css"), PathBuf::from(nome_scss_css)] {
            if css.is_file() {
                continue;
            }
            if let Some(b) = ctx.ler(&css) {
                folhas.insert(css, String::from_utf8_lossy(&b).into_owned());
            }
        }
    }
    folhas
}

fn arquivos(dir: &std::path::Path, v: &mut Vec<PathBuf>) {
    let Ok(ls) = std::fs::read_dir(dir) else {
        return;
    };
    let mut es: Vec<_> = ls.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, v);
        } else if p
            .extension()
            .is_some_and(|x| EXTENSOES.iter().any(|e| x == *e))
        {
            v.push(p);
        }
    }
}

impl GeradorNativo for NgEstagioA {
    fn chave(&self) -> &'static str {
        self.chave
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

    fn gerar(
        &self,
        ctx: &mut CtxGerador<'_>,
        pedido: &PedidoNativo,
    ) -> Result<SaidaNativa, String> {
        let raiz = &pedido.raiz_do_pacote;
        // Qualquer pacote a que o ngdart se aplica: o da entrada e as
        // dependências com componentes (um `path` como o `limitless_ui` do
        // `example`, e os `hosted` ngdart/ngcompiler, cujos `.template.dart`
        // o DDC pede). Os arquivos que o programa da entrada não carrega o
        // gerador recusa com motivo.
        let Some((programa, nomes_programa)) = ctx.programa else {
            return Err("ngdart (estágio A): sem programa carregado".into());
        };
        // Num recurso existente, a lista de consultas conservadoras do pacote
        // não ganha termos novos. O motor conserva as respostas anteriores e
        // substitui apenas o digest deste arquivo.
        if let Some(saida) = self.tentar_recurso(ctx, pedido) {
            return Ok(saida);
        }
        if let Some(saida) = self.tentar_dart(ctx, pedido) {
            return Ok(saida);
        }
        let t = std::time::Instant::now();
        let mut v = Vec::new();
        for d in ["lib", "web", "test"] {
            let dir = raiz.join(d);
            ctx.registrar(Consulta::Glob {
                dir: dartforge_elements::gerado::chave(&dir),
                padrao: "**".into(),
            });
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
                Some((
                    dartforge_elements::gerado::chave(p),
                    programa.library(u.library).uri.as_str(),
                ))
            })
            .collect();
        for p in &v {
            let k = dartforge_elements::gerado::chave(p);
            let dart = p.extension().is_some_and(|x| x == "dart");
            let uri = biblioteca_de.get(&k).copied();
            match (dart, uri) {
                (true, Some(uri))
                    if !marca_angular(&std::fs::read_to_string(p).unwrap_or_default()) =>
                {
                    ctx.registrar(Consulta::ApiBiblioteca(uri.to_string()));
                    // O nome do arquivo decide o template trivial: a
                    // existência dele também é consulta.
                    ctx.registrar(Consulta::Existe(k));
                }
                _ => ctx.registrar(Consulta::Arquivo(k)),
            }
        }
        // O que o `Resolvedor` pode perguntar de fora do pacote: as
        // bibliotecas que as do pacote alcançam por `import`/`export` (a
        // geração de um arquivo só enxerga o escopo dele). Numa dependência,
        // editar a aplicação não acorda a rodada dela.
        let do_pacote =
            |l: &dartforge_elements::model::Library| match l.uri.strip_prefix("package:") {
                Some(r) => r.split('/').next() == Some(pedido.pacote.as_str()),
                None => l
                    .units
                    .first()
                    .and_then(|&u| programa.unit(u).path.as_ref())
                    .is_some_and(|p| p.starts_with(raiz)),
            };
        let mut vistos = vec![false; programa.libraries.len()];
        let mut fila: Vec<usize> = (0..programa.libraries.len())
            .filter(|&i| !programa.libraries[i].is_sdk && do_pacote(&programa.libraries[i]))
            .collect();
        while let Some(i) = fila.pop() {
            if std::mem::replace(&mut vistos[i], true) {
                continue;
            }
            let l = &programa.libraries[i];
            fila.extend(
                l.imports
                    .iter()
                    .map(|x| x.library.0 as usize)
                    .chain(l.exports.iter().map(|x| x.library.0 as usize)),
            );
            if !l.is_sdk && !do_pacote(l) {
                ctx.registrar(Consulta::FonteBiblioteca(l.uri.clone()));
            }
        }
        let t_consultas = t.elapsed();
        let pacote = dartforge_gerador_ng::Pacote {
            nome: pedido.pacote.clone(),
            raiz: raiz.clone(),
            folhas_geradas: folhas_geradas(ctx, &raiz),
        };
        let resolvedor =
            dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
        let mut nomes = dartforge_intern::Interner::new();
        let (g, placar) =
            dartforge_gerador_ng::gerar_com_apoio(&pacote, &mut nomes, None, Some(&resolvedor));
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (estágio A): consultas {:.1} ms ({} arquivos), gerar_com_apoio {:.1} ms",
                t_consultas.as_secs_f64() * 1000.0,
                v.len(),
                (t.elapsed() - t_consultas).as_secs_f64() * 1000.0
            );
        }
        let mut s = SaidaNativa::default();
        let mut fontes_do_recurso: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        // O gerador escreve para todo arquivo dos diretórios; o motor fica só
        // com as saídas que o plano espera (o `generate_for` do `build.yaml`
        // tira, por exemplo, as folhas de `web/assets/`).
        let planejadas = planejadas(pedido);
        for (p, f) in g.iter() {
            if f.gerador == "ngdart" && planejadas.contains(&dartforge_elements::gerado::chave(p)) {
                s.saidas.insert(p.clone(), f.conteudo.as_bytes().to_vec());
                if p.to_string_lossy().ends_with(".template.dart") {
                    if let Some(fonte) = f.entradas.first() {
                        for recurso in f.entradas.iter().filter(|e| {
                            e.extension().is_some_and(|x| {
                                matches!(x.to_str(), Some("html" | "css" | "scss" | "sass"))
                            })
                        }) {
                            fontes_do_recurso
                                .entry(dartforge_elements::gerado::chave(recurso))
                                .or_default()
                                .push(fonte.clone());
                        }
                    }
                }
            }
        }
        for (i, p) in placar.pendentes.iter().enumerate() {
            // A folha recusada vem pelo destino: a recusa é da entrada.
            let p = &folha_de_destino(p).unwrap_or_else(|| p.clone());
            let m = placar
                .conjuntos
                .get(i)
                .and_then(|c| c.iter().next())
                .map(|m| format!("ngdart (estágio A) recusa: {m:?}"))
                .unwrap_or_else(|| "ngdart (estágio A) recusa".into());
            s.recusas.insert(p.clone(), m);
        }
        s.unidades_geradas = placar.gerados;
        self.cache
            .lock()
            .map_err(|_| "ngdart: cache envenenado")?
            .insert(
                pedido.pacote.clone(),
                CacheNg {
                    saida: clone_saida(&s),
                    fontes_do_recurso,
                    indice: None,
                },
            );
        Ok(s)
    }
}

/// As saídas que o plano espera das ações do pedido, pela chave.
fn planejadas(pedido: &PedidoNativo) -> std::collections::HashSet<PathBuf> {
    pedido
        .acoes
        .iter()
        .flat_map(|a| {
            a.saidas
                .iter()
                .map(|(_, p)| dartforge_elements::gerado::chave(p))
        })
        .collect()
}

/// A folha `x.css` de uma saída `x.css.dart`/`x.css.shim.dart`.
fn folha_de_destino(p: &Path) -> Option<PathBuf> {
    let nome = p.file_name()?.to_string_lossy();
    let base = nome
        .strip_suffix(".shim.dart")
        .or_else(|| nome.strip_suffix(".dart"))?;
    base.ends_with(".css").then(|| p.with_file_name(base))
}

fn clone_saida(s: &SaidaNativa) -> SaidaNativa {
    SaidaNativa {
        saidas: s.saidas.clone(),
        recusas: s.recusas.clone(),
        unidades_geradas: s.unidades_geradas,
        reutilizar_consultas: s.reutilizar_consultas,
    }
}

impl NgEstagioA {
    /// Edição de `.dart` do pacote (B03): regenera só os arquivos que podem
    /// depender do que mudou — o próprio arquivo e quem o alcança por
    /// `import`/`export`, transitivamente, no programa novo. Tudo o que a
    /// geração de um arquivo lê de outra biblioteca (filho de `directives:`,
    /// tipo resolvido, metadados herdados de uma superclasse, seletor) só
    /// pode vir de biblioteca alcançável pelos imports dele: é o fecho que
    /// cobre, com folga, as consultas que o `gerar_arquivo` anota. Os
    /// arquivos do fecho são reindexados antes de gerar.
    ///
    /// Volta ao estágio A (devolve `None`) no que não sabe tratar assim:
    /// arquivo fora do pacote, parte, arquivo sem template anterior, arquivo
    /// recusado antes ou agora, saída que não existia.
    fn tentar_dart(&self, ctx: &mut CtxGerador<'_>, pedido: &PedidoNativo) -> Option<SaidaNativa> {
        let raiz = &pedido.raiz_do_pacote;
        let canon_raiz = std::fs::canonicalize(raiz).ok()?;
        // Os eventos vêm na forma lexical e na canônica: um conjunto só.
        let mut mudados: Vec<PathBuf> = Vec::new();
        for p in ctx.mudados.iter() {
            let c = std::fs::canonicalize(p).ok()?;
            if !mudados.contains(&c) {
                mudados.push(c);
            }
        }
        if mudados.is_empty()
            || !mudados.iter().all(|p| {
                p.starts_with(&canon_raiz)
                    && p.extension().is_some_and(|e| e == "dart")
                    && !p.to_string_lossy().ends_with(".template.dart")
            })
        {
            return None;
        }
        let (programa, nomes_programa) = ctx.programa?;
        // Unidade (canônica) → biblioteca, e o grafo reverso de
        // import/export de todas as bibliotecas carregadas.
        let mut biblioteca_da_unidade: HashMap<PathBuf, usize> = HashMap::new();
        for (i, l) in programa.libraries.iter().enumerate() {
            for &u in &l.units {
                if let Some(p) = programa
                    .unit(u)
                    .path
                    .as_ref()
                    .and_then(|p| std::fs::canonicalize(p).ok())
                {
                    biblioteca_da_unidade.insert(p, i);
                }
            }
        }
        let mut dependentes: Vec<Vec<usize>> = vec![Vec::new(); programa.libraries.len()];
        for (i, l) in programa.libraries.iter().enumerate() {
            let alvos = l
                .imports
                .iter()
                .map(|x| x.library)
                .chain(l.exports.iter().map(|x| x.library));
            for alvo in alvos {
                if let Some(d) = dependentes.get_mut(alvo.0 as usize) {
                    d.push(i);
                }
            }
        }
        let mut fila: Vec<usize> = Vec::new();
        for p in &mudados {
            let &l = biblioteca_da_unidade.get(p)?;
            // Só a unidade principal: uma parte muda a biblioteca inteira, e
            // o estágio A cuida disso.
            let principal = programa.libraries[l]
                .units
                .first()
                .and_then(|&u| programa.unit(u).path.as_ref());
            if principal
                .and_then(|x| std::fs::canonicalize(x).ok())
                .as_ref()
                != Some(p)
            {
                return None;
            }
            fila.push(l);
        }
        let mut vistos = vec![false; programa.libraries.len()];
        let mut fecho: Vec<PathBuf> = Vec::new();
        while let Some(l) = fila.pop() {
            if std::mem::replace(&mut vistos[l], true) {
                continue;
            }
            fila.extend(dependentes[l].iter().copied());
            let lib = &programa.libraries[l];
            if lib.is_sdk {
                continue;
            }
            let Some(p) = lib
                .units
                .first()
                .and_then(|&u| programa.unit(u).path.as_ref())
            else {
                continue;
            };
            let Ok(c) = std::fs::canonicalize(p) else {
                continue;
            };
            let gerado = c.to_string_lossy().ends_with(".template.dart");
            if c.starts_with(&canon_raiz) && !gerado {
                fecho.push(dartforge_elements::gerado::chave(p));
            }
        }
        fecho.sort();
        let mut cache = self.cache.lock().ok()?;
        let cache = cache.get_mut(&pedido.pacote)?;
        let pacote = dartforge_gerador_ng::Pacote {
            nome: pedido.pacote.clone(),
            raiz: raiz.clone(),
            folhas_geradas: folhas_geradas(ctx, &raiz),
        };
        let resolvedor =
            dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
        for fonte in &fecho {
            let destino = dartforge_gerador_ng::caminho_do_template(fonte);
            if !cache.saida.saidas.contains_key(&destino) || cache.saida.recusas.contains_key(fonte)
            {
                return None;
            }
        }
        // O índice: do programa novo, se ainda não existe; senão, os arquivos
        // do fecho são reindexados (os outros não alcançam o que mudou).
        let mut textos = Vec::with_capacity(fecho.len());
        let mut nomes = dartforge_intern::Interner::new();
        for fonte in &fecho {
            let texto = std::fs::read_to_string(fonte).ok()?;
            let achados = dartforge_gerador_ng::analisar_arquivo(fonte, &texto, &mut nomes);
            textos.push(achados);
        }
        match cache.indice.as_mut() {
            None => cache.indice = Some(indice_do_pacote(&pacote, &resolvedor)),
            Some(indice) => {
                for (fonte, achados) in fecho.iter().zip(&textos) {
                    indice.atualizar(&pacote, fonte, achados, Some(&resolvedor));
                }
            }
        }
        let indice = cache.indice.as_ref()?;
        let mut novas = Vec::new();
        let mut recursos: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
        for (fonte, achados) in fecho.iter().zip(&textos) {
            let saida = dartforge_gerador_ng::gerar_arquivo(
                &pacote,
                fonte,
                achados,
                Some(&resolvedor),
                &mut nomes,
                indice,
            )
            .ok()?;
            let destino = dartforge_gerador_ng::caminho_do_template(fonte);
            novas.push((destino, saida.template.into_bytes()));
            for (destino, texto) in saida.extras {
                if !cache.saida.saidas.contains_key(&destino) {
                    return None;
                }
                novas.push((destino, texto.into_bytes()));
            }
            let lidos = saida
                .entradas
                .iter()
                .filter(|e| {
                    e.extension().is_some_and(|x| {
                        matches!(x.to_str(), Some("html" | "css" | "scss" | "sass"))
                    })
                })
                .map(|e| dartforge_elements::gerado::chave(e))
                .collect();
            recursos.push((fonte.clone(), lidos));
        }
        for (destino, bytes) in novas {
            cache.saida.saidas.insert(destino, bytes);
        }
        // Os recursos que cada arquivo do fecho lê agora.
        for (fonte, lidos) in recursos {
            for leitores in cache.fontes_do_recurso.values_mut() {
                leitores.retain(|f| *f != fonte);
            }
            for r in lidos {
                cache
                    .fontes_do_recurso
                    .entry(r)
                    .or_default()
                    .push(fonte.clone());
            }
        }
        cache.fontes_do_recurso.retain(|_, v| !v.is_empty());
        if std::env::var_os("DARTFORGE_MOTOR_TEMPOS").is_some() {
            eprintln!(
                "ngdart (estágio B): {} arquivo(s) no fecho de {} edição(ões) .dart",
                fecho.len(),
                mudados.len()
            );
        }
        let mut saida = clone_saida(&cache.saida);
        saida.unidades_geradas = fecho.len();
        saida.reutilizar_consultas = true;
        // As consultas do estágio A sobre os arquivos editados, com o
        // digest novo: o texto (arquivo com Angular) e a API (sem Angular).
        let biblioteca_de: HashMap<PathBuf, &str> = programa
            .units
            .iter()
            .filter_map(|u| {
                Some((
                    dartforge_elements::gerado::chave(u.path.as_ref()?),
                    programa.library(u.library).uri.as_str(),
                ))
            })
            .collect();
        for p in ctx.mudados.clone().iter() {
            let k = dartforge_elements::gerado::chave(p);
            ctx.registrar(Consulta::Arquivo(k.clone()));
            ctx.registrar(Consulta::Existe(k.clone()));
            if let Some(uri) = biblioteca_de.get(&k) {
                ctx.registrar(Consulta::ApiBiblioteca(uri.to_string()));
            }
        }
        Some(saida)
    }

    fn tentar_recurso(
        &self,
        ctx: &mut CtxGerador<'_>,
        pedido: &PedidoNativo,
    ) -> Option<SaidaNativa> {
        let recurso = ctx.mudados.iter().find(|p| {
            p.starts_with(&pedido.raiz_do_pacote)
                && p.extension()
                    .is_some_and(|e| matches!(e.to_str(), Some("html" | "css" | "scss" | "sass")))
        })?;
        // O motor inclui a forma lexical e a forma canônica do mesmo evento;
        // no Windows elas podem ter raízes distintas (links/nomes curtos).
        // Só permitimos o atalho quando *todos* os eventos são esse arquivo.
        let canon_recurso = std::fs::canonicalize(recurso).ok()?;
        if ctx
            .mudados
            .iter()
            .any(|p| std::fs::canonicalize(p).ok().as_ref() != Some(&canon_recurso))
        {
            return None;
        }
        let mut cache = self.cache.lock().ok()?;
        let cache = cache.get_mut(&pedido.pacote)?;
        // Uma folha (o `.css`, ou o `.scss` que o `sass_builder` compila):
        // só as saídas dela, que não dependem de nenhum componente.
        if recurso
            .extension()
            .is_some_and(|e| matches!(e.to_str(), Some("css" | "scss" | "sass")))
        {
            let pacote = dartforge_gerador_ng::Pacote {
                nome: pedido.pacote.clone(),
                raiz: pedido.raiz_do_pacote.clone(),
                folhas_geradas: folhas_geradas(ctx, &pedido.raiz_do_pacote),
            };
            let css = recurso.with_extension("css");
            let mut novas = Vec::new();
            for (destino, r) in dartforge_gerador_ng::gerar_folha(&pacote, &css) {
                // Saída que não era nossa, ou que deixa de ser: o caminho
                // completo decide.
                let bytes = r.ok()?.into_bytes();
                if !cache.saida.saidas.contains_key(&destino) {
                    return None;
                }
                novas.push((destino, bytes));
            }
            for (destino, bytes) in novas {
                cache.saida.saidas.insert(destino, bytes);
            }
            let mut saida = clone_saida(&cache.saida);
            saida.unidades_geradas = 1;
            saida.reutilizar_consultas = true;
            ctx.registrar(Consulta::Arquivo(recurso.clone()));
            return Some(saida);
        }
        let fontes = cache.fontes_do_recurso.get(recurso)?.clone();
        let (programa, nomes_programa) = ctx.programa?;
        let pacote = dartforge_gerador_ng::Pacote {
            nome: pedido.pacote.clone(),
            raiz: pedido.raiz_do_pacote.clone(),
            folhas_geradas: folhas_geradas(ctx, &pedido.raiz_do_pacote),
        };
        let resolvedor =
            dartforge_gerador_ng::resolucao::Resolvedor::novo(programa, nomes_programa);
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
                &pacote,
                &fonte,
                &achados,
                Some(&resolvedor),
                &mut nomes,
                indice,
            )
            .ok()?;
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
            eprintln!(
                "ngdart (estágio B): {} componente(s) para {}",
                cache.fontes_do_recurso[recurso].len(),
                recurso.display()
            );
        }
        let mut saida = clone_saida(&cache.saida);
        saida.unidades_geradas = componentes;
        saida.reutilizar_consultas = true;
        ctx.registrar(Consulta::Arquivo(recurso.clone()));
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
        if fonte.extension().is_none_or(|e| e != "dart")
            || fonte.to_string_lossy().ends_with(".template.dart")
        {
            continue;
        }
        if let Ok(texto) = std::fs::read_to_string(&fonte) {
            let achados =
                dartforge_gerador_ng::analisar_arquivo(Path::new(&fonte), &texto, &mut nomes);
            indice.atualizar(pacote, &fonte, &achados, Some(resolvedor));
        }
    }
    indice
}

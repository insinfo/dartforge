//! Sessão residente do `dartforge dev` (PLANO.md, "o compilador residente").
//!
//! O processo não termina a cada edição: `Interner`, unidades já analisadas e
//! os hashes por biblioteca ficam vivos entre compilações. Uma edição relê os
//! metadados dos arquivos (mtime e tamanho), reanalisa **só** o que mudou e
//! grava só os módulos cujo texto mudou.
//!
//! Disciplina de memória: o que a sessão retém cresce com o **projeto**, não
//! com o número de edições — a unidade nova substitui a velha no mesmo lugar
//! do cache (`crates/dev/tests/plato.rs` vigia com `live_bytes`).
pub mod hashes;

use dartforge_elements::load::load_lenient_incremental;
use dartforge_elements::model::{LibraryId, Program};
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::{CacheUnidades, SdkCache};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use hashes::{hashes_da_biblioteca, HashesBiblioteca};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// O que uma compilação fez, em tempo e em trabalho.
#[derive(Debug, Default, Clone)]
pub struct Relatorio {
    pub carregar: Duration,
    pub outline: Duration,
    pub corpos: Duration,
    pub emissao: Duration,
    pub hashes: Duration,
    pub escrita: Duration,
    /// Unidades que vieram prontas do cache da sessão.
    pub unidades_reaproveitadas: usize,
    pub unidades_reanalisadas: usize,
    pub bibliotecas: usize,
    /// Bibliotecas cujo conteúdo mudou desde a compilação anterior.
    pub corpo_alterado: Vec<String>,
    /// Bibliotecas cuja API pública mudou (as dependentes também recompilam).
    pub api_alterada: Vec<String>,
    /// Dependentes invalidados por mudança de API.
    pub dependentes_invalidados: usize,
    pub modulos: usize,
    /// Módulos cujo texto foi realmente gerado nesta compilação.
    pub modulos_reemitidos: usize,
    /// Bibliotecas reemitidas (`usize::MAX` = programa inteiro).
    pub bibliotecas_reemitidas: usize,
    /// Custo de montar o contexto de emissão (hierarquia, grupos, interop).
    pub emissao_contexto: Duration,
    pub modulos_escritos: usize,
    pub avisos_tipos: usize,
    /// Primeira compilação da sessão (nada reaproveitado).
    pub primeira: bool,
    /// Sub-fases da carga (leitura, parse, diretivas, outline).
    pub carga_detalhe: dartforge_elements::model::TemposCarga,
}

impl Relatorio {
    pub fn total(&self) -> Duration {
        self.carregar + self.outline + self.corpos + self.emissao + self.hashes + self.escrita
    }

    /// Uma linha por fase, no mesmo formato do `compile-js --timings`.
    pub fn texto(&self) -> String {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut s = String::new();
        let c = &self.carga_detalhe;
        for (n, d) in [
            ("carregar", self.carregar),
            ("  leitura+lex paralelos", c.leitura_lex_paralelo),
            ("  leitura+lex em série", c.leitura),
            ("  parse", c.parse),
            ("  diretivas/URIs", c.diretivas),
            ("  outline: declarações", c.outline_declaracoes),
            ("  outline: reexports", c.outline_reexports),
            ("  outline: escopos", c.outline_escopos),
            ("  outline: supertipos", c.outline_supertipos),
            ("outline (tipos)", self.outline),
            ("inferência de corpos", self.corpos),
            ("  emissão: contexto", self.emissao_contexto),
            ("emissão", self.emissao),
            ("hashes", self.hashes),
            ("escrita", self.escrita),
        ] {
            s.push_str(&format!("{n:<24}{:>9.1} ms\n", ms(d)));
        }
        s.push_str(&format!("{:<24}{:>9.1} ms\n", "total", ms(self.total())));
        s.push_str(&format!(
            "unidades: {} reaproveitadas + {} reanalisadas; {} bibliotecas; corpo alterado: {}; API alterada: {} (+{} dependentes); módulos: {} ({} reemitidos, {} escritos); avisos: {}\n",
            self.unidades_reaproveitadas,
            self.unidades_reanalisadas,
            self.bibliotecas,
            self.corpo_alterado.len(),
            self.api_alterada.len(),
            self.dependentes_invalidados,
            self.modulos,
            self.modulos_reemitidos,
            self.modulos_escritos,
            self.avisos_tipos
        ));
        s
    }
}

/// Sessão residente: mantém vivo entre compilações o que não mudou.
pub struct Sessao {
    /// Texto já emitido por classe e por biblioteca, reusado entre
    /// compilações (ver `emitir_modulos_com_cache`).
    fragmentos: std::cell::RefCell<dartforge_emit_js::module::CacheFragmentos>,
    entrada: PathBuf,
    sdk: SdkLayout,
    packages: Option<PathBuf>,
    saida: PathBuf,
    dart_sdk_js: PathBuf,
    /// Arena de nomes: os `SymbolId` das árvores guardadas pertencem a ela.
    interner: Interner,
    unidades: CacheUnidades,
    /// `uri da biblioteca` → hashes da compilação anterior.
    hashes: HashMap<String, HashesBiblioteca>,
    /// JS de cada módulo da última compilação (caminho → texto).
    modulos: HashMap<String, String>,
    /// `uri` → uris que ela importa/exporta (para propagar mudança de API).
    dependentes: HashMap<String, Vec<String>>,
    /// Arquivos invalidados desde a última compilação: só as bibliotecas que
    /// os contêm precisam de hash novo.
    sujos: std::collections::HashSet<PathBuf>,
    primeira: bool,
}

impl Sessao {
    /// Abre uma sessão para uma entrada; não compila ainda.
    ///
    /// `sdk_lib` é o `lib/` do SDK (descoberto pelo ambiente quando `None`).
    pub fn nova(
        entrada: &Path,
        sdk_lib: Option<&Path>,
        packages: Option<&Path>,
        saida: &Path,
    ) -> Result<Sessao, String> {
        let dir = match sdk_lib {
            Some(p) => p.to_path_buf(),
            None => SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
        };
        let sdk = SdkLayout::load(&dir, "dartdevc")?;
        Ok(Sessao {
            fragmentos: Default::default(),
            entrada: entrada.to_path_buf(),
            sdk,
            packages: packages.map(|p| p.to_path_buf()),
            saida: saida.to_path_buf(),
            dart_sdk_js: dartforge_emit_js::dart_sdk_js_padrao(),
            interner: Interner::new(),
            unidades: CacheUnidades::nova(),
            hashes: HashMap::new(),
            modulos: HashMap::new(),
            dependentes: HashMap::new(),
            sujos: std::collections::HashSet::new(),
            primeira: true,
        })
    }

    /// Bytes de fonte retidos pelo cache de unidades (para o teste de platô).
    pub fn bytes_retidos(&self) -> usize {
        self.unidades.bytes_fonte() + self.interner.payload_bytes()
    }

    pub fn unidades_em_cache(&self) -> usize {
        self.unidades.len()
    }

    /// Arquivos cujo mtime ou tamanho mudaram desde a última análise: é o
    /// que o `dartforge dev` consulta a cada intervalo (um `stat` por
    /// unidade, sem ler conteúdo).
    pub fn mudancas(&self) -> Vec<PathBuf> {
        self.unidades.alterados()
    }

    /// Arquivos observados pela sessão.
    pub fn observados(&self) -> Vec<PathBuf> {
        self.unidades.caminhos()
    }

    /// Avisa que um arquivo mudou: a unidade correspondente sai do cache.
    ///
    /// Não é obrigatório — a marca (mtime e tamanho) já detecta a mudança na
    /// compilação seguinte —, mas evita um `stat` inútil e cobre o caso de
    /// edição com o mesmo tamanho dentro da granularidade do relógio.
    pub fn arquivo_mudou(&mut self, path: &Path) {
        let p = dartforge_elements::config::sem_verbatim(
            std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        );
        self.unidades.invalidar(&p);
        self.unidades.invalidar(path);
        self.sujos.insert(p);
        self.sujos.insert(path.to_path_buf());
    }

    /// Compila (ou recompila) e escreve o que mudou.
    pub fn compilar(&mut self) -> Result<Relatorio, String> {
        let mut rel = Relatorio { primeira: self.primeira, ..Default::default() };

        let t = Instant::now();
        // O SDK vem do artefato em `target/dartforge` na primeira compilação;
        // depois as suas unidades já estão no cache da sessão.
        let cache_sdk = if self.unidades.is_empty() {
            SdkCache::abrir_ou_construir(&self.sdk, "dartdevc").ok().map(|(c, _)| c)
        } else {
            None
        };
        let (program, diags) = load_lenient_incremental(
            &self.entrada,
            &self.sdk,
            self.packages.as_deref(),
            &mut self.interner,
            cache_sdk,
            Some(&mut self.unidades),
        );
        rel.carregar = t.elapsed();
        rel.carga_detalhe = program.tempos.clone();
        rel.unidades_reaproveitadas = program.tempos.unidades_reaproveitadas;
        // Reanalisadas = arquivos que a carga precisou ler e analisar de
        // fato (o `Program` pode ter unidades que vieram do artefato do SDK,
        // que não são nem uma coisa nem outra).
        rel.unidades_reanalisadas = program.tempos.arquivos_lidos;
        rel.bibliotecas = program.libraries.len();
        if !diags.is_empty() {
            let msg = format!("{} erro(s) ao carregar: {}", diags.len(), diags[0]);
            // Uma carga falha não pode deixar o cache com meias unidades.
            self.unidades.recolher(program);
            return Err(msg);
        }

        let t = Instant::now();
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &program, &self.interner);
        let (mut outline, diags_outline) =
            dartforge_types::resolve_outline(&program, &self.interner, &mut table, &core);
        rel.outline = t.elapsed();

        // Os hashes vêm antes da inferência de corpos: são eles que dizem o
        // que precisa ser reanalisado e reemitido.
        let t = Instant::now();
        let afetadas = self.comparar_hashes(&program, &mut rel);
        rel.hashes = t.elapsed();

        let t = Instant::now();
        let (bodies, diags_corpos) = match &afetadas {
            None => dartforge_types::infer_program_bodies(&program, &self.interner, &mut table, &core, &mut outline),
            Some(libs) => dartforge_types::infer_bodies_das_bibliotecas(
                &program, &self.interner, &mut table, &core, &mut outline, libs,
            ),
        };
        rel.corpos = t.elapsed();
        rel.avisos_tipos = diags_outline.len() + diags_corpos.len();

        let t = Instant::now();
        let mut emitido = match afetadas {
            // Primeira compilação (ou mudança de grafo): emite tudo.
            None => {
                self.fragmentos.borrow_mut().limpar();
                let e = dartforge_emit_js::emitir_programa_com_cache(&program, &self.interner, &table, &core, &outline, &bodies, Some(&self.fragmentos))
                    .map_err(|ds| ds.first().map(|d| d.to_string()).unwrap_or_else(|| "emissão falhou".into()))?;
                rel.bibliotecas_reemitidas = usize::MAX;
                e
            }
            Some(libs) => {
                rel.bibliotecas_reemitidas = libs.len();
                // Os fragmentos das bibliotecas alteradas saem do cache; o
                // resto do módulo é reusado como texto.
                {
                    let ctx_tmp = dartforge_emit_js::ctx::Ctx::new(&program, &self.interner, &table, &core, &outline, &bodies);
                    self.fragmentos.borrow_mut().invalidar(&ctx_tmp, &libs);
                }
                let (e, contexto) = dartforge_emit_js::emitir_modulos_com_cache(
                    &program, &self.interner, &table, &core, &outline, &bodies, &libs, Some(&self.fragmentos),
                )
                .map_err(|ds| ds.first().map(|d| d.to_string()).unwrap_or_else(|| "emissão falhou".into()))?;
                rel.emissao_contexto = contexto;
                e
            }
        };
        rel.modulos_reemitidos = emitido.modulos.len();
        // O texto dos módulos não reemitidos continua valendo da compilação
        // anterior. Dos reemitidos, só vão para o disco os que mudaram de
        // texto — a comparação é com o que a sessão tem em memória, sem ler o
        // arquivo (o módulo de um pacote grande tem megabytes).
        let anteriores = std::mem::take(&mut self.modulos);
        let mut mudados = Vec::new();
        for (caminho, texto) in emitido.modulos.drain(..) {
            let igual = anteriores.get(&caminho).is_some_and(|t| *t == texto);
            if !igual {
                mudados.push((caminho.clone(), texto.clone()));
            }
            self.modulos.insert(caminho, texto);
        }
        for (caminho, texto) in anteriores {
            self.modulos.entry(caminho).or_insert(texto);
        }
        emitido.modulos = mudados;
        rel.emissao = t.elapsed();
        rel.modulos = self.modulos.len();

        // Escrita: só os módulos reemitidos podem ter texto diferente do que
        // está no disco, então só eles vão para `escrever` (que ainda compara
        // byte a byte antes de gravar). Comparar os 352 do `new_sali` a cada
        // edição custava uma leitura por arquivo.
        let t = Instant::now();
        // Na primeira compilação o diretório pode já ter saída de outra
        // execução: aí vale comparar. Nas seguintes, a sessão já sabe que o
        // texto mudou (comparou com o da compilação anterior, em memória).
        let comparar = rel.primeira;
        rel.modulos_escritos =
            dartforge_emit_js::escrever_opcoes(&emitido, &self.saida, &self.dart_sdk_js, comparar)?;
        rel.escrita = t.elapsed();

        // As unidades voltam para o cache; o resto da compilação morre aqui.
        drop(bodies);
        drop(outline);
        drop(table);
        self.unidades.recolher(program);
        self.primeira = false;
        Ok(rel)
    }

    /// Recalcula os dois hashes por biblioteca do usuário, preenche no
    /// relatório o que mudou e devolve **quais bibliotecas precisam ser
    /// reemitidas**: as que mudaram de conteúdo e, por mudança de API, as
    /// dependentes. `None` quer dizer "emitir tudo" — primeira compilação ou
    /// biblioteca nova/apagada, em que o conjunto anterior não serve.
    fn comparar_hashes(&mut self, program: &Program, rel: &mut Relatorio) -> Option<Vec<LibraryId>> {
        let primeira = self.hashes.is_empty();
        let mut ids: HashMap<String, LibraryId> = HashMap::with_capacity(program.libraries.len());
        let mut novos: HashMap<String, HashesBiblioteca> = HashMap::with_capacity(program.libraries.len());
        let mut api_alterada: Vec<String> = Vec::new();
        let mut conteudo_alterado: Vec<String> = Vec::new();
        let mut novas_ou_perdidas = false;
        for i in 0..program.libraries.len() {
            let lib = LibraryId(i as u32);
            let l = program.library(lib);
            if l.is_sdk || l.units.is_empty() {
                continue;
            }
            // Só as bibliotecas com algum arquivo sujo precisam de hash novo:
            // o resto tem as mesmas árvores da compilação anterior. (Rehash
            // de tudo custava 26 ms por edição no `new_sali/core`.)
            let suja = primeira
                || l.units.iter().any(|&u| {
                    program.unit(u).path.as_ref().is_some_and(|p| self.sujos.contains(p))
                });
            if !suja {
                if let Some(&antigo) = self.hashes.get(&l.uri) {
                    ids.insert(l.uri.clone(), lib);
                    novos.insert(l.uri.clone(), antigo);
                    continue;
                }
            }
            let h = hashes_da_biblioteca(program, lib);
            match self.hashes.get(&l.uri) {
                Some(antigo) => {
                    if antigo.conteudo != h.conteudo {
                        rel.corpo_alterado.push(l.uri.clone());
                        conteudo_alterado.push(l.uri.clone());
                    }
                    if antigo.api != h.api {
                        api_alterada.push(l.uri.clone());
                    }
                }
                // Biblioteca que não existia na compilação anterior.
                None if !primeira => novas_ou_perdidas = true,
                None => {}
            }
            ids.insert(l.uri.clone(), lib);
            novos.insert(l.uri.clone(), h);
        }
        if novos.len() != self.hashes.len() && !primeira {
            novas_ou_perdidas = true;
        }
        // Grafo reverso: quem importa/exporta cada biblioteca.
        let mut dependentes: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..program.libraries.len() {
            let l = &program.libraries[i];
            if l.is_sdk {
                continue;
            }
            for alvo in l.imports.iter().map(|x| x.library).chain(l.exports.iter().map(|x| x.library)) {
                let alvo_uri = &program.libraries[alvo.0 as usize].uri;
                if program.libraries[alvo.0 as usize].is_sdk {
                    continue;
                }
                let e = dependentes.entry(alvo_uri.clone()).or_default();
                if !e.contains(&l.uri) {
                    e.push(l.uri.clone());
                }
            }
        }
        // Fecho transitivo dos dependentes das bibliotecas com API alterada.
        let mut invalidados: Vec<String> = Vec::new();
        let mut fila: Vec<String> = api_alterada.clone();
        while let Some(u) = fila.pop() {
            for d in dependentes.get(&u).into_iter().flatten() {
                if !invalidados.contains(d) && !api_alterada.contains(d) {
                    invalidados.push(d.clone());
                    fila.push(d.clone());
                }
            }
        }
        rel.dependentes_invalidados = invalidados.len();
        self.hashes = novos;
        self.sujos.clear();
        self.dependentes = dependentes;

        // Quem precisa ser reemitido: quem mudou de conteúdo (o JS sai do
        // corpo) e quem depende de uma API alterada. Na primeira compilação,
        // ou quando o conjunto de bibliotecas mudou, emite tudo.
        let decisao = if primeira || novas_ou_perdidas || self.modulos.is_empty() {
            None
        } else {
            let mut libs: Vec<LibraryId> = Vec::new();
            for uri in conteudo_alterado.iter().chain(api_alterada.iter()).chain(invalidados.iter()) {
                if let Some(&id) = ids.get(uri) {
                    if !libs.contains(&id) {
                        libs.push(id);
                    }
                }
            }
            Some(libs)
        };
        rel.api_alterada = api_alterada;
        decisao
    }
}

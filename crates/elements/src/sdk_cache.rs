//! Cache do SDK analisado: as unidades (`fonte + AST`) de cada `dart:x`,
//! serializadas uma vez em `target/dartforge/sdk-<hash>.bin`.
//!
//! O que se poupa é ler e analisar sintaticamente ~180 arquivos do SDK a
//! cada compilação; o outline (`build_outline`) e a resolução de tipos
//! continuam a correr sobre o `Program` como antes, porque dependem do
//! programa inteiro e custam poucos milissegundos.
//!
//! Formato: cabeçalho `postcard` com a lista de símbolos internados (na
//! ordem em que o `Interner` os criou ao analisar o SDK inteiro) e, por
//! biblioteca, um blob `postcard` independente com as unidades — só as
//! bibliotecas que o programa importa são decodificadas. Cada biblioteca leva
//! também as `dart:` que importa ou exporta, para a carga decodificar de uma
//! vez, em paralelo, o fecho de tudo o que vai precisar.
//!
//! Invalidação: o nome do arquivo leva um hash de `libraries.json`, do
//! arquivo `version` do SDK, do nome da seção (`dartdevc`) e do próprio
//! `ast.rs` (qualquer mudança na árvore muda o hash). Um arquivo que não
//! decodifica é ignorado e reconstruído.
use crate::load::string_lit_value;
use crate::model::UnitRole;
use crate::sdk::SdkLayout;
use dartforge_frontend::ast::{Ast, CompilationUnit, DirectiveKind};
use dartforge_intern::Interner;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Versão do formato; mudar invalida todos os caches.
const FORMATO: u32 = 3; // 3: partes de patch com papel `Patch` (o main já tinha ido a 2)

/// Uma unidade do SDK já analisada.
#[derive(Serialize, Deserialize)]
pub struct UnitCache {
    pub uri: String,
    pub path: PathBuf,
    pub source: String,
    pub ast: Ast,
    pub unit: CompilationUnit,
    pub role: UnitRole,
}

#[derive(Serialize, Deserialize)]
struct Cabecalho<'a> {
    formato: u32,
    /// Textos do `Interner` na ordem dos `SymbolId`.
    #[serde(borrow)]
    simbolos: Vec<&'a str>,
    /// Nome da biblioteca (`core`) → blob `postcard` de `Vec<UnitCache>` e
    /// nomes das `dart:` que as suas unidades importam ou exportam.
    #[serde(borrow)]
    bibliotecas: Vec<(&'a str, &'a [u8], Vec<&'a str>)>,
}

/// Cache aberto: o arquivo inteiro em memória e, por cima dele, os intervalos
/// dos símbolos e do blob de cada biblioteca (nada é copiado ao abrir; cada
/// blob é decodificado quando a carga o pede).
pub struct SdkCache {
    bytes: Vec<u8>,
    simbolos: Vec<(usize, usize)>,
    blobs: HashMap<String, (usize, usize)>,
    /// `dart:` importadas ou exportadas por biblioteca (sem o prefixo).
    dependencias: HashMap<String, Vec<String>>,
    /// Blobs já decodificados, retirados pela carga.
    pub unidades_decodificadas: usize,
}

impl SdkCache {
    /// Decodifica o cabeçalho de `bytes` e guarda só intervalos.
    fn de_bytes(bytes: Vec<u8>) -> Option<SdkCache> {
        let base = bytes.as_ptr() as usize;
        let intervalo = |p: *const u8, n: usize| ((p as usize) - base, (p as usize) - base + n);
        let (simbolos, blobs, dependencias) = {
            let cab: Cabecalho<'_> = postcard::from_bytes(&bytes).ok()?;
            if cab.formato != FORMATO {
                return None;
            }
            let simbolos = cab.simbolos.iter().map(|s| intervalo(s.as_ptr(), s.len())).collect();
            let blobs = cab
                .bibliotecas
                .iter()
                .map(|(nome, b, _)| (nome.to_string(), intervalo(b.as_ptr(), b.len())))
                .collect();
            let dependencias = cab
                .bibliotecas
                .iter()
                .map(|(nome, _, deps)| (nome.to_string(), deps.iter().map(|d| d.to_string()).collect()))
                .collect();
            (simbolos, blobs, dependencias)
        };
        Some(SdkCache { bytes, simbolos, blobs, dependencias, unidades_decodificadas: 0 })
    }
    /// Hash FNV-1a de tudo o que invalida o cache.
    pub fn hash(sdk: &SdkLayout, target: &str) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |bytes: &[u8]| {
            for b in bytes {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            h ^= 0xff;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(&FORMATO.to_le_bytes());
        mix(target.as_bytes());
        // A raiz entra na chave: as unidades guardadas trazem o caminho
        // absoluto de cada arquivo, e dois SDKs com `libraries.json` igual em
        // diretórios diferentes (um SDK simulado de teste, por exemplo)
        // produziriam o mesmo hash com caminhos que não existem mais.
        mix(sdk.root.to_string_lossy().as_bytes());
        mix(&std::fs::read(sdk.root.join("libraries.json")).unwrap_or_default());
        mix(&std::fs::read(sdk.root.join("../version")).unwrap_or_default());
        // Sobreposição (`load_com_sobreposicao`): cada troca e o conteúdo do
        // substituto, em ordem estável.
        let mut trocas: Vec<(&PathBuf, &PathBuf)> = sdk.substituicoes.iter().collect();
        trocas.sort();
        for (original, novo) in trocas {
            mix(original.to_string_lossy().as_bytes());
            mix(&std::fs::read(novo).unwrap_or_default());
        }
        mix(include_str!("../../frontend/src/ast.rs").as_bytes());
        mix(include_str!("../../frontend/src/text.rs").as_bytes());
        format!("{h:016x}")
    }

    /// Diretório do cache: `DARTFORGE_CACHE_DIR`; senão o `target/` de onde o
    /// executável corre (`target/release/dartforge.exe`, testes em
    /// `target/debug/deps/`) mais `dartforge/`; senão `target/dartforge` no
    /// diretório atual.
    pub fn diretorio() -> PathBuf {
        if let Some(d) = std::env::var_os("DARTFORGE_CACHE_DIR") {
            return PathBuf::from(d);
        }
        if let Ok(exe) = std::env::current_exe() {
            let mut p = exe.as_path();
            while let Some(parent) = p.parent() {
                if parent.file_name().is_some_and(|n| n == "target") {
                    return parent.join("dartforge");
                }
                p = parent;
            }
        }
        PathBuf::from("target").join("dartforge")
    }

    /// Caminho do arquivo de cache para este SDK e seção.
    pub fn caminho(sdk: &SdkLayout, target: &str) -> PathBuf {
        Self::diretorio().join(format!("sdk-{}.bin", Self::hash(sdk, target)))
    }

    /// Lê e decodifica o cabeçalho de `caminho`; `None` se não existe ou não decodifica.
    pub fn abrir(caminho: &Path) -> Option<SdkCache> {
        let bytes = std::fs::read(caminho).ok()?;
        Self::de_bytes(bytes)
    }

    /// Analisa o SDK inteiro (todas as bibliotecas de `sdk`, com patches e
    /// partes) com um `Interner` novo e grava o cache em `caminho`.
    ///
    /// Devolve o cache pronto para uso (sem reler o arquivo).
    pub fn construir_e_gravar(sdk: &SdkLayout, caminho: &Path) -> Result<SdkCache, String> {
        let mut interner = Interner::new();
        let mut nomes: Vec<&String> = sdk.libraries.keys().collect();
        nomes.sort();
        let mut bibliotecas = Vec::with_capacity(nomes.len());
        for nome in nomes {
            let lib = &sdk.libraries[nome];
            let uri = format!("dart:{nome}");
            let mut unidades = Vec::new();
            let Some(principal) = analisar(sdk, &lib.path, &uri, UnitRole::Library, &mut interner)? else {
                // Biblioteca sem arquivo de origem: não entra no cache; a
                // carga normal emite o diagnóstico.
                continue;
            };
            unidades.push(principal);
            for patch in &lib.patches {
                let patch_uri = format!(
                    "{uri}#patch_{}",
                    patch.file_name().unwrap_or_default().to_string_lossy()
                );
                if let Some(u) = analisar(sdk, patch, &patch_uri, UnitRole::Patch, &mut interner)? {
                    unidades.push(u);
                }
            }
            // Partes na mesma ordem em que `load_lenient` as descobre: percorre
            // as unidades já carregadas (principal, patches, partes) e anexa as
            // partes de cada uma ao fim.
            let mut i = 0;
            while i < unidades.len() {
                let novas = partes_de(sdk, &unidades[i], &mut interner)?;
                unidades.extend(novas);
                i += 1;
            }
            let blob = postcard::to_allocvec(&unidades).map_err(|e| format!("serializar dart:{nome}: {e}"))?;
            let mut deps: Vec<String> = Vec::new();
            for u in &unidades {
                for d in &u.unit.directives {
                    let uri = match &d.kind {
                        DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } => uri,
                        _ => continue,
                    };
                    if let Some(dep) = string_lit_value(uri).and_then(|v| v.strip_prefix("dart:").map(str::to_string)) {
                        if !deps.contains(&dep) {
                            deps.push(dep);
                        }
                    }
                }
            }
            bibliotecas.push((nome.clone(), blob, deps));
        }
        let bytes = {
            let cab = Cabecalho {
                formato: FORMATO,
                simbolos: interner.textos().collect(),
                bibliotecas: bibliotecas
                    .iter()
                    .map(|(n, b, d)| (n.as_str(), b.as_slice(), d.iter().map(String::as_str).collect()))
                    .collect(),
            };
            postcard::to_allocvec(&cab).map_err(|e| format!("serializar cache do SDK: {e}"))?
        };
        if let Some(dir) = caminho.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        // Escrita atômica: outro processo pode estar lendo o arquivo.
        let tmp = caminho.with_extension(format!("bin.{}", std::process::id()));
        std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        if std::fs::rename(&tmp, caminho).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        Self::de_bytes(bytes).ok_or_else(|| "cache do SDK recém-gravado não decodifica".to_string())
    }

    /// Abre o cache de `sdk` (em [`SdkCache::caminho`]) ou constrói e grava.
    /// `Err` só quando nem o SDK pôde ser analisado.
    pub fn abrir_ou_construir(sdk: &SdkLayout, target: &str) -> Result<(SdkCache, bool), String> {
        let caminho = Self::caminho(sdk, target);
        if let Some(c) = Self::abrir(&caminho) {
            return Ok((c, true));
        }
        Self::construir_e_gravar(sdk, &caminho).map(|c| (c, false))
    }

    /// Interna os símbolos do cache em `interner`, na ordem original. Falha
    /// (devolve `false`) se `interner` já tinha outros símbolos e os ids não
    /// coincidem — nesse caso o cache não pode ser usado nesta carga.
    pub fn preparar_interner(&self, interner: &mut Interner) -> bool {
        for (i, &(a, b)) in self.simbolos.iter().enumerate() {
            // O cabeçalho decodificou estes bytes como `&str`: são UTF-8.
            let texto = std::str::from_utf8(&self.bytes[a..b]).unwrap_or("");
            if interner.intern(texto).as_u32() as usize != i {
                return false;
            }
        }
        true
    }

    /// Retira e decodifica as unidades de `dart:<nome>`; `None` se não está
    /// no cache ou já foi retirada.
    pub fn retirar(&mut self, nome: &str) -> Option<Vec<UnitCache>> {
        let (a, b) = self.blobs.remove(nome)?;
        let unidades: Vec<UnitCache> = postcard::from_bytes(&self.bytes[a..b]).ok()?;
        self.unidades_decodificadas += unidades.len();
        Some(unidades)
    }

    /// Retira e decodifica de uma vez as bibliotecas de `nomes` e o fecho das
    /// `dart:` que elas importam ou exportam (o que ainda está no cache), em
    /// até 8 threads: os blobs são independentes e a decodificação não toca no
    /// `Interner`. As que não decodificam ficam de fora — como em
    /// [`SdkCache::retirar`], a carga então as lê dos arquivos. Uma do fecho
    /// que a carga não peça (import condicional resolvido para outra) só custa
    /// a decodificação.
    ///
    /// Decodificar em série custava tanto quanto reanalisar o SDK no perfil
    /// `dev` (`serde` sem otimização: ~120 ms contra ~145 ms de leitura +
    /// parse nas 21 bibliotecas que `dart:core`/`convert`/`math`… puxam), e o
    /// teste de desempenho do cache virava cara ou coroa no runner.
    pub fn retirar_varios(&mut self, nomes: &[&str]) -> HashMap<String, Vec<UnitCache>> {
        let mut pedidos: Vec<(String, (usize, usize))> = Vec::new();
        let mut pendentes: Vec<String> = nomes.iter().map(|n| n.to_string()).collect();
        while let Some(nome) = pendentes.pop() {
            let Some(r) = self.blobs.remove(&nome) else { continue };
            if let Some(deps) = self.dependencias.get(&nome) {
                pendentes.extend(deps.iter().filter(|d| self.blobs.contains_key(*d)).cloned());
            }
            pedidos.push((nome, r));
        }
        // Maiores primeiro: o maior blob (`dart:core`) não fica para o fim.
        pedidos.sort_by_key(|(_, (a, b))| std::cmp::Reverse(b - a));
        let bytes = &self.bytes;
        let decodificar = |(a, b): (usize, usize)| postcard::from_bytes::<Vec<UnitCache>>(&bytes[a..b]).ok();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8).min(pedidos.len());
        let mut saida = HashMap::new();
        if threads < 2 {
            for (nome, r) in pedidos {
                if let Some(u) = decodificar(r) {
                    saida.insert(nome, u);
                }
            }
        } else {
            let proximo = std::sync::atomic::AtomicUsize::new(0);
            let lotes: Vec<Vec<(String, Vec<UnitCache>)>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..threads)
                    .map(|_| {
                        s.spawn(|| {
                            let mut meus = Vec::new();
                            loop {
                                let i = proximo.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some((nome, r)) = pedidos.get(i) else { break };
                                if let Some(u) = decodificar(*r) {
                                    meus.push((nome.clone(), u));
                                }
                            }
                            meus
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
            });
            saida.extend(lotes.into_iter().flatten());
        }
        self.unidades_decodificadas += saida.values().map(Vec::len).sum::<usize>();
        saida
    }

    pub fn tem(&self, nome: &str) -> bool {
        self.blobs.contains_key(nome)
    }

    pub fn bibliotecas(&self) -> usize {
        self.blobs.len()
    }

    pub fn simbolos(&self) -> usize {
        self.simbolos.len()
    }
}

/// Lê e analisa um arquivo; `Ok(None)` se não existe, `Err` se tem erro de sintaxe
/// (o SDK oficial não tem: um erro aqui é lacuna do parser e não entra no cache).
fn analisar(sdk: &SdkLayout, path: &Path, uri: &str, role: UnitRole, interner: &mut Interner) -> Result<Option<UnitCache>, String> {
    // A sobreposição troca o conteúdo, não o caminho (`SdkLayout::substituto`).
    let Ok(source) = std::fs::read_to_string(sdk.substituto(path).unwrap_or(path)) else {
        return Ok(None);
    };
    let parsed = dartforge_frontend::parser::parse(&source, interner);
    if let Some(d) = parsed.diagnostics.first() {
        return Err(format!("{}:{}: {}", path.display(), d.span.start, d.message));
    }
    // Mesmo `path` que `load_lenient` guarda: o do `libraries.json` para
    // biblioteca e patch, o canônico para partes (quem chama já canonizou).
    Ok(Some(UnitCache { uri: uri.to_string(), path: path.to_path_buf(), source, ast: parsed.ast, unit: parsed.unit, role }))
}

/// As partes declaradas por `part 'x.dart';` em `unidade`, analisadas.
fn partes_de(sdk: &SdkLayout, unidade: &UnitCache, interner: &mut Interner) -> Result<Vec<UnitCache>, String> {
    let mut saida = Vec::new();
    for d in &unidade.unit.directives {
        let DirectiveKind::Part { uri } = &d.kind else { continue };
        let Some(rel) = string_lit_value(uri) else { continue };
        let path = unidade.path.parent().unwrap_or(Path::new(".")).join(&rel);
        // Mesma normalização lexical do carregador: o caminho é a chave do
        // cache da sessão residente e tem de bater com o que `load.rs` monta.
        let canonico = crate::load::normalizar(&path);
        let part_uri = url::Url::from_file_path(&canonico)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| canonico.to_string_lossy().to_string());
        // Parte de patch é patch (ver `load.rs`).
        let papel = if unidade.role == UnitRole::Patch { UnitRole::Patch } else { UnitRole::Part };
        if let Some(p) = analisar(sdk, &canonico, &part_uri, papel, interner)? {
            saida.push(p);
        }
    }
    Ok(saida)
}

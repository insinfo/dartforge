//! Cache em disco: permite que um processo novo pule o front-end inteiro.
//!
//! A sessão em memória já reaproveita a saída enquanto o compilador continua
//! aberto. Este módulo cobre o outro cenário da tabela de desempenho: rodar o
//! compilador de novo, do zero, sem que nada tenha mudado.
//!
//! # Por que o arquivo guarda as fontes inteiras
//!
//! A verificação do projeto é por **conteúdo exato**, nunca por mtime, tamanho
//! ou hash — existe um teste que edita um arquivo restaurando mtime e tamanho e
//! exige que o cache seja invalidado. Manter essa garantia num processo novo
//! exige ter com o que comparar, e a única coisa com que se pode comparar
//! exatamente é o próprio texto. O custo em disco é uma cópia das fontes
//! alcançáveis pela entrada, e está sujeito a um orçamento explícito.
//!
//! Uma impressão digital criptográfica reduziria o arquivo, mas trocaria uma
//! garantia demonstrável por uma probabilística. Se esse custo em disco se
//! tornar um problema medido, a troca deve ser discutida como mudança de
//! contrato, não introduzida em silêncio.
//!
//! # O que a chave cobre
//!
//! Versão do formato, versão do compilador, opções de compilação, alvo, caminho
//! canônico da entrada, cada unidade alcançável com seu caminho e seu texto, e o
//! conteúdo do `package_config.json` quando existe. Qualquer divergência é uma
//! falta de cache, nunca um acerto duvidoso.
use crate::{CompileOptions, Optimization};
use dartforge_packages::SourceGraph;
use std::path::{Path, PathBuf};

/// Muda sempre que o formato do arquivo deixa de ser compatível.
const FORMAT_VERSION: u32 = 1;
/// Cabeçalho que identifica o arquivo e evita ler algo alheio por engano.
const MAGIC: &str = "DARTFORGE-CACHE";
/// Orçamento padrão de um registro; acima disso a gravação é descartada.
const DEFAULT_MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;

/// Cache persistente de saídas, indexado por entrada e opções.
///
/// Um registro é gravado por caminho de entrada. Regravar substitui, porque
/// manter histórico exigiria uma política de expurgo que ainda não existe.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
    max_entry_bytes: usize,
}

impl DiskCache {
    /// Usa `root` como diretório dos registros, criado sob demanda.
    ///
    /// ```
    /// use dartforge_compiler::DiskCache;
    /// let cache = DiskCache::new(std::path::Path::new(".dart_tool/dartforge"));
    /// assert_eq!(cache.max_entry_bytes(), 64 * 1024 * 1024);
    /// ```
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
        }
    }

    /// Define o orçamento por registro; zero desativa a gravação.
    pub fn with_max_entry_bytes(mut self, bytes: usize) -> Self {
        self.max_entry_bytes = bytes;
        self
    }

    /// Orçamento por registro atualmente configurado.
    pub fn max_entry_bytes(&self) -> usize {
        self.max_entry_bytes
    }

    /// Caminho do registro de uma entrada canônica sob uma combinação de opções.
    ///
    /// As opções entram no nome porque cada combinação produz um JavaScript
    /// diferente: sem isso, compilar com `--optimize` apagaria o registro da
    /// compilação sem otimização, e alternar entre as duas nunca acertaria.
    ///
    /// O nome vem de uma dispersão apenas para ser válido em qualquer sistema de
    /// arquivos. Uma colisão não produz acerto indevido: entrada, opções e alvo
    /// são gravados dentro do arquivo e conferidos na leitura.
    fn entry_path(&self, entry: &Path, options: CompileOptions, target: &str) -> PathBuf {
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        let mut absorve = |bytes: &[u8]| {
            for byte in bytes {
                key ^= u64::from(*byte);
                key = key.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        absorve(entry.as_os_str().as_encoded_bytes());
        absorve(options_key(options).as_bytes());
        absorve(target.as_bytes());
        self.root.join(format!("{key:016x}.dfcache"))
    }

    /// Lê um registro e devolve o JavaScript quando tudo confere.
    ///
    /// Relê do disco cada fonte registrada e compara byte a byte. Qualquer
    /// divergência, arquivo ausente, registro corrompido ou chave diferente
    /// devolve `None`, e o chamador compila normalmente.
    pub fn load(
        &self,
        entry: &Path,
        options: CompileOptions,
        target: &str,
    ) -> Option<CachedCompilation> {
        // A chave vem do caminho canônico, como na gravação: no Windows o
        // caminho cru e o canônico diferem no prefixo, e usar um de cada lado
        // produziria uma falta de cache silenciosa e permanente.
        let canonical = std::fs::canonicalize(entry).ok()?;
        let bytes = std::fs::read(self.entry_path(&canonical, options, target)).ok()?;
        let mut reader = Reader::new(&bytes);
        if reader.line()? != MAGIC
            || reader.line()? != FORMAT_VERSION.to_string()
            || reader.line()? != compiler_version()
            || reader.line()? != options_key(options)
            || reader.line()? != target
        {
            return None;
        }
        let stored_entry = reader.block()?;
        if stored_entry != canonical.as_os_str().as_encoded_bytes() {
            return None;
        }
        let config = reader.block()?;
        if !config.is_empty() {
            let mut parts = config.splitn(2, |byte| *byte == 0);
            let path = parts.next()?;
            let text = parts.next()?;
            let path = PathBuf::from(String::from_utf8(path.to_vec()).ok()?);
            if std::fs::read(&path).ok()? != text {
                return None;
            }
        }
        let count: usize = reader.line()?.parse().ok()?;
        let mut units = 0usize;
        let mut source_bytes = 0usize;
        for _ in 0..count {
            let path = reader.block()?;
            let source = reader.block()?;
            let path = PathBuf::from(String::from_utf8(path.to_vec()).ok()?);
            // Unidade sintética de biblioteca SDK: não tem arquivo em disco.
            if path.to_str().is_some_and(|text| text.starts_with("dart:")) {
                units += 1;
                continue;
            }
            if std::fs::read(&path).ok()? != source {
                return None;
            }
            units += 1;
            source_bytes = source_bytes.saturating_add(source.len());
        }
        let javascript = String::from_utf8(reader.block()?.to_vec()).ok()?;
        Some(CachedCompilation {
            javascript,
            units,
            source_bytes,
        })
    }

    /// Grava o registro correspondente a uma compilação bem-sucedida.
    ///
    /// Falhas de escrita são silenciosas de propósito: um cache que não pôde ser
    /// gravado é uma oportunidade perdida, não um erro de compilação. A gravação
    /// passa por arquivo temporário e renomeação, para que um processo
    /// interrompido não deixe um registro pela metade.
    pub fn store(
        &self,
        entry: &Path,
        options: CompileOptions,
        target: &str,
        graph: &SourceGraph,
        javascript: &str,
    ) {
        if self.max_entry_bytes == 0 {
            return;
        }
        let Ok(canonical) = std::fs::canonicalize(entry) else {
            return;
        };
        let mut bytes = Vec::new();
        push_line(&mut bytes, MAGIC);
        push_line(&mut bytes, &FORMAT_VERSION.to_string());
        push_line(&mut bytes, compiler_version());
        push_line(&mut bytes, &options_key(options));
        push_line(&mut bytes, target);
        push_block(&mut bytes, canonical.as_os_str().as_encoded_bytes());
        match &graph.config_origin {
            Some((path, text)) => {
                let mut joined = path.as_os_str().as_encoded_bytes().to_vec();
                joined.push(0);
                joined.extend_from_slice(text.as_bytes());
                push_block(&mut bytes, &joined);
            }
            None => push_block(&mut bytes, &[]),
        }
        push_line(&mut bytes, &graph.units.len().to_string());
        for unit in &graph.units {
            push_block(&mut bytes, unit.path.as_os_str().as_encoded_bytes());
            push_block(&mut bytes, unit.source.as_bytes());
        }
        push_block(&mut bytes, javascript.as_bytes());
        if bytes.len() > self.max_entry_bytes {
            return;
        }
        if std::fs::create_dir_all(&self.root).is_err() {
            return;
        }
        let target_path = self.entry_path(&canonical, options, target);
        let temporary = target_path.with_extension("dfcache.tmp");
        if std::fs::write(&temporary, &bytes).is_ok() {
            let _ = std::fs::rename(&temporary, &target_path);
        }
    }

    /// Remove todos os registros gravados, ignorando o que não existir.
    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Saída reaproveitada do disco, com o trabalho que ela dispensou.
#[derive(Debug, Clone)]
pub struct CachedCompilation {
    /// Módulo ES completo, idêntico ao que a compilação produziria.
    pub javascript: String,
    /// Unidades que o registro cobre, incluindo as sintéticas de SDK.
    pub units: usize,
    /// Bytes de fonte relidos e conferidos nesta validação.
    pub source_bytes: usize,
}

/// Identidade do compilador; muda a cada versão publicada.
fn compiler_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Texto curto e estável que distingue combinações de opções.
fn options_key(options: CompileOptions) -> String {
    format!(
        "o={} m={} t={}",
        match options.optimization {
            Optimization::None => "none",
            Optimization::Constants => "const",
        },
        u8::from(options.merge_identical_functions),
        u8::from(options.tree_shaking)
    )
}

/// Acrescenta uma linha de cabeçalho terminada em `\n`.
fn push_line(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(text.as_bytes());
    bytes.push(b'\n');
}

/// Acrescenta um bloco precedido do seu comprimento em decimal.
///
/// O comprimento explícito permite guardar texto com qualquer conteúdo, sem
/// escapes e sem depender de o conteúdo não ter quebras de linha.
fn push_block(bytes: &mut Vec<u8>, payload: &[u8]) {
    push_line(bytes, &payload.len().to_string());
    bytes.extend_from_slice(payload);
    bytes.push(b'\n');
}

/// Leitor posicional do formato; devolve `None` em qualquer inconsistência.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    /// Começa no início do conteúdo lido do arquivo.
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    /// Lê uma linha de cabeçalho, sem o terminador.
    fn line(&mut self) -> Option<&'a str> {
        let rest = self.bytes.get(self.position..)?;
        let end = rest.iter().position(|byte| *byte == b'\n')?;
        self.position += end + 1;
        std::str::from_utf8(&rest[..end]).ok()
    }
    /// Lê um bloco precedido do comprimento, conferindo o terminador.
    fn block(&mut self) -> Option<&'a [u8]> {
        let length: usize = self.line()?.parse().ok()?;
        let start = self.position;
        let end = start.checked_add(length)?;
        if self.bytes.get(end) != Some(&b'\n') {
            return None;
        }
        self.position = end + 1;
        self.bytes.get(start..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um bloco com quebras de linha e bytes nulos atravessa o formato intacto.
    #[test]
    fn blocks_survive_arbitrary_content() {
        let payload = b"linha\noutra\0fim\n";
        let mut bytes = Vec::new();
        push_line(&mut bytes, MAGIC);
        push_block(&mut bytes, payload);
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.line(), Some(MAGIC));
        assert_eq!(reader.block(), Some(&payload[..]));
        assert_eq!(reader.block(), None);
    }

    /// Um arquivo truncado é recusado em vez de produzir um bloco parcial.
    #[test]
    fn a_truncated_record_is_rejected() {
        let mut bytes = Vec::new();
        push_block(&mut bytes, b"conteudo");
        bytes.truncate(bytes.len() - 3);
        assert_eq!(Reader::new(&bytes).block(), None);
    }

    /// Opções diferentes produzem chaves diferentes.
    #[test]
    fn every_option_changes_the_key() {
        let base = CompileOptions::default();
        let mut seen = std::collections::HashSet::new();
        for optimization in [Optimization::None, Optimization::Constants] {
            for merge in [false, true] {
                for shake in [false, true] {
                    assert!(seen.insert(options_key(CompileOptions {
                        optimization,
                        merge_identical_functions: merge,
                        tree_shaking: shake,
                        ..base
                    })));
                }
            }
        }
        assert_eq!(seen.len(), 8);
    }
}

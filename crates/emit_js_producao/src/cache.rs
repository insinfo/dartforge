//! Cache em disco do índice do `dart_sdk.js` ([`IndiceSdk`]).
//!
//! A classificação do runtime — varredura das ~14 mil declarações,
//! sub-divisão por membro, referências e seletores das ~45 mil unidades — só
//! depende do texto do `dart_sdk.js` e de `por_membro`, não do programa
//! (`docs/JS-PRODUCAO.md` §1.6, "índice do `dart_sdk.js` em cache"). Cada
//! compilação de produção a refazia; agora ela é guardada em
//! `<cache>/jsprod-sdk-<chave>.bin` e lida de volta.
//!
//! **Chave.** Um hash FNV-1a de 64 bits sobre: a versão do formato, o texto
//! inteiro do `dart_sdk.js`, `por_membro` e o **código** da classificação
//! (`sdk.rs`, `varredura.rs`, `alcance.rs`, `bundle.rs`, embutidos na
//! compilação). Outro runtime (outro SDK, outro build do DDC), outra
//! configuração ou outra versão da classificação dão outra chave, e o índice
//! velho nunca é usado para eles.
//!
//! **Conferência ao ler.** O cabeçalho repete a chave, o tamanho do texto e
//! um segundo hash do texto (outra semente); um arquivo que não decodifica,
//! de outra versão ou de outro texto (colisão de nome) é ignorado e
//! reconstruído. Escrita atômica (arquivo temporário e `rename`), porque
//! compilações paralelas (o `crates/diferencial` roda quatro) disputam o
//! mesmo arquivo.
//!
//! **Descarte.** No máximo [`MAXIMO`] índices no diretório: ao gravar um
//! novo, os mais antigos saem (`docs/PESQUISA-OTIMIZACAO.md` §2.1: nenhum
//! cache cresce sem política de descarte).
//!
//! `DARTFORGE_JSPROD_CACHE=0` desliga (constrói sempre, não grava);
//! `DARTFORGE_CACHE_DIR` muda o diretório, como no cache do SDK Dart.

use crate::sdk::{IndiceSdk, indexar};
use std::path::{Path, PathBuf};

/// Versão do formato; mudar invalida todos os índices.
const FORMATO: u32 = 1;

/// Índices guardados no diretório, no máximo.
pub const MAXIMO: usize = 4;

/// De onde veio o índice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origem {
    /// Lido do disco.
    Cache,
    /// Construído agora (e gravado, se o cache está ligado).
    Construido,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Arquivo {
    formato: u32,
    chave: u64,
    tamanho_do_texto: u64,
    /// Hash do texto com outra semente: confere que o arquivo é deste
    /// texto mesmo com colisão na chave.
    conferencia: u64,
    indice: IndiceSdk,
}

fn fnv(semente: u64, partes: &[&[u8]]) -> u64 {
    let mut h = semente;
    for p in partes {
        for b in *p {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A chave do índice de `texto` com `por_membro`.
pub fn chave(texto: &str, por_membro: bool) -> u64 {
    fnv(
        0xcbf2_9ce4_8422_2325,
        &[
            &FORMATO.to_le_bytes(),
            texto.as_bytes(),
            &[u8::from(por_membro)],
            include_str!("sdk.rs").as_bytes(),
            include_str!("varredura.rs").as_bytes(),
            include_str!("alcance.rs").as_bytes(),
            include_str!("bundle.rs").as_bytes(),
        ],
    )
}

fn conferencia(texto: &str) -> u64 {
    fnv(0x84222325cbf29ce4, &[texto.as_bytes()])
}

/// O diretório do cache (o mesmo do cache do SDK Dart).
pub fn diretorio() -> PathBuf {
    dartforge_elements::sdk_cache::SdkCache::diretorio()
}

/// O arquivo do índice de `texto` com `por_membro` em `dir`.
pub fn caminho(dir: &Path, texto: &str, por_membro: bool) -> PathBuf {
    dir.join(format!("jsprod-sdk-{:016x}.bin", chave(texto, por_membro)))
}

/// Lê o índice de `arquivo`, se é deste `texto` com `por_membro`.
pub fn ler(arquivo: &Path, texto: &str, por_membro: bool) -> Option<IndiceSdk> {
    let bytes = std::fs::read(arquivo).ok()?;
    let a: Arquivo = postcard::from_bytes(&bytes).ok()?;
    let valido = a.formato == FORMATO
        && a.chave == chave(texto, por_membro)
        && a.tamanho_do_texto == texto.len() as u64
        && a.conferencia == conferencia(texto);
    valido.then_some(a.indice)
}

/// Grava o índice de `texto` em `dir` e descarta os mais antigos.
///
/// # Erros
///
/// Falha de escrita ou de serialização; quem chama segue sem o cache.
pub fn gravar(dir: &Path, texto: &str, por_membro: bool, indice: &IndiceSdk) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let a = Arquivo {
        formato: FORMATO,
        chave: chave(texto, por_membro),
        tamanho_do_texto: texto.len() as u64,
        conferencia: conferencia(texto),
        indice: indice.clone(),
    };
    let bytes = postcard::to_allocvec(&a).map_err(|e| e.to_string())?;
    let destino = caminho(dir, texto, por_membro);
    // Temporário com o id do processo: duas compilações simultâneas não
    // escrevem no mesmo arquivo, e o `rename` é atômico.
    let tmp = dir.join(format!(".jsprod-sdk-{}-{:016x}.tmp", std::process::id(), a.chave));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", destino.display())
    })?;
    descartar_antigos(dir, &destino);
    Ok(destino)
}

/// Mantém no máximo [`MAXIMO`] índices em `dir`, os mais recentes; `manter`
/// (o que acabou de ser gravado) nunca sai.
fn descartar_antigos(dir: &Path, manter: &Path) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    let mut indices: Vec<(std::time::SystemTime, PathBuf)> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("jsprod-sdk-") && n.ends_with(".bin")))
        .filter_map(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (t, p)))
        .collect();
    // Mais recente primeiro; empate pelo nome, para a ordem não depender do
    // sistema de arquivos.
    indices.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut mantidos = usize::from(indices.iter().any(|(_, p)| p == manter));
    for (_, p) in indices {
        if p == manter {
            continue;
        }
        if mantidos < MAXIMO {
            mantidos += 1;
        } else {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// O índice de `texto`: do cache em `dir` quando há um válido, senão
/// construído (e gravado). `dir` `None` desliga o cache.
pub fn obter_em(dir: Option<&Path>, texto: &str, por_membro: bool) -> (IndiceSdk, Origem) {
    if let Some(d) = dir
        && let Some(i) = ler(&caminho(d, texto, por_membro), texto, por_membro)
    {
        return (i, Origem::Cache);
    }
    let indice = indexar(texto, por_membro);
    if let Some(d) = dir
        && let Err(e) = gravar(d, texto, por_membro, &indice)
    {
        eprintln!("aviso: cache do índice do runtime: {e}");
    }
    (indice, Origem::Construido)
}

/// Como [`obter_em`], no diretório padrão, salvo `DARTFORGE_JSPROD_CACHE=0`.
pub fn obter(texto: &str, por_membro: bool) -> (IndiceSdk, Origem) {
    let ligado = !std::env::var("DARTFORGE_JSPROD_CACHE").is_ok_and(|v| v == "0");
    let dir = ligado.then(diretorio);
    obter_em(dir.as_deref(), texto, por_membro)
}

#[cfg(test)]
mod testes {
    use super::*;

    const SDK: &str = concat!(
        "var core = Object.create(dart.library);\n",
        "export { dart, core };\n",
        "core.print = function print(o) { dart.dsend(o, \"toString\", []); };\n",
        "core.Morta = class Morta { f() { return 1; } };\n",
    );

    fn dir_temporario(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dartforge-jsprod-cache-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A segunda compilação lê o índice do disco, e ele é o mesmo que a
    /// construção daria.
    #[test]
    fn segunda_leitura_vem_do_cache_e_e_igual() {
        let d = dir_temporario("reuso");
        let (a, o1) = obter_em(Some(&d), SDK, true);
        let (b, o2) = obter_em(Some(&d), SDK, true);
        assert_eq!(o1, Origem::Construido);
        assert_eq!(o2, Origem::Cache);
        assert_eq!(a, b);
        assert_eq!(a, indexar(SDK, true));
        assert_eq!(a.seletores_dinamicos(), vec!["toString".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Outro runtime ou outra configuração não aceitam o índice guardado:
    /// nem pelo nome do arquivo, nem forçando a leitura do arquivo errado.
    #[test]
    fn runtime_ou_configuracao_incompativel_reconstroi() {
        let d = dir_temporario("incompativel");
        let (_, o) = obter_em(Some(&d), SDK, true);
        assert_eq!(o, Origem::Construido);
        let outro = SDK.replace("Morta", "Viva");
        let (i, o) = obter_em(Some(&d), &outro, true);
        assert_eq!(o, Origem::Construido, "outro texto do runtime");
        assert_eq!(i, indexar(&outro, true));
        let (_, o) = obter_em(Some(&d), SDK, false);
        assert_eq!(o, Origem::Construido, "outra granularidade");
        // O arquivo de um texto, lido como se fosse de outro, é recusado.
        assert!(ler(&caminho(&d, SDK, true), &outro, true).is_none());
        assert!(ler(&caminho(&d, SDK, true), SDK, false).is_none());
        assert!(ler(&caminho(&d, SDK, true), SDK, true).is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Arquivo corrompido é ignorado e reconstruído.
    #[test]
    fn arquivo_corrompido_reconstroi() {
        let d = dir_temporario("corrompido");
        let (_, _) = obter_em(Some(&d), SDK, true);
        std::fs::write(caminho(&d, SDK, true), b"lixo").unwrap();
        let (i, o) = obter_em(Some(&d), SDK, true);
        assert_eq!(o, Origem::Construido);
        assert_eq!(i, indexar(SDK, true));
        let (_, o) = obter_em(Some(&d), SDK, true);
        assert_eq!(o, Origem::Cache, "regravado");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O diretório não passa de [`MAXIMO`] índices.
    #[test]
    fn descarta_os_mais_antigos() {
        let d = dir_temporario("descarte");
        for k in 0..MAXIMO + 3 {
            let t = format!("{SDK}core.X{k} = class X{k} {{}};\n");
            obter_em(Some(&d), &t, true);
        }
        let n = std::fs::read_dir(&d).unwrap().filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with("jsprod-sdk-")).count();
        assert_eq!(n, MAXIMO);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Podar com o índice lido do disco dá o mesmo texto que podar do zero.
    #[test]
    fn poda_pelo_cache_e_identica() {
        let d = dir_temporario("poda");
        obter_em(Some(&d), SDK, true);
        let (i, o) = obter_em(Some(&d), SDK, true);
        assert_eq!(o, Origem::Cache);
        let raizes = vec!["core.print".to_string()];
        assert_eq!(crate::sdk::podar_com_indice(SDK, &i, &raizes), crate::sdk::podar(SDK, &raizes, true));
        let _ = std::fs::remove_dir_all(&d);
    }
}

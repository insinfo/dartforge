//! Persistência do estado do motor entre processos (`docs/BUILD-MOTOR.md`
//! §4.1): o equivalente do `asset_graph.json` do `build_runner`. Um processo
//! novo reaproveita as ações cujas consultas continuam com o mesmo digest, em
//! vez de executá-las de novo. **Só quando o usuário pede** (`--estado`,
//! [`VARIAVEL`]): a regra governante 6 do `PLANO.md` proíbe contabilidade em
//! disco que ninguém pediu.
//!
//! O arquivo guarda, por ação, a chave da ação (fase, opções, entrada e
//! saídas previstas), a origem, as consultas com os digests e o digest de
//! cada saída; o conteúdo das saídas fica em `blobs/`, endereçado pelo
//! blake3. Só entra o que dá para revalidar sem o processo anterior:
//!
//! * consultas de arquivo (`Arquivo`, `Existe`, `Glob`, `GlobAtivos`) — uma
//!   consulta semântica depende do banco da sessão, que um processo novo não
//!   tem igual (numa passada única ele nem responde);
//! * ações que não são de um gerador por pacote (a revalidação dele é a
//!   rodada do pacote, que fica em memória);
//! * nada pendente nem medido.
//!
//! As entradas que a consulta não vê ficam na chave: a versão do motor, a
//! identidade do executável (o código dos geradores nativos), o plano, as
//! versões do lock e o modo (`--release`). Para uma ação do executor Dart, o
//! código do builder — os arquivos do depfile do bootstrap fora dos pacotes
//! `hosted`, pelo conteúdo.
use crate::consulta::{Consulta, Digest};
use crate::grafo::AssetId;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Variável de ambiente que liga o estado em disco no `dev`, `serve` e
/// `compile-js` (e no `dartforge build`, além do `--estado`).
pub const VARIAVEL: &str = "DARTFORGE_BUILD_ESTADO";

/// O usuário pediu o estado em disco pelo ambiente (`DARTFORGE_BUILD_ESTADO`
/// com valor não vazio e diferente de `0`)? Sem pedido nada vai ao disco:
/// regra governante 6 do `PLANO.md` ("o disco só recebe o que o usuário
/// pedir").
pub fn pedido_no_ambiente() -> bool {
    std::env::var_os(VARIAVEL).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Formato do arquivo; muda quando o conteúdo muda de forma.
const FORMATO: u64 = 1;

/// Diretório do estado de um projeto.
pub fn diretorio(dir_raiz: &Path) -> PathBuf {
    dir_raiz
        .join(".dart_tool")
        .join("dartforge")
        .join("build")
        .join("estado")
}

/// Quem produziu a ação salva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrigemSalva {
    Nativo(String),
    Dart,
    Apoio,
    Omitida,
}

/// Uma ação salva.
#[derive(Debug, Clone)]
pub struct AcaoSalva {
    pub chave: String,
    pub origem: OrigemSalva,
    pub motivo: Option<String>,
    pub consultas: Vec<(Consulta, Option<Digest>)>,
    pub saidas: Vec<(AssetId, Option<Digest>)>,
    pub do_apoio: Vec<AssetId>,
    pub apagados: Vec<AssetId>,
}

/// O estado salvo de um projeto.
#[derive(Debug, Clone, Default)]
pub struct Estado {
    /// Chave global (motor, executável, plano, lock, modo).
    pub chave: String,
    /// O código dos builders Dart: arquivo e digest do conteúdo. `None`: não
    /// há ação Dart salva.
    pub codigo_dart: Option<Vec<(PathBuf, Digest)>>,
    pub acoes: Vec<AcaoSalva>,
}

/// Identidade do executável que roda o motor: o código dos geradores
/// nativos não está em nenhuma consulta. Caminho, tamanho e data do arquivo.
pub fn identidade_do_executavel() -> String {
    let Ok(exe) = std::env::current_exe() else {
        return "?".into();
    };
    let meta = std::fs::metadata(&exe).ok();
    let tamanho = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let data = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}|{tamanho}|{data}", exe.display())
}

fn hex(d: &Digest) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex(s: &str) -> Option<Digest> {
    if s.len() != 64 {
        return None;
    }
    let mut d = [0u8; 32];
    for (i, par) in d.iter_mut().enumerate() {
        *par = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(d)
}

fn texto(p: &Path) -> Option<String> {
    p.to_str().map(str::to_string)
}

/// Uma consulta persistível em JSON; `None` para as semânticas.
fn consulta_json(c: &Consulta) -> Option<Value> {
    Some(match c {
        Consulta::Arquivo(p) => json!({"t": "arquivo", "p": texto(p)?}),
        Consulta::Existe(p) => json!({"t": "existe", "p": texto(p)?}),
        Consulta::Glob { dir, padrao } => json!({"t": "glob", "p": texto(dir)?, "padrao": padrao}),
        Consulta::GlobAtivos {
            dir,
            padrao,
            candidatos,
        } => {
            json!({"t": "glob_ativos", "p": texto(dir)?, "padrao": padrao, "candidatos": candidatos})
        }
        _ => return None,
    })
}

fn consulta_de_json(v: &Value) -> Option<Consulta> {
    let p = || v.get("p").and_then(Value::as_str).map(PathBuf::from);
    let padrao = || v.get("padrao").and_then(Value::as_str).map(str::to_string);
    Some(match v.get("t")?.as_str()? {
        "arquivo" => Consulta::Arquivo(p()?),
        "existe" => Consulta::Existe(p()?),
        "glob" => Consulta::Glob {
            dir: p()?,
            padrao: padrao()?,
        },
        "glob_ativos" => {
            let mut candidatos = Vec::new();
            for c in v.get("candidatos")?.as_array()? {
                candidatos.push((c.get(0)?.as_str()?.to_string(), c.get(1)?.as_bool()?));
            }
            Consulta::GlobAtivos {
                dir: p()?,
                padrao: padrao()?,
                candidatos,
            }
        }
        _ => return None,
    })
}

/// A consulta pode ser salva (é de arquivo)?
pub fn persistivel(c: &Consulta) -> bool {
    consulta_json(c).is_some()
}

fn ids(v: &[AssetId]) -> Value {
    Value::Array(v.iter().map(|id| Value::String(id.texto())).collect())
}

fn ids_de(v: Option<&Value>) -> Option<Vec<AssetId>> {
    v?.as_array()?
        .iter()
        .map(|x| AssetId::de_texto(x.as_str()?))
        .collect()
}

fn acao_json(a: &AcaoSalva) -> Option<Value> {
    let origem = match &a.origem {
        OrigemSalva::Nativo(n) => json!({"nativo": n}),
        OrigemSalva::Dart => json!("dart"),
        OrigemSalva::Apoio => json!("apoio"),
        OrigemSalva::Omitida => json!("omitida"),
    };
    let mut consultas = Vec::with_capacity(a.consultas.len());
    for (c, d) in &a.consultas {
        let mut v = consulta_json(c)?;
        v["d"] = d
            .as_ref()
            .map(|d| Value::String(hex(d)))
            .unwrap_or(Value::Null);
        consultas.push(v);
    }
    let saidas: Vec<Value> = a
        .saidas
        .iter()
        .map(|(id, d)| json!([id.texto(), d.as_ref().map(hex)]))
        .collect();
    Some(json!({
        "chave": a.chave,
        "origem": origem,
        "motivo": a.motivo,
        "consultas": consultas,
        "saidas": saidas,
        "do_apoio": ids(&a.do_apoio),
        "apagados": ids(&a.apagados),
    }))
}

fn acao_de_json(v: &Value) -> Option<AcaoSalva> {
    let origem = match v.get("origem")? {
        Value::String(s) if s == "dart" => OrigemSalva::Dart,
        Value::String(s) if s == "apoio" => OrigemSalva::Apoio,
        Value::String(s) if s == "omitida" => OrigemSalva::Omitida,
        o => OrigemSalva::Nativo(o.get("nativo")?.as_str()?.to_string()),
    };
    let mut consultas = Vec::new();
    for c in v.get("consultas")?.as_array()? {
        let d = match c.get("d")? {
            Value::Null => None,
            d => Some(de_hex(d.as_str()?)?),
        };
        consultas.push((consulta_de_json(c)?, d));
    }
    let mut saidas = Vec::new();
    for s in v.get("saidas")?.as_array()? {
        let id = AssetId::de_texto(s.get(0)?.as_str()?)?;
        let d = match s.get(1)? {
            Value::Null => None,
            d => Some(de_hex(d.as_str()?)?),
        };
        saidas.push((id, d));
    }
    Some(AcaoSalva {
        chave: v.get("chave")?.as_str()?.to_string(),
        origem,
        motivo: v.get("motivo").and_then(Value::as_str).map(str::to_string),
        consultas,
        saidas,
        do_apoio: ids_de(v.get("do_apoio"))?,
        apagados: ids_de(v.get("apagados"))?,
    })
}

/// Lê o estado salvo. Arquivo ausente, de outro formato ou corrompido:
/// `None` (o motor começa do zero, como sem estado).
pub fn ler(dir: &Path) -> Option<Estado> {
    let texto = std::fs::read_to_string(dir.join("estado.json")).ok()?;
    let v: Value = serde_json::from_str(&texto).ok()?;
    if v.get("formato")?.as_u64()? != FORMATO {
        return None;
    }
    let codigo_dart = match v.get("codigo_dart")? {
        Value::Null => None,
        c => {
            let mut l = Vec::new();
            for par in c.as_array()? {
                l.push((
                    PathBuf::from(par.get(0)?.as_str()?),
                    de_hex(par.get(1)?.as_str()?)?,
                ));
            }
            Some(l)
        }
    };
    let acoes = v
        .get("acoes")?
        .as_array()?
        .iter()
        .map(acao_de_json)
        .collect::<Option<Vec<_>>>()?;
    Some(Estado {
        chave: v.get("chave")?.as_str()?.to_string(),
        codigo_dart,
        acoes,
    })
}

/// Conteúdo de uma saída salva, conferido pelo digest.
pub fn ler_blob(dir: &Path, d: &Digest) -> Option<Arc<[u8]>> {
    let b = std::fs::read(dir.join("blobs").join(hex(d))).ok()?;
    (blake3::hash(&b).as_bytes() == d).then(|| Arc::from(b))
}

/// Grava o estado e os conteúdos novos, e apaga os blobs que nenhuma ação
/// referencia mais. A gravação do JSON é atômica (arquivo temporário e
/// `rename`).
///
/// # Erros
///
/// Falha de escrita no diretório do estado.
pub fn gravar(
    dir: &Path,
    estado: &Estado,
    conteudos: &[(Digest, Arc<[u8]>)],
) -> Result<(), String> {
    let blobs = dir.join("blobs");
    std::fs::create_dir_all(&blobs).map_err(|e| format!("{}: {e}", blobs.display()))?;
    let mut vivos = std::collections::HashSet::new();
    for (d, b) in conteudos {
        let nome = hex(d);
        let p = blobs.join(&nome);
        if !p.is_file() {
            let tmp = blobs.join(format!("{nome}.tmp"));
            std::fs::write(&tmp, &b[..]).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        vivos.insert(nome);
    }
    let acoes: Vec<Value> = estado.acoes.iter().filter_map(acao_json).collect();
    let codigo = estado.codigo_dart.as_ref().map(|l| {
        Value::Array(
            l.iter()
                .filter_map(|(p, d)| Some(json!([texto(p)?, hex(d)])))
                .collect(),
        )
    });
    let v = json!({
        "formato": FORMATO,
        "chave": estado.chave,
        "codigo_dart": codigo,
        "acoes": acoes,
    });
    let arq = dir.join("estado.json");
    let tmp = dir.join("estado.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&v).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &arq).map_err(|e| format!("{}: {e}", arq.display()))?;
    if let Ok(ls) = std::fs::read_dir(&blobs) {
        for e in ls.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !vivos.contains(&n) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ida_e_volta_do_estado_e_blob_conferido() {
        let dir = tempfile::tempdir().unwrap();
        let conteudo: Arc<[u8]> = Arc::from(&b"saida"[..]);
        let d = *blake3::hash(&conteudo).as_bytes();
        let acao = AcaoSalva {
            chave: "k".into(),
            origem: OrigemSalva::Nativo("sass_builder:sass_builder".into()),
            motivo: None,
            consultas: vec![
                (Consulta::Arquivo("/p/lib/a.scss".into()), Some(d)),
                (Consulta::Existe("/p/lib/b.scss".into()), None),
                (
                    Consulta::GlobAtivos {
                        dir: "/p".into(),
                        padrao: "lib/**".into(),
                        candidatos: vec![("lib/a.g.dart".into(), true)],
                    },
                    Some(d),
                ),
            ],
            saidas: vec![
                (AssetId::novo("p", "lib/a.css"), Some(d)),
                (AssetId::novo("p", "lib/a.css.map"), None),
            ],
            do_apoio: vec![],
            apagados: vec![AssetId::novo("p", "lib/a.scss")],
        };
        let e = Estado {
            chave: "c".into(),
            codigo_dart: Some(vec![("/p/tool/b.dart".into(), d)]),
            acoes: vec![acao],
        };
        gravar(dir.path(), &e, &[(d, conteudo.clone())]).unwrap();
        let lido = ler(dir.path()).unwrap();
        assert_eq!(lido.chave, "c");
        assert_eq!(lido.codigo_dart, e.codigo_dart);
        assert_eq!(lido.acoes[0].consultas, e.acoes[0].consultas);
        assert_eq!(lido.acoes[0].saidas, e.acoes[0].saidas);
        assert_eq!(lido.acoes[0].origem, e.acoes[0].origem);
        assert_eq!(lido.acoes[0].apagados, e.acoes[0].apagados);
        assert_eq!(ler_blob(dir.path(), &d).as_deref(), Some(&b"saida"[..]));
        // Blob adulterado não é aceito.
        std::fs::write(dir.path().join("blobs").join(hex(&d)), b"outro").unwrap();
        assert!(ler_blob(dir.path(), &d).is_none());
        // Consulta semântica não é persistível.
        assert!(!persistivel(&Consulta::ApiBiblioteca(
            "package:p/a.dart".into()
        )));
        // Blob sem referência sai na próxima gravação.
        gravar(
            dir.path(),
            &Estado {
                chave: "c".into(),
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(!dir.path().join("blobs").join(hex(&d)).exists());
    }
}

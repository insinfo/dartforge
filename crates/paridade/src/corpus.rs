//! O corpus de paridade, `corpus/diagnosticos/<grupo>/`.
//!
//! Cada grupo é um pacote Dart (`pubspec.yaml` versionado; o
//! `.dart_tool/package_config.json` é escrito por [`preparar`] e fica fora do
//! git) com um `grupo.json` que diz qual SDK é o oráculo e quais pacotes de
//! apoio (`corpus/diagnosticos/pacotes/<nome>`) ele enxerga. A expectativa é
//! **sempre** a que o oráculo gravou (`oraculo.jsonl`); as anotações das
//! fontes de onde os casos vieram (`// [analyzer] ...`, `[diag.x]`) são do SDK
//! `main` e serviram só de semente.
//!
//! [`gerar`] monta os grupos a partir de `references/` (máquina local):
//! * `linguagem`: `references/dart-sdk/tests/language`, os arquivos com
//!   marcas `// [analyzer]` e o fecho dos imports/partes relativos deles;
//! * `analyzer`: `references/dart-sdk/pkg/analyzer/test/src/diagnostics`, um
//!   arquivo por trecho de `assertErrorsInCode`/`assertNoErrorsInCode`/
//!   `resolveTestCodeWithDiagnostics`, exceto os testes que montam contexto
//!   (outros arquivos, opções, pacotes);
//! * `*-3.13`: os arquivos que declaram `// @dart = 3.7` ou mais, cujo
//!   oráculo é o 3.13.4.

use crate::oraculo::{FNV_INICIO, SdkOraculo, fnv};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// `grupo.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grupo {
    pub sdk: SdkOraculo,
    /// Pacotes de apoio em `corpus/diagnosticos/pacotes/`.
    #[serde(default)]
    pub pacotes: Vec<String>,
    /// De onde os casos vieram (texto livre, para o relatório).
    #[serde(default)]
    pub origem: String,
}

/// Raiz do corpus no repositório.
pub fn raiz_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnosticos")
}

/// Grupos do corpus, em ordem de nome.
pub fn grupos(raiz: &Path) -> Vec<(String, Grupo)> {
    let mut v = Vec::new();
    let Ok(ents) = std::fs::read_dir(raiz) else { return v };
    for e in ents.flatten() {
        let g = e.path().join("grupo.json");
        if let Ok(t) = std::fs::read_to_string(&g) {
            if let Ok(grupo) = serde_json::from_str::<Grupo>(&t) {
                v.push((e.file_name().to_string_lossy().into_owned(), grupo));
            }
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Os `.dart` de um diretório, recursivo, sem `.dart_tool`, ordenados.
pub fn arquivos_dart(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(ents) = std::fs::read_dir(&d) else { continue };
        for e in ents.flatten() {
            let p = e.path();
            let nome = e.file_name();
            let nome = nome.to_string_lossy();
            if p.is_dir() {
                if !nome.starts_with('.') && nome != "build" {
                    pilha.push(p);
                }
            } else if nome.ends_with(".dart") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Os `.dart` de um diretório, recursivo, sem pular nada (saídas geradas).
pub fn arquivos_dart_todos(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pilha = vec![dir.to_path_buf()];
    while let Some(d) = pilha.pop() {
        let Ok(ents) = std::fs::read_dir(&d) else { continue };
        for e in ents.flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.extension().is_some_and(|x| x == "dart") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Hash das fontes do grupo (caminho relativo + conteúdo, em ordem).
pub fn hash_fontes(dir: &Path) -> String {
    let mut h = FNV_INICIO;
    for a in arquivos_dart(dir) {
        let rel = crate::oraculo::relativo(&a, dir).unwrap_or_default();
        h = fnv(rel.as_bytes(), h);
        h = fnv(&[0], h);
        h = fnv(&std::fs::read(&a).unwrap_or_default(), h);
    }
    format!("{h:016x}")
}

/// Escreve `.dart_tool/package_config.json` do grupo (URIs relativas).
/// Devolve o caminho dele.
pub fn preparar(dir: &Path, nome: &str, g: &Grupo) -> std::io::Result<PathBuf> {
    let pacote = nome_pacote(nome);
    let mut pkgs = vec![serde_json::json!({
        "name": pacote, "rootUri": "../", "packageUri": "lib/", "languageVersion": g.sdk.linguagem()
    })];
    for p in &g.pacotes {
        pkgs.push(serde_json::json!({
            "name": p, "rootUri": format!("../../pacotes/{p}"), "packageUri": "lib/", "languageVersion": "3.4"
        }));
    }
    let cfg = serde_json::json!({ "configVersion": 2, "packages": pkgs, "generator": "dartforge-paridade" });
    let destino = dir.join(".dart_tool").join("package_config.json");
    std::fs::create_dir_all(destino.parent().expect("pai"))?;
    std::fs::write(&destino, serde_json::to_string_pretty(&cfg).expect("JSON") + "\n")?;
    Ok(destino)
}

/// Nome de pacote Dart válido para o grupo.
pub fn nome_pacote(grupo: &str) -> String {
    format!("corpus_{}", grupo.replace(['-', '.'], "_"))
}

fn escrever_grupo(dir: &Path, nome: &str, g: &Grupo) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("pubspec.yaml"),
        format!("name: {}\npublish_to: none\nenvironment:\n  sdk: {}\n", nome_pacote(nome), g.sdk.restricao()),
    )?;
    std::fs::write(dir.join("grupo.json"), serde_json::to_string_pretty(g).expect("JSON") + "\n")
}

/// `// @dart = 3.7` ou mais no começo do arquivo (antes de código)?
pub fn declara_3_7_ou_mais(texto: &str) -> bool {
    for l in texto.lines().take(40) {
        let t = l.trim();
        if let Some(r) = t.strip_prefix("//") {
            let r: String = r.chars().filter(|c| !c.is_whitespace()).collect();
            if let Some(v) = r.strip_prefix("@dart=") {
                let mut it = v.split('.');
                let maj: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                let min: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                return (maj, min) >= (3, 7);
            }
        }
    }
    false
}

/// URIs relativas de `import`/`export`/`part` num arquivo (texto cru).
fn uris_relativas(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    for l in texto.lines() {
        let t = l.trim_start();
        let resto = ["import ", "export ", "part "].iter().find_map(|k| t.strip_prefix(k));
        let Some(resto) = resto else { continue };
        let resto = resto.trim_start();
        let Some(q) = resto.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
        let Some(fim) = resto[1..].find(q) else { continue };
        let uri = &resto[1..1 + fim];
        if !uri.contains(':') && uri.ends_with(".dart") {
            v.push(uri.to_string());
        }
    }
    v
}

fn normal(p: &Path) -> PathBuf {
    crate::analise::chave(p)
}

/// Monta os grupos a partir de `references` (substitui os `.dart` deles;
/// o oráculo precisa ser regravado depois).
pub fn gerar(referencias: &Path, destino: &Path, pub_cache: &Path) -> Result<String, String> {
    let mut rel = String::new();
    // Pacotes de apoio.
    copiar_arvore(&referencias.join("dart-sdk/pkg/expect/lib"), &destino.join("pacotes/expect/lib"))?;
    copiar_arvore(&pub_cache.join("meta-1.16.0/lib"), &destino.join("pacotes/meta/lib"))?;
    let pacotes = vec!["expect".to_string(), "meta".to_string()];

    // --- linguagem
    let base = referencias.join("dart-sdk/tests/language");
    let mut sementes = BTreeSet::new();
    for a in arquivos_dart(&base) {
        let t = std::fs::read_to_string(&a).unwrap_or_default();
        if t.contains("// [analyzer]") {
            sementes.insert(normal(&a));
        }
    }
    let n_sementes = sementes.len();
    let mut fecho: BTreeSet<PathBuf> = BTreeSet::new();
    let mut pilha: Vec<PathBuf> = sementes.iter().cloned().collect();
    while let Some(a) = pilha.pop() {
        if !fecho.insert(a.clone()) {
            continue;
        }
        let t = std::fs::read_to_string(&a).unwrap_or_default();
        for u in uris_relativas(&t) {
            let alvo = normal(&a.parent().expect("pai").join(&u));
            if alvo.is_file() && alvo.starts_with(normal(&base)) && !fecho.contains(&alvo) {
                pilha.push(alvo);
            }
        }
    }
    let mut por_grupo: BTreeMap<&str, usize> = BTreeMap::new();
    for g in ["linguagem", "linguagem-3.13"] {
        let _ = std::fs::remove_dir_all(destino.join(g));
    }
    for a in &fecho {
        let t = std::fs::read(a).map_err(|e| e.to_string())?;
        let g = if declara_3_7_ou_mais(&String::from_utf8_lossy(&t)) { "linguagem-3.13" } else { "linguagem" };
        let r = a.strip_prefix(normal(&base)).map_err(|_| "fora da base")?;
        let d = destino.join(g).join(r);
        std::fs::create_dir_all(d.parent().expect("pai")).map_err(|e| e.to_string())?;
        std::fs::write(&d, t).map_err(|e| e.to_string())?;
        *por_grupo.entry(g).or_default() += 1;
    }
    for (g, sdk) in [("linguagem", SdkOraculo::V362), ("linguagem-3.13", SdkOraculo::V3134)] {
        if por_grupo.contains_key(g) {
            escrever_grupo(
                &destino.join(g),
                g,
                &Grupo {
                    sdk,
                    pacotes: pacotes.clone(),
                    origem: "references/dart-sdk/tests/language (marcas // [analyzer] + fecho de imports)".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }
    rel.push_str(&format!(
        "linguagem: {n_sementes} arquivos com // [analyzer], {} com o fecho; {:?}\n",
        fecho.len(),
        por_grupo
    ));

    // --- analyzer
    for g in ["analyzer", "analyzer-3.13"] {
        let _ = std::fs::remove_dir_all(destino.join(g));
    }
    let base = referencias.join("dart-sdk/pkg/analyzer/test/src/diagnostics");
    let mut n_arquivos = 0;
    let mut n_trechos: BTreeMap<&str, usize> = BTreeMap::new();
    let mut n_pulados = 0;
    for a in arquivos_dart(&base) {
        if a.parent() != Some(base.as_path()) {
            continue;
        }
        let t = std::fs::read_to_string(&a).map_err(|e| e.to_string())?;
        let stem = a.file_stem().expect("nome").to_string_lossy().trim_end_matches("_test").to_string();
        let (trechos, pulados) = extrair_trechos(&t);
        n_pulados += pulados;
        if trechos.is_empty() {
            continue;
        }
        n_arquivos += 1;
        let mut usados = BTreeSet::new();
        for (classe, metodo, codigo) in trechos {
            let mut nome = format!("{}__{}", curto(&classe), metodo.trim_start_matches("test_"));
            let mut k = 2;
            while !usados.insert(nome.clone()) {
                nome = format!("{}__{}_{k}", curto(&classe), metodo.trim_start_matches("test_"));
                k += 1;
            }
            let g = if declara_3_7_ou_mais(&codigo) { "analyzer-3.13" } else { "analyzer" };
            let d = destino.join(g).join(&stem).join(format!("{}.dart", nome_curto(&nome)));
            std::fs::create_dir_all(d.parent().expect("pai")).map_err(|e| e.to_string())?;
            std::fs::write(&d, codigo).map_err(|e| e.to_string())?;
            *n_trechos.entry(g).or_default() += 1;
        }
    }
    for (g, sdk) in [("analyzer", SdkOraculo::V362), ("analyzer-3.13", SdkOraculo::V3134)] {
        if n_trechos.contains_key(g) {
            escrever_grupo(
                &destino.join(g),
                g,
                &Grupo {
                    sdk,
                    pacotes: pacotes.clone(),
                    origem: "references/dart-sdk/pkg/analyzer/test/src/diagnostics (um arquivo por trecho)".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }
    rel.push_str(&format!(
        "analyzer: {n_arquivos} arquivos de teste, trechos {n_trechos:?}, {n_pulados} testes pulados (montam contexto)\n"
    ));
    Ok(rel)
}

/// Nome de arquivo curto o bastante para o limite de 260 caracteres do
/// Windows (o checkout do CI e as worktrees têm prefixos longos): até 48
/// caracteres; acima disso, 39 + `_` + 8 hexadecimais do hash do nome.
pub fn nome_curto(stem: &str) -> String {
    if stem.len() <= 48 {
        return stem.to_string();
    }
    let mut corte = 39;
    while !stem.is_char_boundary(corte) {
        corte -= 1;
    }
    format!("{}_{:08x}", &stem[..corte], fnv(stem.as_bytes(), FNV_INICIO) as u32)
}

/// Renomeia os arquivos de nome longo de um grupo já gravado e reescreve o
/// registro do oráculo (os offsets não mudam; só o caminho). Devolve quantos.
pub fn encurtar(dir: &Path) -> Result<usize, String> {
    let (mut regs, mut meta) = crate::oraculo::ler(dir)?;
    let mut mapa = std::collections::BTreeMap::new();
    // Os testes de linguagem se importam pelo nome: lá não se renomeia (só
    // se regrava o registro com o hash das fontes).
    let renomear = !dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("linguagem"));
    for a in arquivos_dart(dir).into_iter().filter(|_| renomear) {
        let stem = a.file_stem().expect("nome").to_string_lossy().into_owned();
        let novo = nome_curto(&stem);
        if novo != stem {
            let destino = a.with_file_name(format!("{novo}.dart"));
            std::fs::rename(&a, &destino).map_err(|e| e.to_string())?;
            let de = crate::oraculo::relativo(&a, dir).expect("relativo");
            let para = crate::oraculo::relativo(&destino, dir).expect("relativo");
            mapa.insert(de, para);
        }
    }
    for r in regs.iter_mut() {
        if let Some(n) = mapa.get(&r.arquivo) {
            r.arquivo = n.clone();
        }
    }
    meta.hash = hash_fontes(dir);
    crate::oraculo::gravar(dir, regs, &meta).map_err(|e| e.to_string())?;
    Ok(mapa.len())
}

/// Nome curto da classe de teste (`NonBoolConditionTest` → `NonBoolCondition`).
fn curto(classe: &str) -> String {
    classe.trim_end_matches("Test").to_string()
}

fn copiar_arvore(de: &Path, para: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(para);
    for a in arquivos_dart(de) {
        let r = a.strip_prefix(de).map_err(|_| "prefixo")?;
        let d = para.join(r);
        std::fs::create_dir_all(d.parent().expect("pai")).map_err(|e| e.to_string())?;
        std::fs::copy(&a, &d).map_err(|e| format!("{}: {e}", a.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Extração dos trechos dos testes do analyzer.

#[derive(Debug, Clone, PartialEq)]
enum T {
    Id(String),
    /// String literal: (cru, triplo, conteúdo).
    Str(bool, bool, String),
    P(char),
}

/// Tokens com posição; comentários descartados; strings inteiras (sem
/// interpretar escapes nem interpolação).
fn tokens(src: &str) -> Vec<(usize, T)> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut prof = 0;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'\'' || c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'\'') | Some(b'"'))) {
            let ini = i;
            let cru = c == b'r';
            if cru {
                i += 1;
            }
            let q = b[i];
            let triplo = b[i..].starts_with(&[q, q, q]);
            let n = if triplo { 3 } else { 1 };
            i += n;
            let conteudo_ini = i;
            // Interpolação `${...}` com strings dentro: basta contar chaves.
            let mut chaves = 0i32;
            loop {
                if i >= b.len() {
                    break;
                }
                if chaves == 0 && ((triplo && b[i..].starts_with(&[q, q, q])) || (!triplo && b[i] == q)) {
                    break;
                }
                if !cru && b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if !cru && b[i..].starts_with(b"${") {
                    chaves += 1;
                    i += 2;
                    continue;
                }
                if chaves > 0 && b[i] == b'}' {
                    chaves -= 1;
                }
                i += 1;
            }
            let conteudo = src[conteudo_ini..i.min(b.len())].to_string();
            i = (i + n).min(b.len());
            out.push((ini, T::Str(cru, triplo, conteudo)));
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push((ini, T::Id(src[ini..i].to_string())));
        } else {
            let ch = src[i..].chars().next().expect("char");
            out.push((i, T::P(ch)));
            i += ch.len_utf8();
        }
    }
    out
}

/// Chamadas que recebem o código do teste como primeiro argumento.
const CHAMADAS: &[&str] =
    &["assertErrorsInCode", "assertNoErrorsInCode", "resolveTestCodeWithDiagnostics", "assertErrorsInCode2"];

/// Chamadas que montam contexto além do arquivo de teste: o trecho sozinho
/// não reproduz o teste.
const CONTEXTO: &[&str] = &[
    "newFile",
    "writeTestPackageConfig",
    "writeTestPackageConfigWithMeta",
    "createAnalysisOptionsFile",
    "writeTestPackageAnalysisOptionsFile",
    "addTestFile",
    "newPackage",
    "writePackageConfig",
    "newAnalysisOptionsYamlFile",
    "testFilePath",
    "newPubspecYamlFile",
    "createMockSdk",
    "addFile",
];

/// `(classe, método, código)` de cada trecho, e o número de testes pulados.
pub fn extrair_trechos(src: &str) -> (Vec<(String, String, String)>, usize) {
    let t = tokens(src);
    let mut out = Vec::new();
    let mut pulados = 0;
    let mut i = 0;
    let mut prof = 0i32;
    let mut classe: Option<(String, i32, bool)> = None; // nome, profundidade do corpo, setUp com contexto
    while i < t.len() {
        match &t[i].1 {
            T::P('{') => prof += 1,
            T::P('}') => {
                prof -= 1;
                if classe.as_ref().is_some_and(|c| prof < c.1) {
                    classe = None;
                }
            }
            T::Id(k) if k == "class" && prof == 0 => {
                if let Some((_, T::Id(nome))) = t.get(i + 1) {
                    // Corpo da classe: próximo `{`. setUp com contexto pula a classe.
                    let mut j = i + 2;
                    while j < t.len() && t[j].1 != T::P('{') {
                        j += 1;
                    }
                    let fim = fim_do_bloco(&t, j);
                    let ctx = metodo_contexto(&t[j..fim]);
                    classe = Some((nome.clone(), prof + 1, ctx));
                    i = j;
                    continue;
                }
            }
            T::Id(m) if m.starts_with("test_") && classe.as_ref().is_some_and(|c| prof == c.1) => {
                // test_x() async { ... }
                let mut j = i + 1;
                while j < t.len() && t[j].1 != T::P('{') && t[j].1 != T::P(';') {
                    j += 1;
                }
                if j >= t.len() || t[j].1 != T::P('{') {
                    i += 1;
                    continue;
                }
                let fim = fim_do_bloco(&t, j);
                let corpo = &t[j..fim];
                let (nome_classe, _, ctx_classe) = classe.clone().expect("classe");
                let usa_contexto = ctx_classe || corpo.iter().any(|(_, x)| matches!(x, T::Id(n) if CONTEXTO.contains(&n.as_str())));
                let mut achados = Vec::new();
                for w in corpo.windows(3) {
                    if let (T::Id(n), T::P('('), T::Str(cru, true, codigo)) = (&w[0].1, &w[1].1, &w[2].1) {
                        if CHAMADAS.contains(&n.as_str()) {
                            if !*cru && (codigo.contains('$') || codigo.contains('\\')) {
                                continue;
                            }
                            // Na string de várias linhas do Dart, espaços e a quebra logo
                            // depois das aspas de abertura não fazem parte do valor.
                            let sem_espacos = codigo.trim_start_matches([' ', '\t']);
                            let valor = sem_espacos
                                .strip_prefix("\r\n")
                                .or_else(|| sem_espacos.strip_prefix('\n'))
                                .unwrap_or(codigo);
                            achados.push(valor.to_string());
                        }
                    }
                }
                if usa_contexto {
                    pulados += 1;
                } else {
                    for c in achados {
                        out.push((nome_classe.clone(), m.clone(), c));
                    }
                }
                // o laço segue a partir do `{`, contando profundidade normalmente
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    (out, pulados)
}

fn fim_do_bloco(t: &[(usize, T)], abre: usize) -> usize {
    let mut p = 0;
    let mut j = abre;
    while j < t.len() {
        match t[j].1 {
            T::P('{') => p += 1,
            T::P('}') => {
                p -= 1;
                if p == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    t.len()
}

/// O corpo da classe tem `setUp` que monta contexto?
fn metodo_contexto(corpo: &[(usize, T)]) -> bool {
    let mut i = 0;
    while i < corpo.len() {
        if let T::Id(n) = &corpo[i].1 {
            if n == "setUp" {
                let mut j = i;
                while j < corpo.len() && corpo[j].1 != T::P('{') && corpo[j].1 != T::P(';') {
                    j += 1;
                }
                if j < corpo.len() && corpo[j].1 == T::P('{') {
                    let fim = fim_do_bloco(corpo, j);
                    if corpo[j..fim].iter().any(|(_, x)| matches!(x, T::Id(n) if CONTEXTO.contains(&n.as_str()))) {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn extrai_trechos_e_pula_contexto() {
        let src = r#"
class ATest extends PubPackageResolutionTest {
  test_um() async {
    await assertErrorsInCode(r'''
f() { return 3 ? 2 : 1; }
''', [error(X, 1, 2)]);
  }
  test_dois() async {
    newFile('$testPackageLibPath/a.dart', '');
    await assertNoErrorsInCode(r'''
import 'a.dart';
''');
  }
  test_tres() async {
    await resolveTestCodeWithDiagnostics('''
var s = '${1}';
''');
  }
}
"#;
        let (v, pulados) = extrair_trechos(src);
        assert_eq!(pulados, 1);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "test_um");
        assert_eq!(v[0].2, "f() { return 3 ? 2 : 1; }\n");
    }

    #[test]
    fn versao_declarada() {
        assert!(declara_3_7_ou_mais("// @dart = 3.8\nvoid main() {}"));
        assert!(!declara_3_7_ou_mais("// @dart=2.19\n"));
        assert!(!declara_3_7_ou_mais("void main() {}"));
    }
}

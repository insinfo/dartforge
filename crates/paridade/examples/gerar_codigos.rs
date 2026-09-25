//! Gera `crates/diagnostics/src/codigos_g.rs`: a tabela de códigos do
//! `package:analyzer` 6.11.0 (o analyzer do SDK 3.6.2), lida das fontes na
//! cache do pub.
//!
//! ```text
//! cargo run -p dartforge-paridade --example gerar_codigos [-- <pub-cache/hosted/pub.dev>]
//! ```
//!
//! Determinístico: a mesma fonte dá o mesmo arquivo, byte a byte. As fontes
//! são os `.g.dart` gerados do `messages.yaml` do analyzer (e os dois arquivos
//! escritos à mão, do scanner e dos TODOs), lidos por um tokenizador de Dart
//! pequeno: `static const <Classe> <NOME> = [const] <Classe>(<args>);`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Classe do analyzer → módulo Rust, na ordem da tabela.
const CLASSES: &[(&str, &str, &str)] = &[
    ("CompileTimeErrorCode", "compile_time_error", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("StaticWarningCode", "static_warning", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("WarningCode", "warning", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("HintCode", "hint", "analyzer-6.11.0/lib/src/dart/error/hint_codes.g.dart"),
    ("FfiCode", "ffi", "analyzer-6.11.0/lib/src/dart/error/ffi_code.g.dart"),
    ("ParserErrorCode", "parser", "analyzer-6.11.0/lib/src/dart/error/syntactic_errors.g.dart"),
    ("ScannerErrorCode", "scanner", "_fe_analyzer_shared-76.0.0/lib/src/scanner/errors.dart"),
    ("TodoCode", "todo", "analyzer-6.11.0/lib/src/dart/error/todo_codes.dart"),
];

/// Códigos do analyzer do SDK 3.13.4 que a tabela 6.11 não tem e que o
/// parser emite (construtores primários, 3.13). Transcritos do
/// `messages.yaml` da referência (`references/dart-sdk/pkg/_fe_analyzer_shared`
/// e `pkg/analyzer`) e conferidos, texto e tipo, contra o oráculo 3.13.4
/// gravado em `corpus/diagnosticos`. Vão no fim da tabela: os índices dos
/// códigos da 6.11 não mudam.
///
/// (classe, módulo, constante, nome, unico, mensagem, correção, tipo)
const SUPLEMENTO_3_13: &[(&str, &str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY",
        "non_redirecting_generative_constructor_with_primary",
        "CompileTimeErrorCode.NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY",
        "Classes with primary constructors can't have non-redirecting generative constructors.",
        "Try making the constructor redirect to the primary constructor, or remove the primary constructor.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION",
        "primary_constructor_body_without_declaration",
        "CompileTimeErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION",
        "A primary constructor body requires a primary constructor declaration.",
        "Try adding the primary constructor declaration.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS",
        "multiple_primary_constructor_body_declarations",
        "CompileTimeErrorCode.MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS",
        "Only one primary constructor body declaration is allowed.",
        "Try removing all but one of the primary constructor body declarations.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY",
        "primary_constructor_body_with_expression_body",
        "CompileTimeErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY",
        "A primary constructor body can't use '=>'.",
        "Try using a block body.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY",
        "const_primary_constructor_with_body",
        "ParserErrorCode.CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY",
        "The body part of a constant primary constructor can't have a block body.",
        "Try replacing the block body with a semicolon, or removing the 'const' modifier.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY",
        "const_primary_constructor_with_body",
        "ParserErrorCode.CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY",
        "The body part of a constant primary constructor can't have an expression body.",
        "Try replacing the expression body with a semicolon, or removing the 'const' modifier.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER",
        "primary_constructor_body_with_modifier",
        "ParserErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER",
        "A primary constructor body can't have the modifier '{0}'.",
        "Try removing the modifier.",
        "SYNTACTIC_ERROR",
    ),
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Id(String),
    Str(String),
    P(char),
    Seta,
}

fn tokenizar(src: &str) -> Vec<Tok> {
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
            let cru = c == b'r';
            if cru {
                i += 1;
            }
            let q = b[i];
            let triplo = b[i..].starts_with(&[q, q, q]);
            let fim: Vec<u8> = if triplo { vec![q, q, q] } else { vec![q] };
            i += fim.len();
            let mut s = Vec::new();
            while i < b.len() && !b[i..].starts_with(&fim) {
                if b[i] == b'\\' && !cru {
                    let e = b[i + 1];
                    match e {
                        b'n' => s.push(b'\n'),
                        b't' => s.push(b'\t'),
                        b'r' => s.push(b'\r'),
                        _ => s.push(e),
                    }
                    i += 2;
                } else {
                    s.push(b[i]);
                    i += 1;
                }
            }
            i += fim.len();
            out.push(Tok::Str(String::from_utf8(s).expect("UTF-8")));
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push(Tok::Id(src[ini..i].to_string()));
        } else if b[i..].starts_with(b"=>") {
            out.push(Tok::Seta);
            i += 2;
        } else {
            let ch = src[i..].chars().next().unwrap();
            out.push(Tok::P(ch));
            i += ch.len_utf8();
        }
    }
    out
}

#[derive(Debug)]
struct Entrada {
    classe: String,
    constante: String,
    nome: String,
    unico: String,
    mensagem: String,
    correcao: Option<String>,
    documentado: bool,
}

/// Um argumento: nome (se nomeado) e tokens do valor.
fn argumentos(toks: &[Tok]) -> Vec<(Option<String>, Vec<Tok>)> {
    let mut args = Vec::new();
    let mut atual: Vec<Tok> = Vec::new();
    let mut prof = 0i32;
    for t in toks {
        match t {
            Tok::P('(') | Tok::P('[') | Tok::P('{') => prof += 1,
            Tok::P(')') | Tok::P(']') | Tok::P('}') => prof -= 1,
            _ => {}
        }
        if prof == 0 && *t == Tok::P(',') {
            args.push(std::mem::take(&mut atual));
        } else {
            atual.push(t.clone());
        }
    }
    if !atual.is_empty() {
        args.push(atual);
    }
    args.into_iter()
        .map(|a| {
            if let [Tok::Id(n), Tok::P(':'), ..] = a.as_slice() {
                (Some(n.clone()), a[2..].to_vec())
            } else {
                (None, a)
            }
        })
        .collect()
}

fn texto(v: &[Tok]) -> Option<String> {
    let mut s = String::new();
    let mut algum = false;
    for t in v {
        match t {
            Tok::Str(x) => {
                s.push_str(x);
                algum = true;
            }
            _ => return None,
        }
    }
    algum.then_some(s)
}

/// Lê as entradas e o par (tipo, severidade) de cada classe do arquivo.
fn ler_classe(src: &str, classe: &str) -> (Vec<Entrada>, String, String) {
    let t = tokenizar(src);
    let mut i = 0;
    // `class <classe> extends ... {`
    while i + 1 < t.len() && !(t[i] == Tok::Id("class".into()) && t[i + 1] == Tok::Id(classe.into())) {
        i += 1;
    }
    assert!(i + 1 < t.len(), "classe {classe} não encontrada");
    while t[i] != Tok::P('{') {
        i += 1;
    }
    i += 1;
    let mut prof = 1;
    let mut entradas = Vec::new();
    let mut tipo = None;
    let mut severidade = None;
    while i < t.len() && prof > 0 {
        match &t[i] {
            Tok::P('{') => prof += 1,
            Tok::P('}') => prof -= 1,
            Tok::Id(s) if prof == 1 && s == "static" && t.get(i + 1) == Some(&Tok::Id("const".into())) => {
                // static const <Classe> <NOME> = [const] <Classe>( ... ) ;
                let Some(Tok::Id(tipo_decl)) = t.get(i + 2) else { panic!("static const sem tipo") };
                let Some(Tok::Id(constante)) = t.get(i + 3) else { panic!("static const sem nome") };
                if tipo_decl != classe {
                    i += 1;
                    continue;
                }
                let mut j = i + 4;
                assert_eq!(t[j], Tok::P('='), "{constante}");
                j += 1;
                if t[j] == Tok::Id("const".into()) {
                    j += 1;
                }
                assert_eq!(t[j], Tok::Id(classe.into()), "{constante}");
                j += 1;
                assert_eq!(t[j], Tok::P('('), "{constante}");
                let ini = j + 1;
                let mut p = 1;
                j += 1;
                while p > 0 {
                    match t[j] {
                        Tok::P('(') => p += 1,
                        Tok::P(')') => p -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                let args = argumentos(&t[ini..j - 1]);
                let pos: Vec<String> = args.iter().filter(|a| a.0.is_none()).map(|a| texto(&a.1).expect("string")).collect();
                let nomeado = |n: &str| args.iter().find(|a| a.0.as_deref() == Some(n)).map(|a| &a.1);
                let nome = pos[0].clone();
                let mensagem = if classe == "TodoCode" { "{0}".to_string() } else { pos[1].clone() };
                let unico_curto = nomeado("uniqueName").and_then(|v| texto(v)).unwrap_or_else(|| nome.clone());
                entradas.push(Entrada {
                    classe: classe.to_string(),
                    constante: constante.clone(),
                    nome: nome.to_lowercase(),
                    unico: format!("{classe}.{unico_curto}"),
                    mensagem,
                    correcao: nomeado("correctionMessage").and_then(|v| texto(v)),
                    documentado: nomeado("hasPublishedDocs").is_some_and(|v| v.as_slice() == [Tok::Id("true".into())]),
                });
                i = j;
                continue;
            }
            Tok::Id(s) if prof == 1 && (s == "ErrorSeverity" || s == "ErrorType") && t.get(i + 1) == Some(&Tok::Id("get".into())) => {
                let getter = match &t[i + 2] {
                    Tok::Id(g) => g.clone(),
                    _ => String::new(),
                };
                assert_eq!(t[i + 3], Tok::Seta);
                // `ErrorType.X.severity`, `ErrorSeverity.X` ou `ErrorType.X`.
                let mut j = i + 4;
                let mut expr = Vec::new();
                while t[j] != Tok::P(';') {
                    if let Tok::Id(x) = &t[j] {
                        expr.push(x.clone());
                    }
                    j += 1;
                }
                let valor = match expr.as_slice() {
                    [a, x, s] if a == "ErrorType" && s == "severity" => format!("tipo:{x}"),
                    [a, x] if a == "ErrorSeverity" || a == "ErrorType" => x.clone(),
                    _ => panic!("getter {getter} de {classe} não reconhecido: {expr:?}"),
                };
                if getter == "errorSeverity" {
                    severidade = Some(valor);
                } else if getter == "type" {
                    tipo = Some(valor);
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    (entradas, tipo.expect("getter type"), severidade.expect("getter errorSeverity"))
}

fn tipo_rust(t: &str) -> &'static str {
    match t {
        "TODO" => "TipoErro::Todo",
        "HINT" => "TipoErro::Hint",
        "COMPILE_TIME_ERROR" => "TipoErro::CompileTimeError",
        "CHECKED_MODE_COMPILE_TIME_ERROR" => "TipoErro::CheckedModeCompileTimeError",
        "STATIC_WARNING" => "TipoErro::StaticWarning",
        "SYNTACTIC_ERROR" => "TipoErro::SyntacticError",
        "LINT" => "TipoErro::Lint",
        _ => panic!("ErrorType {t}"),
    }
}

/// `ErrorType.X.severity` (`errors.dart:196-238` do `_fe_analyzer_shared`).
fn severidade_rust(s: &str) -> &'static str {
    match s {
        "ERROR" | "tipo:COMPILE_TIME_ERROR" | "tipo:SYNTACTIC_ERROR" | "tipo:CHECKED_MODE_COMPILE_TIME_ERROR" => {
            "Severidade::Error"
        }
        "WARNING" | "tipo:STATIC_WARNING" => "Severidade::Warning",
        "INFO" | "tipo:HINT" | "tipo:LINT" | "tipo:TODO" => "Severidade::Info",
        _ => panic!("severidade {s}"),
    }
}

fn main() {
    let raiz = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("PUB_CACHE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| Path::new(&l).join("Pub").join("Cache")))
            .expect("defina PUB_CACHE");
        base.join("hosted").join("pub.dev")
    });
    let mut todas: Vec<(Entrada, &str, String, String)> = Vec::new();
    let mut resumo = String::new();
    for (classe, modulo, arquivo) in CLASSES {
        let caminho = raiz.join(arquivo);
        let src = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
        let (entradas, tipo, sev) = ler_classe(&src, classe);
        let _ = writeln!(resumo, "{classe}: {}", entradas.len());
        for e in entradas {
            todas.push((e, modulo, tipo.clone(), sev.clone()));
        }
    }
    for (classe, modulo, constante, nome, unico, mensagem, correcao, tipo) in SUPLEMENTO_3_13 {
        let e = Entrada {
            classe: classe.to_string(),
            constante: constante.to_string(),
            nome: nome.to_string(),
            unico: unico.to_string(),
            mensagem: mensagem.to_string(),
            correcao: Some(correcao.to_string()),
            documentado: false,
        };
        todas.push((e, modulo, tipo.to_string(), format!("tipo:{tipo}")));
    }
    let _ = writeln!(resumo, "suplemento 3.13.4 (construtores primários): {}", SUPLEMENTO_3_13.len());
    let n = todas.len();
    let mut s = String::new();
    s.push_str("// GERADO por `cargo run -p dartforge-paridade --example gerar_codigos`. NÃO EDITE.\n");
    s.push_str("// Fonte: analyzer-6.11.0 e _fe_analyzer_shared-76.0.0 (SDK Dart 3.6), cache do pub.\n");
    for l in resumo.lines() {
        let _ = writeln!(s, "// {l}");
    }
    s.push_str("#![allow(missing_docs)]\n\nuse crate::{InfoCodigo, Severidade, TipoErro};\n\n");
    let _ = writeln!(s, "pub(crate) static TABELA: [InfoCodigo; {n}] = [");
    for (e, _, tipo, sev) in &todas {
        let _ = writeln!(
            s,
            "    InfoCodigo {{ nome: {:?}, unico: {:?}, mensagem: {:?}, correcao: {}, tipo: {}, severidade: {}, documentado: {} }},",
            e.nome,
            e.unico,
            e.mensagem,
            match &e.correcao {
                Some(c) => format!("Some({c:?})"),
                None => "None".into(),
            },
            tipo_rust(tipo),
            severidade_rust(sev),
            e.documentado
        );
    }
    s.push_str("];\n\n");
    let mut por_unico: Vec<(&str, usize)> = todas.iter().enumerate().map(|(i, e)| (e.0.unico.as_str(), i)).collect();
    por_unico.sort();
    for w in por_unico.windows(2) {
        assert_ne!(w[0].0, w[1].0, "uniqueName repetido");
    }
    let _ = writeln!(s, "pub(crate) static POR_UNICO: [(&str, u16); {n}] = [");
    for (u, i) in &por_unico {
        let _ = writeln!(s, "    ({u:?}, {i}),");
    }
    s.push_str("];\n\npub mod modulos {\n");
    for (classe, modulo, _) in CLASSES {
        let _ = writeln!(s, "    /// `{classe}`.\n    pub mod {modulo} {{\n        use crate::Codigo;");
        for (i, (e, m, _, _)) in todas.iter().enumerate() {
            if m == modulo {
                let _ = writeln!(s, "        pub const {}: Codigo = Codigo({i});", e.constante.to_uppercase());
            }
        }
        s.push_str("    }\n");
        let _ = &classe;
    }
    s.push_str("}\n");
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("../diagnostics/src/codigos_g.rs");
    std::fs::write(&destino, s).expect("gravar tabela");
    print!("{resumo}");
    println!("total: {n} -> {}", destino.display());
    for (e, ..) in &todas {
        debug_assert!(!e.classe.is_empty());
    }
}

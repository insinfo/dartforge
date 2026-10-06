//! Compactação do arquivo de produção (etapa 7 do plano, parte segura).
//!
//! Tira comentários e o espaço que não separa tokens, sem tocar em nome
//! nenhum: nada que um programa observe muda. A renomeação de locais e
//! propriedades (`docs/JS-PRODUCAO.md` §3) fica de fora — ver o fim do §3 lá.
//!
//! As regras, cada uma pela razão que a torna segura:
//!
//! * **quebra de linha fica**: uma sequência de espaço com quebra (ou um
//!   comentário de bloco com quebra) vira uma quebra só. A inserção
//!   automática de ponto e vírgula (`return`↵`x`, `a`↵`++b`) depende da
//!   quebra e não do resto do espaço, então o sentido é o mesmo;
//! * **espaço entre tokens só onde separa**: entre dois caracteres de
//!   palavra (`var x`, `return 1`), entre `+ +` e `- -` (`a + +b` não é
//!   `a++b`), entre `/` e `/` ou `*` (não abre comentário), entre `<` e `!` e
//!   entre `-` e `>` (`<!--` e `-->` são comentários HTML em script), e entre
//!   número e `.` (`1 .x` não é `1.x`);
//! * **literais intactos**: cadeias, *template literals* (com as expressões
//!   `${…}` dentro) e expressões regulares são copiados byte a byte. A barra
//!   é regex no começo de expressão (depois de pontuação que não fecha
//!   expressão e das palavras-chave que esperam uma, `return`/`typeof`/…) e
//!   divisão depois de palavra, número, literal, `)`, `]` e `++`/`--`; uma
//!   "regex" que chega ao fim da linha sem fechar volta a ser divisão.
//!
//! A saída é determinística (uma passada, sem tabela) e tem os mesmos tokens
//! na mesma ordem que a entrada — é o que o teste confere, além do corpus
//! inteiro executado com o arquivo compactado.

/// Espécie do último token significativo, para decidir regex × divisão e
/// o espaço entre tokens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ultimo {
    Nada,
    /// Palavra: identificador, palavra-chave ou número. `regex_depois` diz
    /// se é uma palavra-chave depois da qual vem expressão; `numero`, se é
    /// um literal numérico.
    Palavra { regex_depois: bool, numero: bool },
    /// Cadeia, template ou regex.
    Literal,
    /// Pontuação: o caractere, e se ele forma `++`/`--` com o anterior.
    Pontuacao { c: u8, dobrado: bool },
}

/// Espaço pendente entre o último token e o próximo.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Espaco {
    Nenhum,
    Simples,
    Quebra,
}

fn e_palavra(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 || c == b'\\'
}

const PALAVRAS_ANTES_DE_EXPRESSAO: &[&[u8]] = &[
    b"return", b"typeof", b"instanceof", b"in", b"of", b"new", b"delete", b"void", b"throw", b"case", b"do", b"else", b"yield", b"await",
];

/// Compacta `src` (JS) conforme as regras do módulo.
pub fn compactar(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut ultimo = Ultimo::Nada;
    let mut pendente = Espaco::Nenhum;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Espaço e quebras.
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' || c == 0x0b || c == 0x0c {
            let q = if c == b'\n' || c == b'\r' { Espaco::Quebra } else { Espaco::Simples };
            pendente = pendente.max(q);
            i += 1;
            continue;
        }
        // Comentários.
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            pendente = pendente.max(Espaco::Simples);
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let fim = src[i + 2..].find("*/").map(|k| i + 2 + k + 2).unwrap_or(b.len());
            let com_quebra = b[i..fim].iter().any(|&x| x == b'\n' || x == b'\r');
            pendente = pendente.max(if com_quebra { Espaco::Quebra } else { Espaco::Simples });
            i = fim;
            continue;
        }
        // O próximo token: [i, fim) e a espécie dele.
        let (fim, especie) = if c == b'"' || c == b'\'' {
            (fim_de_cadeia(b, i), Ultimo::Literal)
        } else if c == b'`' {
            (fim_de_template(b, i), Ultimo::Literal)
        } else if c == b'/' && regex_permitida(ultimo) {
            match fim_de_regex(b, i) {
                Some(f) => (f, Ultimo::Literal),
                None => (i + 1, Ultimo::Pontuacao { c, dobrado: false }),
            }
        } else if e_palavra(c) {
            let numero = c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit));
            let f = if numero { fim_de_numero(b, i) } else { fim_de_palavra(b, i) };
            let regex_depois = !numero && PALAVRAS_ANTES_DE_EXPRESSAO.contains(&&b[i..f]);
            (f, Ultimo::Palavra { regex_depois, numero })
        } else if c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            (fim_de_numero(b, i), Ultimo::Palavra { regex_depois: false, numero: true })
        } else {
            let dobrado = (c == b'+' || c == b'-') && matches!(ultimo, Ultimo::Pontuacao { c: d, dobrado: false } if d == c) && pendente == Espaco::Nenhum;
            (i + 1, Ultimo::Pontuacao { c, dobrado })
        };
        match pendente {
            Espaco::Quebra if !out.is_empty() => out.push(b'\n'),
            Espaco::Simples if precisa_de_espaco(ultimo, out.last().copied(), b[i]) => out.push(b' '),
            _ => {}
        }
        out.extend_from_slice(&b[i..fim]);
        pendente = Espaco::Nenhum;
        ultimo = especie;
        i = fim;
    }
    if !out.is_empty() && src.ends_with('\n') {
        out.push(b'\n');
    }
    // Só bytes da entrada, cortados em fronteiras de token ASCII: UTF-8 válido.
    String::from_utf8(out).expect("a compactação corta só em fronteiras ASCII")
}

fn regex_permitida(u: Ultimo) -> bool {
    match u {
        Ultimo::Nada => true,
        Ultimo::Palavra { regex_depois, .. } => regex_depois,
        Ultimo::Literal => false,
        Ultimo::Pontuacao { c, dobrado } => !dobrado && !matches!(c, b')' | b']'),
    }
}

/// Se o espaço entre o token anterior (que termina em `a`) e o próximo (que
/// começa em `b`) precisa ficar.
fn precisa_de_espaco(ultimo: Ultimo, a: Option<u8>, b: u8) -> bool {
    let Some(a) = a else { return false };
    if e_palavra(a) && e_palavra(b) {
        return true;
    }
    if matches!(ultimo, Ultimo::Palavra { numero: true, .. }) && b == b'.' {
        return true;
    }
    matches!((a, b), (b'+', b'+') | (b'-', b'-') | (b'/', b'/') | (b'/', b'*') | (b'<', b'!') | (b'-', b'>'))
}

fn fim_de_palavra(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && e_palavra(b[i]) {
        // `\uXXXX` em identificador: o escape inteiro é da palavra.
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    i.min(b.len())
}

fn fim_de_numero(b: &[u8], mut i: usize) -> usize {
    let hex = b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(b'x' | b'X'));
    while i < b.len() {
        let c = b[i];
        // O sinal só é do número logo depois do expoente (`1e-5`).
        let sinal_de_expoente = (c == b'+' || c == b'-') && !hex && i > 0 && matches!(b[i - 1], b'e' | b'E');
        if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || sinal_de_expoente {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn fim_de_cadeia(b: &[u8], i: usize) -> usize {
    let aspa = b[i];
    let mut j = i + 1;
    while j < b.len() && b[j] != aspa {
        if b[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(b.len())
}

/// Fim de um *template literal* que começa em `i`, com as expressões
/// `${…}` (que podem ter cadeias, templates, comentários e chaves).
fn fim_de_template(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'`' => return j + 1,
            b'$' if b.get(j + 1) == Some(&b'{') => j = fim_de_expressao_de_template(b, j + 2),
            _ => j += 1,
        }
    }
    b.len()
}

/// Fim (depois da `}`) da expressão de template que começa em `i`.
fn fim_de_expressao_de_template(b: &[u8], mut i: usize) -> usize {
    let mut prof = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = fim_de_cadeia(b, i),
            b'`' => i = fim_de_template(b, i),
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'{' => {
                prof += 1;
                i += 1;
            }
            b'}' => {
                if prof == 0 {
                    return i + 1;
                }
                prof -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// Fim de uma regex que começa em `i` (com as flags), ou `None` se não
/// fecha antes do fim da linha (então a barra era divisão).
fn fim_de_regex(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    let mut classe = false;
    // `//` já foi lido como comentário; `/` seguido de `*` também.
    loop {
        let c = *b.get(j)?;
        match c {
            b'\n' | b'\r' => return None,
            b'\\' => j += 1,
            b'[' => classe = true,
            b']' => classe = false,
            b'/' if !classe => break,
            _ => {}
        }
        j += 1;
    }
    j += 1;
    while j < b.len() && (b[j].is_ascii_alphabetic()) {
        j += 1;
    }
    Some(j)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tira_comentarios_e_indentacao() {
        let js = "// cabeçalho\nfunction f(a, b) {\n  /* soma */\n  return a + b;\n}\n";
        assert_eq!(compactar(js), "function f(a,b){\nreturn a+b;\n}\n");
    }

    #[test]
    fn preserva_quebras_que_decidem_ponto_e_virgula() {
        // `return`↵`x` devolve `undefined`; `a`↵`++b` incrementa `b`.
        assert_eq!(compactar("return\n  x"), "return\nx");
        assert_eq!(compactar("a\n++b"), "a\n++b");
        // Comentário de bloco com quebra é quebra.
        assert_eq!(compactar("return /*\n*/ x"), "return\nx");
    }

    #[test]
    fn espaco_que_separa_tokens_fica() {
        assert_eq!(compactar("a + +b"), "a+ +b");
        assert_eq!(compactar("a - -b"), "a- -b");
        assert_eq!(compactar("a++ + b"), "a++ +b");
        assert_eq!(compactar("var x = typeof y"), "var x=typeof y");
        assert_eq!(compactar("1 .toString()"), "1 .toString()");
        assert_eq!(compactar("x < !y"), "x< !y");
        assert_eq!(compactar("x-- > y"), "x-- >y");
        assert_eq!(compactar("a / /re/.source.length"), "a/ /re/.source.length");
    }

    #[test]
    fn literais_sao_copiados_como_estao() {
        assert_eq!(compactar("s = 'a  // b'  ;"), "s='a  // b';");
        assert_eq!(compactar("s = \"x \\\" /* y */\""), "s=\"x \\\" /* y */\"");
        assert_eq!(compactar("t = `a  ${ f( '}' ) }  b`"), "t=`a  ${ f( '}' ) }  b`");
        assert_eq!(compactar("r = /a  b[/ ]c/g.test(x)"), "r=/a  b[/ ]c/g.test(x)");
        assert_eq!(compactar("return /  x/.exec(s)"), "return/  x/.exec(s)");
    }

    #[test]
    fn barra_depois_de_expressao_e_divisao() {
        assert_eq!(compactar("x = a / b / c"), "x=a/b/c");
        assert_eq!(compactar("x = (a) / 2 / 3"), "x=(a)/2/3");
        assert_eq!(compactar("x = a++ / 2 / 3"), "x=a++/2/3");
        assert_eq!(compactar("x = 1e-5 / 2"), "x=1e-5/2");
    }

    #[test]
    fn e_deterministica_e_idempotente() {
        let js = "var a = {b: 1, // c\n  d: [1, 2]};\nfunction g() { return a.b  +  a.d[0]; }\n";
        let um = compactar(js);
        assert_eq!(um, compactar(js));
        assert_eq!(compactar(&um), um);
    }
}

/// Minificação de identificadores por escopo e de espaço
/// (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §7.2), com o `oxc`: parser → semântica
/// → *mangler* → impressor minificado.
///
/// O arquivo é embrulhado numa função, para que os nomes de topo (`dart`,
/// `core`, `L$pacote__lib`, `t$R`) também sejam locais e encurtem — nada de
/// fora do arquivo os cita. Globais livres (`self`, `window`, `console`, os
/// construtores do DOM) não têm declaração no arquivo e por isso nunca são
/// renomeados; escopo com `eval` direto fica intacto (regra do *mangler*).
/// **Propriedades não mudam**: o nome de membro Dart é observável no contrato
/// do DDC (`dsend(o, "foo")`, `NoSuchMethodError`, interop).
///
/// # Erros
///
/// O texto não é JS válido para o parser (o que seria defeito do emissor):
/// a primeira mensagem do parser.
/// *Strings* literais com forma de identificador: nomes que chegam à
/// execução por texto (`dsend(o, "foo")`, `"a" in opts`,
/// `defineExtensionMethods(C, ["foo"])`) e por isso não podem ser renomeados.
struct StringsIdentificador(std::collections::HashSet<String>);

impl<'a> oxc_ast_visit::Visit<'a> for StringsIdentificador {
    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        self.guardar(it.value.as_str());
    }
    fn visit_template_literal(&mut self, it: &oxc_ast::ast::TemplateLiteral<'a>) {
        for q in &it.quasis {
            if let Some(c) = &q.value.cooked {
                self.guardar(c.as_str());
            }
        }
        oxc_ast_visit::walk::walk_template_literal(self, it);
    }
}

impl StringsIdentificador {
    fn guardar(&mut self, v: &str) {
        let mut cs = v.chars();
        let ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if ok && !self.0.contains(v) {
            self.0.insert(v.to_string());
        }
    }
}

/// *Strings* literais em posição de expressão (não chaves de objeto nem
/// diretivas), com o trecho do fonte de cada ocorrência.
struct OcorrenciasDeString(std::collections::HashMap<String, Vec<(u32, u32)>>);

impl<'a> oxc_ast_visit::Visit<'a> for OcorrenciasDeString {
    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        // Com forma de identificador fica literal: no acesso `x["_n$1c"]` a
        // compressão o troca por `x._n$1c`, que o renomeio de propriedades
        // encurta — uma variável no lugar impediria as duas coisas.
        let v = it.value.as_str();
        let mut cs = v.chars();
        if cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            return;
        }
        self.0.entry(v.to_string()).or_default().push((it.span.start, it.span.end));
    }
    fn visit_property_key(&mut self, it: &oxc_ast::ast::PropertyKey<'a>) {
        if matches!(it, oxc_ast::ast::PropertyKey::StringLiteral(_)) {
            return;
        }
        oxc_ast_visit::walk::walk_property_key(self, it);
    }
    fn visit_directive(&mut self, _it: &oxc_ast::ast::Directive<'a>) {}
}

/// Deduplicação de *strings* (`docs/JS-PRODUCAO-TAMANHO.md` §3.4): toda
/// *string* repetida que compensa vira uma `var t$S…` no topo da IIFE do
/// arquivo, e cada ocorrência em posição de expressão passa a citar a
/// variável (as receitas rti `t$R("core|int")`, os nomes de `dsend`, as
/// mensagens). O valor continua literal na declaração: a reserva de nomes do
/// renomeio de propriedades o enxerga.
fn deduplicar_strings(fonte: &str) -> Result<String, String> {
    use oxc_ast_visit::Visit;
    let alocador = oxc_allocator::Allocator::default();
    let lido = oxc_parser::Parser::new(&alocador, fonte, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = lido.diagnostics.first() {
        return Err(format!("oxc não leu o arquivo de produção: {e}"));
    }
    let mut oc = OcorrenciasDeString(std::collections::HashMap::new());
    oc.visit_program(&lido.program);
    // Compensa quando as `n` cópias custam mais que a declaração e as `n`
    // referências (≈ 3 bytes cada depois da minificação).
    let mut escolhidas: Vec<(String, Vec<(u32, u32)>)> = oc
        .0
        .into_iter()
        .filter(|(_, v)| {
            let n = v.len();
            let tam = (v[0].1 - v[0].0) as usize;
            n >= 2 && tam * n > tam + 8 + 3 * n
        })
        .collect();
    escolhidas.sort_by(|a, b| a.0.cmp(&b.0));
    let cabeca = "(function () {\n";
    if escolhidas.is_empty() || !fonte.starts_with(cabeca) {
        return Ok(fonte.to_string());
    }
    let mut trocas: Vec<(usize, usize, String)> = Vec::new();
    let mut decl = String::new();
    for (i, (_, v)) in escolhidas.iter().enumerate() {
        let nome = format!("t$S{i}");
        let (a, b) = v[0];
        decl.push_str(if decl.is_empty() { "var " } else { ",\n" });
        decl.push_str(&format!("{nome} = {}", &fonte[a as usize..b as usize]));
        for &(a, b) in v {
            trocas.push((a as usize, b as usize, nome.clone()));
        }
    }
    decl.push_str(";\n");
    trocas.sort_by_key(|t| t.0);
    let mut out = String::with_capacity(fonte.len());
    out.push_str(cabeca);
    out.push_str(&decl);
    let mut pos = cabeca.len();
    for (a, b, nome) in trocas {
        if a < pos {
            continue;
        }
        out.push_str(&fonte[pos..a]);
        out.push_str(&nome);
        pos = b;
    }
    out.push_str(&fonte[pos..]);
    Ok(out)
}

/// Como [`minificar_nomes`], renomeando também as propriedades de
/// `nomes.renomeaveis` que não estão reservadas nem aparecem como *string*
/// (`docs/JS-PRODUCAO-TAMANHO.md` §3.1), com o `PropertyMangler` do
/// `oxc_minifier`.
pub fn minificar_com_propriedades(js: &str, nomes: &crate::propriedades::Nomes) -> Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_ast_visit::Visit;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_minifier::{ManglePropertiesOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    let fonte = format!("(function () {{\n{js}\n}})();\n");
    // `DARTFORGE_JSPROD_DEDUP=0` desliga a deduplicação de strings.
    // `this` repetido vira local (`aliasthis.rs`); `DARTFORGE_JSPROD_THIS=0` desliga.
    let fonte = if std::env::var("DARTFORGE_JSPROD_THIS").map_or(true, |v| v != "0") { crate::aliasthis::aplicar(&fonte) } else { fonte };
    let fonte = if std::env::var("DARTFORGE_JSPROD_DEDUP").map_or(true, |v| v != "0") { deduplicar_strings(&fonte)? } else { fonte };
    let alocador = Allocator::default();
    let lido = Parser::new(&alocador, &fonte, SourceType::cjs()).parse();
    if let Some(e) = lido.diagnostics.first() {
        return Err(format!("oxc não leu o arquivo de produção: {e}"));
    }
    let mut programa = lido.program;
    // As *strings* com forma de identificador são lidas **antes** da
    // compressão: ela junta listas de *strings* (`["a","b"]` vira
    // `"a.b".split(".")` no `defineExtensionAccessors`), e o nome some do
    // texto sem deixar de ser lido em execução. E de novo depois, para o que
    // ela criar. `DARTFORGE_JSPROD_COMPRIMIR=0` desliga a compressão.
    let mut strings = StringsIdentificador(std::collections::HashSet::new());
    strings.visit_program(&programa);
    if std::env::var("DARTFORGE_JSPROD_COMPRIMIR").map_or(true, |v| v != "0") {
        let mut c = oxc_minifier::CompressOptions::smallest();
        c.treeshake.manual_pure_functions = ["dart.privateName", "dart.fnType", "dart.gFnType", "t$R"].iter().map(|s| s.to_string()).collect();
        let _ = Minifier::new(MinifierOptions { mangle: None, mangle_properties: None, compress: Some(c) }).minify(&alocador, &mut programa);
    }
    strings.visit_program(&programa);
    let mut livres: Vec<&str> = nomes
        .renomeaveis
        .iter()
        .map(String::as_str)
        .filter(|n| !nomes.reservados.contains(*n) && !strings.0.contains(*n) && n.len() > 1)
        .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') && !n.as_bytes()[0].is_ascii_digit())
        .collect();
    livres.sort_unstable();
    // O `include` do `oxc` é uma regex; a lista de nomes livres não cabe
    // numa. Então: uma coleta com tudo elegível dá os candidatos que o `oxc`
    // enxerga; os que não são livres entram nos reservados; a segunda coleta
    // renomeia só os livres.
    if !livres.is_empty() {
        let livres: std::collections::HashSet<&str> = livres.iter().copied().collect();
        let mut base = ManglePropertiesOptions::from_pattern(".")?;
        for r in nomes.reservados.iter().chain(strings.0.iter()) {
            base.reserved.insert(r.as_str().into());
        }
        let mut sonda = oxc_minifier::PropertyMangler::new_in(base.clone(), &alocador);
        sonda.collect(&programa);
        let candidatos: Vec<String> = sonda.assign().keys().map(|k| k.as_str().to_string()).collect();
        let mut o = base;
        for c in candidatos {
            // Propriedade de nome privado do programa (`_x$1c`, `$C$x$1c`):
            // gerada pelo emissor, só citada pelo próprio texto.
            let privado = c.rsplit_once('$').is_some_and(|(b, t)| !b.is_empty() && nomes.tags.contains(&format!("${t}")));
            if !livres.contains(c.as_str()) && !(privado && !nomes.reservados.contains(&c) && !strings.0.contains(&c)) {
                o.reserved.insert(c.as_str().into());
            }
        }
        let mut pm = oxc_minifier::PropertyMangler::new_in(o, &alocador);
        pm.collect(&programa);
        pm.assign();
        pm.rewrite(&mut programa);
    }
    let opcoes = MinifierOptions {
        mangle: Some(oxc_minifier::MangleOptions { top_level: Some(true), ..oxc_minifier::MangleOptions::default() }),
        mangle_properties: None,
        compress: None,
    };
    let ret = Minifier::new(opcoes).minify(&alocador, &mut programa);
    let saida = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(ret.scoping)
        .with_private_member_mappings(ret.class_private_mappings)
        .build(&programa);
    Ok(saida.code)
}

pub fn minificar_nomes(js: &str) -> Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_mangler::{MangleOptions, Mangler};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    let fonte = format!("(function () {{\n{js}\n}})();\n");
    let alocador = Allocator::default();
    let tipo = SourceType::cjs();
    let lido = Parser::new(&alocador, &fonte, tipo).parse();
    if let Some(e) = lido.diagnostics.first() {
        return Err(format!("oxc não leu o arquivo de produção: {e}"));
    }
    let mangle = Mangler::new().with_options(MangleOptions { top_level: Some(true), ..MangleOptions::default() }).build(&lido.program);
    let saida = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(Some(mangle.scoping))
        .with_private_member_mappings(Some(mangle.class_private_mappings))
        .build(&lido.program);
    Ok(saida.code)
}

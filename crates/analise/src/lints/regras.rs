//! O primeiro lote de regras de lint, as que só olham a árvore
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `camel_case_types`,
//! `camel_case_extensions`, `non_constant_identifier_names`,
//! `empty_catches`, `avoid_empty_else`,
//! `curly_braces_in_flow_control_structures`,
//! `use_string_in_part_of_directives`,
//! `prefer_generic_function_type_aliases`, `provide_deprecation_message` e
//! `avoid_relative_lib_imports`.
//!
//! Escritas com os emissores do `main` do SDK abertos e depois conferidas
//! contra os da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`, extraído
//! da tag em 2026-10-05). Nenhuma das dez difere na regra.
//! Diferenças conhecidas: em `non_constant_identifier_names` ficam
//! de fora os campos de registro e o atalho `:nome` de padrão; em
//! `prefer_generic_function_type_aliases` a sugestão usa o texto como está
//! escrito, sem a normalização do `toSource`; `@deprecated` é reconhecido
//! pelo nome.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, DeclKind, DirectiveKind, ForInTarget, FunctionKind, MemberKind, PatternKind, StmtId, StmtKind, TypedefKind,
};
use dartforge_intern::Interner;

/// Um relato de lint.
#[derive(Debug, PartialEq, Eq)]
pub struct RelatoDeLint {
    pub codigo: &'static CodigoLint,
    pub span: Span,
    pub args: Vec<String>,
}

impl RelatoDeLint {
    /// A mensagem com os argumentos no lugar.
    pub fn mensagem(&self) -> String {
        let mut saida = self.codigo.mensagem.to_string();
        for (i, a) in self.args.iter().enumerate() {
            saida = saida.replace(&format!("{{{i}}}"), a);
        }
        saida
    }

    /// A mensagem de correção, com os argumentos no lugar.
    pub fn correcao(&self) -> Option<String> {
        self.codigo.correcao.map(|molde| {
            let mut saida = molde.to_string();
            for (i, a) in self.args.iter().enumerate() {
                saida = saida.replace(&format!("{{{i}}}"), a);
            }
            saida
        })
    }
}

/// `isCamelCase`: `^_*(?:\$+_+)*[$?A-Z][$?a-zA-Z\d]*$`.
pub fn e_camel_case(nome: &str) -> bool {
    let mut resto = nome.trim_start_matches('_');
    // Grupos `$…_…`, só quando há sublinhado depois dos cifrões.
    loop {
        let sem_cifroes = resto.trim_start_matches('$');
        if sem_cifroes.len() == resto.len() || !sem_cifroes.starts_with('_') {
            break;
        }
        resto = sem_cifroes.trim_start_matches('_');
    }
    let mut letras = resto.chars();
    letras.next().is_some_and(|p| p == '$' || p == '?' || p.is_ascii_uppercase())
        && letras.all(|x| x == '$' || x == '?' || x.is_ascii_alphanumeric())
}

/// `isLowerCamelCase`: uma maiúscula sozinha, `_`, ou
/// `^_*[?$a-z][a-z\d?$]*(?:(?:[A-Z]|_\d)[a-z\d?$]*)*_?$`.
pub fn e_lower_camel_case(nome: &str) -> bool {
    if nome == "_" || (nome.len() == 1 && nome.as_bytes()[0].is_ascii_uppercase()) {
        return true;
    }
    let b = nome.trim_start_matches('_').as_bytes();
    let minuscula = |x: u8| x.is_ascii_lowercase() || x.is_ascii_digit() || x == b'?' || x == b'$';
    if !b.first().is_some_and(|p| p.is_ascii_lowercase() || *p == b'?' || *p == b'$') {
        return false;
    }
    let mut i = 1;
    while i < b.len() {
        if minuscula(b[i]) || b[i].is_ascii_uppercase() {
            i += 1;
        } else if b[i] == b'_' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 2;
        } else {
            // Um `_` final é aceito; qualquer outra coisa não.
            return b[i] == b'_' && i + 1 == b.len();
        }
    }
    true
}

struct Ctx<'a> {
    u: Unidade<'a>,
    interner: &'a Interner,
    ligada: &'a dyn Fn(&str) -> bool,
    out: Vec<RelatoDeLint>,
}

impl<'a> Ctx<'a> {
    fn texto(&self, n: ast::Name) -> &'a str {
        self.interner.resolve(n.sym)
    }

    fn relatar(&mut self, codigo: &'static CodigoLint, span: Span, args: &[&str]) {
        self.out.push(RelatoDeLint { codigo, span, args: args.iter().map(|a| a.to_string()).collect() });
    }

    /// `camel_case_types` e `camel_case_extensions`.
    fn nomes_de_tipos(&mut self) {
        let (tipos, extensoes) = ((self.ligada)("camel_case_types"), (self.ligada)("camel_case_extensions"));
        for &d in self.u.unit.declarations.iter() {
            let decl = self.u.ast.decl(d);
            if decl.augment {
                continue;
            }
            let (nome, de_extensao) = match &decl.kind {
                DeclKind::Class(x) => (Some(x.name), false),
                DeclKind::Mixin(x) => (Some(x.name), false),
                DeclKind::Enum(x) => (Some(x.name), false),
                DeclKind::ExtensionType(x) => (Some(x.name), false),
                DeclKind::Typedef(x) => (Some(x.name), false),
                DeclKind::Extension(x) => (x.name, true),
                _ => (None, false),
            };
            let Some(nome) = nome else { continue };
            let texto = self.texto(nome);
            if e_camel_case(texto) {
                continue;
            }
            if de_extensao && extensoes {
                self.relatar(&c::CAMEL_CASE_EXTENSIONS, nome.span, &[texto]);
            } else if !de_extensao && tipos {
                self.relatar(&c::CAMEL_CASE_TYPES, nome.span, &[texto]);
            }
        }
    }

    fn identificador(&mut self, nome: ast::Name, sublinhados_valem: bool) {
        let texto = self.texto(nome);
        if sublinhados_valem && !texto.is_empty() && texto.bytes().all(|b| b == b'_') {
            return;
        }
        if !e_lower_camel_case(texto) {
            self.relatar(&c::NON_CONSTANT_IDENTIFIER_NAMES, nome.span, &[texto]);
        }
    }

    fn parametros(&mut self, lista: &'a [ast::Parameter]) {
        for p in lista {
            // Um `this.x` leva o nome do campo.
            if let (false, Some(n)) = (p.this_, p.name) {
                self.identificador(n, true);
            }
            if let Some(internos) = &p.function_parameters {
                self.parametros(internos);
            }
        }
    }

    /// `non_constant_identifier_names`.
    fn nomes_nao_constantes(&mut self) {
        let a = self.u.ast;
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) if !l.const_ => {
                    for v in l.variables.iter() {
                        self.identificador(v.name, false);
                    }
                }
                StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => self.identificador(*name, false),
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            self.identificador(*n, true);
                        }
                    }
                }
                _ => {}
            }
        }
        for p in a.patterns.iter() {
            if let PatternKind::Variable { final_, var_, ty, name } = &p.kind
                && (*final_ || *var_ || ty.is_some())
            {
                self.identificador(*name, false);
            }
        }
        for d in a.decls.iter().filter(|d| !d.augment) {
            if let DeclKind::Variables(l) = &d.kind
                && !l.const_
            {
                for v in l.variables.iter() {
                    self.identificador(v.name, false);
                }
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            match &m.kind {
                MemberKind::Field(l) if !l.const_ => {
                    for v in l.variables.iter() {
                        self.identificador(v.name, false);
                    }
                }
                MemberKind::Constructor(k) => {
                    if let Some(n) = k.name {
                        self.identificador(n, true);
                    }
                    self.parametros(&k.parameters);
                }
                _ => {}
            }
        }
        for f in a.functions.iter() {
            if let (Some(n), false) = (f.name, f.kind == FunctionKind::Operator) {
                self.identificador(n, false);
            }
            if let Some(ps) = &f.parameters {
                self.parametros(ps);
            }
        }
    }

    /// `empty_catches`, `avoid_empty_else` e
    /// `curly_braces_in_flow_control_structures`.
    fn comandos(&mut self) {
        let a = self.u.ast;
        let fonte = self.u.fonte;
        let (vazios, senao_vazio, chaves) =
            ((self.ligada)("empty_catches"), (self.ligada)("avoid_empty_else"), (self.ligada)("curly_braces_in_flow_control_structures"));
        let e_bloco = |s: StmtId| matches!(a.stmt(s).kind, StmtKind::Block(_));
        let linha = |offset: usize| fonte.as_bytes()[..offset.min(fonte.len())].iter().filter(|b| **b == b'\n').count();
        // Os `if` que são o `else` de outro `if`.
        let de_senao: Vec<StmtId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::If { else_: Some(e), .. } if matches!(a.stmt(*e).kind, StmtKind::If { .. }) => Some(*e),
                _ => None,
            })
            .collect();
        for (i, s) in a.stmts.iter().enumerate() {
            match &s.kind {
                StmtKind::Try { catches, .. } if vazios => {
                    for k in catches.iter() {
                        let so_sublinhados = k.exception.is_some_and(|n| {
                            let t = self.texto(n);
                            !t.is_empty() && t.bytes().all(|b| b == b'_')
                        });
                        let corpo = a.stmt(k.body);
                        let sem_comandos = matches!(&corpo.kind, StmtKind::Block(l) if l.is_empty());
                        // Sem comentário antes do `}`: só brancos entre as chaves.
                        let miolo = fonte.get(corpo.span.start + 1..corpo.span.end.saturating_sub(1)).unwrap_or("x");
                        if !so_sublinhados && sem_comandos && miolo.trim().is_empty() {
                            self.relatar(&c::EMPTY_CATCHES, corpo.span, &[]);
                        }
                    }
                }
                StmtKind::If { then, else_, .. } => {
                    if senao_vazio
                        && let Some(e) = else_
                        && matches!(a.stmt(*e).kind, StmtKind::Empty)
                        && a.stmt(*e).span.end > a.stmt(*e).span.start
                    {
                        self.relatar(&c::AVOID_EMPTY_ELSE, a.stmt(*e).span, &[]);
                    }
                    if !chaves {
                        continue;
                    }
                    match else_ {
                        None => {
                            if de_senao.contains(&StmtId(i as u32)) {
                                if !e_bloco(*then) {
                                    self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                                }
                            } else if !e_bloco(*then) && linha(s.span.start) != linha(a.stmt(*then).span.end) {
                                // Um `if` sem `else` numa linha só é aceito.
                                self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                            }
                        }
                        Some(e) => {
                            if !e_bloco(*then) {
                                self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*then).span, &["an if"]);
                            }
                            if !e_bloco(*e) && !matches!(a.stmt(*e).kind, StmtKind::If { .. }) {
                                self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*e).span, &["an if"]);
                            }
                        }
                    }
                }
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } if chaves && !e_bloco(*body) => {
                    self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a for"]);
                }
                StmtKind::While { body, .. } if chaves && !e_bloco(*body) => {
                    self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a while"]);
                }
                StmtKind::DoWhile { body, .. } if chaves && !e_bloco(*body) => {
                    self.relatar(&c::CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES, a.stmt(*body).span, &["a do"]);
                }
                _ => {}
            }
        }
    }

    /// `use_string_in_part_of_directives` e `avoid_relative_lib_imports`.
    fn diretivas(&mut self) {
        let (parte_de, relativo) = ((self.ligada)("use_string_in_part_of_directives"), (self.ligada)("avoid_relative_lib_imports"));
        for d in self.u.unit.directives.iter() {
            match &d.kind {
                DirectiveKind::PartOf { uri: None, name } if parte_de && !name.is_empty() => {
                    self.relatar(&c::USE_STRING_IN_PART_OF_DIRECTIVES, d.span, &[]);
                }
                DirectiveKind::Import { uri, .. } if relativo => {
                    // Sem esquema e com `/lib/` no caminho.
                    let Some(texto) = dartforge_elements::load::string_lit_value(uri) else { continue };
                    let sem_esquema = !texto.split(['/', '?', '#']).next().is_some_and(|p| p.contains(':'));
                    let caminho = texto.split(['?', '#']).next().unwrap_or("");
                    if sem_esquema && caminho.contains("/lib/") {
                        self.relatar(&c::AVOID_RELATIVE_LIB_IMPORTS, uri.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }

    /// `prefer_generic_function_type_aliases`: o `typedef` da forma antiga.
    fn typedefs_antigos(&mut self) {
        let fonte = self.u.fonte;
        for &d in self.u.unit.declarations.iter() {
            let decl = self.u.ast.decl(d);
            let DeclKind::Typedef(x) = &decl.kind else { continue };
            if !matches!(x.kind, TypedefKind::Legacy { .. }) {
                continue;
            }
            // `typedef R nome<T>(params);`: o retorno fica entre a palavra e
            // o nome; os parâmetros, do nome ao `;`.
            let antes = fonte.get(decl.span.start..x.name.span.start).unwrap_or("");
            let retorno = antes.rfind("typedef").map_or("", |k| antes[k + "typedef".len()..].trim());
            let depois = fonte.get(x.name.span.end..decl.span.end).unwrap_or("").trim_end();
            let Some(resto) = depois.strip_suffix(';') else { continue };
            let sugestao = if retorno.is_empty() { format!("Function{}", resto.trim()) } else { format!("{retorno} Function{}", resto.trim()) };
            self.relatar(&c::PREFER_GENERIC_FUNCTION_TYPE_ALIASES, x.name.span, &[sugestao.as_str()]);
        }
    }

    /// `provide_deprecation_message`: `@deprecated` sem argumentos.
    fn deprecados_sem_mensagem(&mut self) {
        let a = self.u.ast;
        let listas = a
            .decls
            .iter()
            .map(|d| &d.metadata[..])
            .chain(a.members.iter().map(|m| &m.metadata[..]))
            .chain(self.u.unit.directives.iter().map(|d| &d.metadata[..]));
        let mut achados: Vec<Span> = Vec::new();
        for lista in listas {
            for m in lista {
                if m.arguments.is_none() && m.name.last().is_some_and(|n| self.texto(*n) == "deprecated") {
                    achados.push(m.span);
                }
            }
        }
        for span in achados {
            self.relatar(&c::PROVIDE_DEPRECATION_MESSAGE, span, &[]);
        }
    }
}

/// Roda as regras ligadas (`ligada(nome)`) sobre uma unidade.
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool) -> Vec<RelatoDeLint> {
    let mut ctx = Ctx { u, interner, ligada, out: Vec::new() };
    if ligada("camel_case_types") || ligada("camel_case_extensions") {
        ctx.nomes_de_tipos();
    }
    if ligada("non_constant_identifier_names") {
        ctx.nomes_nao_constantes();
    }
    if ligada("empty_catches") || ligada("avoid_empty_else") || ligada("curly_braces_in_flow_control_structures") {
        ctx.comandos();
    }
    if ligada("use_string_in_part_of_directives") || ligada("avoid_relative_lib_imports") {
        ctx.diretivas();
    }
    if ligada("prefer_generic_function_type_aliases") {
        ctx.typedefs_antigos();
    }
    if ligada("provide_deprecation_message") {
        ctx.deprecados_sem_mensagem();
    }
    ctx.out.sort_by_key(|r| (r.span.start, r.span.end));
    ctx.out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(regra, texto do intervalo)` de cada relato, com todas ligadas.
    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        executar(u, &nomes, &|_| true).into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn formas_dos_nomes() {
        assert!(e_camel_case("Foo") && e_camel_case("_Foo") && e_camel_case("$Foo") && e_camel_case("F"));
        assert!(!e_camel_case("foo") && !e_camel_case("Foo_Bar"));
        assert!(e_lower_camel_case("foo") && e_lower_camel_case("_fooBar") && e_lower_camel_case("foo_1") && e_lower_camel_case("A"));
        assert!(!e_lower_camel_case("Foo") && !e_lower_camel_case("foo_bar") && !e_lower_camel_case("__"));
    }

    #[test]
    fn nomes() {
        assert_eq!(achados("class foo {}\n"), vec![("camel_case_types", "foo".to_string())]);
        assert_eq!(achados("extension minha on int {}\n"), vec![("camel_case_extensions", "minha".to_string())]);
        assert_eq!(achados("void Faz(int um_dois) {}\n"), vec![
            ("non_constant_identifier_names", "Faz".to_string()),
            ("non_constant_identifier_names", "um_dois".to_string()),
        ]);
        assert!(achados("const MAX = 1;\nvoid f(int _, int __) {}\n").is_empty());
    }

    #[test]
    fn comandos_de_fluxo() {
        assert_eq!(achados("void f() {\n  try {} catch (e) {}\n}\n"), vec![("empty_catches", "{}".to_string())]);
        assert!(achados("void f() {\n  try {} catch (_) {}\n  try {} catch (e) {\n    // nada\n  }\n}\n").is_empty());
        assert_eq!(
            achados("void f(bool b) {\n  if (b) return;\n  if (b)\n    return;\n  while (b) f(b);\n}\n"),
            vec![
                ("curly_braces_in_flow_control_structures", "return;".to_string()),
                ("curly_braces_in_flow_control_structures", "f(b);".to_string()),
            ]
        );
        assert_eq!(achados("void f(bool b) {\n  if (b) {} else ;\n}\n"), vec![
            ("avoid_empty_else", ";".to_string()),
            ("curly_braces_in_flow_control_structures", ";".to_string()),
        ]);
    }

    #[test]
    fn diretivas_e_typedefs() {
        assert_eq!(achados("import '../lib/a.dart';\n"), vec![("avoid_relative_lib_imports", "'../lib/a.dart'".to_string())]);
        assert!(achados("import 'package:a/lib/a.dart';\n").is_empty());
        let relatos = {
            let fonte = "typedef int F(int x);\n";
            let mut nomes = Interner::new();
            let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
            executar(Unidade { ast: &p.ast, unit: &p.unit, fonte }, &nomes, &|_| true)
        };
        assert_eq!(relatos.len(), 1);
        assert_eq!(relatos[0].args, vec!["int Function(int x)".to_string()]);
        assert_eq!(achados("@deprecated\nvoid f() {}\n"), vec![("provide_deprecation_message", "@deprecated".to_string())]);
    }
}

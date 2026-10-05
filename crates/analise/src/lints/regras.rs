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
//! `non_constant_identifier_names` segue os doze visitantes do original
//! (também os campos de record, o nome do construtor de tipo de extensão, a
//! variável de `for`/`for-in` de coleção, os parâmetros de `Function(…)`, a
//! variável de padrão de declaração sem palavra-chave, o atalho `:nome`
//! fora e, em augmentation, só os parâmetros nomeados pulados);
//! `prefer_generic_function_type_aliases` monta a sugestão com o `toSource`
//! (`dartforge_frontend::fonte`); `provide_deprecation_message` pede o
//! elemento da anotação (`dartforge_types::anotacoes`, só com a semântica
//! da unidade) e olha toda anotação da unidade.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, DeclKind, DirectiveKind, ForInTarget, FunctionKind, MemberKind, PatternKind, StmtId, StmtKind, TypedefKind,
};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// Um relato de lint.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// `visitFormalParameterList`: os nomes de todo parâmetro que não é
    /// `this.x` (o `super.x` conta), com os sublinhados valendo; numa lista de
    /// augmentation, os nomeados ficam de fora. As listas aninhadas (de
    /// parâmetro-função) são visitadas sem a regra de augmentation.
    fn parametros(&mut self, lista: &'a [ast::Parameter], de_augmentation: bool) {
        for p in lista {
            if !(de_augmentation && p.kind == ast::ParameterKind::Named)
                && let (false, Some(n)) = (p.this_, p.name)
            {
                self.identificador(n, true);
            }
            if let Some(internos) = &p.function_parameters {
                self.parametros(internos, false);
            }
        }
    }

    /// O nome do campo posicional de tipo record escrito depois do tipo
    /// `t` (a árvore não o guarda): o identificador que o parser consumiu.
    fn nome_posicional(&self, t: ast::TypeId) -> Option<(Span, &'a str)> {
        let fonte = self.u.fonte;
        let b = fonte.as_bytes();
        let i = dartforge_frontend::fonte::pular_brancos(b, self.u.ast.ty(t).span.end);
        let identificador = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
        if !b.get(i).is_some_and(|&c| identificador(c) && !c.is_ascii_digit()) {
            return None;
        }
        let mut j = i;
        while j < b.len() && identificador(b[j]) {
            j += 1;
        }
        Some((Span { start: i, end: j }, &fonte[i..j]))
    }

    /// `non_constant_identifier_names`: os doze visitantes do original.
    fn nomes_nao_constantes(&mut self) {
        let a = self.u.ast;
        // As funções de declarações `augment` (a lista de parâmetros delas
        // pula os nomeados; o nome delas não é olhado).
        let mut aumentadas: HashSet<ast::FunctionId> = HashSet::new();
        for d in a.decls.iter().filter(|d| d.augment) {
            if let DeclKind::Function(f) = &d.kind {
                aumentadas.insert(*f);
            }
        }
        for m in a.members.iter().filter(|m| m.augment) {
            if let MemberKind::Method(f) = &m.kind {
                aumentadas.insert(*f);
            }
        }
        // `visitVariableDeclaration`/`visitVariableDeclarationStatement`:
        // as listas sem `const`, fora de augmentation.
        let mut listas: Vec<&'a ast::VariableList> = Vec::new();
        // `visitForEachPartsWithDeclaration`: a variável do `for-in`.
        let mut do_laco: Vec<ast::Name> = Vec::new();
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) => listas.push(l),
                StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => listas.push(l),
                StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => do_laco.push(*name),
                // `visitCatchClause`.
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
        // O `for` e o `for-in` de coleção.
        fn de_colecao<'b>(el: &'b ast::CollectionElement, listas: &mut Vec<&'b ast::VariableList>, do_laco: &mut Vec<ast::Name>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ast::ForInit::Variables(l)) = init {
                        listas.push(l);
                    }
                    de_colecao(body, listas, do_laco);
                }
                ast::CollectionElement::ForIn { target, body, .. } => {
                    if let ForInTarget::Declared { name, .. } = target {
                        do_laco.push(*name);
                    }
                    de_colecao(body, listas, do_laco);
                }
                ast::CollectionElement::If { then, else_, .. } => {
                    de_colecao(then, listas, do_laco);
                    if let Some(x) = else_ {
                        de_colecao(x, listas, do_laco);
                    }
                }
                _ => {}
            }
        }
        for e in a.exprs.iter() {
            if let ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } = &e.kind {
                for el in elements.iter() {
                    de_colecao(el, &mut listas, &mut do_laco);
                }
            }
        }
        for d in a.decls.iter().filter(|d| !d.augment) {
            if let DeclKind::Variables(l) = &d.kind {
                listas.push(l);
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind {
                listas.push(l);
            }
        }
        for l in listas {
            if !l.const_ {
                for v in l.variables.iter() {
                    self.identificador(v.name, false);
                }
            }
        }
        for n in do_laco {
            self.identificador(n, false);
        }
        // `visitDeclaredVariablePattern` e `visitPatternField`: as variáveis
        // de padrão de declaração (não as do padrão de atribuição), salvo a
        // que é direto o padrão de um campo `:nome`.
        let mut de_atribuicao: HashSet<ast::PatternId> = HashSet::new();
        let mut pilha: Vec<ast::PatternId> = a
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ast::ExprKind::PatternAssign { pattern, .. } => Some(*pattern),
                _ => None,
            })
            .collect();
        while let Some(p) = pilha.pop() {
            if !de_atribuicao.insert(p) {
                continue;
            }
            match &a.pattern(p).kind {
                PatternKind::Or(l, r) | PatternKind::And(l, r) => pilha.extend([*l, *r]),
                PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
                    pilha.push(*x)
                }
                PatternKind::List { elements, .. } => pilha.extend(elements.iter().filter_map(|e| match e {
                    ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => Some(*x),
                    ast::ListPatternElement::Rest(None) => None,
                })),
                PatternKind::Map { entries, .. } => pilha.extend(entries.iter().map(|e| e.value)),
                PatternKind::Record { fields } | PatternKind::Object { fields, .. } => pilha.extend(fields.iter().map(|f| f.pattern)),
                _ => {}
            }
        }
        let mut atalhos: HashSet<ast::PatternId> = HashSet::new();
        for p in a.patterns.iter() {
            if let PatternKind::Record { fields } | PatternKind::Object { fields, .. } = &p.kind {
                for f in fields.iter() {
                    let sp = a.pattern(f.pattern).span;
                    if f.name.is_some_and(|n| n.span.start >= sp.start && n.span.end <= sp.end) {
                        atalhos.insert(f.pattern);
                    }
                }
            }
        }
        for (k, p) in a.patterns.iter().enumerate() {
            let id = ast::PatternId(k as u32);
            if let PatternKind::Variable { name, .. } = &p.kind
                && !de_atribuicao.contains(&id)
                && !atalhos.contains(&id)
            {
                self.identificador(*name, false);
            }
        }
        // `visitExtensionTypeDeclaration`: o nome do construtor da
        // representação.
        for d in a.decls.iter() {
            if let DeclKind::ExtensionType(x) = &d.kind
                && let Some(n) = x.constructor
            {
                self.identificador(n, false);
            }
        }
        // `visitConstructorDeclaration` (fora de augmentation) e a lista de
        // parâmetros do construtor.
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                if !m.augment
                    && let Some(n) = k.name
                {
                    self.identificador(n, true);
                }
                self.parametros(&k.parameters, m.augment);
            }
        }
        // `visitFunctionDeclaration` e `visitMethodDeclaration` (não
        // operador, fora de augmentation), e as listas de parâmetros.
        for (k, f) in a.functions.iter().enumerate() {
            let aumentada = aumentadas.contains(&ast::FunctionId(k as u32));
            if let (Some(n), false, false) = (f.name, f.kind == FunctionKind::Operator, aumentada) {
                self.identificador(n, false);
            }
            if let Some(ps) = &f.parameters {
                self.parametros(ps, aumentada);
            }
        }
        // As listas dos typedefs antigos e dos tipos `Function(…)`;
        // `visitRecordTypeAnnotation`: os nomes dos campos.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                self.parametros(parameters, false);
            }
        }
        for t in a.types.iter() {
            match &t.kind {
                ast::TypeKind::Function { parameters, .. } => self.parametros(parameters, false),
                ast::TypeKind::Record { positional, named } => {
                    for &p in positional.iter() {
                        if let Some((span, texto)) = self.nome_posicional(p)
                            && !e_lower_camel_case(texto)
                        {
                            self.relatar(&c::NON_CONSTANT_IDENTIFIER_NAMES, span, &[texto]);
                        }
                    }
                    for (n, _) in named.iter() {
                        self.identificador(*n, false);
                    }
                }
                _ => {}
            }
        }
        // `visitRecordLiteral`: os rótulos dos campos nomeados.
        for e in a.exprs.iter() {
            if let ast::ExprKind::Record { named, .. } = &e.kind {
                for (n, _) in named.iter() {
                    self.identificador(*n, false);
                }
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

    /// `prefer_generic_function_type_aliases`: o `typedef` da forma antiga
    /// (com o `;` escrito), com a sugestão montada pelo `toSource` do retorno,
    /// dos parâmetros de tipo e dos parâmetros.
    fn typedefs_antigos(&mut self) {
        let a = self.u.ast;
        let fonte = self.u.fonte;
        let interner = self.interner;
        for &d in self.u.unit.declarations.iter() {
            let decl = a.decl(d);
            let DeclKind::Typedef(x) = &decl.kind else { continue };
            let TypedefKind::Legacy { return_type, parameters } = &x.kind else { continue };
            // `node.semicolon.isSynthetic`: sem o `;`, nada.
            if !fonte.get(..decl.span.end).is_some_and(|t| t.trim_end().ends_with(';')) {
                continue;
            }
            let retorno = match return_type {
                Some(t) => format!("{} ", dartforge_frontend::fonte::de_tipo(a, fonte, interner, *t)),
                None => String::new(),
            };
            let tipos = dartforge_frontend::fonte::de_parametros_de_tipo(a, fonte, interner, &x.type_params);
            let lista = dartforge_frontend::fonte::de_parametros(a, fonte, interner, parameters);
            let sugestao = format!("{retorno}Function{tipos}{lista}");
            self.relatar(&c::PREFER_GENERIC_FUNCTION_TYPE_ALIASES, x.name.span, &[sugestao.as_str()]);
        }
    }

    /// `provide_deprecation_message`: toda anotação sem argumentos cujo
    /// elemento é o `deprecated` do `dart:core` (`isDeprecated`). Pede a
    /// semântica da unidade.
    fn deprecados_sem_mensagem(&mut self, sem: Option<&super::Semantica<'_>>) {
        let Some(sem) = sem else { return };
        let interner = self.interner;
        let achados: Vec<Span> = dartforge_frontend::pais::todas_as_anotacoes(self.u.ast, self.u.unit)
            .into_iter()
            .filter(|m| m.arguments.is_none() && dartforge_types::anotacoes::e_deprecated_do_core(sem.program, interner, sem.unidade, m))
            .map(|m| m.span)
            .collect();
        for span in achados {
            self.relatar(&c::PROVIDE_DEPRECATION_MESSAGE, span, &[]);
        }
    }
}

/// Roda as regras ligadas (`ligada(nome)`) sobre uma unidade.
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
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
        ctx.deprecados_sem_mensagem(sem);
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
        executar(u, &nomes, &|_| true, None).into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
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
            executar(Unidade { ast: &p.ast, unit: &p.unit, fonte }, &nomes, &|_| true, None)
        };
        assert_eq!(relatos.len(), 1);
        assert_eq!(relatos[0].args, vec!["int Function(int x)".to_string()]);
        // Sem a semântica da unidade, o elemento da anotação não é conhecido.
        assert!(achados("@deprecated\nvoid f() {}\n").is_empty());
    }
}

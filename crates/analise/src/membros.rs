//! Verificações locais de declarações e membros do `ErrorVerifier` do
//! analyzer 6.11.0 (`src/generated/error_verifier.dart`), as que dependem só
//! da forma escrita e do escopo da biblioteca:
//!
//! * `_checkForConflicting*TypeVariableErrorCodes`: parâmetro de tipo com o
//!   nome da declaração ou de um membro dela
//!   (`conflicting_type_variable_and_container`/`_and_member`);
//! * `visitFieldDeclaration`: `const_instance_field`,
//!   `extension_type_declares_instance_field`, `non_final_field_in_enum`;
//! * `visitMethodDeclaration`/`visitFunctionDeclaration`: setters
//!   (`wrong_number_of_parameters_for_setter`, `non_void_return_for_setter`),
//!   membros de `Object` em extensões e tipos de extensão, membros abstratos
//!   em tipo de extensão;
//! * `visitConstructorDeclaration` e o `ReturnTypeVerifier`:
//!   `return_in_generative_constructor`;
//! * `visitFieldFormalParameter`: `this.x` fora de construtor, em `factory` ou
//!   em construtor redirecionador (`field_initializer_*`).
//!
//! As posições são as do analyzer: o nome (`atToken`) ou o nó inteiro
//! (`atNode`, que começa no comentário de documentação ou na anotação).

use dartforge_diagnostics::{Codigo, Diagnostic, Span, codigos::compile_time_error as c};
use dartforge_elements::model::{DeclRef, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{
    self, DeclKind, FunctionBody, FunctionKind, Initializer, MemberKind, Parameter, StmtId,
    StmtKind, TypeKind, TypedefKind,
};
use dartforge_frontend::features::Feature;
use dartforge_intern::{Interner, SymbolId};

/// Contexto de uma unidade da biblioteca.
struct Ctx<'a> {
    programa: &'a Program,
    nomes: &'a Interner,
    u: UnitId,
    ast: &'a ast::Ast,
    fonte: &'a str,
    saida: Vec<(UnitId, Diagnostic)>,
}

fn diag(codigo: Codigo, span: Span, args: &[&str]) -> Diagnostic {
    Diagnostic::com_codigo(codigo, span, args.iter().copied())
}

/// Os nomes de membro de `Object` (`MethodDeclarationExtension.hasObjectMemberName`).
fn membro_de_object(nome: &str) -> bool {
    matches!(
        nome,
        "==" | "hashCode" | "toString" | "runtimeType" | "noSuchMethod"
    )
}

/// O lexema do nome de um membro (`unary-` é escrito `-`).
fn lexema<'a>(nomes: &'a Interner, f: &ast::Function) -> Option<&'a str> {
    let n = nomes.resolve(f.name?.sym);
    Some(if n == "unary-" { "-" } else { n })
}

/// Começo de um nó anotado (`AnnotatedNodeImpl.beginToken`): o comentário
/// de documentação logo antes, se houver, senão o próprio início.
fn inicio_com_documentacao(fonte: &str, inicio: usize) -> usize {
    let antes = &fonte[..inicio];
    let sem_espaco = antes.trim_end();
    // `/** ... */` imediatamente antes.
    if let Some(sem_fim) = sem_espaco.strip_suffix("*/") {
        if let Some(i) = sem_fim.rfind("/**")
            && !sem_fim[i..].contains("*/")
        {
            return i;
        }
        return inicio;
    }
    // Linhas `///` consecutivas.
    let mut resultado = inicio;
    let mut fim = sem_espaco.len();
    loop {
        let linha_ini = sem_espaco[..fim].rfind('\n').map_or(0, |i| i + 1);
        let linha = sem_espaco[linha_ini..fim].trim_start();
        if linha.starts_with("///") && !linha.starts_with("////") {
            resultado = fim - linha.len();
            if linha_ini == 0 {
                break;
            }
            fim = sem_espaco[..linha_ini].trim_end().len();
            if !sem_espaco[fim..linha_ini].chars().all(char::is_whitespace) {
                break;
            }
        } else {
            break;
        }
    }
    resultado
}

/// O membro que começa em `inicio` vem logo depois do fim de outra
/// declaração (`;`, `}`), da abertura do corpo (`{`) ou de uma anotação? Na
/// recuperação de erro, o nosso parser pode descartar palavras antes do
/// membro (`augment factory A()`, `int new()`) que o do analyzer usa; aí a
/// forma do membro não é a mesma e nada se relata sobre ele.
fn precedido_de_limite(fonte: &str, inicio: usize) -> bool {
    let mut antes = &fonte[..inicio];
    loop {
        let t = antes.trim_end();
        if let Some(sem) = t.strip_suffix("*/") {
            match sem.rfind("/*") {
                Some(i) => {
                    antes = &sem[..i];
                    continue;
                }
                None => return false,
            }
        }
        // Comentário de linha: a última linha que começa (depois de espaços) com `//`.
        let ini_linha = t.rfind('\n').map_or(0, |i| i + 1);
        let linha = &t[ini_linha..];
        if let Some(j) = linha.find("//")
            && !linha[..j].contains(['\'', '"'])
        {
            antes = &t[..ini_linha + j];
            continue;
        }
        return t.is_empty() || t.ends_with([';', '}', '{']);
    }
}

impl Ctx<'_> {
    fn nome(&self, s: SymbolId) -> &str {
        self.nomes.resolve(s)
    }

    fn relatar(&mut self, codigo: Codigo, span: Span, args: &[&str]) {
        self.saida.push((self.u, diag(codigo, span, args)));
    }

    /// `type is VoidType` para uma anotação escrita: `void`, ou um alias
    /// (`typedef V = void;`) que o expande.
    fn e_void(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, prof: u32) -> bool {
        let no = ast_.ty(t);
        match &no.kind {
            TypeKind::Void => true,
            TypeKind::Named { name, args } if args.is_empty() && prof < 8 => {
                let b = match &name[..] {
                    [n] => self.programa.lookup_na_unidade(u, n.sym),
                    [p, n] => self.programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
                    _ => None,
                };
                let Some(Element::Typedef(tid)) = b.and_then(|b| b.getter) else {
                    return false;
                };
                let td = self.programa.typedef(tid);
                let ast_td = &self.programa.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return false;
                };
                match d.kind {
                    TypedefKind::Alias(corpo) => self.e_void(td.decl.unit, ast_td, corpo, prof + 1),
                    TypedefKind::Legacy { .. } => false,
                }
            }
            _ => false,
        }
    }

    /// `_checkForWrongNumberOfParametersForSetter` e
    /// `_checkForNonVoidReturnTypeForSetter`. Um setter sem lista de
    /// parâmetros ganha uma vazia na recuperação do parser.
    fn setter(&mut self, f: &ast::Function) {
        let Some(nome) = f.name else { return };
        let params = f.parameters.as_deref().unwrap_or(&[]);
        if params.len() != 1 || params[0].kind != ast::ParameterKind::Required {
            self.relatar(c::WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER, nome.span, &[]);
        }
        if let Some(t) = f.return_type
            && !self.e_void(self.u, self.ast, t, 0)
        {
            self.relatar(c::NON_VOID_RETURN_FOR_SETTER, self.ast.ty(t).span, &[]);
        }
    }

    /// O `FieldFormalParameter` sem o valor padrão (o `DefaultFormalParameter`
    /// o envolve).
    fn span_sem_padrao(&self, p: &Parameter) -> Span {
        match p.default_value {
            None => p.span,
            Some(v) => {
                let ini = self.ast.expr(v).span.start;
                let fim = p.name.map_or(p.span.start, |n| n.span.end);
                let trecho = self.fonte.get(fim..ini).unwrap_or("");
                let t = trecho.trim_end();
                let t = t
                    .strip_suffix('=')
                    .or_else(|| t.strip_suffix(':'))
                    .unwrap_or(t)
                    .trim_end();
                Span {
                    start: p.span.start,
                    end: fim + t.len(),
                }
            }
        }
    }

    /// `this.x` fora de construtor: nos parâmetros de `ps` e nos das
    /// funções-parâmetro dentro deles.
    fn formais_fora(&mut self, ps: &[Parameter]) {
        for p in ps {
            if p.this_ {
                let s = self.span_sem_padrao(p);
                self.relatar(c::FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR, s, &[]);
            }
            if let Some(inner) = &p.function_parameters {
                self.formais_fora(inner);
            }
        }
    }

    /// `ReturnTypeVerifier.verifyReturnStatement` num construtor gerador:
    /// `return e;` no corpo (não em funções aninhadas).
    fn retornos(&mut self, s: StmtId) {
        let ast_ = self.ast;
        match &ast_.stmt(s).kind {
            StmtKind::Return(Some(e)) => {
                let span = ast_.expr(*e).span;
                self.relatar(c::RETURN_IN_GENERATIVE_CONSTRUCTOR, span, &[]);
            }
            StmtKind::Block(v) => v.iter().for_each(|&x| self.retornos(x)),
            StmtKind::If { then, else_, .. } => {
                self.retornos(*then);
                if let Some(e) = else_ {
                    self.retornos(*e);
                }
            }
            StmtKind::For { body, .. }
            | StmtKind::ForIn { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::DoWhile { body, .. }
            | StmtKind::Labeled { body, .. } => self.retornos(*body),
            StmtKind::Switch { cases, .. } => {
                for caso in cases.iter() {
                    caso.body.iter().for_each(|&x| self.retornos(x));
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.retornos(*body);
                for cc in catches.iter() {
                    self.retornos(cc.body);
                }
                if let Some(f) = finally_ {
                    self.retornos(*f);
                }
            }
            _ => {}
        }
    }

    /// O corpo `=> e;` (`ExpressionFunctionBody`): da seta ao `;`.
    fn corpo_de_expressao(&self, e: ast::ExprId) -> Span {
        let s = self.ast.expr(e).span;
        let antes = &self.fonte[..s.start];
        let inicio = antes.rfind("=>").unwrap_or(s.start);
        let depois = &self.fonte[s.end..];
        let fim = match depois.trim_start().strip_prefix(';') {
            Some(_) => s.end + (depois.len() - depois.trim_start().len()) + 1,
            None => s.end,
        };
        Span {
            start: inicio,
            end: fim,
        }
    }

    /// `_checkForConflictingInitializerErrorCodes`: redirecionamento e
    /// `super(...)` na lista de inicializadores. `superclasse` é o nome que o
    /// analyzer exibe para o supertipo (`None` onde não se sabe aqui).
    fn inicializadores(
        &mut self,
        k: &ast::Constructor,
        container: Container,
        superclasse: Option<&str>,
    ) {
        let e_enum = container == Container::Enum;
        let mut redirecionamentos = 0;
        let mut supers = 0;
        let mut ultimo_super = None;
        for (i, ini) in k.initializers.iter().enumerate() {
            match ini {
                Initializer::Redirect { span, .. } => {
                    if redirecionamentos > 0 {
                        self.relatar(c::MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS, *span, &[]);
                    }
                    redirecionamentos += 1;
                }
                Initializer::Super { span, .. } => {
                    if e_enum {
                        self.relatar(
                            c::SUPER_IN_ENUM_CONSTRUCTOR,
                            Span {
                                start: span.start,
                                end: span.start + 5,
                            },
                            &[],
                        );
                    } else if supers == 1 {
                        self.relatar(c::MULTIPLE_SUPER_INITIALIZERS, *span, &[]);
                    }
                    ultimo_super = Some(i);
                    supers += 1;
                }
                _ => {}
            }
        }
        if redirecionamentos > 0 {
            for ini in k.initializers.iter() {
                match ini {
                    Initializer::Super { span, .. } if !e_enum => {
                        self.relatar(c::SUPER_IN_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    Initializer::Field { span, .. } => {
                        self.relatar(c::FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    Initializer::Assert { span, .. } => {
                        self.relatar(c::ASSERT_IN_REDIRECTING_CONSTRUCTOR, *span, &[]);
                    }
                    _ => {}
                }
            }
        }
        if !e_enum
            && redirecionamentos == 0
            && supers == 1
            && let Some(i) = ultimo_super
            && i + 1 != k.initializers.len()
            && let Initializer::Super {
                span, constructor, ..
            } = &k.initializers[i]
            && let Some(sup) = superclasse
        {
            let nome = match constructor {
                Some(n) => format!("{sup}.{}", self.nome(n.sym)),
                None => sup.to_string(),
            };
            self.relatar(
                c::SUPER_INVOCATION_NOT_LAST,
                Span {
                    start: span.start,
                    end: span.start + 5,
                },
                &[&nome],
            );
        }
    }

    fn construtor(&mut self, k: &ast::Constructor) {
        if !k.factory {
            match &k.body {
                FunctionBody::Expression(e) => {
                    let s = self.corpo_de_expressao(*e);
                    self.relatar(c::RETURN_IN_GENERATIVE_CONSTRUCTOR, s, &[]);
                }
                FunctionBody::Block(b) => self.retornos(*b),
                _ => {}
            }
        }
        let redireciona = k
            .initializers
            .iter()
            .any(|i| matches!(i, Initializer::Redirect { .. }));
        for p in k.parameters.iter() {
            if p.this_ {
                let s = self.span_sem_padrao(p);
                if k.factory {
                    self.relatar(c::FIELD_INITIALIZER_FACTORY_CONSTRUCTOR, s, &[]);
                } else if redireciona {
                    self.relatar(c::FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR, s, &[]);
                }
            }
            if let Some(inner) = &p.function_parameters {
                self.formais_fora(inner);
            }
        }
    }
}

/// `TypeAliasSelfReferenceFinder` (`summary2/type_alias.dart`): o `typedef`
/// chega a si mesmo pelos tipos que escreve e pelos limites dos parâmetros de
/// tipo das classes, mixins e `typedef` que nomeia (enum e tipo de extensão
/// não são seguidos). Só se seguem declarações desta biblioteca (o analyzer
/// segue as do ciclo de bibliotecas em ligação).
struct AchaAutoReferencia<'a> {
    programa: &'a Program,
    lib: LibraryId,
    alvo: DeclRef,
    visitados: Vec<DeclRef>,
    achou: bool,
}

impl AchaAutoReferencia<'_> {
    fn declaracao(&self, u: UnitId, nome: &[ast::Name], escopo: &[SymbolId]) -> Option<DeclRef> {
        let b = match nome {
            [n] if escopo.contains(&n.sym) => return None,
            [n] => self.programa.lookup_na_unidade(u, n.sym),
            [p, n] => self.programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        }?;
        let (lib, decl) = match b.getter? {
            Element::Class(id) => (self.programa.class(id).library, self.programa.class(id).decl?),
            Element::Typedef(id) => (self.programa.typedef(id).library, self.programa.typedef(id).decl),
            _ => return None,
        };
        (lib == self.lib).then_some(decl)
    }

    fn tipo(&mut self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, escopo: &mut Vec<SymbolId>) {
        if self.achou {
            return;
        }
        match &ast_.ty(t).kind {
            TypeKind::Named { name, args } => {
                if let Some(d) = self.declaracao(u, name, escopo) {
                    if d == self.alvo {
                        self.achou = true;
                        return;
                    }
                    if !self.visitados.contains(&d) {
                        self.visitados.push(d);
                        let ast_d = &self.programa.unit(d.unit).ast;
                        match &ast_d.decl(d.decl).kind {
                            DeclKind::Class(c) => self.parametros_de_tipo(d.unit, ast_d, &c.type_params, &mut Vec::new()),
                            DeclKind::Mixin(m) => self.parametros_de_tipo(d.unit, ast_d, &m.type_params, &mut Vec::new()),
                            DeclKind::Typedef(_) => self.typedef(d),
                            _ => {}
                        }
                    }
                }
                for &a in args.iter() {
                    self.tipo(u, ast_, a, escopo);
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = escopo.len();
                self.parametros_de_tipo(u, ast_, type_params, escopo);
                self.parametros(u, ast_, parameters, escopo);
                if let Some(r) = return_type {
                    self.tipo(u, ast_, *r, escopo);
                }
                escopo.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                for &p in positional.iter().chain(named.iter().map(|(_, t)| t)) {
                    self.tipo(u, ast_, p, escopo);
                }
            }
            TypeKind::Void => {}
        }
    }

    /// Os limites (`_typeParameterList`), com os nomes já no escopo.
    fn parametros_de_tipo(&mut self, u: UnitId, ast_: &ast::Ast, tps: &[ast::TypeParameter], escopo: &mut Vec<SymbolId>) {
        escopo.extend(tps.iter().map(|p| p.name.sym));
        for p in tps {
            if let Some(b) = p.bound {
                self.tipo(u, ast_, b, escopo);
            }
        }
    }

    /// `_formalParameterList`: os tipos dos parâmetros simples e das
    /// funções-parâmetro (não os de `this.x`/`super.x`).
    fn parametros(&mut self, u: UnitId, ast_: &ast::Ast, ps: &[Parameter], escopo: &mut Vec<SymbolId>) {
        for p in ps {
            if p.this_ || p.super_ {
                continue;
            }
            let n = escopo.len();
            escopo.extend(p.function_type_params.iter().map(|x| x.name.sym));
            if let Some(t) = p.ty {
                self.tipo(u, ast_, t, escopo);
            }
            if let Some(inner) = &p.function_parameters {
                self.parametros(u, ast_, inner, escopo);
            }
            escopo.truncate(n);
        }
    }

    fn typedef(&mut self, d: DeclRef) {
        let ast_ = &self.programa.unit(d.unit).ast;
        let DeclKind::Typedef(td) = &ast_.decl(d.decl).kind else { return };
        let mut escopo = Vec::new();
        self.parametros_de_tipo(d.unit, ast_, &td.type_params, &mut escopo);
        match &td.kind {
            TypedefKind::Alias(t) => self.tipo(d.unit, ast_, *t, &mut escopo),
            TypedefKind::Legacy { return_type, parameters } => {
                self.parametros(d.unit, ast_, parameters, &mut escopo);
                if let Some(r) = return_type {
                    self.tipo(d.unit, ast_, *r, &mut escopo);
                }
            }
        }
    }
}

/// A espécie da declaração que contém os membros.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Classe,
    Mixin,
    Enum,
    Extensao,
    TipoDeExtensao,
}

/// Os nomes que `getNamedConstructor`/`getMethod`/`getGetter`/`getSetter`
/// acham na declaração.
fn nomes_de_membros(
    ast_: &ast::Ast,
    membros: &[ast::MemberId],
    construtores: bool,
) -> Vec<SymbolId> {
    let mut v = Vec::new();
    for &m in membros {
        match &ast_.member(m).kind {
            MemberKind::Field(vl) => v.extend(vl.variables.iter().map(|x| x.name.sym)),
            MemberKind::Method(f) => {
                let f = ast_.function(*f);
                if f.kind != FunctionKind::Operator
                    && let Some(n) = f.name
                {
                    v.push(n.sym);
                }
            }
            MemberKind::Constructor(k) => {
                if construtores && let Some(n) = k.name {
                    v.push(n.sym);
                }
            }
        }
    }
    v
}

/// Os diagnósticos desta família na biblioteca `lib`.
pub fn verificar(
    programa: &Program,
    lib: LibraryId,
    nomes: &Interner,
) -> Vec<(UnitId, Diagnostic)> {
    let biblioteca = programa.library(lib);
    let curinga = biblioteca.features.tem(Feature::WildcardVariables);
    let mut saida = Vec::new();
    for &u in &biblioteca.units {
        let unidade = programa.unit(u);
        if unidade.role == dartforge_elements::model::UnitRole::Patch {
            continue;
        }
        let mut cx = Ctx {
            programa,
            nomes,
            u,
            ast: &unidade.ast,
            fonte: &unidade.source,
            saida: Vec::new(),
        };
        let ast_ = &unidade.ast;
        for &id in &unidade.unit.declarations {
            let decl = ast_.decl(id);
            let (container, nome, tps, membros, extra): (
                Container,
                Option<ast::Name>,
                &[ast::TypeParameter],
                &[ast::MemberId],
                Vec<SymbolId>,
            ) = match &decl.kind {
                DeclKind::Class(d) => (
                    Container::Classe,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::Mixin(d) => (
                    Container::Mixin,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::Enum(d) => (
                    Container::Enum,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    d.constants.iter().map(|k| k.name.sym).collect(),
                ),
                DeclKind::Extension(d) => (
                    Container::Extensao,
                    d.name,
                    &d.type_params,
                    &d.members,
                    Vec::new(),
                ),
                DeclKind::ExtensionType(d) => (
                    Container::TipoDeExtensao,
                    Some(d.name),
                    &d.type_params,
                    &d.members,
                    vec![d.representation_name.sym],
                ),
                DeclKind::Function(f) => {
                    let f = ast_.function(*f);
                    if f.kind == FunctionKind::Setter {
                        cx.setter(f);
                    }
                    continue;
                }
                DeclKind::Typedef(td) => {
                    // `_checkForTypeAliasCannotReferenceItself`.
                    let alvo = DeclRef { unit: u, decl: id };
                    let mut achador = AchaAutoReferencia { programa, lib, alvo, visitados: Vec::new(), achou: false };
                    achador.typedef(alvo);
                    if achador.achou {
                        cx.relatar(c::TYPE_ALIAS_CANNOT_REFERENCE_ITSELF, td.name.span, &[]);
                    }
                    continue;
                }
                _ => continue,
            };

            let primario = match &decl.kind {
                DeclKind::Class(d) => d.primary_constructor,
                DeclKind::Enum(d) => d.primary_constructor,
                _ => None,
            };
            // `enclosingClass.supertype.element.displayName` de uma classe.
            let superclasse: Option<String> = match &decl.kind {
                DeclKind::Class(d) => match d.extends {
                    None => Some("Object".to_string()),
                    Some(t) => match &ast_.ty(t).kind {
                        TypeKind::Named { name, .. } => {
                            let b = match &name[..] {
                                [n] => programa.lookup_na_unidade(u, n.sym),
                                [p, n] => programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
                                _ => None,
                            };
                            match b.filter(|b| !b.ambiguous).and_then(|b| b.getter) {
                                Some(Element::Class(id)) => {
                                    Some(nomes.resolve(programa.class(id).name).to_string())
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                },
                _ => None,
            };
            // Parâmetros de tipo em conflito (só na declaração, não nas augmentations).
            if !decl.augment {
                let construtores =
                    matches!(container, Container::Classe | Container::TipoDeExtensao);
                let mut membros_nomes = nomes_de_membros(ast_, membros, construtores);
                membros_nomes.extend(extra.iter().copied());
                for tp in tps {
                    let texto = cx.nome(tp.name.sym).to_string();
                    let pula_curinga = curinga
                        && texto == "_"
                        && !matches!(container, Container::Enum | Container::Extensao);
                    if pula_curinga {
                        continue;
                    }
                    let (c_container, c_membro) = match container {
                        Container::Classe => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_CLASS,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_CLASS,
                        ),
                        Container::Mixin => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_MIXIN,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_MIXIN,
                        ),
                        Container::Enum => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_ENUM,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_ENUM,
                        ),
                        Container::Extensao => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_EXTENSION,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION,
                        ),
                        Container::TipoDeExtensao => (
                            c::CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE,
                            c::CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION_TYPE,
                        ),
                    };
                    if nome.is_some_and(|n| n.sym == tp.name.sym) {
                        cx.relatar(c_container, tp.name.span, &[&texto]);
                    }
                    if membros_nomes.contains(&tp.name.sym) {
                        cx.relatar(c_membro, tp.name.span, &[&texto]);
                    }
                }
            }

            for &m in membros {
                let membro = ast_.member(m);
                match &membro.kind {
                    MemberKind::Field(vl) => {
                        if !vl.static_ && vl.const_ {
                            // A palavra `const` antes do tipo ou do primeiro nome.
                            let ate = vl
                                .ty
                                .map(|t| ast_.ty(t).span.start)
                                .or(vl.variables.first().map(|v| v.name.span.start));
                            if let Some(ate) = ate {
                                let ini = membro
                                    .metadata
                                    .last()
                                    .map_or(membro.span.start, |a| a.span.end);
                                if let Some(i) =
                                    cx.fonte.get(ini..ate).and_then(|t| t.rfind("const"))
                                {
                                    let s = Span {
                                        start: ini + i,
                                        end: ini + i + 5,
                                    };
                                    cx.relatar(c::CONST_INSTANCE_FIELD, s, &[]);
                                }
                            }
                        }
                        if container == Container::TipoDeExtensao && !vl.static_ && !vl.external {
                            for v in vl.variables.iter() {
                                cx.relatar(
                                    c::EXTENSION_TYPE_DECLARES_INSTANCE_FIELD,
                                    v.name.span,
                                    &[],
                                );
                            }
                        }
                        if container == Container::Enum
                            && !vl.static_
                            && !vl.final_
                            && let Some(v) = vl.variables.first()
                        {
                            cx.relatar(c::NON_FINAL_FIELD_IN_ENUM, v.name.span, &[]);
                        }
                    }
                    MemberKind::Method(fid) => {
                        let f = ast_.function(*fid);
                        if f.kind == FunctionKind::Setter {
                            cx.setter(f);
                        }
                        let Some(nome_m) = f.name else { continue };
                        let lex = lexema(nomes, f).unwrap_or("");
                        if membro_de_object(lex) {
                            match container {
                                Container::Extensao => cx.relatar(
                                    c::EXTENSION_DECLARES_MEMBER_OF_OBJECT,
                                    nome_m.span,
                                    &[],
                                ),
                                Container::TipoDeExtensao => cx.relatar(
                                    c::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT,
                                    nome_m.span,
                                    &[],
                                ),
                                _ => {}
                            }
                        }
                        if container == Container::TipoDeExtensao
                            && !f.static_
                            && !f.external
                            && matches!(f.body, FunctionBody::Empty)
                            && let Some(n) = nome
                        {
                            let ini = inicio_com_documentacao(cx.fonte, membro.span.start);
                            let s = Span {
                                start: ini,
                                end: membro.span.end,
                            };
                            let tipo = cx.nome(n.sym).to_string();
                            cx.relatar(c::EXTENSION_TYPE_WITH_ABSTRACT_MEMBER, s, &[lex, &tipo]);
                        }
                    }
                    MemberKind::Constructor(k) => {
                        if k.parte_primaria
                            || primario == Some(m)
                            || !precedido_de_limite(cx.fonte, membro.span.start)
                        {
                            continue;
                        }
                        cx.construtor(k);
                        cx.inicializadores(k, container, superclasse.as_deref());
                    }
                }
            }
        }
        // `this.x` em qualquer função que não seja construtor (métodos,
        // funções de topo e locais, expressões de função).
        for f in &ast_.functions {
            if let Some(ps) = &f.parameters {
                cx.formais_fora(ps);
            }
        }
        saida.append(&mut cx.saida);
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;
    use std::path::PathBuf;

    /// Um SDK mínimo com o `dart:core`.
    fn sdk(raiz: &std::path::Path) -> SdkLayout {
        let _ = fs::remove_dir_all(raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(
            raiz.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        fs::write(
            raiz.join("sdk/lib/core/core.dart"),
            "class Object {} class int {} class String {} class Null {} class bool {}",
        )
        .unwrap();
        SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap()
    }

    /// `(código, trecho, mensagem)` de cada diagnóstico de `fonte`.
    fn rodar(nome: &str, fonte: &str) -> Vec<(String, String, String)> {
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/membros-{nome}-{}",
            std::process::id()
        ));
        let sdk = sdk(&raiz);
        let entrada = raiz.join("main.dart");
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let mut v: Vec<(String, String, String)> =
            verificar(&programa, programa.entry.unwrap(), &nomes)
                .into_iter()
                .map(|(_, d)| {
                    (
                        d.code.map_or("", |c| c.info().nome).to_string(),
                        fonte[d.span.start..d.span.end].to_string(),
                        d.message,
                    )
                })
                .collect();
        v.sort();
        fs::remove_dir_all(&raiz).unwrap();
        v
    }

    fn t(c: &str, trecho: &str, m: &str) -> (String, String, String) {
        (c.to_string(), trecho.to_string(), m.to_string())
    }

    /// Casos do corpus do analyzer, com o que o `dart analyze` 3.6.2 relata
    /// (`corpus/diagnosticos/analyzer/oraculo.jsonl`).
    #[test]
    fn parametros_de_tipo_em_conflito() {
        let v = rodar(
            "tv",
            "// @dart = 3.6\nclass A<_> {\n  _() {}\n}\nenum E<E> {\n  v\n}\nextension X<T> on int { int get T => 0; }\nmixin M<M> {}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "conflicting_type_variable_and_container",
                    "E",
                    "'E' can't be used to name both a type parameter and the enum in which the type parameter is defined."
                ),
                t(
                    "conflicting_type_variable_and_container",
                    "M",
                    "'M' can't be used to name both a type parameter and the mixin in which the type parameter is defined."
                ),
                t(
                    "conflicting_type_variable_and_member",
                    "T",
                    "'T' can't be used to name both a type parameter and a member in this extension."
                ),
                t(
                    "conflicting_type_variable_and_member",
                    "_",
                    "'_' can't be used to name both a type parameter and a member in this class."
                ),
            ]
        );
    }

    #[test]
    fn campos() {
        let v = rodar(
            "campos",
            "mixin C {\n  const int f = 0;\n}\nenum E { v; int a = 0, b = 1; static int s = 0; final int f = 0; }\nextension type T(int it) { int x = 0; external int y; static int z = 0; }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "const_instance_field",
                    "const",
                    "Only static fields can be declared as const."
                ),
                t(
                    "extension_type_declares_instance_field",
                    "x",
                    "Extension types can't declare instance fields."
                ),
                t(
                    "non_final_field_in_enum",
                    "a",
                    "Enums can only declare final fields."
                ),
            ]
        );
    }

    #[test]
    fn formais_de_campo_fora_de_construtor() {
        let v = rodar(
            "formais",
            "class A {\n  int x = 0;\n  m([this.x = 0]) {}\n  A.named() {}\n  A(this.x) : this.named();\n  factory A.f(this.x) => throw 0;\n  A.g(void f(this.x));\n}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "field_initializer_factory_constructor",
                    "this.x",
                    "Initializing formal parameters can't be used in factory constructors."
                ),
                t(
                    "field_initializer_outside_constructor",
                    "this.x",
                    "Initializing formal parameters can only be used in constructors."
                ),
                t(
                    "field_initializer_outside_constructor",
                    "this.x",
                    "Initializing formal parameters can only be used in constructors."
                ),
                t(
                    "field_initializer_redirecting_constructor",
                    "this.x",
                    "The redirecting constructor can't have a field initializer."
                ),
            ]
        );
    }

    #[test]
    fn membros_de_object_e_abstratos_em_extensoes() {
        let v = rodar(
            "object",
            "extension type E(int it) {\n  int get hashCode => 0;\n  /// doc\n  void foo();\n  static void bar() {}\n}\nextension X on int { String toString() => ''; }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "extension_declares_member_of_object",
                    "toString",
                    "Extensions can't declare members with the same name as a member declared by 'Object'."
                ),
                t(
                    "extension_type_declares_member_of_object",
                    "hashCode",
                    "Extension types can't declare members with the same name as a member declared by 'Object'."
                ),
                t(
                    "extension_type_with_abstract_member",
                    "/// doc\n  void foo();",
                    "'foo' must have a method body because 'E' is an extension type."
                ),
            ]
        );
    }

    /// `setter/declaration_test.dart` e `getter/syntax_get_set_syntax_test.dart`
    /// da linguagem: tipo de retorno que não é `void` (alias de `void` vale)
    /// e número de parâmetros.
    #[test]
    fn setters() {
        let v = rodar(
            "setters",
            "typedef V = void;\nint set a(int x) {}\nV set b(int x) {}\nset c(int x, int y) {}\nclass K { void set d([int? x]) {} dynamic set e(x) {} }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "non_void_return_for_setter",
                    "dynamic",
                    "The return type of the setter must be 'void' or absent."
                ),
                t(
                    "non_void_return_for_setter",
                    "int",
                    "The return type of the setter must be 'void' or absent."
                ),
                t(
                    "wrong_number_of_parameters_for_setter",
                    "c",
                    "Setters must declare exactly one required positional parameter."
                ),
                t(
                    "wrong_number_of_parameters_for_setter",
                    "d",
                    "Setters must declare exactly one required positional parameter."
                ),
            ]
        );
    }

    /// `return_in_generative_constructor` do corpus: `=> e;` inteiro, e a
    /// expressão de `return e;` no bloco (não em funções aninhadas).
    #[test]
    fn retorno_em_construtor_gerador() {
        let v = rodar(
            "retorno",
            "class A {\n  A() => A.b();\n  A.b() { if (true) return 0; return; }\n  A.c() { f() { return 1; } }\n  factory A.d() => A();\n}\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "return_in_generative_constructor",
                    "0",
                    "Constructors can't return values."
                ),
                t(
                    "return_in_generative_constructor",
                    "=> A.b();",
                    "Constructors can't return values."
                ),
            ]
        );
    }

    /// A lista de inicializadores (`_checkForConflictingInitializerErrorCodes`),
    /// nas formas de `variable/initializer_super_last_test.dart` e do corpus
    /// do analyzer (oráculo 3.6.2).
    #[test]
    fn lista_de_inicializadores() {
        let v = rodar(
            "inicializadores",
            "class S { S(); S.named(); }\nclass C extends S {\n  int x = 0;\n  C.a(int x) : super.named(), x = x;\n  C() : this.a(0), this.b();\n  C.b() : this(), x = 1, assert(true), super();\n  C.c() : super(), super();\n}\nclass D { int y; D() : super(), y = 0; }\nenum E { v; const E() : super(); }\n",
        );
        assert_eq!(
            v,
            vec![
                t(
                    "assert_in_redirecting_constructor",
                    "assert(true)",
                    "A redirecting constructor can't have an 'assert' initializer."
                ),
                t(
                    "field_initializer_redirecting_constructor",
                    "x = 1",
                    "The redirecting constructor can't have a field initializer."
                ),
                t(
                    "multiple_redirecting_constructor_invocations",
                    "this.b()",
                    "Constructors can have only one 'this' redirection, at most."
                ),
                t(
                    "multiple_super_initializers",
                    "super()",
                    "A constructor can have at most one 'super' initializer."
                ),
                t(
                    "super_in_enum_constructor",
                    "super",
                    "The enum constructor can't have a 'super' initializer."
                ),
                t(
                    "super_in_redirecting_constructor",
                    "super()",
                    "The redirecting constructor can't have a 'super' initializer."
                ),
                t(
                    "super_invocation_not_last",
                    "super",
                    "The superconstructor call must be last in an initializer list: 'Object'."
                ),
                t(
                    "super_invocation_not_last",
                    "super",
                    "The superconstructor call must be last in an initializer list: 'S.named'."
                ),
            ]
        );
    }

    /// Membro que a recuperação do nosso parser separou das palavras que o
    /// precedem (`int new() => 1;`, `augment factory A() => …`): o analyzer
    /// o vê com outra forma, e nada se relata.
    #[test]
    fn construtor_de_recuperacao_nao_relata() {
        assert!(precedido_de_limite("class A {\n  // x\n  /* y */ A()", 27));
        assert!(!precedido_de_limite("class A {\n  int new()", 16));
    }

    /// `type_alias_cannot_reference_itself` (corpus, oráculo 3.6.2): direto,
    /// por outro `typedef`, pelo limite de um parâmetro de tipo de classe; o
    /// parâmetro de tipo com o mesmo nome esconde o `typedef`.
    #[test]
    fn typedef_que_referencia_a_si_mesmo() {
        let v = rodar(
            "typedef",
            "typedef F = void Function(F);\ntypedef G = List<H>;\ntypedef H = G Function();\nclass C<T extends K> {}\ntypedef K = C;\ntypedef L<L> = L Function();\ntypedef int M(M x);\ntypedef N = List<int>;\n",
        );
        let m = "Typedefs can't reference themselves directly or recursively via another typedef.";
        assert_eq!(
            v,
            vec![
                t("type_alias_cannot_reference_itself", "F", m),
                t("type_alias_cannot_reference_itself", "G", m),
                t("type_alias_cannot_reference_itself", "H", m),
                t("type_alias_cannot_reference_itself", "K", m),
                t("type_alias_cannot_reference_itself", "M", m),
            ]
        );
    }
}

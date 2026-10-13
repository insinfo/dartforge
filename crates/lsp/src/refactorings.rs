//! As refatorações do servidor do Dart 3.6.2 na listagem de
//! `textDocument/codeAction` (docs/LSP-ESPECIFICACAO.md §13.11.1 e a parte A
//! de §13.11.11): o `isAvailable()` de cada uma, sobre a árvore na forma do
//! analyzer (a resolvida: as chamadas de construtor sem `new` são
//! `InstanceCreationExpression`) e os elementos resolvidos dos
//! identificadores.
//!
//! | Refatoração | Regra |
//! | --- | --- |
//! | `Move '…' to file` | `nodeCovering`, `_selectedNodes` e `_membersToMove`, com as subclasses de `sealed` (§13.11.9 b) |
//! | `Extract Method` | `_checkSelection` com o `_ExtractMethodAnalyzer` inteiro (§13.11.3 b) |
//! | `Extract Local Variable` | `_checkSelection` do `ExtractLocalRefactoringImpl` (§13.11.4 b) |
//! | `Inline Local Variable` | `NodeLocator(offset)` com `LocalVariableElement` (§13.11.5 b) |
//! | `Inline Method` | `NodeLocator(offset)` com executável não sintético, não operador, não gerador (§13.11.6 b) |
//! | `Convert Getter to Method` | `getElementOfNode`: getter explícito do workspace (§13.11.7 b) |
//! | `Convert Method to Getter` | `getElementOfNode`: função de topo ou método do workspace, não `void`, sem parâmetros (§13.11.8 b) |
//!
//! O `Extract Widget` exige o Flutter e as três experimentais exigem a
//! configuração `dart.experimentalRefactors` (§13.11.10): nenhuma é listada.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::{self, Arvore, Marca};
use crate::project::Project;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, Ast, AsyncModifier, DeclKind, ExprId, ExprKind, UnaryOp};
use dartforge_frontend::token::{Kind, Token};
use dartforge_types::{MemberRef, Resolved, Type, UnitBodyTypes};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Uma refatoração listada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refactoring {
    pub title: String,
    /// `CodeActionKind` (`refactor.extract`, `refactor.move`…).
    pub kind: &'static str,
    pub command: RefactoringCommand,
}

/// O comando da ação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefactoringCommand {
    /// `refactor.perform` com o `RefactoringKind` (`EXTRACT_METHOD`…).
    Legacy(&'static str),
    /// `dart.refactor.move_top_level_to_file`, com o arquivo padrão.
    Move { path_pattern: PathBuf },
}

/// Um `refactor.perform`/`refactor.validate` (§13.11.2), com os offsets já
/// em bytes.
#[derive(Debug, Clone)]
pub struct RefactoringRequest {
    /// O `RefactoringKind` (`EXTRACT_METHOD`…).
    pub kind: String,
    pub offset: usize,
    pub length: usize,
    pub options: Option<serde_json::Map<String, serde_json::Value>>,
    /// `refactor.validate`: só as condições iniciais.
    pub only_validate: bool,
}

/// O resultado de uma refatoração executada.
#[derive(Debug, Clone)]
pub enum RefactoringResult {
    /// O arquivo não é analisado (`FileNotAnalyzed`).
    NotAnalyzed,
    /// O `RefactoringStatus` tem erro (`hasError`): a mensagem dele.
    Error(String),
    /// `ComputeStatusFailure` do framework novo, com o motivo.
    Failure(Option<String>),
    /// `InvalidCommandArguments`.
    InvalidArguments(String),
    /// `UnhandledError` (uma exceção no servidor do Dart).
    InternalError(String),
    /// A validação passou.
    Valid,
    /// A mudança: as edições e o arquivo criado (URI e conteúdo).
    Change { edits: Vec<crate::Edit>, create: Option<(String, String)> },
}

/// O elemento de um nó, nas espécies que as regras distinguem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Elem {
    None,
    /// `LocalVariableElement` (local, de `for-in`, de `catch`, de padrão).
    LocalVariable,
    Parameter,
    /// `FunctionElement` de uma função local.
    LocalFunction(ast::FunctionId),
    /// Função de topo, método, acessor, operador ou construtor.
    Function(FunctionElementId),
    /// Campo ou variável de topo (o acessor sintético).
    Variable,
    /// `PrefixElement`.
    Prefix,
    Other,
}

const DIRECTIVES: &[&str] = &[
    "ImportDirective",
    "ExportDirective",
    "PartDirective",
    "PartOfDirective",
    "LibraryDirective",
    "AugmentationImportDirective",
];

const FOR_PARTS: &[&str] = &["ForPartsWithDeclarations", "ForPartsWithExpression", "ForPartsWithPattern"];

use dartforge_analysis::arvore_analyzer::{COMANDOS, EXPRESSOES};

/// As subclasses de `FunctionBody`.
const BODIES: &[&str] = &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"];

/// As subclasses de `StringLiteral`.
const STRINGS: &[&str] = &["SimpleStringLiteral", "StringInterpolation", "AdjacentStrings"];

/// As subclasses de `CompilationUnitMember`.
const UNIT_MEMBERS: &[&str] = &[
    "ClassDeclaration",
    "MixinDeclaration",
    "EnumDeclaration",
    "ExtensionDeclaration",
    "ExtensionTypeDeclaration",
    "FunctionDeclaration",
    "TopLevelVariableDeclaration",
    "ClassTypeAlias",
    "FunctionTypeAlias",
    "GenericTypeAlias",
];

/// A espécie é uma subclasse de `Statement`.
pub(crate) fn is_command_kind(e: &str) -> bool {
    COMANDOS.contains(&e)
}

/// A espécie é uma subclasse de `Expression`.
pub(crate) fn is_expression_kind(e: &str) -> bool {
    EXPRESSOES.contains(&e)
}

/// `Character.isWhitespace` do `countLeadingWhitespaces`.
fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// O `RefactoringStatus` da análise de seleção: o primeiro fatal (nenhum
/// passo depois dele o desfaz nem o troca).
pub(crate) struct Analysis {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) fatal: Option<&'static str>,
    pub(crate) selected: Vec<usize>,
    pub(crate) coverage: Option<usize>,
}

/// O que o `_checkSelection` do Extract Method escolheu.
#[derive(Debug, Clone)]
pub(crate) struct MethodSelection {
    /// `_selectionRange`.
    pub(crate) range: (usize, usize),
    /// `_selectionExpression`.
    pub(crate) expression: Option<usize>,
    /// `_selectionFunctionExpression`.
    pub(crate) closure: Option<usize>,
    /// `_selectionStatements`.
    pub(crate) commands: Vec<usize>,
}

/// O que o `_checkSelection` do Extract Local escolheu.
#[derive(Debug, Clone)]
pub(crate) struct LocalSelection {
    /// `selectionRange` (o da expressão única, ou o aparado).
    pub(crate) range: (usize, usize),
    /// `singleExpression`.
    pub(crate) expression: Option<usize>,
    /// `stringLiteralPart` (o texto da seleção original).
    pub(crate) string_part: Option<String>,
    /// `coveringFunctionBody`.
    pub(crate) body: usize,
}

/// A unidade preparada para as regras.
pub(crate) struct Context<'p> {
    pub(crate) p: &'p Project,
    pub(crate) unit: UnitId,
    pub(crate) source: &'p str,
    pub(crate) ast: &'p Ast,
    pub(crate) bodies: &'p UnitBodyTypes,
    pub(crate) tree: Arvore,
    pub(crate) comments: Vec<Span>,
    pub(crate) tokens: Vec<Token>,
    /// Alvos de atribuição e operandos de `++`/`--` (o `writeOrReadElement`
    /// deles é o de escrita).
    pub(crate) writes: HashSet<ExprId>,
    /// O alvo de cada chamada sem `new` que invoca construtor, com o
    /// construtor.
    pub(crate) constructors_by_target: HashMap<ExprId, FunctionElementId>,
    pub(crate) prefixes: HashSet<ExprId>,
}

impl<'p> Context<'p> {
    pub(crate) fn new(p: &'p Project, unit: UnitId) -> Context<'p> {
        let prog = p.program();
        let u = prog.unit(unit);
        let ast = &u.ast;
        let source = u.source.as_str();
        let bodies = &p.query.bodies.units[unit.0 as usize];
        let library = u.library;
        let mut constructors: HashSet<ExprId> = HashSet::new();
        let mut constructors_by_target = HashMap::new();
        let mut prefixes = HashSet::new();
        let mut writes = HashSet::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, .. } => {
                    if let Some(Resolved::Constructor(f)) = bodies.get_resolved(id) {
                        constructors.insert(id);
                        let target_ = match &ast.expr(*target).kind {
                            ExprKind::TypeArguments { target, .. } => *target,
                            _ => *target,
                        };
                        constructors_by_target.insert(target_, *f);
                    }
                }
                ExprKind::InstanceCreation { keyword: None, constructor: None, .. } => {
                    // `A<T>()`: o parser já a dá como criação; é chamada de
                    // função genérica só quando resolve para outra coisa.
                    if !matches!(bodies.get_resolved(id), Some(r) if !matches!(r, Resolved::Constructor(_))) {
                        constructors.insert(id);
                    }
                }
                ExprKind::Identifier(n) => {
                    let prefix = match bodies.get_resolved(id) {
                        Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => true,
                        None => crate::project::is_prefix(prog, library, n.sym),
                        _ => false,
                    };
                    if prefix {
                        prefixes.insert(id);
                    }
                }
                ExprKind::Assign { target, .. } => {
                    writes.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    writes.insert(*operand);
                }
                _ => {}
            }
        }
        let before_3 = prog.library(library).features.versao().major < 3;
        let tree = arvore_analyzer::construir_resolvida(source, ast, &u.unit, before_3, &constructors, &prefixes);
        Context {
            p,
            unit,
            source,
            ast,
            bodies,
            tree,
            comments: dartforge_frontend::comentarios::Comentarios::de(source).todos().to_vec(),
            tokens: dartforge_frontend::lexer::lex(source).unwrap_or_default(),
            writes,
            constructors_by_target,
            prefixes,
        }
    }

    // -- A árvore ---------------------------------------------------------------

    pub(crate) fn kind(&self, n: usize) -> &'static str {
        self.tree.nos[n].especie
    }

    pub(crate) fn parent(&self, n: usize) -> Option<usize> {
        self.tree.nos[n].pai
    }

    pub(crate) fn children(&self, n: usize) -> &[usize] {
        &self.tree.nos[n].filhos
    }

    pub(crate) fn is_expression(&self, n: usize) -> bool {
        EXPRESSOES.contains(&self.kind(n))
    }

    pub(crate) fn is_command(&self, n: usize) -> bool {
        COMANDOS.contains(&self.kind(n))
    }

    /// `n` e os ancestrais (`withParents`).
    pub(crate) fn with_parents(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(n), move |&k| self.parent(k))
    }

    /// O `methodName` de uma `MethodInvocation`: o último `SimpleIdentifier`
    /// filho.
    pub(crate) fn method_name(&self, n: usize) -> Option<usize> {
        self.children(n).iter().rev().copied().find(|&f| self.kind(f) == "SimpleIdentifier")
    }

    // -- Os elementos -------------------------------------------------------------

    /// O elemento do identificador de `x` (a expressão da [`Mark::Expr`]):
    /// `is_static` dá o `staticElement` (nulo numa escrita de propriedade ou
    /// de variável de topo); senão o `writeOrReadElement` (o setter numa
    /// escrita).
    pub(crate) fn element(&self, x: ExprId, is_static: bool) -> Elem {
        let prog = self.p.program();
        if let Some(&f) = self.constructors_by_target.get(&x) {
            // `A.n(…)`: o nome do construtor; `A(…)`, `p.A(…)`: o tipo.
            return match &self.ast.expr(x).kind {
                ExprKind::Property { target, name, .. } if !self.prefixes.contains(target) && prog.function(f).name == name.sym => Elem::Function(f),
                _ => Elem::Other,
            };
        }
        let write = self.writes.contains(&x);
        let r = match self.bodies.get_resolved(x) {
            Some(r) => r,
            None => {
                return if self.prefixes.contains(&x) { Elem::Prefix } else { Elem::None };
            }
        };
        match r {
            Resolved::Local(_) | Resolved::Parameter { .. } => match self.bodies.declaracao_local(x) {
                Some(d) => self.local_element(d),
                None if matches!(r, Resolved::Parameter { .. }) => Elem::Parameter,
                None => Elem::LocalVariable,
            },
            Resolved::Element(Element::Function(f))
            | Resolved::Member { member: MemberRef::Function(f), .. }
            | Resolved::ExtensionMember { member: f, .. } => {
                let fe = prog.function(*f);
                if fe.kind == FunctionKind::ImplicitAccessor {
                    return if write && is_static { Elem::None } else { Elem::Variable };
                }
                if write {
                    if is_static {
                        return Elem::None;
                    }
                    return match self.setter_of(*f) {
                        Some(s) if prog.function(s).kind == FunctionKind::ImplicitAccessor => Elem::Variable,
                        Some(s) => Elem::Function(s),
                        None => Elem::None,
                    };
                }
                Elem::Function(*f)
            }
            Resolved::Element(Element::Variable(_)) | Resolved::Member { member: MemberRef::Variable(_), .. } => {
                if write && is_static {
                    Elem::None
                } else {
                    Elem::Variable
                }
            }
            Resolved::Element(Element::Prefix(..)) | Resolved::Prefix(_) => Elem::Prefix,
            Resolved::Constructor(f) => Elem::Function(*f),
            _ => Elem::Other,
        }
    }

    /// O elemento local declarado em `d` (o offset do nome): parâmetro,
    /// função local ou variável local.
    pub(crate) fn local_element(&self, d: usize) -> Elem {
        if crate::project::parameter_in(self.ast, d).is_some() {
            return Elem::Parameter;
        }
        if let Some(i) = self.ast.functions.iter().position(|f| f.name.is_some_and(|n| n.span.start == d)) {
            return Elem::LocalFunction(ast::FunctionId(i as u32));
        }
        Elem::LocalVariable
    }

    /// O setter que faz par com o acessor `f` (o próprio, se já é setter).
    pub(crate) fn setter_of(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let prog = self.p.program();
        let fe = prog.function(f);
        if fe.kind == FunctionKind::Setter {
            return Some(f);
        }
        let base = self.p.name(fe.name).trim_end_matches('=');
        let members: Vec<FunctionElementId> = if let Some(c) = fe.class {
            let c = prog.class(c);
            c.instance_members.values().chain(c.static_members.values()).copied().collect()
        } else if let Some(x) = fe.extension {
            let x = prog.extension(x);
            x.instance_members.values().chain(x.static_members.values()).copied().collect()
        } else {
            return match prog.lookup(fe.library, fe.name).and_then(|b| b.setter) {
                Some(Element::Function(s)) => Some(s),
                _ => None,
            };
        };
        members.into_iter().find(|&m| {
            let me = prog.function(m);
            matches!(me.kind, FunctionKind::Setter | FunctionKind::ImplicitAccessor)
                && self.p.name(me.name).trim_end_matches('=') == base
                && (me.kind == FunctionKind::Setter || self.p.name(me.name).ends_with('='))
        })
    }

    /// O elemento de um `SimpleIdentifier` da árvore.
    pub(crate) fn identifier_element(&self, n: usize, is_static: bool) -> Elem {
        match self.tree.nos[n].marca {
            Marca::Expr(x) => self.element(x, is_static),
            _ => self.combinator_element(n).unwrap_or(Elem::None),
        }
    }

    /// O elemento de um nome de `show`/`hide` (o `element` que o analyzer
    /// dá ao identificador do combinador): o nome no namespace exportado da
    /// biblioteca que a diretiva importa ou exporta.
    fn combinator_element(&self, n: usize) -> Option<Elem> {
        let c = self.parent(n).filter(|&p| matches!(self.kind(p), "ShowCombinator" | "HideCombinator"))?;
        self.parent(c)?;
        let prog = self.p.program();
        let u = prog.unit(self.unit);
        let (start, end) = (self.tree.nos[n].inicio, self.tree.nos[n].fim);
        // A diretiva que contém o nome.
        let d = u.unit.directives.iter().position(|x| x.span.start <= start && end <= x.span.end)?;
        let lib = prog.library(u.library);
        let target = lib
            .imports
            .iter()
            .filter(|i| i.unit == self.unit && i.directive == d)
            .map(|i| i.library)
            .chain(lib.exports.iter().filter(|e| e.unit == self.unit && e.directive == d).map(|e| e.library))
            .next()?;
        let sym = self.p.query.names.lookup(self.node_text(n))?;
        let b = prog.library(target).exported.get(&sym)?;
        Some(match b.getter.or(b.setter)? {
            Element::Function(f) if prog.function(f).kind == FunctionKind::ImplicitAccessor => Elem::Variable,
            Element::Function(f) => Elem::Function(f),
            Element::Variable(_) => Elem::Variable,
            _ => Elem::Other,
        })
    }

    /// O `declaredElement` de uma `FunctionDeclaration`/`MethodDeclaration`.
    pub(crate) fn declared_element(&self, n: usize) -> Elem {
        let Marca::Funcao(fid) = self.tree.nos[n].marca else { return Elem::None };
        if self.kind(n) == "FunctionDeclaration" && self.parent(n).is_some_and(|p| self.kind(p) == "FunctionDeclarationStatement") {
            return Elem::LocalFunction(fid);
        }
        match self.p.node_function(self.unit, fid) {
            Some(f) => Elem::Function(f),
            None => Elem::None,
        }
    }

    /// `server.getElementOfNode(node)` com o `ElementLocator`
    /// (`AN:src/dart/ast/element_locator.dart`), nas espécies de nó que
    /// podem dar um executável.
    pub(crate) fn node_element(&self, n: usize) -> Elem {
        match self.kind(n) {
            "SimpleIdentifier" => {
                if self.parent(n).is_some_and(|p| self.kind(p) == "LibraryIdentifier") {
                    return Elem::Other;
                }
                self.identifier_element(n, false)
            }
            "FunctionDeclaration" | "MethodDeclaration" => self.declared_element(n),
            "MethodInvocation" => match self.method_name(n) {
                Some(m) => self.identifier_element(m, true),
                None => Elem::None,
            },
            "PrefixedIdentifier" => match self.children(n).get(1) {
                Some(&i) => self.identifier_element(i, true),
                None => Elem::None,
            },
            "PrefixExpression" => match self.tree.nos[n].marca {
                Marca::Operador(x) => match self.bodies.get_resolved(x) {
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => Elem::Function(*f),
                    _ => Elem::None,
                },
                _ => Elem::None,
            },
            _ => Elem::Other,
        }
    }

    pub(crate) fn executable(&self, e: Elem) -> bool {
        matches!(e, Elem::Function(_) | Elem::LocalFunction(_))
    }

    pub(crate) fn synthetic(&self, e: Elem) -> bool {
        match e {
            Elem::Function(f) => matches!(self.p.program().function(f).kind, FunctionKind::ImplicitAccessor | FunctionKind::SyntheticConstructor),
            _ => false,
        }
    }

    pub(crate) fn operator(&self, e: Elem) -> bool {
        matches!(e, Elem::Function(f) if self.p.program().function(f).kind == FunctionKind::Operator)
    }

    pub(crate) fn generator(&self, e: Elem) -> bool {
        let prog = self.p.program();
        let modifier = match e {
            Elem::LocalFunction(fid) => Some(self.ast.function(fid).modifier),
            Elem::Function(f) => match prog.function(f).node {
                FunctionRef::Function { unit, function } => Some(prog.unit(unit).ast.function(function).modifier),
                _ => None,
            },
            _ => None,
        };
        matches!(modifier, Some(AsyncModifier::SyncStar | AsyncModifier::AsyncStar))
    }

    /// `FunctionElement` (função de topo ou local).
    pub(crate) fn is_function(&self, e: Elem) -> bool {
        match e {
            Elem::LocalFunction(_) => true,
            Elem::Function(f) => {
                let fe = self.p.program().function(f);
                fe.kind == FunctionKind::Function && fe.class.is_none() && fe.extension.is_none()
            }
            _ => false,
        }
    }

    /// `MethodElement` (método ou operador de classe, mixin, enum ou
    /// extensão).
    pub(crate) fn is_method(&self, e: Elem) -> bool {
        match e {
            Elem::Function(f) => {
                let fe = self.p.program().function(f);
                matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) && (fe.class.is_some() || fe.extension.is_some())
            }
            _ => false,
        }
    }

    /// O arquivo do elemento está sob uma raiz analisada
    /// (`workspace.containsElement`).
    pub(crate) fn in_workspace(&self, e: Elem) -> bool {
        match e {
            Elem::Function(f) => self.p.of_project(self.p.program().function(f).library),
            Elem::LocalFunction(_) => self.p.of_project(self.p.program().unit(self.unit).library),
            _ => false,
        }
    }

    /// O tipo de retorno é `void`.
    pub(crate) fn returns_void(&self, e: Elem) -> bool {
        let table = &self.p.query.table;
        let t = match e {
            Elem::Function(f) => match self.p.query.outline.functions.get(f.0 as usize) {
                Some(d) => d.return_type,
                None => return false,
            },
            Elem::LocalFunction(fid) => {
                let Some(name) = self.ast.function(fid).name else { return false };
                match self.bodies.tipo_local(name.span.start).map(|t| table.get(t)) {
                    Some(Type::Function { ret, .. }) => *ret,
                    _ => return false,
                }
            }
            _ => return false,
        };
        matches!(table.get(t), Type::Void)
    }

    pub(crate) fn without_parameters(&self, e: Elem) -> bool {
        match e {
            Elem::Function(f) => self.p.query.outline.functions.get(f.0 as usize).is_some_and(|d| d.parameters.is_empty()),
            Elem::LocalFunction(fid) => self.ast.function(fid).parameters.as_ref().is_none_or(|ps| ps.is_empty()),
            _ => false,
        }
    }

    // -- Texto ------------------------------------------------------------------

    /// `TokenUtils.getTokens(text).isNotEmpty`: há algum token (comentários
    /// não são tokens).
    pub(crate) fn has_tokens(&self, start: usize, end: usize) -> bool {
        if end <= start {
            return false;
        }
        let text = &self.source[start..end];
        match dartforge_frontend::lexer::lex(text) {
            Ok(v) => v.iter().any(|t| t.kind != Kind::Eof),
            Err(_) => !text.trim().is_empty(),
        }
    }

    /// `_isJustWhitespaceOrComment`.
    pub(crate) fn only_blank_or_comment(&self, start: usize, end: usize) -> bool {
        if end <= start {
            return true;
        }
        let t = self.source[start..end].trim();
        if t.is_empty() {
            return true;
        }
        match dartforge_frontend::lexer::lex(t) {
            Ok(v) => !v.iter().any(|x| x.kind != Kind::Eof),
            Err(_) => false,
        }
    }

    // -- O analisador de seleção do Extract Method -------------------------------

    /// `_ExtractMethodAnalyzer(unit, [start, end)).analyze()`.
    pub(crate) fn analyze(&self, start: usize, end: usize) -> Analysis {
        let mut st = Analysis { start, end, fatal: None, selected: Vec::new(), coverage: None };
        self.visit(0, &mut st);
        st
    }

    /// `invalidSelection`: grava o primeiro fatal e zera os selecionados.
    pub(crate) fn invalidate(&self, st: &mut Analysis, message: &'static str) {
        if st.fatal.is_none() {
            st.fatal = Some(message);
        }
        st.selected.clear();
    }

    /// O despacho do `GeneralizingAstVisitor` com os `visitX` do
    /// `StatementAnalyzer` e do `_ExtractMethodAnalyzer`.
    pub(crate) fn visit(&self, n: usize, st: &mut Analysis) {
        if st.fatal.is_some() {
            return;
        }
        let no = &self.tree.nos[n];
        let kind = no.especie;
        if DIRECTIVES.contains(&kind) {
            // `visitDirective`: a seleção não pode tocar a diretiva (com
            // comprimento), e não desce.
            if !(st.end <= no.inicio) && !(st.start >= no.fim) {
                self.invalidate(st, "Cannot extract a directive.");
                return;
            }
        } else if kind == "ImportPrefixReference" {
            // `visitImportPrefixReference`: sempre que visitado.
            self.invalidate(st, "Cannot extract an import prefix.");
            return;
        } else if FOR_PARTS.contains(&kind) {
            // `visitForParts`: só os filhos, sem o `visitNode`.
            for &f in &no.filhos {
                self.visit(f, st);
            }
            return;
        }
        self.visit_node(n, st);
        if st.fatal.is_some() {
            return;
        }
        if let Some(m) = self.after_children(n, st) {
            self.invalidate(st, m);
        }
    }

    /// `SelectionAnalyzer.visitNode`.
    pub(crate) fn visit_node(&self, n: usize, st: &mut Analysis) {
        let no = &self.tree.nos[n];
        let (ri, rf) = (no.inicio, no.fim);
        let (si, sf) = (st.start, st.end);
        if si <= ri && rf <= sf {
            // A seleção cobre o nó inteiro.
            if st.selected.is_empty() {
                st.selected.push(n);
            } else {
                let first_parent = self.parent(st.selected[0]);
                if first_parent == no.pai {
                    st.selected.push(n);
                }
                // `_checkParent`.
                if !self.with_parents(n).any(|x| Some(x) == first_parent) {
                    self.invalidate(st, "Not all selected statements are enclosed by the same parent statement.");
                }
            }
            return;
        }
        if ri <= si && sf <= rf {
            st.coverage = Some(n);
        } else if ri <= si && si <= rf {
            // `handleSelectionStartsIn`: nada.
        } else if ri <= sf && sf <= rf {
            // `handleSelectionEndsIn`.
            self.invalidate(
                st,
                "The selection does not cover a set of statements or an expression. Extend selection to a valid range.",
            );
            return;
        } else {
            return;
        }
        for &f in &no.filhos {
            self.visit(f, st);
            if st.fatal.is_some() {
                return;
            }
        }
    }

    /// As verificações que cada `visitX` faz depois do `super`, com a
    /// mensagem do fatal.
    pub(crate) fn after_children(&self, n: usize, st: &Analysis) -> Option<&'static str> {
        let first = st.selected.first().copied();
        let is_first = |x: usize| first == Some(x);
        let children = self.children(n);
        match self.kind(n) {
            "CompilationUnit" => {
                if st.selected.is_empty() {
                    return None;
                }
                // A seleção não começa nem termina num comentário.
                for c in &self.comments {
                    if c.start <= st.start && st.start <= c.end {
                        return Some("Selection begins inside a comment.");
                    }
                    if c.start < st.end && st.end < c.end {
                        return Some("Selection ends inside a comment.");
                    }
                }
                let a = self.tree.nos[st.selected[0]].inicio;
                let b = self.tree.nos[*st.selected.last().unwrap()].fim;
                if self.has_tokens(st.start, a) {
                    return Some("The beginning of the selection contains characters that do not belong to a statement.");
                }
                if self.has_tokens(b, st.end) {
                    return Some("The end of the selection contains characters that do not belong to a statement.");
                }
                None
            }
            "AssignmentExpression" => children.first().is_some_and(|&l| is_first(l)).then_some("Cannot extract the left-hand side of an assignment."),
            "CatchClauseParameter" => is_first(n).then_some("Cannot extract the name part of a declaration."),
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => {
                is_first(n).then_some("Cannot extract a constructor initializer. Select expression part of initializer.")
            }
            "FormalParameterList" => is_first(n).then_some("Cannot extract a parameter list."),
            "FunctionDeclaration" => is_first(n).then_some("Cannot extract a function declaration."),
            "FunctionExpression" => {
                (is_first(n) && self.parent(n).is_some_and(|p| self.kind(p) == "FunctionDeclaration")).then_some("Cannot extract a function declaration.")
            }
            "GenericFunctionType" => is_first(n).then_some("Cannot extract a single type reference."),
            "NamedType" => (is_first(n) || (st.coverage == Some(n) && st.end != st.start)).then_some("Cannot extract a single type reference."),
            "SimpleIdentifier" if is_first(n) => self.identifier_not_extractable(n),
            "VariableDeclaration" => is_first(n).then_some("Cannot extract a variable declaration fragment. Select whole declaration statement."),
            "ForStatement" => self.invalid_for(n, st),
            "DoStatement" => children
                .first()
                .is_some_and(|b| st.selected.contains(b))
                .then_some("Operation not applicable to a 'do' statement's body and expression."),
            "WhileStatement" => match children {
                [c, b, ..] if st.selected.contains(c) && st.selected.contains(b) => {
                    Some("Operation not applicable to a while statement's expression and body.")
                }
                _ => None,
            },
            "SwitchStatement" => st
                .selected
                .iter()
                .any(|s| children.contains(s) && matches!(self.kind(*s), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"))
                .then_some("Selection must either cover whole switch statement or parts of a single case block."),
            "TryStatement" => {
                let pr = first?;
                let body = children.first().copied();
                let finally = children.last().copied().filter(|&f| children.len() > 1 && self.kind(f) == "Block");
                let invalid = Some(pr) == body
                    || Some(pr) == finally
                    || children.iter().filter(|&&c| self.kind(c) == "CatchClause").any(|&c| {
                        let fc = self.children(c);
                        pr == c
                            || fc.last().is_some_and(|&b| self.kind(b) == "Block" && b == pr)
                            || fc.iter().find(|&&x| self.kind(x) == "CatchClauseParameter").is_some_and(|&x| x == pr)
                    });
                invalid.then_some("Selection must either cover whole try statement or parts of try, catch, or finally block.")
            }
            _ => None,
        }
    }

    /// `visitSimpleIdentifier` do `_ExtractMethodAnalyzer`, com o nó como
    /// primeiro selecionado: a primeira das mensagens que se aplica.
    pub(crate) fn identifier_not_extractable(&self, n: usize) -> Option<&'static str> {
        if self.tree.nos[n].marca == Marca::ContextoDeDeclaracao {
            return Some("Cannot extract the name part of a declaration.");
        }
        let e = self.identifier_element(n, false);
        if self.is_function(e) || self.is_method(e) {
            return Some("Cannot extract a single method name.");
        }
        if e == Elem::Prefix {
            return Some("Cannot extract an import prefix.");
        }
        if let Some(p) = self.parent(n) {
            if self.kind(p) == "PrefixedIdentifier" && self.children(p).get(1) == Some(&n) {
                return Some("Cannot extract name part of a property access.");
            }
            if self.kind(p) == "NamedType" {
                return Some("Cannot extract a single type reference.");
            }
        }
        None
    }

    /// As regras de `ForStatement`: as do `StatementAnalyzer` (no `super`)
    /// e depois as do `_ExtractMethodAnalyzer`.
    pub(crate) fn invalid_for(&self, n: usize, st: &Analysis) -> Option<&'static str> {
        let children = self.children(n);
        let &parts = children.first()?;
        if !FOR_PARTS.contains(&self.kind(parts)) {
            return None;
        }
        let p = self.tree.partes_de_for.get(&parts)?;
        let sel = &st.selected;
        let contains = |x: Option<usize>| x.is_some_and(|x| sel.contains(&x));
        let start = contains(p.inicio);
        let condition = contains(p.condicao);
        let updates = p.atualizacoes.iter().any(|u| sel.contains(u));
        let body = contains(children.get(1).copied());
        if start && condition {
            return Some("Operation not applicable to a 'for' statement's initializer and condition.");
        }
        if condition && updates {
            return Some("Operation not applicable to a 'for' statement's condition and updaters.");
        }
        if updates && body {
            return Some("Operation not applicable to a 'for' statement's updaters and body.");
        }
        if self.kind(parts) == "ForPartsWithDeclarations" && p.inicio.is_some() && sel.first().copied() == p.inicio {
            return Some("Cannot extract initialization part of a 'for' statement.");
        }
        sel.last().is_some_and(|l| p.atualizacoes.contains(l)).then_some("Cannot extract increment part of a 'for' statement.")
    }

    // -- As regras ----------------------------------------------------------------

    /// `ExtractMethodRefactoringImpl._checkSelection()`.
    pub(crate) fn method_selection(&self, o: usize, l: usize) -> Result<MethodSelection, &'static str> {
        if o == 0 {
            return Err("The selection offset must be greater than zero.");
        }
        if o + l >= self.source.len() {
            return Err("The selection end offset must be less than the length of the file.");
        }
        if l == 0
            && let Some(f) = self.implicit_closure(o)
        {
            let no = &self.tree.nos[f];
            return Ok(MethodSelection { range: (no.inicio, no.fim), expression: None, closure: Some(f), commands: Vec::new() });
        }
        let st = self.analyze(o, o + l);
        if let Some(m) = st.fatal {
            return Err(m);
        }
        let mut selected = st.selected;
        let mut range = (o, o + l);
        if selected.is_empty() {
            let mut current = st.coverage;
            while let Some(k) = current {
                if self.is_command(k) {
                    break;
                }
                if self.is_expression(k) && self.extractable(k) {
                    selected.push(k);
                    range = (self.tree.nos[k].inicio, self.tree.nos[k].fim);
                    break;
                }
                current = self.parent(k);
            }
        }
        if !selected.is_empty() {
            let single = selected[0];
            if selected.len() == 1 && !self.includes_something_outside(range, single) && self.is_expression(single) {
                return Ok(if self.kind(single) == "FunctionExpression" {
                    MethodSelection { range, expression: None, closure: Some(single), commands: Vec::new() }
                } else {
                    MethodSelection { range, expression: Some(single), closure: None, commands: Vec::new() }
                });
            }
            if selected.iter().all(|&k| self.is_command(k)) {
                return Ok(MethodSelection { range, expression: None, closure: None, commands: selected });
            }
        }
        Err("Can only extract a single expression or a set of statements.")
    }

    /// `ExtractMethodRefactoringImpl.isAvailable()`.
    pub(crate) fn extract_method(&self, o: usize, l: usize) -> bool {
        self.method_selection(o, l).is_ok()
    }

    /// `_isExtractable(range.node(n))`: a análise da faixa exata sem
    /// nenhum problema.
    pub(crate) fn extractable(&self, n: usize) -> bool {
        let no = &self.tree.nos[n];
        self.analyze(no.inicio, no.fim).fatal.is_none()
    }

    /// `_selectionIncludesNonWhitespaceOutsideNode`.
    pub(crate) fn includes_something_outside(&self, (si, sf): (usize, usize), n: usize) -> bool {
        let no = &self.tree.nos[n];
        if !(si <= no.inicio && no.fim <= sf) {
            return false;
        }
        !self.only_blank_or_comment(si, no.inicio) || !self.only_blank_or_comment(no.fim, sf)
    }

    /// `_findFunctionExpression`: a closure cujos parênteses contêm o cursor,
    /// ou a do argumento nomeado cujo rótulo tem o cursor.
    pub(crate) fn implicit_closure(&self, o: usize) -> Option<usize> {
        let n = self.tree.localizar_exclusivo(o)?;
        if let Some(f) = self.with_parents(n).find(|&k| self.kind(k) == "FunctionExpression")
            && let Some(&ps) = self.children(f).iter().find(|&&c| self.kind(c) == "FormalParameterList")
            && self.tree.nos[ps].inicio <= o
            && o <= self.tree.nos[ps].fim
            && !self.parent(f).is_some_and(|p| self.kind(p) == "FunctionDeclaration")
        {
            return Some(f);
        }
        if self.kind(n) == "SimpleIdentifier"
            && let Some(label) = self.parent(n)
            && self.kind(label) == "Label"
            && let Some(named) = self.parent(label)
            && self.kind(named) == "NamedExpression"
            && let Some(&expr) = self.children(named).get(1)
            && self.kind(expr) == "FunctionExpression"
        {
            return Some(expr);
        }
        None
    }

    /// `ExtractLocalRefactoringImpl._checkSelection()`.
    pub(crate) fn local_selection(&self, o: usize, l: usize) -> Result<LocalSelection, &'static str> {
        if o == 0 {
            return Err("The selection offset must be greater than zero.");
        }
        if o + l >= self.source.len() {
            return Err("The selection end offset must be less than the length of the file.");
        }
        let selection_text = self.source[o..o + l].to_string();
        let bytes = selection_text.as_bytes();
        let initial = bytes.iter().take_while(|&&b| is_blank(b)).count();
        let finals = bytes.iter().rev().take_while(|&&b| is_blank(b)).count();
        let start = o + initial;
        let end = (o + l).saturating_sub(finals).max(o);
        let coverage = self.tree.localizar(start, end).unwrap_or(0);
        let Some(body) = self.with_parents(coverage).find(|&k| BODIES.contains(&self.kind(k))) else {
            return Err("An expression inside a function must be selected to activate this refactoring.");
        };
        let no = &self.tree.nos[coverage];
        if STRINGS.contains(&no.especie) && end > start && start > no.inicio && end < no.fim {
            return Ok(LocalSelection { range: (start, end), expression: None, string_part: Some(selection_text), body });
        }
        let mut single: Option<usize> = None;
        let mut without_covered = true;
        let mut current = Some(coverage);
        while let Some(k) = current {
            let parent = self.parent(k);
            current = parent;
            let kind = self.kind(k);
            if matches!(kind, "ArgumentList" | "AssignmentExpression" | "NamedExpression" | "TypeArgumentList") {
                continue;
            }
            if matches!(kind, "ConstructorName" | "Label" | "NamedType") {
                single = None;
                without_covered = true;
                continue;
            }
            if let Some(p) = parent
                && ((self.kind(p) == "PrefixedIdentifier" && self.children(p).get(1) == Some(&k))
                    || (self.kind(p) == "PropertyAccess" && self.children(p).last() == Some(&k) && kind == "SimpleIdentifier"))
            {
                continue;
            }
            if !self.is_expression(k) {
                break;
            }
            if kind == "MethodInvocation"
                && let Some(m) = self.method_name(k)
            {
                let e = self.identifier_element(m, true);
                if self.executable(e) && self.returns_void(e) {
                    if single.is_none() {
                        return Err("Cannot extract the void expression.");
                    }
                    break;
                }
            }
            if without_covered {
                if kind == "SimpleIdentifier" {
                    if self.tree.nos[k].marca == Marca::ContextoDeDeclaracao {
                        return Err("Cannot extract the name part of a declaration.");
                    }
                    let e = self.identifier_element(k, true);
                    if self.is_function(e) || self.is_method(e) {
                        continue;
                    }
                }
                if let Some(p) = parent
                    && self.kind(p) == "AssignmentExpression"
                    && self.children(p).first() == Some(&k)
                {
                    return Err("Cannot extract the left-hand side of an assignment.");
                }
            }
            if single.is_none() {
                single = Some(k);
            }
            without_covered = false;
        }
        match single {
            Some(k) => {
                let no = &self.tree.nos[k];
                Ok(LocalSelection { range: (no.inicio, no.fim), expression: Some(k), string_part: None, body })
            }
            None => Err("Expression must be selected to activate this refactoring."),
        }
    }

    /// `ExtractLocalRefactoringImpl.isAvailable()`.
    pub(crate) fn extract_local(&self, o: usize, l: usize) -> bool {
        self.local_selection(o, l).is_ok()
    }

    /// `InlineLocalRefactoringImpl.isAvailable()`.
    pub(crate) fn local_inline(&self, o: usize) -> bool {
        let Some(n) = self.tree.localizar(o, o) else { return false };
        match self.kind(n) {
            "SimpleIdentifier" => self.identifier_element(n, true) == Elem::LocalVariable,
            "VariableDeclaration" => self.tree.nos[n].marca == Marca::VariavelLocal,
            _ => false,
        }
    }

    /// O elemento do `InlineMethodRefactoringImpl._checkOffset`, e se veio
    /// da declaração.
    pub(crate) fn inline_method_element(&self, o: usize) -> Option<(Elem, usize, bool)> {
        let n = self.tree.localizar(o, o)?;
        let (e, declaration) = match self.kind(n) {
            "FunctionDeclaration" | "MethodDeclaration" => (self.declared_element(n), true),
            "SimpleIdentifier" => (self.identifier_element(n, false), false),
            _ => return None,
        };
        Some((e, n, declaration))
    }

    /// `InlineMethodRefactoringImpl.isAvailable()`.
    pub(crate) fn inline_method(&self, o: usize) -> bool {
        let Some((e, _, _)) = self.inline_method_element(o) else { return false };
        self.executable(e) && !self.synthetic(e) && !self.operator(e) && !self.generator(e)
    }

    /// O elemento de `getElementOfNode(NodeLocator(offset))`.
    pub(crate) fn element_at_cursor(&self, o: usize) -> Elem {
        match self.tree.localizar(o, o) {
            Some(n) => self.node_element(n),
            None => Elem::None,
        }
    }

    /// `Convert Getter to Method`: getter explícito do workspace.
    pub(crate) fn convert_getter(&self, o: usize) -> bool {
        let e = self.element_at_cursor(o);
        matches!(e, Elem::Function(f) if self.p.program().function(f).kind == FunctionKind::Getter) && self.in_workspace(e)
    }

    /// `ConvertMethodToGetterRefactoringImpl._checkElement`: a mensagem do
    /// fatal, ou nada.
    pub(crate) fn convert_method_error(&self, e: Elem) -> Option<String> {
        if !self.in_workspace(e) {
            return Some("Only methods in your workspace can be converted.".to_string());
        }
        if matches!(e, Elem::LocalFunction(_)) {
            return Some("Only top-level functions can be converted to getters.".to_string());
        }
        if !self.is_function(e) && !self.is_method(e) {
            return Some("Only class methods or top-level functions can be converted to getters.".to_string());
        }
        if self.returns_void(e) {
            let kind = if self.is_function(e) { "function" } else { "method" };
            return Some(format!("Cannot convert {kind} returning void."));
        }
        if !self.without_parameters(e) {
            return Some("Only methods without parameters can be converted to getters.".to_string());
        }
        None
    }

    /// `Convert Method to Getter`.
    pub(crate) fn convert_method(&self, o: usize) -> bool {
        let e = self.element_at_cursor(o);
        self.executable(e) && self.convert_method_error(e).is_none()
    }

    // -- Move top-level to file ---------------------------------------------------

    /// O token que termina exatamente em `pos` é identificador.
    pub(crate) fn identifier_ends_in(&self, pos: usize) -> bool {
        let i = self.tokens.partition_point(|t| t.span.end < pos);
        self.tokens.get(i).is_some_and(|t| t.span.end == pos && t.kind == Kind::Ident)
    }

    /// O token que começa exatamente em `pos` é identificador.
    pub(crate) fn identifier_starts_in(&self, pos: usize) -> bool {
        let i = self.tokens.partition_point(|t| t.span.start < pos);
        self.tokens.get(i).is_some_and(|t| t.span.start == pos && t.kind == Kind::Ident && t.span.end > t.span.start)
    }

    /// `nodeCovering(offset, length)`
    /// (`AN:src/utilities/extensions/ast.dart:86-141`).
    pub(crate) fn coverage(&self, o: usize, l: usize) -> Option<usize> {
        let end = o + l;
        if end > self.source.len() {
            return None;
        }
        let contains = |k: usize| {
            let no = &self.tree.nos[k];
            if l == 0 {
                if o == no.inicio && self.identifier_ends_in(o) {
                    return false;
                }
                if o == no.fim && self.identifier_starts_in(o) {
                    return false;
                }
            }
            no.inicio <= o && no.fim >= end
        };
        let mut current = 0usize;
        while let Some(f) = self.children(current).iter().copied().find(|&f| contains(f)) {
            current = f;
        }
        Some(current)
    }

    /// A declaração de topo (`DeclId`) do nó membro da unidade.
    pub(crate) fn node_declaration(&self, n: usize) -> Option<ast::DeclId> {
        match self.tree.nos[n].marca {
            Marca::Decl(d) => Some(d),
            Marca::Funcao(fid) => {
                let unit = &self.p.program().unit(self.unit).unit;
                unit.declarations.iter().copied().find(|&d| matches!(self.ast.decl(d).kind, DeclKind::Function(f) if f == fid))
            }
            _ => None,
        }
    }

    /// O nome declarado (o token) do membro.
    pub(crate) fn declaration_name(&self, d: ast::DeclId) -> Option<ast::Name> {
        match &self.ast.decl(d).kind {
            DeclKind::Class(c) => Some(c.name),
            DeclKind::Mixin(m) => Some(m.name),
            DeclKind::Enum(e) => Some(e.name),
            DeclKind::Extension(e) => e.name,
            DeclKind::ExtensionType(e) => Some(e.name),
            DeclKind::Typedef(t) => Some(t.name),
            DeclKind::Function(f) => self.ast.function(*f).name,
            DeclKind::Variables(_) => None,
        }
    }

    /// A classe declarada por `d` desta unidade.
    pub(crate) fn declaration_class(&self, unit: UnitId, d: ast::DeclId) -> Option<ClassId> {
        self.p
            .program()
            .classes
            .iter()
            .position(|c| c.decl.is_some_and(|r| r.unit == unit && r.decl == d))
            .map(|i| ClassId(i as u32))
    }

    /// `ClassElement.isSealed`.
    pub(crate) fn sealed(&self, c: ClassId) -> bool {
        let prog = self.p.program();
        let Some(r) = prog.class(c).decl else { return false };
        matches!(&prog.unit(r.unit).ast.decl(r.decl).kind, DeclKind::Class(cd) if cd.modifiers.sealed)
    }

    /// `sealedSuperclassElements` da declaração `d` da unidade `unit`:
    /// as classes `sealed` de `extends`/`implements`/`with` (classe, não
    /// alias) ou de `implements`/`on` (mixin).
    pub(crate) fn sealed_superclasses(&self, unit: UnitId, d: ast::DeclId) -> Vec<ClassId> {
        let ast = &self.p.program().unit(unit).ast;
        let types: Vec<ast::TypeId> = match &ast.decl(d).kind {
            DeclKind::Class(c) if !c.mixin_application => c.extends.iter().chain(c.implements.iter()).chain(c.with.iter()).copied().collect(),
            DeclKind::Mixin(m) => m.implements.iter().chain(m.on.iter()).copied().collect(),
            _ => Vec::new(),
        };
        types.into_iter().filter_map(|t| self.p.type_class(unit, t)).filter(|&c| self.sealed(c)).collect()
    }

    /// `MoveTopLevelToFile.isAvailable()`: o título e o arquivo padrão.
    pub(crate) fn move_(&self, o: usize, l: usize, create_files: bool) -> Option<(String, PathBuf)> {
        if !create_files {
            return None;
        }
        let candidates = self.members_a_move(o, l)?;
        let prog = self.p.program();
        // O título e o arquivo padrão.
        let count: usize = candidates
            .iter()
            .map(|(d, _)| match &self.ast.decl(*d).kind {
                DeclKind::Variables(vl) => vl.variables.len(),
                _ => 1,
            })
            .sum();
        let first = candidates.first().and_then(|(_, n)| n.clone());
        let title = if count == 1 {
            format!("Move '{}' to file", first.as_deref().unwrap_or("null"))
        } else {
            format!("Move {count} declarations to file")
        };
        let file = match &first {
            Some(n) => file_name(n),
            None => "newFile.dart".to_string(),
        };
        let path = prog.unit(self.unit).path.as_deref()?;
        let folder = path.parent()?;
        Some((title, folder.join(file)))
    }

    /// `_membersToMove()`: as declarações (com o nome) do único grupo, na
    /// ordem de inserção.
    pub(crate) fn members_a_move(&self, o: usize, l: usize) -> Option<Vec<(ast::DeclId, Option<String>)>> {
        let end = o + l;
        let coverage = self.coverage(o, l)?;
        // `_selectedNodes`.
        let in_token = |s: Span| s.start <= o && end <= s.end;
        let selected: Vec<usize> = match self.kind(coverage) {
            "CompilationUnit" => {
                // `nodesInRange`: as diretivas, ou, sem elas, as
                // declarações que a seleção toca com comprimento.
                let children = self.children(0);
                let in_range = |directives: bool| -> Vec<usize> {
                    let list: Vec<usize> = children
                        .iter()
                        .copied()
                        .filter(|&f| DIRECTIVES.contains(&self.kind(f)) == directives && self.kind(f) != "ScriptTag")
                        .collect();
                    let mut first = list.len();
                    while first > 0 && self.tree.nos[list[first - 1]].fim > o {
                        first -= 1;
                    }
                    list[first..].iter().copied().take_while(|&k| self.tree.nos[k].inicio < end).collect()
                };
                let mut nos = in_range(true);
                if nos.is_empty() {
                    nos = in_range(false);
                }
                if nos.is_empty() || nos.iter().any(|&k| !UNIT_MEMBERS.contains(&self.kind(k))) {
                    return None;
                }
                nos
            }
            "VariableDeclaration" => {
                let list = self.parent(coverage)?;
                let decl = self.parent(list)?;
                if self.kind(decl) != "TopLevelVariableDeclaration" {
                    return None;
                }
                let d = self.node_declaration(decl)?;
                let DeclKind::Variables(vl) = &self.ast.decl(d).kind else { return None };
                if vl.variables.len() != 1 || !in_token(vl.variables[0].name.span) {
                    return None;
                }
                vec![decl]
            }
            e if UNIT_MEMBERS.contains(&e) => vec![coverage],
            _ => return None,
        };
        // `_membersToMove`.
        let several = selected.len() > 1;
        let valid = |name: Option<ast::Name>| several || name.is_some_and(|n| in_token(n.span));
        let mut candidates: Vec<(ast::DeclId, Option<String>)> = Vec::new();
        for &k in &selected {
            let d = self.node_declaration(k)?;
            let name = self.declaration_name(d);
            let text = |n: Option<ast::Name>| n.map(|n| self.source[n.span.start..n.span.end].to_string());
            let entry = match self.kind(k) {
                "ClassDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "MixinDeclaration" | "ClassTypeAlias" | "FunctionTypeAlias" | "GenericTypeAlias"
                    if valid(name) =>
                {
                    text(name)
                }
                "FunctionDeclaration" if self.parent(k) == Some(0) && valid(name) => text(name),
                "TopLevelVariableDeclaration" => match &self.ast.decl(d).kind {
                    DeclKind::Variables(vl) if vl.variables.len() == 1 => text(Some(vl.variables[0].name)),
                    _ => None,
                },
                _ => return None,
            };
            candidates.push((d, entry));
        }
        // `_SealedSubclassIndex`.
        let candidate_classes: HashSet<ClassId> = candidates.iter().filter_map(|(d, _)| self.declaration_class(self.unit, *d)).collect();
        let declarations = self.p.program().unit(self.unit).unit.declarations.clone();
        let mut subclasses: HashMap<ClassId, Vec<ast::DeclId>> = HashMap::new();
        for &d in declarations.iter() {
            let class = self.declaration_class(self.unit, d);
            for s in self.sealed_superclasses(self.unit, d) {
                let list = subclasses.entry(s).or_default();
                if !list.contains(&d) {
                    list.push(d);
                }
                if class.is_some_and(|c| candidate_classes.contains(&c)) && !candidate_classes.contains(&s) {
                    return None;
                }
            }
        }
        // `findSubclassesOfSealedRecursively`: os membros e, depois, as
        // subclasses diretas de cada um (o mesmo, recursivamente), na ordem
        // do conjunto do Dart; as novas entram no fim.
        fn find(cx: &Context<'_>, members: &[ast::DeclId], subclasses: &HashMap<ClassId, Vec<ast::DeclId>>, stack: &mut Vec<ast::DeclId>) -> Vec<ast::DeclId> {
            let mut v: Vec<ast::DeclId> = Vec::new();
            for &m in members {
                if !v.contains(&m) {
                    v.push(m);
                }
            }
            for &m in members {
                if stack.contains(&m) {
                    continue;
                }
                let Some(c) = cx.declaration_class(cx.unit, m) else { continue };
                let Some(subs) = subclasses.get(&c) else { continue };
                stack.push(m);
                for x in find(cx, subs, subclasses, stack) {
                    if !v.contains(&x) {
                        v.push(x);
                    }
                }
                stack.pop();
            }
            v
        }
        let original: Vec<ast::DeclId> = candidates.iter().map(|(d, _)| *d).collect();
        for d in find(self, &original, &subclasses, &mut Vec::new()) {
            if !candidates.iter().any(|(x, _)| *x == d) {
                let name = self.declaration_name(d).map(|n| self.source[n.span.start..n.span.end].to_string());
                candidates.push((d, name));
            }
        }
        // Nenhuma outra parte da biblioteca estende uma `sealed` deste
        // arquivo.
        let prog = self.p.program();
        let library = prog.unit(self.unit).library;
        for &u2 in prog.library(library).units.iter() {
            if u2 == self.unit {
                continue;
            }
            for &d in prog.unit(u2).unit.declarations.iter() {
                for s in self.sealed_superclasses(u2, d) {
                    if prog.class(s).decl.is_some_and(|r| r.unit == self.unit) {
                        return None;
                    }
                }
            }
        }
        Some(candidates)
    }
}

/// `String.toFileName`: cada maiúscula que não é a primeira letra vira `_` e
/// a letra; tudo em minúsculas; mais `.dart`.
pub(crate) fn file_name(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 8);
    for (i, c) in name.char_indices() {
        if c.is_ascii_uppercase() && i != 0 {
            s.push('_');
        }
        s.push(c);
    }
    format!("{}.dart", s.to_lowercase())
}

impl Project {
    /// As refatorações listadas em `[offset, offset + length)` da
    /// unidade, na ordem do Dart: Move, Extract Method, Extract Local
    /// Variable, Inline Local Variable, Inline Method, Convert Getter to
    /// Method, Convert Method to Getter. `create_files` é o
    /// `supportsFileCreation` do cliente.
    pub(crate) fn refactorings(&self, unit: UnitId, offset: usize, length: usize, create_files: bool) -> Vec<Refactoring> {
        let cx = Context::new(self, unit);
        let mut v = Vec::new();
        if let Some((title, path_pattern)) = cx.move_(offset, length, create_files) {
            v.push(Refactoring { title, kind: "refactor.move", command: RefactoringCommand::Move { path_pattern } });
        }
        let legacy = |title: &str, kind_: &'static str, kind: &'static str| Refactoring {
            title: title.to_string(),
            kind: kind_,
            command: RefactoringCommand::Legacy(kind),
        };
        if cx.extract_method(offset, length) {
            v.push(legacy("Extract Method", "refactor.extract", "EXTRACT_METHOD"));
        }
        if cx.extract_local(offset, length) {
            v.push(legacy("Extract Local Variable", "refactor.extract", "EXTRACT_LOCAL_VARIABLE"));
        }
        if cx.local_inline(offset) {
            v.push(legacy("Inline Local Variable", "refactor.inline", "INLINE_LOCAL_VARIABLE"));
        }
        if cx.inline_method(offset) {
            v.push(legacy("Inline Method", "refactor.inline", "INLINE_METHOD"));
        }
        if cx.convert_getter(offset) {
            v.push(legacy("Convert Getter to Method", "refactor.rewrite", "CONVERT_GETTER_TO_METHOD"));
        }
        if cx.convert_method(offset) {
            v.push(legacy("Convert Method to Getter", "refactor.rewrite", "CONVERT_METHOD_TO_GETTER"));
        }
        v
    }
}

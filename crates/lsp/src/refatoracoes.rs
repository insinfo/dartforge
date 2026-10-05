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
use crate::projeto::Projeto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, Ast, AsyncModifier, DeclKind, ExprId, ExprKind, UnaryOp};
use dartforge_frontend::token::{Kind, Token};
use dartforge_types::{MemberRef, Resolved, Type, UnitBodyTypes};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Uma refatoração listada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refatoracao {
    pub titulo: String,
    /// `CodeActionKind` (`refactor.extract`, `refactor.move`…).
    pub especie: &'static str,
    pub comando: ComandoDeRefatoracao,
}

/// O comando da ação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComandoDeRefatoracao {
    /// `refactor.perform` com o `RefactoringKind` (`EXTRACT_METHOD`…).
    Legado(&'static str),
    /// `dart.refactor.move_top_level_to_file`, com o arquivo padrão.
    Mover { caminho_padrao: PathBuf },
}

/// Um `refactor.perform`/`refactor.validate` (§13.11.2), com os offsets já
/// em bytes.
#[derive(Debug, Clone)]
pub struct PedidoDeRefatoracao {
    /// O `RefactoringKind` (`EXTRACT_METHOD`…).
    pub kind: String,
    pub offset: usize,
    pub comprimento: usize,
    pub opcoes: Option<serde_json::Map<String, serde_json::Value>>,
    /// `refactor.validate`: só as condições iniciais.
    pub so_validar: bool,
}

/// O resultado de uma refatoração executada.
#[derive(Debug, Clone)]
pub enum ResultadoDeRefatoracao {
    /// O arquivo não é analisado (`FileNotAnalyzed`).
    NaoAnalisado,
    /// O `RefactoringStatus` tem erro (`hasError`): a mensagem dele.
    Erro(String),
    /// `ComputeStatusFailure` do framework novo, com o motivo.
    Falha(Option<String>),
    /// `InvalidCommandArguments`.
    ArgumentosInvalidos(String),
    /// `UnhandledError` (uma exceção no servidor do Dart).
    ErroInterno(String),
    /// A validação passou.
    Valido,
    /// A mudança: as edições e o arquivo criado (URI e conteúdo).
    Mudanca { edicoes: Vec<crate::Edicao>, criar: Option<(String, String)> },
}

/// O elemento de um nó, nas espécies que as regras distinguem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Elem {
    Nenhum,
    /// `LocalVariableElement` (local, de `for-in`, de `catch`, de padrão).
    VariavelLocal,
    Parametro,
    /// `FunctionElement` de uma função local.
    FuncaoLocal(ast::FunctionId),
    /// Função de topo, método, acessor, operador ou construtor.
    Funcao(FunctionElementId),
    /// Campo ou variável de topo (o acessor sintético).
    Variavel,
    /// `PrefixElement`.
    Prefixo,
    Outro,
}

const DIRETIVAS: &[&str] = &[
    "ImportDirective",
    "ExportDirective",
    "PartDirective",
    "PartOfDirective",
    "LibraryDirective",
    "AugmentationImportDirective",
];

const PARTES_DE_FOR: &[&str] = &["ForPartsWithDeclarations", "ForPartsWithExpression", "ForPartsWithPattern"];

/// As subclasses de `Expression` que a árvore produz.
const EXPRESSOES: &[&str] = &[
    "SimpleIdentifier",
    "PrefixedIdentifier",
    "MethodInvocation",
    "PropertyAccess",
    "IntegerLiteral",
    "DoubleLiteral",
    "BooleanLiteral",
    "NullLiteral",
    "SimpleStringLiteral",
    "StringInterpolation",
    "AdjacentStrings",
    "SymbolLiteral",
    "ListLiteral",
    "SetOrMapLiteral",
    "RecordLiteral",
    "InstanceCreationExpression",
    "FunctionExpression",
    "FunctionExpressionInvocation",
    "FunctionReference",
    "IndexExpression",
    "PrefixExpression",
    "PostfixExpression",
    "BinaryExpression",
    "ConditionalExpression",
    "IsExpression",
    "AsExpression",
    "AssignmentExpression",
    "PatternAssignment",
    "CascadeExpression",
    "AwaitExpression",
    "ThrowExpression",
    "RethrowExpression",
    "ThisExpression",
    "SuperExpression",
    "SwitchExpression",
    "ParenthesizedExpression",
    "NamedExpression",
    "DotShorthand",
];

/// As subclasses de `Statement`.
const COMANDOS: &[&str] = &[
    "Block",
    "ExpressionStatement",
    "VariableDeclarationStatement",
    "PatternVariableDeclarationStatement",
    "ReturnStatement",
    "IfStatement",
    "ForStatement",
    "WhileStatement",
    "DoStatement",
    "SwitchStatement",
    "TryStatement",
    "BreakStatement",
    "ContinueStatement",
    "YieldStatement",
    "LabeledStatement",
    "EmptyStatement",
    "AssertStatement",
    "FunctionDeclarationStatement",
];

/// As subclasses de `FunctionBody`.
const CORPOS: &[&str] = &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"];

/// As subclasses de `StringLiteral`.
const STRINGS: &[&str] = &["SimpleStringLiteral", "StringInterpolation", "AdjacentStrings"];

/// As subclasses de `CompilationUnitMember`.
const MEMBROS_DE_UNIDADE: &[&str] = &[
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
pub(crate) fn e_comando_especie(e: &str) -> bool {
    COMANDOS.contains(&e)
}

/// A espécie é uma subclasse de `Expression`.
pub(crate) fn e_expressao_especie(e: &str) -> bool {
    EXPRESSOES.contains(&e)
}

/// `Character.isWhitespace` do `countLeadingWhitespaces`.
fn e_branco(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// O `RefactoringStatus` da análise de seleção: o primeiro fatal (nenhum
/// passo depois dele o desfaz nem o troca).
pub(crate) struct Analise {
    pub(crate) ini: usize,
    pub(crate) fim: usize,
    pub(crate) fatal: Option<&'static str>,
    pub(crate) selecionados: Vec<usize>,
    pub(crate) cobertura: Option<usize>,
}

/// O que o `_checkSelection` do Extract Method escolheu.
#[derive(Debug, Clone)]
pub(crate) struct SelecaoDeMetodo {
    /// `_selectionRange`.
    pub(crate) faixa: (usize, usize),
    /// `_selectionExpression`.
    pub(crate) expressao: Option<usize>,
    /// `_selectionFunctionExpression`.
    pub(crate) closure: Option<usize>,
    /// `_selectionStatements`.
    pub(crate) comandos: Vec<usize>,
}

/// O que o `_checkSelection` do Extract Local escolheu.
#[derive(Debug, Clone)]
pub(crate) struct SelecaoLocal {
    /// `selectionRange` (o da expressão única, ou o aparado).
    pub(crate) faixa: (usize, usize),
    /// `singleExpression`.
    pub(crate) expressao: Option<usize>,
    /// `stringLiteralPart` (o texto da seleção original).
    pub(crate) parte_de_string: Option<String>,
    /// `coveringFunctionBody`.
    pub(crate) corpo: usize,
}

/// A unidade preparada para as regras.
pub(crate) struct Contexto<'p> {
    pub(crate) p: &'p Projeto,
    pub(crate) unidade: UnitId,
    pub(crate) fonte: &'p str,
    pub(crate) ast: &'p Ast,
    pub(crate) corpos: &'p UnitBodyTypes,
    pub(crate) arvore: Arvore,
    pub(crate) comentarios: Vec<Span>,
    pub(crate) tokens: Vec<Token>,
    /// Alvos de atribuição e operandos de `++`/`--` (o `writeOrReadElement`
    /// deles é o de escrita).
    pub(crate) escritas: HashSet<ExprId>,
    /// O alvo de cada chamada sem `new` que invoca construtor, com o
    /// construtor.
    pub(crate) construtores_por_alvo: HashMap<ExprId, FunctionElementId>,
    pub(crate) prefixos: HashSet<ExprId>,
}

impl<'p> Contexto<'p> {
    pub(crate) fn novo(p: &'p Projeto, unidade: UnitId) -> Contexto<'p> {
        let prog = p.programa();
        let u = prog.unit(unidade);
        let ast = &u.ast;
        let fonte = u.source.as_str();
        let corpos = &p.consulta.corpos.units[unidade.0 as usize];
        let biblioteca = u.library;
        let mut construtores: HashSet<ExprId> = HashSet::new();
        let mut construtores_por_alvo = HashMap::new();
        let mut prefixos = HashSet::new();
        let mut escritas = HashSet::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, .. } => {
                    if let Some(Resolved::Constructor(f)) = corpos.get_resolved(id) {
                        construtores.insert(id);
                        let alvo = match &ast.expr(*target).kind {
                            ExprKind::TypeArguments { target, .. } => *target,
                            _ => *target,
                        };
                        construtores_por_alvo.insert(alvo, *f);
                    }
                }
                ExprKind::InstanceCreation { keyword: None, constructor: None, .. } => {
                    // `A<T>()`: o parser já a dá como criação; é chamada de
                    // função genérica só quando resolve para outra coisa.
                    if !matches!(corpos.get_resolved(id), Some(r) if !matches!(r, Resolved::Constructor(_))) {
                        construtores.insert(id);
                    }
                }
                ExprKind::Identifier(n) => {
                    let prefixo = match corpos.get_resolved(id) {
                        Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => true,
                        None => crate::projeto::eh_prefixo(prog, biblioteca, n.sym),
                        _ => false,
                    };
                    if prefixo {
                        prefixos.insert(id);
                    }
                }
                ExprKind::Assign { target, .. } => {
                    escritas.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    escritas.insert(*operand);
                }
                _ => {}
            }
        }
        let antes_de_3 = prog.library(biblioteca).features.versao().major < 3;
        let arvore = arvore_analyzer::construir_resolvida(fonte, ast, &u.unit, antes_de_3, &construtores, &prefixos);
        Contexto {
            p,
            unidade,
            fonte,
            ast,
            corpos,
            arvore,
            comentarios: dartforge_frontend::comentarios::Comentarios::de(fonte).todos().to_vec(),
            tokens: dartforge_frontend::lexer::lex(fonte).unwrap_or_default(),
            escritas,
            construtores_por_alvo,
            prefixos,
        }
    }

    // -- A árvore ---------------------------------------------------------------

    pub(crate) fn especie(&self, n: usize) -> &'static str {
        self.arvore.nos[n].especie
    }

    pub(crate) fn pai(&self, n: usize) -> Option<usize> {
        self.arvore.nos[n].pai
    }

    pub(crate) fn filhos(&self, n: usize) -> &[usize] {
        &self.arvore.nos[n].filhos
    }

    pub(crate) fn e_expressao(&self, n: usize) -> bool {
        EXPRESSOES.contains(&self.especie(n))
    }

    pub(crate) fn e_comando(&self, n: usize) -> bool {
        COMANDOS.contains(&self.especie(n))
    }

    /// `n` e os ancestrais (`withParents`).
    pub(crate) fn com_pais(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(n), move |&k| self.pai(k))
    }

    /// O `methodName` de uma `MethodInvocation`: o último `SimpleIdentifier`
    /// filho.
    pub(crate) fn nome_do_metodo(&self, n: usize) -> Option<usize> {
        self.filhos(n).iter().rev().copied().find(|&f| self.especie(f) == "SimpleIdentifier")
    }

    // -- Os elementos -------------------------------------------------------------

    /// O elemento do identificador de `x` (a expressão da [`Marca::Expr`]):
    /// `estatico` dá o `staticElement` (nulo numa escrita de propriedade ou
    /// de variável de topo); senão o `writeOrReadElement` (o setter numa
    /// escrita).
    pub(crate) fn elemento(&self, x: ExprId, estatico: bool) -> Elem {
        let prog = self.p.programa();
        if let Some(&f) = self.construtores_por_alvo.get(&x) {
            // `A.n(…)`: o nome do construtor; `A(…)`, `p.A(…)`: o tipo.
            return match &self.ast.expr(x).kind {
                ExprKind::Property { target, name, .. } if !self.prefixos.contains(target) && prog.function(f).name == name.sym => Elem::Funcao(f),
                _ => Elem::Outro,
            };
        }
        let escrita = self.escritas.contains(&x);
        let r = match self.corpos.get_resolved(x) {
            Some(r) => r,
            None => {
                return if self.prefixos.contains(&x) { Elem::Prefixo } else { Elem::Nenhum };
            }
        };
        match r {
            Resolved::Local(_) | Resolved::Parameter { .. } => match self.corpos.declaracao_local(x) {
                Some(d) => self.elemento_local(d),
                None if matches!(r, Resolved::Parameter { .. }) => Elem::Parametro,
                None => Elem::VariavelLocal,
            },
            Resolved::Element(Element::Function(f))
            | Resolved::Member { member: MemberRef::Function(f), .. }
            | Resolved::ExtensionMember { member: f, .. } => {
                let fe = prog.function(*f);
                if fe.kind == FunctionKind::ImplicitAccessor {
                    return if escrita && estatico { Elem::Nenhum } else { Elem::Variavel };
                }
                if escrita {
                    if estatico {
                        return Elem::Nenhum;
                    }
                    return match self.setter_de(*f) {
                        Some(s) if prog.function(s).kind == FunctionKind::ImplicitAccessor => Elem::Variavel,
                        Some(s) => Elem::Funcao(s),
                        None => Elem::Nenhum,
                    };
                }
                Elem::Funcao(*f)
            }
            Resolved::Element(Element::Variable(_)) | Resolved::Member { member: MemberRef::Variable(_), .. } => {
                if escrita && estatico {
                    Elem::Nenhum
                } else {
                    Elem::Variavel
                }
            }
            Resolved::Element(Element::Prefix(..)) | Resolved::Prefix(_) => Elem::Prefixo,
            Resolved::Constructor(f) => Elem::Funcao(*f),
            _ => Elem::Outro,
        }
    }

    /// O elemento local declarado em `d` (o offset do nome): parâmetro,
    /// função local ou variável local.
    pub(crate) fn elemento_local(&self, d: usize) -> Elem {
        if crate::projeto::parametro_em(self.ast, d).is_some() {
            return Elem::Parametro;
        }
        if let Some(i) = self.ast.functions.iter().position(|f| f.name.is_some_and(|n| n.span.start == d)) {
            return Elem::FuncaoLocal(ast::FunctionId(i as u32));
        }
        Elem::VariavelLocal
    }

    /// O setter que faz par com o acessor `f` (o próprio, se já é setter).
    pub(crate) fn setter_de(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let prog = self.p.programa();
        let fe = prog.function(f);
        if fe.kind == FunctionKind::Setter {
            return Some(f);
        }
        let base = self.p.nome(fe.name).trim_end_matches('=');
        let membros: Vec<FunctionElementId> = if let Some(c) = fe.class {
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
        membros.into_iter().find(|&m| {
            let me = prog.function(m);
            matches!(me.kind, FunctionKind::Setter | FunctionKind::ImplicitAccessor)
                && self.p.nome(me.name).trim_end_matches('=') == base
                && (me.kind == FunctionKind::Setter || self.p.nome(me.name).ends_with('='))
        })
    }

    /// O elemento de um `SimpleIdentifier` da árvore.
    pub(crate) fn elemento_do_identificador(&self, n: usize, estatico: bool) -> Elem {
        match self.arvore.nos[n].marca {
            Marca::Expr(x) => self.elemento(x, estatico),
            _ => Elem::Nenhum,
        }
    }

    /// O `declaredElement` de uma `FunctionDeclaration`/`MethodDeclaration`.
    pub(crate) fn elemento_declarado(&self, n: usize) -> Elem {
        let Marca::Funcao(fid) = self.arvore.nos[n].marca else { return Elem::Nenhum };
        if self.especie(n) == "FunctionDeclaration" && self.pai(n).is_some_and(|p| self.especie(p) == "FunctionDeclarationStatement") {
            return Elem::FuncaoLocal(fid);
        }
        match self.p.funcao_do_no(self.unidade, fid) {
            Some(f) => Elem::Funcao(f),
            None => Elem::Nenhum,
        }
    }

    /// `server.getElementOfNode(node)` com o `ElementLocator`
    /// (`AN:src/dart/ast/element_locator.dart`), nas espécies de nó que
    /// podem dar um executável.
    pub(crate) fn elemento_do_no(&self, n: usize) -> Elem {
        match self.especie(n) {
            "SimpleIdentifier" => {
                if self.pai(n).is_some_and(|p| self.especie(p) == "LibraryIdentifier") {
                    return Elem::Outro;
                }
                self.elemento_do_identificador(n, false)
            }
            "FunctionDeclaration" | "MethodDeclaration" => self.elemento_declarado(n),
            "MethodInvocation" => match self.nome_do_metodo(n) {
                Some(m) => self.elemento_do_identificador(m, true),
                None => Elem::Nenhum,
            },
            "PrefixedIdentifier" => match self.filhos(n).get(1) {
                Some(&i) => self.elemento_do_identificador(i, true),
                None => Elem::Nenhum,
            },
            "PrefixExpression" => match self.arvore.nos[n].marca {
                Marca::Operador(x) => match self.corpos.get_resolved(x) {
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => Elem::Funcao(*f),
                    _ => Elem::Nenhum,
                },
                _ => Elem::Nenhum,
            },
            _ => Elem::Outro,
        }
    }

    pub(crate) fn executavel(&self, e: Elem) -> bool {
        matches!(e, Elem::Funcao(_) | Elem::FuncaoLocal(_))
    }

    pub(crate) fn sintetico(&self, e: Elem) -> bool {
        match e {
            Elem::Funcao(f) => matches!(self.p.programa().function(f).kind, FunctionKind::ImplicitAccessor | FunctionKind::SyntheticConstructor),
            _ => false,
        }
    }

    pub(crate) fn operador(&self, e: Elem) -> bool {
        matches!(e, Elem::Funcao(f) if self.p.programa().function(f).kind == FunctionKind::Operator)
    }

    pub(crate) fn gerador(&self, e: Elem) -> bool {
        let prog = self.p.programa();
        let modificador = match e {
            Elem::FuncaoLocal(fid) => Some(self.ast.function(fid).modifier),
            Elem::Funcao(f) => match prog.function(f).node {
                FunctionRef::Function { unit, function } => Some(prog.unit(unit).ast.function(function).modifier),
                _ => None,
            },
            _ => None,
        };
        matches!(modificador, Some(AsyncModifier::SyncStar | AsyncModifier::AsyncStar))
    }

    /// `FunctionElement` (função de topo ou local).
    pub(crate) fn e_funcao(&self, e: Elem) -> bool {
        match e {
            Elem::FuncaoLocal(_) => true,
            Elem::Funcao(f) => {
                let fe = self.p.programa().function(f);
                fe.kind == FunctionKind::Function && fe.class.is_none() && fe.extension.is_none()
            }
            _ => false,
        }
    }

    /// `MethodElement` (método ou operador de classe, mixin, enum ou
    /// extensão).
    pub(crate) fn e_metodo(&self, e: Elem) -> bool {
        match e {
            Elem::Funcao(f) => {
                let fe = self.p.programa().function(f);
                matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) && (fe.class.is_some() || fe.extension.is_some())
            }
            _ => false,
        }
    }

    /// O arquivo do elemento está sob uma raiz analisada
    /// (`workspace.containsElement`).
    pub(crate) fn no_workspace(&self, e: Elem) -> bool {
        match e {
            Elem::Funcao(f) => self.p.do_projeto(self.p.programa().function(f).library),
            Elem::FuncaoLocal(_) => self.p.do_projeto(self.p.programa().unit(self.unidade).library),
            _ => false,
        }
    }

    /// O tipo de retorno é `void`.
    pub(crate) fn retorna_void(&self, e: Elem) -> bool {
        let tabela = &self.p.consulta.tabela;
        let t = match e {
            Elem::Funcao(f) => match self.p.consulta.outline.functions.get(f.0 as usize) {
                Some(d) => d.return_type,
                None => return false,
            },
            Elem::FuncaoLocal(fid) => {
                let Some(nome) = self.ast.function(fid).name else { return false };
                match self.corpos.tipo_local(nome.span.start).map(|t| tabela.get(t)) {
                    Some(Type::Function { ret, .. }) => *ret,
                    _ => return false,
                }
            }
            _ => return false,
        };
        matches!(tabela.get(t), Type::Void)
    }

    pub(crate) fn sem_parametros(&self, e: Elem) -> bool {
        match e {
            Elem::Funcao(f) => self.p.consulta.outline.functions.get(f.0 as usize).is_some_and(|d| d.parameters.is_empty()),
            Elem::FuncaoLocal(fid) => self.ast.function(fid).parameters.as_ref().is_none_or(|ps| ps.is_empty()),
            _ => false,
        }
    }

    // -- Texto ------------------------------------------------------------------

    /// `TokenUtils.getTokens(texto).isNotEmpty`: há algum token (comentários
    /// não são tokens).
    pub(crate) fn tem_tokens(&self, ini: usize, fim: usize) -> bool {
        if fim <= ini {
            return false;
        }
        let texto = &self.fonte[ini..fim];
        match dartforge_frontend::lexer::lex(texto) {
            Ok(v) => v.iter().any(|t| t.kind != Kind::Eof),
            Err(_) => !texto.trim().is_empty(),
        }
    }

    /// `_isJustWhitespaceOrComment`.
    pub(crate) fn so_branco_ou_comentario(&self, ini: usize, fim: usize) -> bool {
        if fim <= ini {
            return true;
        }
        let t = self.fonte[ini..fim].trim();
        if t.is_empty() {
            return true;
        }
        match dartforge_frontend::lexer::lex(t) {
            Ok(v) => !v.iter().any(|x| x.kind != Kind::Eof),
            Err(_) => false,
        }
    }

    // -- O analisador de seleção do Extract Method -------------------------------

    /// `_ExtractMethodAnalyzer(unit, [ini, fim)).analyze()`.
    pub(crate) fn analisar(&self, ini: usize, fim: usize) -> Analise {
        let mut st = Analise { ini, fim, fatal: None, selecionados: Vec::new(), cobertura: None };
        self.visitar(0, &mut st);
        st
    }

    /// `invalidSelection`: grava o primeiro fatal e zera os selecionados.
    pub(crate) fn invalidar(&self, st: &mut Analise, mensagem: &'static str) {
        if st.fatal.is_none() {
            st.fatal = Some(mensagem);
        }
        st.selecionados.clear();
    }

    /// O despacho do `GeneralizingAstVisitor` com os `visitX` do
    /// `StatementAnalyzer` e do `_ExtractMethodAnalyzer`.
    pub(crate) fn visitar(&self, n: usize, st: &mut Analise) {
        if st.fatal.is_some() {
            return;
        }
        let no = &self.arvore.nos[n];
        let especie = no.especie;
        if DIRETIVAS.contains(&especie) {
            // `visitDirective`: a seleção não pode tocar a diretiva (com
            // comprimento), e não desce.
            if !(st.fim <= no.inicio) && !(st.ini >= no.fim) {
                self.invalidar(st, "Cannot extract a directive.");
                return;
            }
        } else if especie == "ImportPrefixReference" {
            // `visitImportPrefixReference`: sempre que visitado.
            self.invalidar(st, "Cannot extract an import prefix.");
            return;
        } else if PARTES_DE_FOR.contains(&especie) {
            // `visitForParts`: só os filhos, sem o `visitNode`.
            for &f in &no.filhos {
                self.visitar(f, st);
            }
            return;
        }
        self.visitar_no(n, st);
        if st.fatal.is_some() {
            return;
        }
        if let Some(m) = self.depois_dos_filhos(n, st) {
            self.invalidar(st, m);
        }
    }

    /// `SelectionAnalyzer.visitNode`.
    pub(crate) fn visitar_no(&self, n: usize, st: &mut Analise) {
        let no = &self.arvore.nos[n];
        let (ri, rf) = (no.inicio, no.fim);
        let (si, sf) = (st.ini, st.fim);
        if si <= ri && rf <= sf {
            // A seleção cobre o nó inteiro.
            if st.selecionados.is_empty() {
                st.selecionados.push(n);
            } else {
                let pai_do_primeiro = self.pai(st.selecionados[0]);
                if pai_do_primeiro == no.pai {
                    st.selecionados.push(n);
                }
                // `_checkParent`.
                if !self.com_pais(n).any(|x| Some(x) == pai_do_primeiro) {
                    self.invalidar(st, "Not all selected statements are enclosed by the same parent statement.");
                }
            }
            return;
        }
        if ri <= si && sf <= rf {
            st.cobertura = Some(n);
        } else if ri <= si && si <= rf {
            // `handleSelectionStartsIn`: nada.
        } else if ri <= sf && sf <= rf {
            // `handleSelectionEndsIn`.
            self.invalidar(
                st,
                "The selection does not cover a set of statements or an expression. Extend selection to a valid range.",
            );
            return;
        } else {
            return;
        }
        for &f in &no.filhos {
            self.visitar(f, st);
            if st.fatal.is_some() {
                return;
            }
        }
    }

    /// As verificações que cada `visitX` faz depois do `super`, com a
    /// mensagem do fatal.
    pub(crate) fn depois_dos_filhos(&self, n: usize, st: &Analise) -> Option<&'static str> {
        let primeiro = st.selecionados.first().copied();
        let e_primeiro = |x: usize| primeiro == Some(x);
        let filhos = self.filhos(n);
        match self.especie(n) {
            "CompilationUnit" => {
                if st.selecionados.is_empty() {
                    return None;
                }
                // A seleção não começa nem termina num comentário.
                for c in &self.comentarios {
                    if c.start <= st.ini && st.ini <= c.end {
                        return Some("Selection begins inside a comment.");
                    }
                    if c.start < st.fim && st.fim < c.end {
                        return Some("Selection ends inside a comment.");
                    }
                }
                let a = self.arvore.nos[st.selecionados[0]].inicio;
                let b = self.arvore.nos[*st.selecionados.last().unwrap()].fim;
                if self.tem_tokens(st.ini, a) {
                    return Some("The beginning of the selection contains characters that do not belong to a statement.");
                }
                if self.tem_tokens(b, st.fim) {
                    return Some("The end of the selection contains characters that do not belong to a statement.");
                }
                None
            }
            "AssignmentExpression" => filhos.first().is_some_and(|&l| e_primeiro(l)).then_some("Cannot extract the left-hand side of an assignment."),
            "CatchClauseParameter" => e_primeiro(n).then_some("Cannot extract the name part of a declaration."),
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => {
                e_primeiro(n).then_some("Cannot extract a constructor initializer. Select expression part of initializer.")
            }
            "FormalParameterList" => e_primeiro(n).then_some("Cannot extract a parameter list."),
            "FunctionDeclaration" => e_primeiro(n).then_some("Cannot extract a function declaration."),
            "FunctionExpression" => {
                (e_primeiro(n) && self.pai(n).is_some_and(|p| self.especie(p) == "FunctionDeclaration")).then_some("Cannot extract a function declaration.")
            }
            "GenericFunctionType" => e_primeiro(n).then_some("Cannot extract a single type reference."),
            "NamedType" => (e_primeiro(n) || (st.cobertura == Some(n) && st.fim != st.ini)).then_some("Cannot extract a single type reference."),
            "SimpleIdentifier" if e_primeiro(n) => self.identificador_nao_extraivel(n),
            "VariableDeclaration" => e_primeiro(n).then_some("Cannot extract a variable declaration fragment. Select whole declaration statement."),
            "ForStatement" => self.for_invalido(n, st),
            "DoStatement" => filhos
                .first()
                .is_some_and(|b| st.selecionados.contains(b))
                .then_some("Operation not applicable to a 'do' statement's body and expression."),
            "WhileStatement" => match filhos {
                [c, b, ..] if st.selecionados.contains(c) && st.selecionados.contains(b) => {
                    Some("Operation not applicable to a while statement's expression and body.")
                }
                _ => None,
            },
            "SwitchStatement" => st
                .selecionados
                .iter()
                .any(|s| filhos.contains(s) && matches!(self.especie(*s), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"))
                .then_some("Selection must either cover whole switch statement or parts of a single case block."),
            "TryStatement" => {
                let pr = primeiro?;
                let corpo = filhos.first().copied();
                let finalmente = filhos.last().copied().filter(|&f| filhos.len() > 1 && self.especie(f) == "Block");
                let invalido = Some(pr) == corpo
                    || Some(pr) == finalmente
                    || filhos.iter().filter(|&&c| self.especie(c) == "CatchClause").any(|&c| {
                        let fc = self.filhos(c);
                        pr == c
                            || fc.last().is_some_and(|&b| self.especie(b) == "Block" && b == pr)
                            || fc.iter().find(|&&x| self.especie(x) == "CatchClauseParameter").is_some_and(|&x| x == pr)
                    });
                invalido.then_some("Selection must either cover whole try statement or parts of try, catch, or finally block.")
            }
            _ => None,
        }
    }

    /// `visitSimpleIdentifier` do `_ExtractMethodAnalyzer`, com o nó como
    /// primeiro selecionado: a primeira das mensagens que se aplica.
    pub(crate) fn identificador_nao_extraivel(&self, n: usize) -> Option<&'static str> {
        if self.arvore.nos[n].marca == Marca::ContextoDeDeclaracao {
            return Some("Cannot extract the name part of a declaration.");
        }
        let e = self.elemento_do_identificador(n, false);
        if self.e_funcao(e) || self.e_metodo(e) {
            return Some("Cannot extract a single method name.");
        }
        if e == Elem::Prefixo {
            return Some("Cannot extract an import prefix.");
        }
        if let Some(p) = self.pai(n) {
            if self.especie(p) == "PrefixedIdentifier" && self.filhos(p).get(1) == Some(&n) {
                return Some("Cannot extract name part of a property access.");
            }
            if self.especie(p) == "NamedType" {
                return Some("Cannot extract a single type reference.");
            }
        }
        None
    }

    /// As regras de `ForStatement`: as do `StatementAnalyzer` (no `super`)
    /// e depois as do `_ExtractMethodAnalyzer`.
    pub(crate) fn for_invalido(&self, n: usize, st: &Analise) -> Option<&'static str> {
        let filhos = self.filhos(n);
        let &partes = filhos.first()?;
        if !PARTES_DE_FOR.contains(&self.especie(partes)) {
            return None;
        }
        let p = self.arvore.partes_de_for.get(&partes)?;
        let sel = &st.selecionados;
        let contem = |x: Option<usize>| x.is_some_and(|x| sel.contains(&x));
        let inicio = contem(p.inicio);
        let condicao = contem(p.condicao);
        let atualizacoes = p.atualizacoes.iter().any(|u| sel.contains(u));
        let corpo = contem(filhos.get(1).copied());
        if inicio && condicao {
            return Some("Operation not applicable to a 'for' statement's initializer and condition.");
        }
        if condicao && atualizacoes {
            return Some("Operation not applicable to a 'for' statement's condition and updaters.");
        }
        if atualizacoes && corpo {
            return Some("Operation not applicable to a 'for' statement's updaters and body.");
        }
        if self.especie(partes) == "ForPartsWithDeclarations" && p.inicio.is_some() && sel.first().copied() == p.inicio {
            return Some("Cannot extract initialization part of a 'for' statement.");
        }
        sel.last().is_some_and(|l| p.atualizacoes.contains(l)).then_some("Cannot extract increment part of a 'for' statement.")
    }

    // -- As regras ----------------------------------------------------------------

    /// `ExtractMethodRefactoringImpl._checkSelection()`.
    pub(crate) fn selecao_de_metodo(&self, o: usize, l: usize) -> Result<SelecaoDeMetodo, &'static str> {
        if o == 0 {
            return Err("The selection offset must be greater than zero.");
        }
        if o + l >= self.fonte.len() {
            return Err("The selection end offset must be less than the length of the file.");
        }
        if l == 0
            && let Some(f) = self.closure_implicita(o)
        {
            let no = &self.arvore.nos[f];
            return Ok(SelecaoDeMetodo { faixa: (no.inicio, no.fim), expressao: None, closure: Some(f), comandos: Vec::new() });
        }
        let st = self.analisar(o, o + l);
        if let Some(m) = st.fatal {
            return Err(m);
        }
        let mut selecionados = st.selecionados;
        let mut faixa = (o, o + l);
        if selecionados.is_empty() {
            let mut atual = st.cobertura;
            while let Some(k) = atual {
                if self.e_comando(k) {
                    break;
                }
                if self.e_expressao(k) && self.extraivel(k) {
                    selecionados.push(k);
                    faixa = (self.arvore.nos[k].inicio, self.arvore.nos[k].fim);
                    break;
                }
                atual = self.pai(k);
            }
        }
        if !selecionados.is_empty() {
            let unico = selecionados[0];
            if selecionados.len() == 1 && !self.inclui_algo_fora(faixa, unico) && self.e_expressao(unico) {
                return Ok(if self.especie(unico) == "FunctionExpression" {
                    SelecaoDeMetodo { faixa, expressao: None, closure: Some(unico), comandos: Vec::new() }
                } else {
                    SelecaoDeMetodo { faixa, expressao: Some(unico), closure: None, comandos: Vec::new() }
                });
            }
            if selecionados.iter().all(|&k| self.e_comando(k)) {
                return Ok(SelecaoDeMetodo { faixa, expressao: None, closure: None, comandos: selecionados });
            }
        }
        Err("Can only extract a single expression or a set of statements.")
    }

    /// `ExtractMethodRefactoringImpl.isAvailable()`.
    pub(crate) fn extract_method(&self, o: usize, l: usize) -> bool {
        self.selecao_de_metodo(o, l).is_ok()
    }

    /// `_isExtractable(range.node(n))`: a análise da faixa exata sem
    /// nenhum problema.
    pub(crate) fn extraivel(&self, n: usize) -> bool {
        let no = &self.arvore.nos[n];
        self.analisar(no.inicio, no.fim).fatal.is_none()
    }

    /// `_selectionIncludesNonWhitespaceOutsideNode`.
    pub(crate) fn inclui_algo_fora(&self, (si, sf): (usize, usize), n: usize) -> bool {
        let no = &self.arvore.nos[n];
        if !(si <= no.inicio && no.fim <= sf) {
            return false;
        }
        !self.so_branco_ou_comentario(si, no.inicio) || !self.so_branco_ou_comentario(no.fim, sf)
    }

    /// `_findFunctionExpression`: a closure cujos parênteses contêm o cursor,
    /// ou a do argumento nomeado cujo rótulo tem o cursor.
    pub(crate) fn closure_implicita(&self, o: usize) -> Option<usize> {
        let n = self.arvore.localizar_exclusivo(o)?;
        if let Some(f) = self.com_pais(n).find(|&k| self.especie(k) == "FunctionExpression")
            && let Some(&ps) = self.filhos(f).iter().find(|&&c| self.especie(c) == "FormalParameterList")
            && self.arvore.nos[ps].inicio <= o
            && o <= self.arvore.nos[ps].fim
            && !self.pai(f).is_some_and(|p| self.especie(p) == "FunctionDeclaration")
        {
            return Some(f);
        }
        if self.especie(n) == "SimpleIdentifier"
            && let Some(rotulo) = self.pai(n)
            && self.especie(rotulo) == "Label"
            && let Some(nomeada) = self.pai(rotulo)
            && self.especie(nomeada) == "NamedExpression"
            && let Some(&expr) = self.filhos(nomeada).get(1)
            && self.especie(expr) == "FunctionExpression"
        {
            return Some(expr);
        }
        None
    }

    /// `ExtractLocalRefactoringImpl._checkSelection()`.
    pub(crate) fn selecao_local(&self, o: usize, l: usize) -> Result<SelecaoLocal, &'static str> {
        if o == 0 {
            return Err("The selection offset must be greater than zero.");
        }
        if o + l >= self.fonte.len() {
            return Err("The selection end offset must be less than the length of the file.");
        }
        let texto_da_selecao = self.fonte[o..o + l].to_string();
        let bytes = texto_da_selecao.as_bytes();
        let iniciais = bytes.iter().take_while(|&&b| e_branco(b)).count();
        let finais = bytes.iter().rev().take_while(|&&b| e_branco(b)).count();
        let ini = o + iniciais;
        let fim = (o + l).saturating_sub(finais).max(o);
        let cobertura = self.arvore.localizar(ini, fim).unwrap_or(0);
        let Some(corpo) = self.com_pais(cobertura).find(|&k| CORPOS.contains(&self.especie(k))) else {
            return Err("An expression inside a function must be selected to activate this refactoring.");
        };
        let no = &self.arvore.nos[cobertura];
        if STRINGS.contains(&no.especie) && fim > ini && ini > no.inicio && fim < no.fim {
            return Ok(SelecaoLocal { faixa: (ini, fim), expressao: None, parte_de_string: Some(texto_da_selecao), corpo });
        }
        let mut unica: Option<usize> = None;
        let mut sem_cobertas = true;
        let mut atual = Some(cobertura);
        while let Some(k) = atual {
            let pai = self.pai(k);
            atual = pai;
            let especie = self.especie(k);
            if matches!(especie, "ArgumentList" | "AssignmentExpression" | "NamedExpression" | "TypeArgumentList") {
                continue;
            }
            if matches!(especie, "ConstructorName" | "Label" | "NamedType") {
                unica = None;
                sem_cobertas = true;
                continue;
            }
            if let Some(p) = pai
                && ((self.especie(p) == "PrefixedIdentifier" && self.filhos(p).get(1) == Some(&k))
                    || (self.especie(p) == "PropertyAccess" && self.filhos(p).last() == Some(&k) && especie == "SimpleIdentifier"))
            {
                continue;
            }
            if !self.e_expressao(k) {
                break;
            }
            if especie == "MethodInvocation"
                && let Some(m) = self.nome_do_metodo(k)
            {
                let e = self.elemento_do_identificador(m, true);
                if self.executavel(e) && self.retorna_void(e) {
                    if unica.is_none() {
                        return Err("Cannot extract the void expression.");
                    }
                    break;
                }
            }
            if sem_cobertas {
                if especie == "SimpleIdentifier" {
                    if self.arvore.nos[k].marca == Marca::ContextoDeDeclaracao {
                        return Err("Cannot extract the name part of a declaration.");
                    }
                    let e = self.elemento_do_identificador(k, true);
                    if self.e_funcao(e) || self.e_metodo(e) {
                        continue;
                    }
                }
                if let Some(p) = pai
                    && self.especie(p) == "AssignmentExpression"
                    && self.filhos(p).first() == Some(&k)
                {
                    return Err("Cannot extract the left-hand side of an assignment.");
                }
            }
            if unica.is_none() {
                unica = Some(k);
            }
            sem_cobertas = false;
        }
        match unica {
            Some(k) => {
                let no = &self.arvore.nos[k];
                Ok(SelecaoLocal { faixa: (no.inicio, no.fim), expressao: Some(k), parte_de_string: None, corpo })
            }
            None => Err("Expression must be selected to activate this refactoring."),
        }
    }

    /// `ExtractLocalRefactoringImpl.isAvailable()`.
    pub(crate) fn extract_local(&self, o: usize, l: usize) -> bool {
        self.selecao_local(o, l).is_ok()
    }

    /// `InlineLocalRefactoringImpl.isAvailable()`.
    pub(crate) fn inline_local(&self, o: usize) -> bool {
        let Some(n) = self.arvore.localizar(o, o) else { return false };
        match self.especie(n) {
            "SimpleIdentifier" => self.elemento_do_identificador(n, true) == Elem::VariavelLocal,
            "VariableDeclaration" => self.arvore.nos[n].marca == Marca::VariavelLocal,
            _ => false,
        }
    }

    /// O elemento do `InlineMethodRefactoringImpl._checkOffset`, e se veio
    /// da declaração.
    pub(crate) fn elemento_de_inline_method(&self, o: usize) -> Option<(Elem, usize, bool)> {
        let n = self.arvore.localizar(o, o)?;
        let (e, declaracao) = match self.especie(n) {
            "FunctionDeclaration" | "MethodDeclaration" => (self.elemento_declarado(n), true),
            "SimpleIdentifier" => (self.elemento_do_identificador(n, false), false),
            _ => return None,
        };
        Some((e, n, declaracao))
    }

    /// `InlineMethodRefactoringImpl.isAvailable()`.
    pub(crate) fn inline_method(&self, o: usize) -> bool {
        let Some((e, _, _)) = self.elemento_de_inline_method(o) else { return false };
        self.executavel(e) && !self.sintetico(e) && !self.operador(e) && !self.gerador(e)
    }

    /// O elemento de `getElementOfNode(NodeLocator(offset))`.
    pub(crate) fn elemento_no_cursor(&self, o: usize) -> Elem {
        match self.arvore.localizar(o, o) {
            Some(n) => self.elemento_do_no(n),
            None => Elem::Nenhum,
        }
    }

    /// `Convert Getter to Method`: getter explícito do workspace.
    pub(crate) fn convert_getter(&self, o: usize) -> bool {
        let e = self.elemento_no_cursor(o);
        matches!(e, Elem::Funcao(f) if self.p.programa().function(f).kind == FunctionKind::Getter) && self.no_workspace(e)
    }

    /// `ConvertMethodToGetterRefactoringImpl._checkElement`: a mensagem do
    /// fatal, ou nada.
    pub(crate) fn erro_de_convert_method(&self, e: Elem) -> Option<String> {
        if !self.no_workspace(e) {
            return Some("Only methods in your workspace can be converted.".to_string());
        }
        if matches!(e, Elem::FuncaoLocal(_)) {
            return Some("Only top-level functions can be converted to getters.".to_string());
        }
        if !self.e_funcao(e) && !self.e_metodo(e) {
            return Some("Only class methods or top-level functions can be converted to getters.".to_string());
        }
        if self.retorna_void(e) {
            let especie = if self.e_funcao(e) { "function" } else { "method" };
            return Some(format!("Cannot convert {especie} returning void."));
        }
        if !self.sem_parametros(e) {
            return Some("Only methods without parameters can be converted to getters.".to_string());
        }
        None
    }

    /// `Convert Method to Getter`.
    pub(crate) fn convert_method(&self, o: usize) -> bool {
        let e = self.elemento_no_cursor(o);
        self.executavel(e) && self.erro_de_convert_method(e).is_none()
    }

    // -- Move top-level to file ---------------------------------------------------

    /// O token que termina exatamente em `pos` é identificador.
    pub(crate) fn identificador_termina_em(&self, pos: usize) -> bool {
        let i = self.tokens.partition_point(|t| t.span.end < pos);
        self.tokens.get(i).is_some_and(|t| t.span.end == pos && t.kind == Kind::Ident)
    }

    /// O token que começa exatamente em `pos` é identificador.
    pub(crate) fn identificador_comeca_em(&self, pos: usize) -> bool {
        let i = self.tokens.partition_point(|t| t.span.start < pos);
        self.tokens.get(i).is_some_and(|t| t.span.start == pos && t.kind == Kind::Ident && t.span.end > t.span.start)
    }

    /// `nodeCovering(offset, length)`
    /// (`AN:src/utilities/extensions/ast.dart:86-141`).
    pub(crate) fn cobertura(&self, o: usize, l: usize) -> Option<usize> {
        let fim = o + l;
        if fim > self.fonte.len() {
            return None;
        }
        let contem = |k: usize| {
            let no = &self.arvore.nos[k];
            if l == 0 {
                if o == no.inicio && self.identificador_termina_em(o) {
                    return false;
                }
                if o == no.fim && self.identificador_comeca_em(o) {
                    return false;
                }
            }
            no.inicio <= o && no.fim >= fim
        };
        let mut atual = 0usize;
        while let Some(f) = self.filhos(atual).iter().copied().find(|&f| contem(f)) {
            atual = f;
        }
        Some(atual)
    }

    /// A declaração de topo (`DeclId`) do nó membro da unidade.
    pub(crate) fn declaracao_do_no(&self, n: usize) -> Option<ast::DeclId> {
        match self.arvore.nos[n].marca {
            Marca::Decl(d) => Some(d),
            Marca::Funcao(fid) => {
                let unidade = &self.p.programa().unit(self.unidade).unit;
                unidade.declarations.iter().copied().find(|&d| matches!(self.ast.decl(d).kind, DeclKind::Function(f) if f == fid))
            }
            _ => None,
        }
    }

    /// O nome declarado (o token) do membro.
    pub(crate) fn nome_da_declaracao(&self, d: ast::DeclId) -> Option<ast::Name> {
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
    pub(crate) fn classe_da_declaracao(&self, unidade: UnitId, d: ast::DeclId) -> Option<ClassId> {
        self.p
            .programa()
            .classes
            .iter()
            .position(|c| c.decl.is_some_and(|r| r.unit == unidade && r.decl == d))
            .map(|i| ClassId(i as u32))
    }

    /// `ClassElement.isSealed`.
    pub(crate) fn selada(&self, c: ClassId) -> bool {
        let prog = self.p.programa();
        let Some(r) = prog.class(c).decl else { return false };
        matches!(&prog.unit(r.unit).ast.decl(r.decl).kind, DeclKind::Class(cd) if cd.modifiers.sealed)
    }

    /// `sealedSuperclassElements` da declaração `d` da unidade `unidade`:
    /// as classes `sealed` de `extends`/`implements`/`with` (classe, não
    /// alias) ou de `implements`/`on` (mixin).
    pub(crate) fn superclasses_seladas(&self, unidade: UnitId, d: ast::DeclId) -> Vec<ClassId> {
        let ast = &self.p.programa().unit(unidade).ast;
        let tipos: Vec<ast::TypeId> = match &ast.decl(d).kind {
            DeclKind::Class(c) if !c.mixin_application => c.extends.iter().chain(c.implements.iter()).chain(c.with.iter()).copied().collect(),
            DeclKind::Mixin(m) => m.implements.iter().chain(m.on.iter()).copied().collect(),
            _ => Vec::new(),
        };
        tipos.into_iter().filter_map(|t| self.p.classe_do_tipo(unidade, t)).filter(|&c| self.selada(c)).collect()
    }

    /// `MoveTopLevelToFile.isAvailable()`: o título e o arquivo padrão.
    pub(crate) fn mover(&self, o: usize, l: usize, criar_arquivos: bool) -> Option<(String, PathBuf)> {
        if !criar_arquivos {
            return None;
        }
        let candidatos = self.membros_a_mover(o, l)?;
        let prog = self.p.programa();
        // O título e o arquivo padrão.
        let contagem: usize = candidatos
            .iter()
            .map(|(d, _)| match &self.ast.decl(*d).kind {
                DeclKind::Variables(vl) => vl.variables.len(),
                _ => 1,
            })
            .sum();
        let primeiro = candidatos.first().and_then(|(_, n)| n.clone());
        let titulo = if contagem == 1 {
            format!("Move '{}' to file", primeiro.as_deref().unwrap_or("null"))
        } else {
            format!("Move {contagem} declarations to file")
        };
        let arquivo = match &primeiro {
            Some(n) => nome_de_arquivo(n),
            None => "newFile.dart".to_string(),
        };
        let caminho = prog.unit(self.unidade).path.as_deref()?;
        let pasta = caminho.parent()?;
        Some((titulo, pasta.join(arquivo)))
    }

    /// `_membersToMove()`: as declarações (com o nome) do único grupo, na
    /// ordem de inserção.
    pub(crate) fn membros_a_mover(&self, o: usize, l: usize) -> Option<Vec<(ast::DeclId, Option<String>)>> {
        let fim = o + l;
        let cobertura = self.cobertura(o, l)?;
        // `_selectedNodes`.
        let em_token = |s: Span| s.start <= o && fim <= s.end;
        let selecionados: Vec<usize> = match self.especie(cobertura) {
            "CompilationUnit" => {
                // `nodesInRange`: as diretivas, ou, sem elas, as
                // declarações que a seleção toca com comprimento.
                let filhos = self.filhos(0);
                let no_intervalo = |diretivas: bool| -> Vec<usize> {
                    let lista: Vec<usize> = filhos
                        .iter()
                        .copied()
                        .filter(|&f| DIRETIVAS.contains(&self.especie(f)) == diretivas && self.especie(f) != "ScriptTag")
                        .collect();
                    let mut primeiro = lista.len();
                    while primeiro > 0 && self.arvore.nos[lista[primeiro - 1]].fim > o {
                        primeiro -= 1;
                    }
                    lista[primeiro..].iter().copied().take_while(|&k| self.arvore.nos[k].inicio < fim).collect()
                };
                let mut nos = no_intervalo(true);
                if nos.is_empty() {
                    nos = no_intervalo(false);
                }
                if nos.is_empty() || nos.iter().any(|&k| !MEMBROS_DE_UNIDADE.contains(&self.especie(k))) {
                    return None;
                }
                nos
            }
            "VariableDeclaration" => {
                let lista = self.pai(cobertura)?;
                let decl = self.pai(lista)?;
                if self.especie(decl) != "TopLevelVariableDeclaration" {
                    return None;
                }
                let d = self.declaracao_do_no(decl)?;
                let DeclKind::Variables(vl) = &self.ast.decl(d).kind else { return None };
                if vl.variables.len() != 1 || !em_token(vl.variables[0].name.span) {
                    return None;
                }
                vec![decl]
            }
            e if MEMBROS_DE_UNIDADE.contains(&e) => vec![cobertura],
            _ => return None,
        };
        // `_membersToMove`.
        let varios = selecionados.len() > 1;
        let valido = |nome: Option<ast::Name>| varios || nome.is_some_and(|n| em_token(n.span));
        let mut candidatos: Vec<(ast::DeclId, Option<String>)> = Vec::new();
        for &k in &selecionados {
            let d = self.declaracao_do_no(k)?;
            let nome = self.nome_da_declaracao(d);
            let texto = |n: Option<ast::Name>| n.map(|n| self.fonte[n.span.start..n.span.end].to_string());
            let entrada = match self.especie(k) {
                "ClassDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "MixinDeclaration" | "ClassTypeAlias" | "FunctionTypeAlias" | "GenericTypeAlias"
                    if valido(nome) =>
                {
                    texto(nome)
                }
                "FunctionDeclaration" if self.pai(k) == Some(0) && valido(nome) => texto(nome),
                "TopLevelVariableDeclaration" => match &self.ast.decl(d).kind {
                    DeclKind::Variables(vl) if vl.variables.len() == 1 => texto(Some(vl.variables[0].name)),
                    _ => None,
                },
                _ => return None,
            };
            candidatos.push((d, entrada));
        }
        // `_SealedSubclassIndex`.
        let classes_candidatas: HashSet<ClassId> = candidatos.iter().filter_map(|(d, _)| self.classe_da_declaracao(self.unidade, *d)).collect();
        let declaracoes = self.p.programa().unit(self.unidade).unit.declarations.clone();
        let mut subclasses: HashMap<ClassId, Vec<ast::DeclId>> = HashMap::new();
        for &d in declaracoes.iter() {
            let classe = self.classe_da_declaracao(self.unidade, d);
            for s in self.superclasses_seladas(self.unidade, d) {
                let lista = subclasses.entry(s).or_default();
                if !lista.contains(&d) {
                    lista.push(d);
                }
                if classe.is_some_and(|c| classes_candidatas.contains(&c)) && !classes_candidatas.contains(&s) {
                    return None;
                }
            }
        }
        // `findSubclassesOfSealedRecursively`: os membros e, depois, as
        // subclasses diretas de cada um (o mesmo, recursivamente), na ordem
        // do conjunto do Dart; as novas entram no fim.
        fn achar(cx: &Contexto<'_>, membros: &[ast::DeclId], subclasses: &HashMap<ClassId, Vec<ast::DeclId>>, pilha: &mut Vec<ast::DeclId>) -> Vec<ast::DeclId> {
            let mut v: Vec<ast::DeclId> = Vec::new();
            for &m in membros {
                if !v.contains(&m) {
                    v.push(m);
                }
            }
            for &m in membros {
                if pilha.contains(&m) {
                    continue;
                }
                let Some(c) = cx.classe_da_declaracao(cx.unidade, m) else { continue };
                let Some(subs) = subclasses.get(&c) else { continue };
                pilha.push(m);
                for x in achar(cx, subs, subclasses, pilha) {
                    if !v.contains(&x) {
                        v.push(x);
                    }
                }
                pilha.pop();
            }
            v
        }
        let originais: Vec<ast::DeclId> = candidatos.iter().map(|(d, _)| *d).collect();
        for d in achar(self, &originais, &subclasses, &mut Vec::new()) {
            if !candidatos.iter().any(|(x, _)| *x == d) {
                let nome = self.nome_da_declaracao(d).map(|n| self.fonte[n.span.start..n.span.end].to_string());
                candidatos.push((d, nome));
            }
        }
        // Nenhuma outra parte da biblioteca estende uma `sealed` deste
        // arquivo.
        let prog = self.p.programa();
        let biblioteca = prog.unit(self.unidade).library;
        for &u2 in prog.library(biblioteca).units.iter() {
            if u2 == self.unidade {
                continue;
            }
            for &d in prog.unit(u2).unit.declarations.iter() {
                for s in self.superclasses_seladas(u2, d) {
                    if prog.class(s).decl.is_some_and(|r| r.unit == self.unidade) {
                        return None;
                    }
                }
            }
        }
        Some(candidatos)
    }
}

/// `String.toFileName`: cada maiúscula que não é a primeira letra vira `_` e
/// a letra; tudo em minúsculas; mais `.dart`.
pub(crate) fn nome_de_arquivo(nome: &str) -> String {
    let mut s = String::with_capacity(nome.len() + 8);
    for (i, c) in nome.char_indices() {
        if c.is_ascii_uppercase() && i != 0 {
            s.push('_');
        }
        s.push(c);
    }
    format!("{}.dart", s.to_lowercase())
}

impl Projeto {
    /// As refatorações listadas em `[offset, offset + comprimento)` da
    /// unidade, na ordem do Dart: Move, Extract Method, Extract Local
    /// Variable, Inline Local Variable, Inline Method, Convert Getter to
    /// Method, Convert Method to Getter. `criar_arquivos` é o
    /// `supportsFileCreation` do cliente.
    pub(crate) fn refatoracoes(&self, unidade: UnitId, offset: usize, comprimento: usize, criar_arquivos: bool) -> Vec<Refatoracao> {
        let cx = Contexto::novo(self, unidade);
        let mut v = Vec::new();
        if let Some((titulo, caminho_padrao)) = cx.mover(offset, comprimento, criar_arquivos) {
            v.push(Refatoracao { titulo, especie: "refactor.move", comando: ComandoDeRefatoracao::Mover { caminho_padrao } });
        }
        let legada = |titulo: &str, especie: &'static str, kind: &'static str| Refatoracao {
            titulo: titulo.to_string(),
            especie,
            comando: ComandoDeRefatoracao::Legado(kind),
        };
        if cx.extract_method(offset, comprimento) {
            v.push(legada("Extract Method", "refactor.extract", "EXTRACT_METHOD"));
        }
        if cx.extract_local(offset, comprimento) {
            v.push(legada("Extract Local Variable", "refactor.extract", "EXTRACT_LOCAL_VARIABLE"));
        }
        if cx.inline_local(offset) {
            v.push(legada("Inline Local Variable", "refactor.inline", "INLINE_LOCAL_VARIABLE"));
        }
        if cx.inline_method(offset) {
            v.push(legada("Inline Method", "refactor.inline", "INLINE_METHOD"));
        }
        if cx.convert_getter(offset) {
            v.push(legada("Convert Getter to Method", "refactor.rewrite", "CONVERT_GETTER_TO_METHOD"));
        }
        if cx.convert_method(offset) {
            v.push(legada("Convert Method to Getter", "refactor.rewrite", "CONVERT_METHOD_TO_GETTER"));
        }
        v
    }
}

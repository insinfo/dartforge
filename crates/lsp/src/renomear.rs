//! `textDocument/prepareRename` e `textDocument/rename` como o servidor do
//! Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md §12.1 a §12.10): o elemento sob o
//! cursor pelo `getElementToRename`, a classe de refatoração do
//! `RenameRefactoring.create`, o `RefactoringStatus` com as mensagens exatas
//! (`checkInitialConditions`, `checkNewName`, `checkFinalConditions`) e as
//! edições de cada classe (`fillChange`). O projeto inteiro é carregado e as
//! referências vêm do modelo comum de [`crate::projeto`] (as mesmas de
//! `references`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::projeto::{Alvo, Concreto, Dono, Projeto, nome_base, palavra};
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Estado, Severidade};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind};
use dartforge_intern::SymbolId;
use std::collections::BTreeSet;
use url::Url;

/// Uma edição de texto: substituir `span` (bytes) de `uri` por `texto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edicao {
    pub uri: String,
    pub span: Span,
    pub texto: String,
}

/// O arquivo que acompanha a classe renomeada (o `RenameFile` no fim dos
/// `documentChanges`, §12.1 passo 17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenomearArquivo {
    /// URI atual do arquivo.
    pub de: String,
    /// URI nova (mesmo diretório, `toFileName` do nome novo).
    pub para: String,
    /// O Dart não edita as diretivas aqui (vêm do `willRenameFiles`): vazio.
    pub diretivas: Vec<Edicao>,
}

/// O resultado do `rename`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Renomeacao {
    pub edicoes: Vec<Edicao>,
    pub arquivo: Option<RenomearArquivo>,
    /// `checkFinalConditions` com erro ou aviso (sem fatal): a mensagem do
    /// prompt `Rename Anyway`/`Cancel`, ou do `-32010` sem prompt.
    pub aviso: Option<String>,
    /// Nada a renomear: a resposta é `null`.
    pub nulo: bool,
}

/// A classe de refatoração (`RenameRefactoring.create`, §12.2).
#[derive(Debug, Clone)]
enum Classe {
    /// `RenameUnitMemberRefactoringImpl`: o elemento de topo (o acessor de
    /// topo já trocado pela variável) ou o prefixo de um tipo.
    MembroDeUnidade(Alvo),
    Construtor(FunctionElementId),
    /// `RenameImportRefactoringImpl`: o import (pela posição em `imports`).
    Import { biblioteca: LibraryId, indice: usize },
    Rotulo { unidade: UnitId, declaracao: usize },
    Biblioteca(LibraryId),
    /// `RenameParameterRefactoringImpl`; `campo`: o `this.x` com campo.
    Parametro { unidade: UnitId, declaracao: usize, campo: Option<VariableId> },
    Local { unidade: UnitId, declaracao: usize },
    ParametroDeTipo { unidade: UnitId, declaracao: usize },
    MembroDeClasse { alvo: Alvo, classe: ClassId, concreto: Option<Concreto> },
    MembroDeExtensao { alvo: Alvo, concreto: Option<Concreto> },
}

/// O que o cursor renomeia: a classe, o intervalo do `prepareRename`, o
/// nome antigo, a espécie e o nome qualificado (mensagens), e onde o
/// elemento está (SDK, workspace).
#[derive(Debug, Clone)]
struct Pedido {
    classe: Classe,
    faixa: Span,
    antigo: String,
    especie: String,
    qualificado: String,
    biblioteca: Option<LibraryId>,
    unidade: Option<UnitId>,
}

/// As palavras reservadas (`KeywordStyle.reserved`).
const RESERVADAS: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if",
    "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// As embutidas e as pseudo (`isBuiltInOrPseudo`).
const EMBUTIDAS: &[&str] = &[
    "Function", "abstract", "as", "augment", "covariant", "deferred", "dynamic", "export", "extension", "external", "factory", "get", "implements",
    "import", "interface", "late", "library", "mixin", "operator", "part", "required", "set", "static", "typedef", "async", "await", "base", "hide",
    "inout", "native", "of", "on", "out", "patch", "sealed", "show", "source", "sync", "when", "yield",
];

/// `_validateIdentifier`.
fn validar_identificador(id: &str, desc: &str, comeco: &str, embutido_permitido: bool) -> Estado {
    if id != id.trim() {
        return Estado::fatal(format!("{desc} must not start or end with a blank."));
    }
    if id.is_empty() {
        return Estado::fatal(format!("{desc} must not be empty."));
    }
    let embutido = EMBUTIDAS.contains(&id);
    if embutido || RESERVADAS.contains(&id) {
        if embutido && embutido_permitido {
            return Estado::aviso("Avoid using built-in identifiers as names.");
        }
        return Estado::fatal(format!("{desc} must not be a keyword."));
    }
    for c in id.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            return Estado::fatal(format!("{desc} must not contain '{c}'."));
        }
    }
    let b = id.as_bytes()[0];
    if !(b.is_ascii_alphabetic() || b == b'_' || b == b'$') {
        return Estado::fatal(format!("{desc} must begin with {comeco}."));
    }
    Estado::default()
}

/// `_validateLowerCamelCase`.
fn validar_minuscula(id: &str, desc: &str, embutido_permitido: bool) -> Estado {
    let desc = format!("{desc} name");
    let e = validar_identificador(id, &desc, "a lowercase letter or underscore", embutido_permitido);
    if !e.ok() {
        return e;
    }
    let b = id.as_bytes()[0];
    if b == b'_' || b == b'$' {
        return Estado::default();
    }
    if !b.is_ascii_lowercase() {
        return Estado::aviso(format!("{desc} should start with a lowercase letter."));
    }
    Estado::default()
}

/// `_validateUpperCamelCase`.
fn validar_maiuscula(id: &str, desc: &str) -> Estado {
    let desc = format!("{desc} name");
    let e = validar_identificador(id, &desc, "an uppercase letter or underscore", false);
    if !e.ok() {
        return e;
    }
    let b = id.as_bytes()[0];
    if b == b'_' || b == b'$' {
        return Estado::default();
    }
    if !b.is_ascii_uppercase() {
        return Estado::aviso(format!("{desc} should start with an uppercase letter."));
    }
    Estado::default()
}

/// `validateLibraryName`.
fn validar_biblioteca(nome: &str) -> Estado {
    if nome.trim_matches([' ', '\t']).is_empty() {
        return Estado::fatal("Library name must not be blank.");
    }
    for c in nome.split('.') {
        let e = validar_identificador(c, "Library name identifier", "a lowercase letter or underscore", false);
        if !e.ok() {
            return e;
        }
    }
    if nome.bytes().any(|b| b.is_ascii_uppercase()) {
        return Estado::aviso("Library name should consist of lowercase identifier separated by dots.");
    }
    Estado::default()
}

/// `String.toFileName`.
pub(crate) fn nome_de_arquivo(nome: &str) -> String {
    crate::refatoracoes::nome_de_arquivo(nome)
}

/// O parâmetro (de função ou construtor) cujo intervalo está em `[a, b]`.
fn parametro_no_intervalo(ast: &ast::Ast, a: usize, b: usize) -> Option<&ast::Parameter> {
    let dentro = |q: &&ast::Parameter| q.span.start >= a && q.span.end <= b;
    for f in &ast.functions {
        if let Some(q) = f.parameters.iter().flatten().find(dentro) {
            return Some(q);
        }
    }
    for m in &ast.members {
        if let ast::MemberKind::Constructor(k) = &m.kind
            && let Some(q) = k.parameters.iter().find(dentro)
        {
            return Some(q);
        }
    }
    None
}

impl Projeto {
    // -- O elemento sob o cursor ------------------------------------------------

    /// `getElementOfNode` + `getElementToRename` + `RenameRefactoring.create`.
    fn pedido_de_renomear(&self, unidade: UnitId, offset: usize) -> Option<Pedido> {
        let p = self.programa();
        let u = p.unit(unidade);
        let fonte = u.source.as_str();
        let cx = Contexto::novo(self, unidade);
        let n = cx.arvore.localizar(offset, offset)?;
        let especie_do_no = cx.especie(n);
        let no = &cx.arvore.nos[n];
        let faixa_do_no = Span { start: no.inicio, end: no.fim };
        match especie_do_no {
            // Fora da tabela do `getElementToRename`.
            "SuperFormalParameter" | "FunctionTypedFormalParameter" => return None,
            // A URI de uma diretiva não é renomeável.
            "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
                if cx.pai(n).is_some_and(|x| matches!(cx.especie(x), "ImportDirective" | "ExportDirective" | "PartDirective" | "PartOfDirective")) =>
            {
                return None;
            }
            "ImportDirective" => {
                // O import da diretiva: com prefixo, o identificador dele; sem,
                // o intervalo vazio no cursor.
                let lib = u.library;
                let indice = p.library(lib).imports.iter().position(|i| {
                    i.unit == unidade
                        && p.unit(i.unit).unit.directives.get(i.directive).is_some_and(|d| no.inicio <= d.span.start && d.span.end <= no.fim)
                })?;
                let imp = &p.library(lib).imports[indice];
                let (faixa, antigo) = match &p.unit(imp.unit).unit.directives[imp.directive].kind {
                    ast::DirectiveKind::Import { prefix: Some(pr), .. } => (pr.span, fonte[pr.span.start..pr.span.end].to_string()),
                    _ => (Span { start: offset, end: offset }, String::new()),
                };
                return Some(Pedido {
                    classe: Classe::Import { biblioteca: lib, indice },
                    faixa,
                    antigo,
                    especie: "import directive".to_string(),
                    qualificado: String::new(),
                    biblioteca: Some(lib),
                    unidade: Some(unidade),
                });
            }
            "ImportPrefixReference" => {
                let texto = cx.texto_do_no(n).trim_end_matches('.').trim().to_string();
                let simbolo = self.consulta.nomes.lookup(&texto)?;
                return Some(Pedido {
                    classe: Classe::MembroDeUnidade(Alvo::Prefixo { biblioteca: u.library, nome: simbolo }),
                    faixa: faixa_do_no,
                    antigo: texto.clone(),
                    especie: "import prefix".to_string(),
                    qualificado: texto,
                    biblioteca: Some(u.library),
                    unidade: Some(unidade),
                });
            }
            "LibraryDirective" => {
                let nome = fonte[no.inicio..no.fim].trim_start_matches("library").trim().trim_end_matches(';').trim().to_string();
                return Some(Pedido {
                    classe: Classe::Biblioteca(u.library),
                    faixa: faixa_do_no,
                    antigo: nome.clone(),
                    especie: "library".to_string(),
                    qualificado: nome,
                    biblioteca: Some(u.library),
                    unidade: Some(unidade),
                });
            }
            "InstanceCreationExpression" => {
                // `new`/`const`: a classe, no nome do tipo.
                let cn = cx.filhos(n).iter().copied().find(|&k| cx.especie(k) == "ConstructorName")?;
                let tipo = *cx.filhos(cn).first()?;
                let inicio = cx.filhos(tipo).iter().copied().find(|&k| cx.especie(k) == "ImportPrefixReference").map_or(cx.arvore.nos[tipo].inicio, |k| cx.arvore.nos[k].fim);
                let d = self.identificar(unidade, inicio).ok()??;
                let Alvo::Topo(Element::Class(c)) = d.alvo else { return None };
                return self.pedido_de_alvo(Alvo::Topo(Element::Class(c)), None, d.nome, unidade);
            }
            "ConstructorDeclaration" => {
                let f = (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| {
                    matches!(p.function(f).kind, FunctionKind::Constructor)
                        && self.nome_da_funcao(f).is_some_and(|(uu, s)| uu == unidade && no.inicio <= s.start && s.end <= no.fim)
                })?;
                let mut pedido = self.pedido_de_alvo(Alvo::Construtor(f), Some(Concreto::Funcao(f)), faixa_do_no, unidade)?;
                pedido.faixa = faixa_do_no;
                return Some(pedido);
            }
            "FieldFormalParameter" => {
                let q = parametro_no_intervalo(&u.ast, no.inicio, no.fim)?;
                let nome = q.name?;
                let campo = (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| {
                    p.class(c).decl.is_some_and(|d| d.unit == unidade && {
                        let s = u.ast.decl(d.decl).span;
                        s.start <= no.inicio && no.fim <= s.end
                    })
                });
                let campo = campo.and_then(|c| p.class(c).fields.iter().copied().find(|v| p.variable(*v).name == nome.sym));
                let antigo = fonte[nome.span.start..nome.span.end].to_string();
                return Some(Pedido {
                    classe: Classe::Parametro { unidade, declaracao: nome.span.start, campo },
                    faixa: nome.span,
                    antigo: antigo.clone(),
                    especie: "parameter".to_string(),
                    qualificado: antigo,
                    biblioteca: Some(u.library),
                    unidade: Some(unidade),
                });
            }
            _ => {}
        }
        // Rótulo de comando, e o de um `break`/`continue`.
        if especie_do_no == "SimpleIdentifier"
            && let Some(pai) = cx.pai(n)
        {
            let rotulo_de_comando = cx.especie(pai) == "Label"
                && cx.pai(pai).is_some_and(|g| matches!(cx.especie(g), "LabeledStatement" | "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"));
            let declaracao = if rotulo_de_comando {
                Some(no.inicio)
            } else if matches!(cx.especie(pai), "BreakStatement" | "ContinueStatement") {
                let nome = cx.texto_do_no(n);
                let mut achado = None;
                'fora: for k in cx.com_pais(pai) {
                    for &l in cx.filhos(k) {
                        if cx.especie(l) == "Label"
                            && let Some(&i) = cx.filhos(l).first()
                            && cx.texto_do_no(i) == nome
                        {
                            achado = Some(cx.arvore.nos[i].inicio);
                            break 'fora;
                        }
                    }
                }
                Some(achado?)
            } else {
                None
            };
            if let Some(declaracao) = declaracao {
                let nome = cx.texto_do_no(n).to_string();
                return Some(Pedido {
                    classe: Classe::Rotulo { unidade, declaracao },
                    faixa: faixa_do_no,
                    antigo: nome.clone(),
                    especie: "label".to_string(),
                    qualificado: nome,
                    biblioteca: Some(u.library),
                    unidade: Some(unidade),
                });
            }
        }
        let d = self.identificar(unidade, offset).ok()??;
        // O prefixo usado numa expressão: o `PrefixElement`, membro da
        // unidade (`RenameUnitMemberRefactoringImpl`); o `as p` da diretiva
        // fica com o `LibraryImportElement` (o ramo `ImportDirective`).
        if let Alvo::Prefixo { biblioteca, nome } = d.alvo {
            let antigo = self.nome(nome).to_string();
            // No `as p` da própria diretiva: o `LibraryImportElement`.
            let na_diretiva = p.library(biblioteca).imports.iter().position(|i| {
                i.unit == unidade
                    && matches!(
                        p.unit(i.unit).unit.directives.get(i.directive).map(|x| &x.kind),
                        Some(ast::DirectiveKind::Import { prefix: Some(pr), .. }) if pr.span.start <= offset && offset <= pr.span.end
                    )
            });
            // Usado numa expressão (um `SimpleIdentifier` com o
            // `PrefixElement`): `getElementOfNode` troca pelo import
            // (`getImportElement`), e o rename é o do import.
            let na_diretiva = na_diretiva.or_else(|| self.import_do_prefixo(unidade, offset, biblioteca, nome));
            if let Some(indice) = na_diretiva {
                return Some(Pedido {
                    classe: Classe::Import { biblioteca, indice },
                    faixa: d.nome,
                    antigo,
                    especie: "import directive".to_string(),
                    qualificado: String::new(),
                    biblioteca: Some(biblioteca),
                    unidade: Some(unidade),
                });
            }
            return Some(Pedido {
                classe: Classe::MembroDeUnidade(Alvo::Prefixo { biblioteca, nome }),
                faixa: d.nome,
                antigo: antigo.clone(),
                especie: "import prefix".to_string(),
                qualificado: antigo,
                biblioteca: Some(biblioteca),
                unidade: Some(unidade),
            });
        }
        self.pedido_de_alvo(d.alvo, d.concreto, d.nome, unidade)
    }

    /// O pedido para um alvo do `identificar` (a regra 1 do `create` já
    /// aplicada: acessor → variável).
    fn pedido_de_alvo(&self, alvo: Alvo, concreto: Option<Concreto>, faixa: Span, unidade: UnitId) -> Option<Pedido> {
        let p = self.programa();
        let pedido = match &alvo {
            Alvo::Topo(el) => {
                let (especie, nome) = self.especie_de_topo(*el)?;
                Pedido {
                    classe: Classe::MembroDeUnidade(alvo.clone()),
                    faixa,
                    antigo: nome.clone(),
                    especie,
                    qualificado: nome,
                    biblioteca: Some(self.biblioteca_do_elemento(*el)),
                    unidade: self.nome_do_elemento_de_topo(*el).map(|(u, _)| u),
                }
            }
            Alvo::Construtor(f) => {
                let fe = p.function(*f);
                let classe = fe.class.map(|c| self.nome(p.class(c).name).to_string()).unwrap_or_default();
                let nome = self.nome(fe.name).to_string();
                let exibido = if nome.is_empty() { classe } else { format!("{classe}.{nome}") };
                Pedido {
                    classe: Classe::Construtor(*f),
                    faixa,
                    antigo: nome,
                    especie: "constructor".to_string(),
                    qualificado: exibido,
                    biblioteca: Some(fe.library),
                    unidade: self.nome_da_funcao(*f).map(|(u, _)| u),
                }
            }
            Alvo::Local { unidade: lu, declaracao } => {
                let un = p.unit(*lu);
                let nome = palavra(&un.source, *declaracao).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
                let (classe, especie) = if crate::projeto::parametro_em(&un.ast, *declaracao).is_some() {
                    (Classe::Parametro { unidade: *lu, declaracao: *declaracao, campo: None }, "parameter")
                } else if un.ast.functions.iter().any(|f| f.name.is_some_and(|x| x.span.start == *declaracao)) {
                    (Classe::Local { unidade: *lu, declaracao: *declaracao }, "function")
                } else {
                    (Classe::Local { unidade: *lu, declaracao: *declaracao }, "local variable")
                };
                Pedido {
                    classe,
                    faixa,
                    antigo: nome.clone(),
                    especie: especie.to_string(),
                    qualificado: nome,
                    biblioteca: Some(un.library),
                    unidade: Some(*lu),
                }
            }
            Alvo::ParametroDeTipo { unidade: lu, declaracao } => {
                let un = p.unit(*lu);
                let nome = palavra(&un.source, *declaracao).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
                Pedido {
                    classe: Classe::ParametroDeTipo { unidade: *lu, declaracao: *declaracao },
                    faixa,
                    antigo: nome.clone(),
                    especie: "type parameter".to_string(),
                    qualificado: nome,
                    biblioteca: Some(un.library),
                    unidade: Some(*lu),
                }
            }
            Alvo::Prefixo { biblioteca, nome } => Pedido {
                classe: Classe::MembroDeUnidade(alvo.clone()),
                faixa,
                antigo: self.nome(*nome).to_string(),
                especie: "import prefix".to_string(),
                qualificado: self.nome(*nome).to_string(),
                biblioteca: Some(*biblioteca),
                unidade: Some(unidade),
            },
            Alvo::Membro { dono, nome, .. } => {
                let (especie, lib, uni) = match concreto {
                    Some(Concreto::Funcao(f)) => {
                        let fe = p.function(f);
                        let especie = match fe.kind {
                            FunctionKind::Function | FunctionKind::Operator => "method",
                            _ => "field",
                        };
                        (especie, fe.library, self.nome_da_funcao(f).map(|(u, _)| u))
                    }
                    Some(Concreto::Variavel(v)) => ("field", p.variable(v).library, self.nome_da_variavel(v).map(|(u, _)| u)),
                    None => ("field", p.unit(unidade).library, None),
                };
                let (classe, qualificado) = match dono {
                    Dono::Classe(c) => (
                        Classe::MembroDeClasse { alvo: alvo.clone(), classe: *c, concreto },
                        format!("{}.{nome}", self.nome(p.class(*c).name)),
                    ),
                    Dono::Extensao(x) => (
                        Classe::MembroDeExtensao { alvo: alvo.clone(), concreto },
                        format!("{}.{nome}", p.extension(*x).name.map(|s| self.nome(s)).unwrap_or("")),
                    ),
                };
                Pedido { classe, faixa, antigo: nome.clone(), especie: especie.to_string(), qualificado, biblioteca: Some(lib), unidade: uni }
            }
        };
        Some(pedido)
    }

    /// A espécie (`kind.displayName`) e o nome de um elemento de topo (o
    /// acessor de topo vira a variável).
    fn especie_de_topo(&self, el: Element) -> Option<(String, String)> {
        let p = self.programa();
        Some(match el {
            Element::Class(c) => (self.especie_de_classe(c), self.nome(p.class(c).name).to_string()),
            Element::Extension(x) => ("extension".to_string(), p.extension(x).name.map(|s| self.nome(s).to_string()).unwrap_or_default()),
            Element::Typedef(t) => {
                let d = p.typedef(t);
                let legado = matches!(&p.unit(d.decl.unit).ast.decl(d.decl.decl).kind, DeclKind::Typedef(x) if matches!(x.kind, ast::TypedefKind::Legacy { .. }));
                (if legado { "function type alias" } else { "type alias" }.to_string(), self.nome(d.name).to_string())
            }
            Element::Function(f) => {
                let fe = p.function(f);
                match fe.kind {
                    FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                        ("top level variable".to_string(), nome_base(self.nome(fe.name)).to_string())
                    }
                    _ => ("function".to_string(), self.nome(fe.name).to_string()),
                }
            }
            Element::Variable(v) => ("top level variable".to_string(), self.nome(p.variable(v).name).to_string()),
            Element::Prefix(_, s) => ("import prefix".to_string(), self.nome(s).to_string()),
        })
    }

    /// `workspace.containsElement`: o arquivo do elemento está sob a raiz.
    fn no_workspace(&self, unidade: Option<UnitId>) -> bool {
        let Some(u) = unidade else { return false };
        self.programa().unit(u).path.as_deref().is_some_and(|c| crate::projeto::dentro(c, &self.raiz))
    }

    // -- As condições -----------------------------------------------------------

    /// `checkInitialConditions`.
    fn condicoes_iniciais(&self, pedido: &Pedido) -> Estado {
        let mut e = Estado::default();
        let p = self.programa();
        if pedido.biblioteca.is_some_and(|l| p.library(l).is_sdk) {
            e.adicionar(Severidade::Fatal, format!("The {} '{}' is defined in the SDK, so cannot be renamed.", pedido.especie, pedido.qualificado));
        }
        if !matches!(pedido.classe, Classe::Import { .. }) && !self.no_workspace(pedido.unidade) {
            e.adicionar(Severidade::Fatal, format!("The {} '{}' is defined outside of the project, so cannot be renamed.", pedido.especie, pedido.qualificado));
        }
        let operador = match &pedido.classe {
            Classe::MembroDeClasse { concreto: Some(Concreto::Funcao(f)), .. } | Classe::MembroDeExtensao { concreto: Some(Concreto::Funcao(f)), .. } => {
                p.function(*f).kind == FunctionKind::Operator
            }
            _ => false,
        };
        if operador {
            e.adicionar(Severidade::Fatal, "Cannot rename operator.");
        }
        e
    }

    /// `checkNewName`.
    fn verificar_nome_novo(&self, pedido: &Pedido, novo: &str) -> Estado {
        let p = self.programa();
        let mut e = Estado::default();
        if novo == pedido.antigo {
            e.adicionar(Severidade::Fatal, "The new name must be different than the current name.");
        }
        match &pedido.classe {
            Classe::MembroDeUnidade(Alvo::Topo(el)) => match el {
                Element::Class(c) => {
                    if p.class(*c).kind == ClassKind::ExtensionType {
                        e.somar(validar_maiuscula(novo, "Extension type"));
                    }
                    e.somar(validar_maiuscula(novo, "Class"));
                }
                Element::Function(f) if matches!(p.function(*f).kind, FunctionKind::Function) => e.somar(validar_minuscula(novo, "Function", true)),
                Element::Function(_) | Element::Variable(_) => e.somar(validar_minuscula(novo, "Variable", true)),
                Element::Typedef(_) => e.somar(validar_maiuscula(novo, "Type alias")),
                Element::Extension(_) | Element::Prefix(..) => {}
            },
            Classe::MembroDeUnidade(_) => {}
            Classe::Construtor(f) => {
                if !novo.is_empty() {
                    e.somar(validar_minuscula(novo, "Constructor", true));
                }
                // `_analyzePossibleConflicts`.
                if let Some(c) = p.function(*f).class {
                    let cl = p.class(c);
                    if self.nome(cl.name) == novo {
                        e.adicionar(Severidade::Erro, "The constructor should not have the same name as the name of the enclosing class.");
                    }
                    for (especie, _) in self.filhos_com_nome(c, novo) {
                        e.adicionar(
                            Severidade::Erro,
                            format!("{} '{}' already declares {especie} with name '{novo}'.", capitalizar(&self.especie_de_classe(c)), self.nome(cl.name)),
                        );
                    }
                }
            }
            Classe::Import { .. } => {
                if !novo.is_empty() {
                    e.somar(validar_identificador(novo, "Import prefix name", "a lowercase letter or underscore", false));
                }
            }
            Classe::Rotulo { .. } => e.somar(validar_minuscula(novo, "Label", true)),
            Classe::Biblioteca(_) => e.somar(validar_biblioteca(novo)),
            Classe::Parametro { .. } => e.somar(validar_minuscula(novo, "Parameter", true)),
            Classe::Local { unidade, declaracao } => {
                let funcao = p.unit(*unidade).ast.functions.iter().any(|f| f.name.is_some_and(|x| x.span.start == *declaracao));
                e.somar(validar_minuscula(novo, if funcao { "Function" } else { "Variable" }, true));
            }
            Classe::ParametroDeTipo { .. } => e.somar(validar_maiuscula(novo, "Type parameter")),
            Classe::MembroDeClasse { concreto, .. } | Classe::MembroDeExtensao { concreto, .. } => match concreto {
                Some(Concreto::Funcao(f)) if matches!(p.function(*f).kind, FunctionKind::Function | FunctionKind::Operator) => {
                    e.somar(validar_minuscula(novo, "Method", true))
                }
                _ => e.somar(validar_minuscula(novo, "Field", true)),
            },
        }
        e
    }

    /// A espécie (`kind.displayName`) de uma classe.
    fn especie_de_classe(&self, c: ClassId) -> String {
        match self.programa().class(c).kind {
            ClassKind::Mixin => "mixin",
            ClassKind::Enum => "enum",
            ClassKind::ExtensionType => "extension type",
            _ => "class",
        }
        .to_string()
    }

    /// Os filhos diretos de `c` com nome-base `nome` (`getChildren`):
    /// acessores, campos, construtores, métodos, parâmetros de tipo; a
    /// espécie e o nome qualificado.
    fn filhos_com_nome(&self, c: ClassId, nome: &str) -> Vec<(String, String)> {
        let p = self.programa();
        let cl = p.class(c);
        let classe = self.nome(cl.name).to_string();
        let mut v = Vec::new();
        let mut membros: Vec<(SymbolId, FunctionElementId)> = cl.instance_members.iter().chain(cl.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
        membros.sort_by_key(|(_, f)| f.0);
        for (s, f) in &membros {
            let fe = p.function(*f);
            if nome_base(self.nome(*s)) == nome && matches!(fe.kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                let setter = fe.kind == FunctionKind::Setter || self.nome(*s).ends_with('=');
                v.push((if setter { "setter" } else { "getter" }.to_string(), nome.to_string()));
            }
        }
        for f in cl.fields.iter() {
            if self.nome(p.variable(*f).name) == nome {
                v.push(("field".to_string(), format!("{classe}.{nome}")));
            }
        }
        for (s, _) in cl.constructors.iter() {
            if self.nome(*s) == nome {
                v.push(("constructor".to_string(), format!("{classe}.{nome}")));
            }
        }
        for (s, f) in &membros {
            if self.nome(*s) == nome && matches!(p.function(*f).kind, FunctionKind::Function | FunctionKind::Operator) {
                v.push(("method".to_string(), format!("{classe}.{nome}")));
            }
        }
        for tp in cl.type_params.iter() {
            if self.nome(tp.name) == nome {
                v.push(("type parameter".to_string(), nome.to_string()));
            }
        }
        v
    }

    /// As referências (sem as declarações) de um alvo, em (unidade, faixa).
    fn referencias_do_alvo(&self, alvo: &Alvo) -> Vec<(UnitId, Span)> {
        let declaracoes = self.declaracoes(alvo);
        let Ok(oc) = self.ocorrencias(alvo, false) else { return Vec::new() };
        // `searchReferences` passa pelo `Search._addResults`: só os arquivos
        // candidatos (os que citam o nome e os da biblioteca), com os
        // buracos do Dart (o redirecionamento de fábrica não cita o nome).
        let candidatos = self.nome_da_busca(alvo).map(|nome| {
            let mut unidades: Vec<UnitId> = declaracoes.iter().map(|(u, _, _)| *u).collect();
            unidades.dedup();
            self.arquivos_candidatos(&nome, &unidades)
        });
        oc.into_iter()
            .filter(|o| !declaracoes.contains(o))
            .filter(|(u, _, _)| candidatos.as_ref().is_none_or(|c| c.contains(u)))
            .map(|(u, a, b)| (u, Span { start: a, end: b }))
            .collect()
    }

    /// O caminho da biblioteca relativo à raiz (`getElementQualifiedName`
    /// de uma biblioteca).
    fn caminho_da_biblioteca(&self, l: LibraryId) -> String {
        let p = self.programa();
        let Some(&u) = p.library(l).units.first() else { return String::new() };
        match p.unit(u).path.as_deref() {
            // `pathContext.relative`: com o separador da plataforma.
            Some(c) => c.strip_prefix(&self.raiz).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| c.to_string_lossy().into_owned()),
            None => p.library(l).uri.clone(),
        }
    }

    /// `checkFinalConditions`.
    fn condicoes_finais(&self, pedido: &Pedido, novo: &str) -> Estado {
        let p = self.programa();
        let mut e = Estado::default();
        match &pedido.classe {
            Classe::MembroDeUnidade(alvo) => {
                // O prefixo (`PrefixElement`) também é membro da unidade.
                if !matches!(alvo, Alvo::Topo(_) | Alvo::Prefixo { .. }) {
                    return e;
                }
                let lib = pedido.biblioteca.unwrap_or(LibraryId(0));
                // `_validateWillConflict`.
                for &u in p.library(lib).units.iter() {
                    let cx = Contexto::novo(self, u);
                    for (especie, nome) in crate::refatoracoes_metodo::elementos_de_topo(&cx, u) {
                        if nome == novo {
                            e.adicionar(Severidade::Erro, format!("Library already declares {especie} with name '{novo}'."));
                        }
                    }
                }
                let referencias = self.referencias_do_alvo(alvo);
                // `_validateWillBeInvisible`.
                if novo.starts_with('_') {
                    for (u, _) in &referencias {
                        let l = p.unit(*u).library;
                        if l != lib {
                            e.adicionar(Severidade::Erro, format!("Renamed {} will be invisible in '{}'.", pedido.especie, self.caminho_da_biblioteca(l)));
                        }
                    }
                }
                // `_validateWillBeShadowed`.
                for (u, s) in &referencias {
                    if let Some(c) = self.classe_que_contem(*u, s.start) {
                        for (especie, qualificado) in self.filhos_com_nome(c, novo) {
                            e.adicionar(Severidade::Erro, format!("Reference to renamed {} will be shadowed by {especie} '{qualificado}'.", pedido.especie));
                        }
                    }
                }
                // `_validateWillShadow`.
                for (especie, qualificado, dona, funcoes) in self.declaracoes_de_membro(novo) {
                    for (u, s) in self.usos_nao_qualificados(&funcoes) {
                        if self.classe_que_contem(u, s.start) == Some(dona) {
                            continue;
                        }
                        let visivel = p.unit(u).library == lib || self.visivel_por_import_sem_prefixo(u, alvo);
                        if !visivel {
                            continue;
                        }
                        e.adicionar(Severidade::Erro, format!("Renamed {} will shadow {especie} '{qualificado}'.", pedido.especie));
                    }
                }
            }
            Classe::MembroDeClasse { alvo, classe, .. } => {
                let classe = *classe;
                // `_checkClassAlreadyDeclares`.
                for (especie, _) in self.filhos_com_nome(classe, novo) {
                    e.adicionar(
                        Severidade::Erro,
                        format!(
                            "{} '{}' already declares {especie} with name '{novo}'.",
                            capitalizar(&self.especie_de_classe(classe)),
                            self.nome(p.class(classe).name)
                        ),
                    );
                }
                let Alvo::Membro { dono, nome, estatico } = alvo else { return e };
                let familia = self.familia(*dono, nome, *estatico, false).ok();
                let referencias = self.referencias_do_alvo(alvo);
                let subclasses: Vec<ClassId> = self.subtipos(classe, false);
                // A classe dona de um elemento se chama como o nome novo.
                if let Some(fam) = &familia {
                    let mut donas: BTreeSet<u32> = BTreeSet::new();
                    for f in &fam.funcoes {
                        if let Some(c) = p.function(*f).class {
                            donas.insert(c.0);
                        }
                    }
                    for c in donas {
                        let c = ClassId(c);
                        if self.nome(p.class(c).name) == novo {
                            e.adicionar(
                                Severidade::Erro,
                                format!("Renamed {} has the same name as the declaring {} '{novo}'.", pedido.especie, self.especie_de_classe(c)),
                            );
                        }
                    }
                }
                // `_getShadowingLocalElement`: a primeira sombra por local.
                if let Some((especie_local, nome_local)) = self.sombra_por_local(&referencias, novo) {
                    e.adicionar(Severidade::Erro, format!("Usage of renamed {} will be shadowed by {especie_local} '{nome_local}'.", pedido.especie));
                }
                // `_checkHierarchy(isRename: true)`.
                let supers = self.supertipos(classe);
                for (especie, qualificado, dona, _) in self.declaracoes_de_membro(novo) {
                    if supers.contains(&dona) {
                        e.adicionar(Severidade::Erro, format!("Renamed {} will shadow {especie} '{qualificado}'.", pedido.especie));
                    }
                    if subclasses.contains(&dona) {
                        e.adicionar(Severidade::Erro, format!("Renamed {} will be shadowed by {especie} '{qualificado}'.", pedido.especie));
                    }
                }
                // `_validateWillBeInvisible`.
                if novo.starts_with('_') {
                    let lib = p.class(classe).library;
                    for (u, _) in &referencias {
                        let l = p.unit(*u).library;
                        if l != lib {
                            e.adicionar(Severidade::Erro, format!("Renamed {} will be invisible in '{}'.", pedido.especie, self.caminho_da_biblioteca(l)));
                        }
                    }
                }
            }
            Classe::MembroDeExtensao { alvo, .. } => {
                if let Alvo::Membro { dono: Dono::Extensao(x), .. } = alvo {
                    let ext = p.extension(*x);
                    let mut membros: Vec<(SymbolId, FunctionElementId)> = ext.instance_members.iter().chain(ext.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
                    membros.sort_by_key(|(_, f)| f.0);
                    let nome_ext = ext.name.map(|s| self.nome(s).to_string()).unwrap_or_default();
                    for (s, f) in membros {
                        if nome_base(self.nome(s)) != novo {
                            continue;
                        }
                        let especie = match p.function(f).kind {
                            FunctionKind::Function | FunctionKind::Operator => "method",
                            FunctionKind::Getter => "getter",
                            FunctionKind::Setter => "setter",
                            _ => "field",
                        };
                        e.adicionar(Severidade::Erro, format!("Extension '{nome_ext}' already declares {especie} with name '{novo}'."));
                    }
                    let referencias = self.referencias_do_alvo(alvo);
                    if let Some((especie_local, nome_local)) = self.sombra_por_local(&referencias, novo) {
                        e.adicionar(Severidade::Erro, format!("Usage of renamed {} will be shadowed by {especie_local} '{nome_local}'.", pedido.especie));
                    }
                }
            }
            Classe::Parametro { unidade, declaracao, .. } => {
                let un = p.unit(*unidade);
                let nomeado = crate::projeto::parametro_em(&un.ast, *declaracao).is_some_and(|(q, _)| q.kind == ast::ParameterKind::Named);
                if novo.starts_with('_') && nomeado {
                    e.adicionar(Severidade::Erro, format!("The parameter '{}' is named and can not be private.", pedido.antigo));
                    return e;
                }
                e.somar(self.conflitos_de_local(*unidade, *declaracao, novo, &pedido.especie));
            }
            Classe::Local { unidade, declaracao } => {
                e.somar(self.conflitos_de_local(*unidade, *declaracao, novo, &pedido.especie));
            }
            Classe::ParametroDeTipo { unidade, declaracao } => {
                let ast = &p.unit(*unidade).ast;
                let irmaos = listas_de_parametros_de_tipo(ast).into_iter().find(|l| l.iter().any(|t| t.name.span.start == *declaracao)).unwrap_or_default();
                if irmaos.iter().any(|t| self.nome(t.name.sym) == novo) {
                    e.adicionar(Severidade::Erro, format!("Duplicate type parameter '{novo}'."));
                }
            }
            Classe::Construtor(_) | Classe::Import { .. } | Classe::Rotulo { .. } | Classe::Biblioteca(_) => {}
        }
        e
    }

    /// A classe, mixin, enum ou tipo de extensão que contém o offset.
    fn classe_que_contem(&self, u: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.programa();
        p.classes
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let d = c.decl?;
                let s = p.unit(d.unit).ast.decl(d.decl).span;
                (d.unit == u && s.start <= offset && offset < s.end).then_some((s.end - s.start, ClassId(i as u32)))
            })
            .min_by_key(|(t, _)| *t)
            .map(|(_, c)| c)
    }

    /// `searchMemberDeclarations(nome)`: campos (não sintéticos), acessores
    /// declarados e métodos com o nome, de toda classe do programa: a
    /// espécie, o nome qualificado, a dona e as funções cujas referências
    /// contam.
    fn declaracoes_de_membro(&self, nome: &str) -> Vec<(String, String, ClassId, Vec<FunctionElementId>)> {
        let p = self.programa();
        let mut v = Vec::new();
        for (i, c) in p.classes.iter().enumerate() {
            let dona = ClassId(i as u32);
            let mut membros: Vec<(SymbolId, FunctionElementId)> = c.instance_members.iter().chain(c.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
            membros.sort_by_key(|(_, f)| f.0);
            for (s, f) in &membros {
                let chave = self.nome(*s);
                if nome_base(chave) != nome {
                    continue;
                }
                let fe = p.function(*f);
                let qualificado = format!("{}.{nome}", self.nome(c.name));
                match fe.kind {
                    FunctionKind::Function | FunctionKind::Operator => v.push(("method".to_string(), qualificado, dona, vec![*f])),
                    FunctionKind::ImplicitAccessor if !chave.ends_with('=') => {
                        let mut fs = vec![*f];
                        if let Some((_, g)) = membros.iter().find(|(s2, _)| self.nome(*s2) == format!("{chave}=")) {
                            fs.push(*g);
                        }
                        v.push(("field".to_string(), qualificado, dona, fs));
                    }
                    FunctionKind::Getter => v.push(("getter".to_string(), nome.to_string(), dona, vec![*f])),
                    FunctionKind::Setter => v.push(("setter".to_string(), nome.to_string(), dona, vec![*f])),
                    _ => {}
                }
            }
        }
        v
    }

    /// Os usos sem qualificação (identificador solto) resolvidos para uma
    /// das funções, nas bibliotecas do projeto.
    fn usos_nao_qualificados(&self, funcoes: &[FunctionElementId]) -> Vec<(UnitId, Span)> {
        let p = self.programa();
        let mut v = Vec::new();
        for u in self.unidades() {
            let ast = &p.unit(u).ast;
            let corpos = &self.consulta.corpos.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if let Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(g), .. }) = corpos.get_resolved(ast::ExprId(i as u32))
                    && funcoes.contains(g)
                {
                    v.push((u, n.span));
                }
            }
        }
        v
    }

    /// `_isVisibleAt`: algum import sem prefixo da biblioteca da referência
    /// traz o elemento.
    fn visivel_por_import_sem_prefixo(&self, u: UnitId, alvo: &Alvo) -> bool {
        let p = self.programa();
        let Alvo::Topo(el) = alvo else { return false };
        let lib = p.unit(u).library;
        p.library(lib)
            .imports
            .iter()
            .filter(|i| i.prefix.is_none())
            .any(|i| p.library(i.library).exported.values().any(|b| b.getter == Some(*el) || b.setter == Some(*el)))
    }

    /// A primeira sombra por local (`_getShadowingLocalElement`): para cada
    /// referência não qualificada, um local chamado `novo` (função de topo
    /// ou local, parâmetro simples, variável local) visível nela.
    fn sombra_por_local(&self, referencias: &[(UnitId, Span)], novo: &str) -> Option<(String, String)> {
        let p = self.programa();
        for (u, s) in referencias {
            let un = p.unit(*u);
            let ast = &un.ast;
            let solto = ast.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Identifier(n) if n.span == *s));
            if !solto {
                continue;
            }
            let cx = Contexto::novo(self, *u);
            for (nome, (a, b)) in crate::refatoracoes_embutir::faixas_visiveis(&cx, 0) {
                if nome == novo && !(b <= s.start) && !(a >= s.end) {
                    let funcao = ast.functions.iter().any(|f| f.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == novo && x.span.start >= a));
                    let especie = if funcao { "function" } else if ast.functions.iter().any(|f| f.parameters.iter().flatten().any(|q| q.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == novo))) { "parameter" } else { "local variable" };
                    return Some((especie.to_string(), nome));
                }
            }
            // As funções de topo também são `FunctionDeclaration` com
            // `FunctionElement`, visíveis na unidade inteira.
            for d in ast.decls.iter() {
                if let DeclKind::Function(f) = &d.kind
                    && let Some(n) = ast.function(*f).name
                    && &un.source[n.span.start..n.span.end] == novo
                    && matches!(ast.function(*f).kind, ast::FunctionKind::Function)
                {
                    return Some(("function".to_string(), novo.to_string()));
                }
            }
        }
        None
    }

    /// O `ConflictValidatorVisitor` (§12.9): duplicata de função ou variável
    /// local visível junto do alvo, e uso de outro elemento chamado `novo`
    /// dentro do intervalo do alvo.
    fn conflitos_de_local(&self, u: UnitId, declaracao: usize, novo: &str, especie_alvo: &str) -> Estado {
        let p = self.programa();
        let un = p.unit(u);
        let cx = Contexto::novo(self, u);
        let visiveis = crate::refatoracoes_embutir::faixas_visiveis(&cx, 0);
        let nome_alvo = palavra(&un.source, declaracao).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
        let faixa_do_alvo = visiveis.iter().find(|(n, (a, b))| *n == nome_alvo && *a <= declaracao && declaracao <= *b).map(|(_, f)| *f);
        let mut e = Estado::default();
        let Some((ai, af)) = faixa_do_alvo else { return e };
        let mut conflitantes: Vec<(usize, usize)> = Vec::new();
        // As declarações duplicadas (funções locais e variáveis locais; os
        // parâmetros não são visitados).
        for (nome, (a, b)) in &visiveis {
            if nome != novo || *b <= ai || *a >= af {
                continue;
            }
            let funcao = un.ast.functions.iter().any(|f| f.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == novo && x.span.start >= *a && x.span.end <= *b));
            let variavel = un.ast.stmts.iter().any(|s| match &s.kind {
                ast::StmtKind::Variables(l) => l.variables.iter().any(|v| &un.source[v.name.span.start..v.name.span.end] == novo && v.name.span.start >= *a && v.name.span.end <= *b),
                _ => false,
            });
            if !funcao && !variavel {
                continue;
            }
            e.adicionar(Severidade::Erro, format!("Duplicate {} '{novo}'.", if funcao { "function" } else { "local variable" }));
            conflitantes.push((*a, *b));
        }
        // Os usos de outro elemento chamado `novo` no intervalo do alvo.
        let corpos = &self.consulta.corpos.units[u.0 as usize];
        let arquivo_de = |x: Option<UnitId>| {
            x.and_then(|x| p.unit(x).path.as_deref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned())).unwrap_or_default()
        };
        for (i, x) in un.ast.exprs.iter().enumerate() {
            let ExprKind::Identifier(n) = &x.kind else { continue };
            if self.nome(n.sym) != novo || n.span.start < ai || n.span.start > af {
                continue;
            }
            let id = ast::ExprId(i as u32);
            if let Some(d) = corpos.declaracao_local(id)
                && conflitantes.iter().any(|(a, b)| *a <= d && d <= *b)
            {
                continue;
            }
            let (especie, qualificado, arquivo) = match corpos.get_resolved(id) {
                Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => {
                    let fe = p.function(*f);
                    let c = fe.class.map(|c| self.nome(p.class(c).name).to_string()).unwrap_or_default();
                    let especie = match fe.kind {
                        FunctionKind::Function | FunctionKind::Operator => "method",
                        _ => "field",
                    };
                    (especie, format!("{c}.{novo}"), arquivo_de(self.nome_da_funcao(*f).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Variable(v), .. }) => {
                    let c = p.variable(*v).class.map(|c| self.nome(p.class(c).name).to_string()).unwrap_or_default();
                    ("field", format!("{c}.{novo}"), arquivo_de(self.nome_da_variavel(*v).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Element(el)) => {
                    let (especie, _) = self.especie_de_topo(*el).unwrap_or_default();
                    let especie: &'static str = match especie.as_str() {
                        "function" => "function",
                        "top level variable" => "top level variable",
                        "class" => "class",
                        "mixin" => "mixin",
                        "enum" => "enum",
                        _ => "type alias",
                    };
                    (especie, novo.to_string(), arquivo_de(self.nome_do_elemento_de_topo(*el).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Parameter { .. }) => ("parameter", novo.to_string(), arquivo_de(Some(u))),
                Some(dartforge_types::Resolved::Local(_)) => {
                    // `ElementKind.displayName`: o local declarado numa lista de
                    // parâmetros é `parameter`.
                    let parametro = corpos.declaracao_local(id).is_some_and(|d| {
                        un.ast.functions.iter().any(|f| f.parameters.iter().flatten().any(|q| q.name.is_some_and(|x| x.span.start == d)))
                    });
                    (if parametro { "parameter" } else { "local variable" }, novo.to_string(), arquivo_de(Some(u)))
                }
                _ => continue,
            };
            e.adicionar(Severidade::Erro, format!("Usage of {especie} \"{qualificado}\" declared in \"{arquivo}\" will be shadowed by renamed {especie_alvo}."));
        }
        e
    }

    // -- As edições -------------------------------------------------------------

    /// `fillChange` de cada classe.
    fn edicoes_do_renomear(&self, pedido: &Pedido, novo: &str) -> Result<Vec<Edicao>, String> {
        let p = self.programa();
        let mut edicoes: Vec<Edicao> = Vec::new();
        let por = |u: UnitId, s: Span, texto: &str, edicoes: &mut Vec<Edicao>| {
            if !self.no_workspace(Some(u)) {
                return;
            }
            if let Some(uri) = self.uri_da_unidade(u)
                && !edicoes.iter().any(|e| e.uri == uri && e.span == s)
            {
                edicoes.push(Edicao { uri, span: s, texto: texto.to_string() });
            }
        };
        match &pedido.classe {
            Classe::Construtor(f) => return self.edicoes_de_construtor(*f, novo),
            Classe::Import { biblioteca, indice } => return Ok(self.edicoes_de_import(*biblioteca, *indice, novo)),
            Classe::MembroDeUnidade(alvo) | Classe::MembroDeExtensao { alvo, .. } => {
                for (u, a, b) in self.declaracoes(alvo) {
                    por(u, Span { start: a, end: b }, novo, &mut edicoes);
                }
                for (u, s) in self.referencias_do_alvo(alvo) {
                    por(u, s, novo, &mut edicoes);
                }
            }
            Classe::MembroDeClasse { alvo, .. } => {
                for (u, a, b) in self.declaracoes(alvo) {
                    por(u, Span { start: a, end: b }, novo, &mut edicoes);
                }
                for (u, s) in self.referencias_do_alvo(alvo) {
                    // O `this.x` nomeado que vira privado: `{T x}` e o
                    // inicializador `_novo = x`.
                    if novo.starts_with('_')
                        && let Some(e) = self.parametro_privado(u, s, novo)
                    {
                        edicoes.extend(e);
                        continue;
                    }
                    // As referências dos `this.x` (o início de um argumento
                    // posicional opcional, de comprimento 0, que vira
                    // inserção, e os `super.x`) só entram com nome novo
                    // público (`fillChange`).
                    if novo.starts_with('_') && (s.start == s.end || self.e_super_formal(u, s)) {
                        continue;
                    }
                    por(u, s, novo, &mut edicoes);
                }
            }
            Classe::Parametro { unidade, declaracao, campo } => {
                if let Some(v) = campo {
                    // O campo: a declaração e todas as referências dele.
                    let alvo = self.membro_de_variavel(*v);
                    if let Some((u, s)) = self.nome_da_variavel(*v) {
                        por(u, s, novo, &mut edicoes);
                    }
                    for (u, s) in self.referencias_do_alvo(&alvo) {
                        por(u, s, novo, &mut edicoes);
                    }
                } else {
                    let alvo = Alvo::Local { unidade: *unidade, declaracao: *declaracao };
                    let un = p.unit(*unidade);
                    let nomeado = crate::projeto::parametro_em(&un.ast, *declaracao).is_some_and(|(q, _)| q.kind == ast::ParameterKind::Named);
                    let fim = palavra(&un.source, *declaracao).map_or(*declaracao, |s| s.end);
                    por(*unidade, Span { start: *declaracao, end: fim }, novo, &mut edicoes);
                    // `RenameParameterRefactoringImpl._getElements`: o nomeado
                    // das sobrescritas é renomeado com a declaração (o modo de
                    // renomear das ocorrências, que inclui os homônimos).
                    let referencias: Vec<(UnitId, Span)> = match self.ocorrencias(&alvo, true) {
                        Ok(oc) => oc
                            .into_iter()
                            .filter(|&(u, a, _)| !(u == *unidade && a == *declaracao))
                            .map(|(u, a, b)| (u, Span { start: a, end: b }))
                            .collect(),
                        Err(_) => self.referencias_do_alvo(&alvo),
                    };
                    for (u, s) in referencias {
                        // Sem as implícitas de comprimento 0; num posicional,
                        // sem os `super.x`.
                        if s.start == s.end {
                            continue;
                        }
                        if !nomeado && self.e_super_formal(u, s) {
                            continue;
                        }
                        por(u, s, novo, &mut edicoes);
                    }
                    // O `[x]` do doc do membro.
                    for (u, s) in self.docs_de_parametro(*unidade, *declaracao) {
                        por(u, s, novo, &mut edicoes);
                    }
                }
            }
            Classe::Local { unidade, declaracao } | Classe::ParametroDeTipo { unidade, declaracao } => {
                let alvo = match &pedido.classe {
                    Classe::Local { .. } => Alvo::Local { unidade: *unidade, declaracao: *declaracao },
                    _ => Alvo::ParametroDeTipo { unidade: *unidade, declaracao: *declaracao },
                };
                for (u, a, b) in self.ocorrencias(&alvo, true)? {
                    por(u, Span { start: a, end: b }, novo, &mut edicoes);
                }
            }
            Classe::Rotulo { unidade, declaracao } => {
                let un = p.unit(*unidade);
                let cx = Contexto::novo(self, *unidade);
                let fim = palavra(&un.source, *declaracao).map_or(*declaracao, |s| s.end);
                por(*unidade, Span { start: *declaracao, end: fim }, novo, &mut edicoes);
                let nome = &un.source[*declaracao..fim];
                for (k, no) in cx.arvore.nos.iter().enumerate() {
                    if no.especie == "SimpleIdentifier"
                        && cx.pai(k).is_some_and(|x| matches!(cx.especie(x), "BreakStatement" | "ContinueStatement"))
                        && cx.texto_do_no(k) == nome
                    {
                        por(*unidade, Span { start: no.inicio, end: no.fim }, novo, &mut edicoes);
                    }
                }
            }
            Classe::Biblioteca(l) => {
                for &u in p.library(*l).units.iter() {
                    for d in p.unit(u).unit.directives.iter() {
                        let nome: &[ast::Name] = match &d.kind {
                            ast::DirectiveKind::Library { name } => name,
                            ast::DirectiveKind::PartOf { name, .. } => name,
                            _ => continue,
                        };
                        if let (Some(a), Some(b)) = (nome.first(), nome.last()) {
                            por(u, Span { start: a.span.start, end: b.span.end }, novo, &mut edicoes);
                        }
                    }
                }
            }
        }
        Ok(edicoes)
    }

    /// O `super.x` em `s`.
    fn e_super_formal(&self, u: UnitId, s: Span) -> bool {
        let ast = &self.programa().unit(u).ast;
        ast.members.iter().any(|m| match &m.kind {
            ast::MemberKind::Constructor(k) => k.parameters.iter().any(|q| q.super_ && q.name.is_some_and(|n| n.span == s)),
            _ => false,
        })
    }

    /// Os `[x]` do comentário de documentação do membro que declara o
    /// parâmetro.
    fn docs_de_parametro(&self, unidade: UnitId, declaracao: usize) -> Vec<(UnitId, Span)> {
        let u = self.programa().unit(unidade);
        let nome = palavra(&u.source, declaracao).map(|s| u.source[s.start..s.end].to_string()).unwrap_or_default();
        let mut v = Vec::new();
        for c in crate::dartdoc::comentarios(&u.source) {
            for r in &c.referencias {
                if r.len() != 1 || r[0].1 != nome {
                    continue;
                }
                // O comentário documenta a declaração logo depois dele
                // (membro, declaração de topo ou função local), que declara
                // o parâmetro.
                let declara = |ps: &[ast::Parameter]| ps.iter().any(|q| q.name.is_some_and(|n| n.span.start == declaracao));
                let membro = u.ast.members.iter().filter(|m| m.span.start >= c.span.end).min_by_key(|m| m.span.start);
                let topo = u.ast.decls.iter().filter(|d| d.span.start >= c.span.end).min_by_key(|d| d.span.start);
                let funcao = u.ast.functions.iter().filter(|f| f.span.start >= c.span.end).min_by_key(|f| f.span.start);
                let pm = membro.map_or(usize::MAX, |m| m.span.start);
                let pt = topo.map_or(usize::MAX, |d| d.span.start);
                let pf = funcao.map_or(usize::MAX, |f| f.span.start);
                let documenta = if pm <= pt && pm <= pf {
                    membro.is_some_and(|m| match &m.kind {
                        ast::MemberKind::Constructor(k) => declara(&k.parameters),
                        ast::MemberKind::Method(f) => declara(u.ast.function(*f).parameters.as_deref().unwrap_or(&[])),
                        ast::MemberKind::Field(_) => false,
                    })
                } else if pt <= pf {
                    topo.is_some_and(|d| matches!(&d.kind, ast::DeclKind::Function(f) if declara(u.ast.function(*f).parameters.as_deref().unwrap_or(&[]))))
                } else {
                    funcao.is_some_and(|f| declara(f.parameters.as_deref().unwrap_or(&[])))
                };
                if documenta {
                    v.push((unidade, r[0].0));
                }
            }
        }
        v
    }

    /// `_addPrivateNamedFormalParameterEdit` para a referência `s` (o `x` de
    /// um `{this.x}`): `{T x}` e o inicializador `_novo = x`.
    fn parametro_privado(&self, u: UnitId, s: Span, novo: &str) -> Option<Vec<Edicao>> {
        let p = self.programa();
        let un = p.unit(u);
        let uri = self.uri_da_unidade(u)?;
        for (mi, m) in un.ast.members.iter().enumerate() {
            let ast::MemberKind::Constructor(k) = &m.kind else { continue };
            let Some(q) = k.parameters.iter().find(|q| q.this_ && q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| n.span == s)) else { continue };
            let nome = q.name?;
            let campo = self
                .construtor_do_no(u, ast::MemberId(mi as u32))
                .and_then(|f| p.function(f).class)
                .and_then(|c| p.class(c).fields.iter().copied().find(|v| p.variable(*v).name == nome.sym))?;
            let tipo = self.consulta.tipo_da_variavel(campo).map(|t| self.consulta.formatar(t)).unwrap_or_else(|| "dynamic".to_string());
            let this_inicio = un.source[..nome.span.start].rfind("this")?;
            let mut v = vec![Edicao { uri: uri.clone(), span: Span { start: this_inicio, end: nome.span.start }, texto: format!("{tipo} ") }];
            let x = &un.source[nome.span.start..nome.span.end];
            let fim = k.parameters.last().map_or(nome.span.end, |q| q.span.end);
            if k.initializers.is_empty() {
                let fecha = un.source[fim..].find(')').map_or(fim, |i| fim + i + 1);
                v.push(Edicao { uri, span: Span { start: fecha, end: fecha }, texto: format!(" : {novo} = {x}") });
            } else {
                let dois_pontos = un.source[fim..].find(':').map_or(fim, |i| fim + i + 1);
                v.push(Edicao { uri, span: Span { start: dois_pontos, end: dois_pontos }, texto: format!(" {novo} = {x},") });
            }
            return Some(v);
        }
        None
    }

    /// `RenameImportRefactoringImpl.fillChange` (§12.8).
    fn edicoes_de_import(&self, lib: LibraryId, indice: usize, novo: &str) -> Vec<Edicao> {
        let p = self.programa();
        let imp = &p.library(lib).imports[indice];
        let mut v = Vec::new();
        let Some(uri) = self.uri_da_unidade(imp.unit) else { return v };
        let un = p.unit(imp.unit);
        let Some(d) = un.unit.directives.get(imp.directive) else { return v };
        let ast::DirectiveKind::Import { uri: lit, prefix, .. } = &d.kind else { return v };
        match (prefix, novo.is_empty()) {
            (Some(pr), true) => v.push(Edicao { uri: uri.clone(), span: Span { start: lit.span.end, end: pr.span.end }, texto: String::new() }),
            (None, false) => v.push(Edicao { uri: uri.clone(), span: Span { start: lit.span.end, end: lit.span.end }, texto: format!(" as {novo}") }),
            (Some(pr), false) => v.push(Edicao { uri: uri.clone(), span: pr.span, texto: novo.to_string() }),
            (None, true) => {}
        }
        // Os usos dos elementos do namespace do import.
        let namespace = &p.library(imp.library).exported;
        let visivel = |s: SymbolId| {
            namespace.contains_key(&s)
                && imp.combinators.iter().all(|c| match c {
                    ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == s),
                    ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == s),
                })
        };
        let substituto = if novo.is_empty() { String::new() } else { format!("{novo}.") };
        let da_diretiva = v.len();
        for &x in p.library(lib).units.iter() {
            let ux = p.unit(x);
            let Some(uri_x) = self.uri_da_unidade(x) else { continue };
            let fx = ux.source.as_str();
            match prefix {
                Some(pr) => {
                    // `RenameImportRefactoringImpl`: o `p.` de cada uso (com o
                    // ponto) vira `novo.` (ou some).
                    let ate_o_proximo = |s: Span| {
                        let ponto = fx[s.end..].find('.').map_or(s.end, |k| s.end + k + 1);
                        let espacos = fx[ponto..].len() - fx[ponto..].trim_start().len();
                        Span { start: s.start, end: ponto + espacos }
                    };
                    let usar = |pn: &ast::Name, v: &mut Vec<Edicao>| {
                        v.push(Edicao { uri: uri_x.clone(), span: ate_o_proximo(pn.span), texto: substituto.clone() });
                    };
                    for anotacao in dartforge_frontend::pais::todas_as_anotacoes(&ux.ast, &ux.unit) {
                        if let [pn, n, ..] = &anotacao.name[..]
                            && pn.sym == pr.sym
                            && visivel(n.sym)
                        {
                            usar(pn, &mut v);
                        }
                    }
                    for e in ux.ast.exprs.iter() {
                        if let ExprKind::Property { target, name, .. } = &e.kind
                            && let ExprKind::Identifier(pn) = &ux.ast.expr(*target).kind
                            && pn.sym == pr.sym
                            && visivel(name.sym)
                        {
                            usar(pn, &mut v);
                        }
                    }
                    for t in ux.ast.types.iter() {
                        if let ast::TypeKind::Named { name, .. } = &t.kind
                            && let [pn, n] = &name[..]
                            && pn.sym == pr.sym
                            && visivel(n.sym)
                        {
                            usar(pn, &mut v);
                        }
                    }
                    // `[p.x]` na documentação: o índice também cita o import.
                    let nome_do_prefixo = &un.source[pr.span.start..pr.span.end];
                    if fx.contains('[') {
                        for c in crate::dartdoc::comentarios(fx) {
                            for r in &c.referencias {
                                if r.len() >= 2
                                    && r[0].1 == nome_do_prefixo
                                    && self.consulta.nomes.lookup(&r[1].1).is_some_and(visivel)
                                {
                                    v.push(Edicao { uri: uri_x.clone(), span: ate_o_proximo(r[0].0), texto: substituto.clone() });
                                }
                            }
                        }
                    }
                }
                None => {
                    // Sem prefixo: `novo.` antes de cada uso não prefixado de
                    // elemento do import (`"$x"` → `"${novo.x}"`).
                    let corpos = &self.consulta.corpos.units[x.0 as usize];
                    for (i, e) in ux.ast.exprs.iter().enumerate() {
                        let ExprKind::Identifier(n) = &e.kind else { continue };
                        let Some(dartforge_types::Resolved::Element(el)) = corpos.get_resolved(ast::ExprId(i as u32)) else { continue };
                        if !namespace.get(&n.sym).is_some_and(|b| b.getter == Some(*el) || b.setter == Some(*el)) || !visivel(n.sym) {
                            continue;
                        }
                        let interpolado = n.span.start > 0 && fx.as_bytes()[n.span.start - 1] == b'$';
                        if interpolado {
                            v.push(Edicao { uri: uri_x.clone(), span: n.span, texto: format!("{{{novo}.{}}}", &fx[n.span.start..n.span.end]) });
                        } else {
                            v.push(Edicao { uri: uri_x.clone(), span: Span { start: n.span.start, end: n.span.start }, texto: substituto.clone() });
                        }
                    }
                    for t in ux.ast.types.iter() {
                        if let ast::TypeKind::Named { name, .. } = &t.kind
                            && let [n] = &name[..]
                            && visivel(n.sym)
                            && tipo_do_import(p, x, n.sym, imp.library)
                        {
                            v.push(Edicao { uri: uri_x.clone(), span: Span { start: n.span.start, end: n.span.start }, texto: substituto.clone() });
                        }
                    }
                }
            }
        }
        // Os usos na ordem da busca: por unidade e pela posição.
        v[da_diretiva..].sort_by_key(|e| e.span.start);
        v
    }

    /// O arquivo da classe renomeada (§12.1 passo 17): só classe, mixin,
    /// enum ou tipo de extensão cujo arquivo se chama `toFileName` do nome
    /// antigo.
    fn arquivo_da_classe(&self, pedido: &Pedido, novo: &str) -> Option<RenomearArquivo> {
        let Classe::MembroDeUnidade(Alvo::Topo(Element::Class(c))) = &pedido.classe else { return None };
        let d = self.programa().class(*c).decl?;
        let caminho = self.programa().unit(d.unit).path.clone()?;
        if caminho.file_name()?.to_str()? != nome_de_arquivo(&pedido.antigo) {
            return None;
        }
        let destino = caminho.with_file_name(nome_de_arquivo(novo));
        Some(RenomearArquivo { de: Url::from_file_path(&caminho).ok()?.to_string(), para: Url::from_file_path(&destino).ok()?.to_string(), diretivas: Vec::new() })
    }

    /// As edições de `RenameConstructorRefactoringImpl.fillChange` (§12.7):
    /// cada referência (`.nome` ou comprimento 0) vira `.novo` (sem nome:
    /// `.new` num tear-off, nada nos outros; a constante de enum sem
    /// argumentos ganha `()`); a chamada implícita do super-construtor ganha
    /// `super.novo()`; a declaração troca `.nome` (ou o vazio no fim do nome
    /// da classe) por `.novo`.
    pub(crate) fn edicoes_de_construtor(&self, f: FunctionElementId, novo: &str) -> Result<Vec<Edicao>, String> {
        let p = self.programa();
        let mut edicoes = Vec::new();
        let declaracao = self.nome_da_funcao(f);
        let ponto_novo = if novo.is_empty() { String::new() } else { format!(".{novo}") };
        for (u, inicio, fim) in self.ocorrencias(&Alvo::Construtor(f), false)? {
            if !self.no_workspace(Some(u)) {
                continue;
            }
            let Some(uri) = self.uri_da_unidade(u) else { continue };
            let unidade = p.unit(u);
            let fonte = unidade.source.as_str();
            let ast = &unidade.ast;
            // A declaração.
            if declaracao == Some((u, Span { start: inicio, end: fim })) {
                let sem_nome = self.nome(p.function(f).name).is_empty();
                let span = if sem_nome {
                    Span { start: fim, end: fim }
                } else {
                    let antes = fonte[..inicio].trim_end();
                    Span { start: if antes.ends_with('.') { antes.len() - 1 } else { inicio }, end: fim }
                };
                edicoes.push(Edicao { uri, span, texto: ponto_novo.clone() });
                continue;
            }
            // A subclasse sem construtor declarado: o nome da classe.
            let classe_sem_construtor = ast.decls.iter().find_map(|d| match &d.kind {
                DeclKind::Class(c) if c.name.span.start == inicio && c.name.span.end == fim => Some(d),
                _ => None,
            });
            if let Some(d) = classe_sem_construtor {
                let DeclKind::Class(c) = &d.kind else { continue };
                let chave = fonte[c.name.span.end..d.span.end].find('{').map(|k| c.name.span.end + k + 1);
                if let Some(o) = chave {
                    let nome = &fonte[c.name.span.start..c.name.span.end];
                    let chamada = if novo.is_empty() { "super()".to_string() } else { format!("super.{novo}()") };
                    edicoes.push(Edicao { uri, span: Span { start: o, end: o }, texto: format!("\n  {nome}() : {chamada};") });
                }
                continue;
            }
            // O construtor da subclasse sem `super(...)` explícito.
            let implicito = ast.members.iter().find_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) if k.class_name.span.start == inicio => Some(k),
                _ => None,
            });
            if let Some(k) = implicito {
                let chamada = if novo.is_empty() { "super()".to_string() } else { format!("super.{novo}()") };
                let (o, texto) = match k.initializers.last() {
                    Some(ultimo) => {
                        let s = match ultimo {
                            ast::Initializer::Field { span, .. }
                            | ast::Initializer::Super { span, .. }
                            | ast::Initializer::Redirect { span, .. }
                            | ast::Initializer::Assert { span, .. } => *span,
                        };
                        (s.end, format!(", {chamada}"))
                    }
                    None => {
                        let fim_dos_parametros = k.parameters.last().map_or(k.name.map_or(k.class_name.span.end, |n| n.span.end), |q| q.span.end);
                        let fecha = fonte[fim_dos_parametros..].find(')').map_or(fim_dos_parametros, |x| fim_dos_parametros + x + 1);
                        (fecha, format!(" : {chamada}"))
                    }
                };
                edicoes.push(Edicao { uri, span: Span { start: o, end: o }, texto });
                continue;
            }
            // As demais referências.
            let tear_off = fonte[inicio..fim] == *".new" && !fonte[fim..].trim_start().starts_with('(');
            let mut texto = if novo.is_empty() {
                if tear_off { ".new".to_string() } else { String::new() }
            } else {
                ponto_novo.clone()
            };
            let constante_sem_argumentos = inicio == fim
                && ast.decls.iter().any(|d| match &d.kind {
                    DeclKind::Enum(e) => e.constants.iter().any(|c| c.name.span.end == inicio && c.arguments.is_none()),
                    _ => false,
                });
            if constante_sem_argumentos {
                texto.push_str("()");
            }
            edicoes.push(Edicao { uri, span: Span { start: inicio, end: fim }, texto });
        }
        Ok(edicoes)
    }
}

/// O nome simples `n` da unidade `u` resolve para o elemento que a
/// biblioteca `alvo` exporta (o import sem prefixo o traz).
fn tipo_do_import(p: &dartforge_elements::model::Program, u: UnitId, n: SymbolId, alvo: LibraryId) -> bool {
    match (p.lookup_na_unidade(u, n).and_then(|b| b.getter), p.library(alvo).exported.get(&n).and_then(|b| b.getter)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// `capitalize`.
fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// As listas de parâmetros de tipo da unidade.
fn listas_de_parametros_de_tipo(ast: &ast::Ast) -> Vec<Vec<&ast::TypeParameter>> {
    let mut v: Vec<Vec<&ast::TypeParameter>> = Vec::new();
    for d in &ast.decls {
        let ps = match &d.kind {
            DeclKind::Class(c) => &c.type_params,
            DeclKind::Mixin(m) => &m.type_params,
            DeclKind::Enum(e) => &e.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        v.push(ps.iter().collect());
    }
    for f in &ast.functions {
        v.push(f.type_params.iter().collect());
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            v.push(type_params.iter().collect());
        }
    }
    v
}

/// `prepareRename`: o intervalo do `getElementToRename` e o `oldName`;
/// `Ok(None)` sem nada renomeável.
///
/// # Erros
///
/// `checkInitialConditions` fatal: a mensagem (`-32010`).
pub(crate) fn preparar(projeto: &Projeto, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else { return Ok(None) };
    let Some(pedido) = projeto.pedido_de_renomear(unidade, offset) else { return Ok(None) };
    let iniciais = projeto.condicoes_iniciais(&pedido);
    if iniciais.tem_fatal() {
        return Err(iniciais.mensagem().unwrap_or_default());
    }
    Ok(Some((pedido.faixa, pedido.antigo)))
}

/// `rename`: as edições, ou a mensagem do `-32010`.
///
/// # Erros
///
/// `checkInitialConditions` fatal, `checkNewName` com erro, ou
/// `checkFinalConditions` fatal: a mensagem.
pub(crate) fn renomear(projeto: &Projeto, uri: &str, offset: usize, novo: &str) -> Result<Renomeacao, String> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else {
        return Ok(Renomeacao { nulo: true, ..Default::default() });
    };
    let Some(pedido) = projeto.pedido_de_renomear(unidade, offset) else {
        return Ok(Renomeacao { nulo: true, ..Default::default() });
    };
    let iniciais = projeto.condicoes_iniciais(&pedido);
    if iniciais.tem_fatal() {
        return Err(iniciais.mensagem().unwrap_or_default());
    }
    let nome = projeto.verificar_nome_novo(&pedido, novo);
    if nome.tem_erro() {
        return Err(nome.mensagem().unwrap_or_default());
    }
    let finais = projeto.condicoes_finais(&pedido, novo);
    if finais.tem_fatal() {
        return Err(finais.mensagem().unwrap_or_default());
    }
    let aviso = if finais.ok() { None } else { finais.mensagem() };
    let edicoes = projeto.edicoes_do_renomear(&pedido, novo)?;
    let arquivo = projeto.arquivo_da_classe(&pedido, novo);
    Ok(Renomeacao { edicoes, arquivo, aviso, nulo: false })
}

#[cfg(test)]
mod testes {
    use super::nome_de_arquivo;

    #[test]
    fn nomes_de_arquivo() {
        assert_eq!(nome_de_arquivo("MinhaClasse"), "minha_classe.dart");
        assert_eq!(nome_de_arquivo("HTTPServer"), "h_t_t_p_server.dart");
        assert_eq!(nome_de_arquivo("A"), "a.dart");
        assert_eq!(nome_de_arquivo("_Foo"), "__foo.dart");
    }
}

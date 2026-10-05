//! As checagens das anotações do `package:meta`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.7), escritas com os
//! emissores da 6.11.0 abertos:
//!
//! * o `AnnotationVerifier` (`analyzer/lib/src/error/annotation_verifier.dart`):
//!   `invalid_factory_method_decl`, `invalid_factory_method_impl`,
//!   `invalid_internal_annotation`, `invalid_literal_annotation`,
//!   `invalid_non_virtual_annotation`, `invalid_reopen_annotation`,
//!   `invalid_annotation_target` (só o caso de `@redeclare`),
//!   `undefined_referenced_parameter`, `invalid_visibility_annotation`,
//!   `invalid_visible_for_overriding_annotation` e
//!   `invalid_visible_outside_template_annotation`;
//! * do `BestPracticesVerifier` (`best_practices_verifier.dart`):
//!   `invalid_required_named_param`, `…_optional_positional_param`,
//!   `…_positional_param`, `import_deferred_library_with_load_function`,
//!   `must_be_immutable`, `invalid_override_of_non_virtual_member`,
//!   `invalid_export_of_internal_element` e
//!   `invalid_export_of_internal_element_indirectly`.
//!
//! As anotações são reconhecidas pelo nome, sem conferir que vêm do
//! `package:meta`. O `invalid_annotation_target` geral (`_checkKinds`) lê o
//! `@Target({...})` da classe da anotação, achada pelo escopo da unidade;
//! parâmetros e parâmetros de tipo não são visitados.
//! Escrito sem compilar nem executar (2026-10-04).

use dartforge_diagnostics::codigos::{hint as h, warning as w};
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId, VariableRef};
use dartforge_frontend::ast::{
    self, Combinator, DeclKind, DirectiveKind, ExprKind, FunctionBody, FunctionKind, MemberKind, ParameterKind, StmtKind, TypeKind,
};
use dartforge_intern::{Interner, SymbolId};

/// Onde a anotação está.
#[derive(Clone, Copy)]
enum Alvo<'a> {
    Topo(&'a ast::Decl),
    Membro { membro: &'a ast::Member, dono: &'a ast::Decl },
    Constante { constante: &'a ast::EnumConstant, dono: &'a ast::Decl },
    Diretiva,
}

struct Ctx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    a: &'a ast::Ast,
    unidade: UnitId,
    em_api_publica: bool,
    /// A diretiva que está sendo visitada é a primeira da unidade.
    primeira_diretiva: bool,
    out: Vec<(UnitId, Diagnostic)>,
}

impl<'a> Ctx<'a> {
    fn texto(&self, n: ast::Name) -> &'a str {
        self.interner.resolve(n.sym)
    }

    fn relatar(&mut self, codigo: Codigo, span: Span, args: &[&str]) {
        self.out.push((self.unidade, Diagnostic::com_codigo(codigo, span, args.iter().copied())));
    }

    /// `node.name`: o identificador da anotação (uma ou duas partes; com
    /// três, a terceira é o nome do construtor), a posição e o texto.
    fn nome_da_anotacao(&self, m: &ast::Annotation) -> (Span, String) {
        let partes = if m.name.len() >= 3 { &m.name[..2] } else { &m.name[..] };
        let span = match (partes.first(), partes.last()) {
            (Some(p), Some(u)) => Span { start: p.span.start, end: u.span.end },
            _ => m.span,
        };
        (span, partes.iter().map(|n| self.texto(*n)).collect::<Vec<_>>().join("."))
    }

    fn tem(&self, m: &ast::Annotation, nome: &str) -> bool {
        m.name.iter().any(|n| self.texto(*n) == nome)
    }

    /// O intervalo de um `VariableDeclaration`: do nome ao fim do
    /// inicializador.
    fn variavel(&self, v: &ast::Variable) -> Span {
        Span { start: v.name.span.start, end: v.initializer.map_or(v.name.span.end, |e| self.a.expr(e).span.end) }
    }

    /// O nome do elemento declarado por uma declaração de topo.
    fn nome_do_topo(&self, d: &ast::Decl) -> Option<&'a str> {
        match &d.kind {
            DeclKind::Class(x) => Some(self.texto(x.name)),
            DeclKind::Mixin(x) => Some(self.texto(x.name)),
            DeclKind::Enum(x) => Some(self.texto(x.name)),
            DeclKind::ExtensionType(x) => Some(self.texto(x.name)),
            DeclKind::Extension(x) => x.name.map(|n| self.texto(n)),
            DeclKind::Typedef(x) => Some(self.texto(x.name)),
            DeclKind::Function(f) => self.a.function(*f).name.map(|n| self.texto(n)),
            DeclKind::Variables(_) => None,
        }
    }

    /// O nome do elemento do alvo, fora as listas de variáveis.
    fn nome_do_alvo(&self, alvo: Alvo<'a>) -> Option<&'a str> {
        match alvo {
            Alvo::Topo(d) => self.nome_do_topo(d),
            Alvo::Membro { membro, .. } => match &membro.kind {
                MemberKind::Method(f) => self.a.function(*f).name.map(|n| self.texto(n)),
                MemberKind::Constructor(k) => Some(k.name.map_or("", |n| self.texto(n))),
                MemberKind::Field(_) => None,
            },
            Alvo::Constante { constante, .. } => Some(self.texto(constante.name)),
            Alvo::Diretiva => None,
        }
    }

    /// As variáveis de uma lista de topo ou de um campo.
    fn variaveis(&self, alvo: Alvo<'a>) -> Option<(&'a ast::VariableList, bool)> {
        match alvo {
            Alvo::Topo(d) => match &d.kind {
                DeclKind::Variables(l) => Some((l, true)),
                _ => None,
            },
            Alvo::Membro { membro, .. } => match &membro.kind {
                MemberKind::Field(l) => Some((l, false)),
                _ => None,
            },
            _ => None,
        }
    }

    /// `new Foo()`, `Foo()` (criação sem `new`, reconhecida pela inicial
    /// maiúscula) ou `null`.
    fn expressao_de_fabrica(&self, e: ast::ExprId) -> bool {
        let maiuscula = |n: ast::Name| self.texto(n).chars().next().is_some_and(char::is_uppercase);
        match &self.a.expr(e).kind {
            ExprKind::InstanceCreation { .. } | ExprKind::Null => true,
            ExprKind::Call { target, .. } => match &self.a.expr(*target).kind {
                ExprKind::Identifier(n) => maiuscula(*n),
                ExprKind::Property { target, .. } => matches!(&self.a.expr(*target).kind, ExprKind::Identifier(n) if maiuscula(*n)),
                _ => false,
            },
            _ => false,
        }
    }

    /// `_checkFactory`.
    fn fabrica(&mut self, alvo: Alvo<'a>) {
        let Alvo::Membro { membro, .. } = alvo else { return };
        let MemberKind::Method(f) = &membro.kind else { return };
        let func = self.a.function(*f);
        let Some(nome) = func.name else { return };
        if func.return_type.is_some_and(|t| matches!(self.a.types[t.0 as usize].kind, TypeKind::Void)) {
            self.relatar(w::INVALID_FACTORY_METHOD_DECL, nome.span, &[self.texto(nome)]);
            return;
        }
        let ok = match &func.body {
            // Abstrato (ou nativo): nada a conferir.
            FunctionBody::Empty | FunctionBody::Native(_) => true,
            FunctionBody::Expression(e) => self.expressao_de_fabrica(*e),
            FunctionBody::Block(s) => match &self.a.stmts[s.0 as usize].kind {
                StmtKind::Block(comandos) => comandos.last().is_some_and(|u| match &self.a.stmts[u.0 as usize].kind {
                    StmtKind::Return(Some(e)) => self.expressao_de_fabrica(*e),
                    _ => false,
                }),
                _ => false,
            },
        };
        if !ok {
            self.relatar(w::INVALID_FACTORY_METHOD_IMPL, nome.span, &[self.texto(nome)]);
        }
    }

    /// `_checkInternal`.
    fn interno(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        let (no_nome, _) = self.nome_da_anotacao(m);
        if let Some((lista, _)) = self.variaveis(alvo) {
            for v in lista.variables.iter() {
                if self.texto(v.name).starts_with('_') {
                    let span = self.variavel(v);
                    self.relatar(w::INVALID_INTERNAL_ANNOTATION, span, &[]);
                }
            }
            return;
        }
        let privado = self.nome_do_alvo(alvo).is_some_and(|n| n.starts_with('_'));
        let classe_privada = match alvo {
            Alvo::Membro { membro, dono } if matches!(membro.kind, MemberKind::Constructor(_)) => {
                self.nome_do_topo(dono).is_some_and(|n| n.starts_with('_'))
            }
            _ => false,
        };
        let construtor = matches!(alvo, Alvo::Membro { membro, .. } if matches!(membro.kind, MemberKind::Constructor(_)));
        if construtor {
            if classe_privada || privado {
                self.relatar(w::INVALID_INTERNAL_ANNOTATION, no_nome, &[]);
            }
        } else if privado || self.em_api_publica {
            self.relatar(w::INVALID_INTERNAL_ANNOTATION, no_nome, &[]);
        }
    }

    /// `_checkNonVirtual`.
    fn nao_virtual(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        let valido = match alvo {
            Alvo::Membro { membro, dono } => match &membro.kind {
                MemberKind::Field(l) => !l.static_,
                MemberKind::Method(f) => {
                    let func = self.a.function(*f);
                    let abstrato = matches!(func.body, FunctionBody::Empty) && !func.external;
                    !(matches!(dono.kind, DeclKind::Extension(_) | DeclKind::ExtensionType(_)) || func.static_ || abstrato)
                }
                MemberKind::Constructor(_) => false,
            },
            _ => false,
        };
        if !valido {
            let (span, _) = self.nome_da_anotacao(m);
            self.relatar(w::INVALID_NON_VIRTUAL_ANNOTATION, span, &[]);
        }
    }

    /// `_checkRedeclare`.
    fn redeclarar(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        let valido = match alvo {
            Alvo::Membro { membro, dono } if matches!(dono.kind, DeclKind::ExtensionType(_)) => match &membro.kind {
                MemberKind::Method(f) => !self.a.function(*f).static_,
                _ => true,
            },
            _ => false,
        };
        if !valido {
            let (span, texto) = self.nome_da_anotacao(m);
            self.relatar(w::INVALID_ANNOTATION_TARGET, span, &[texto.as_str(), "instance members of extension types"]);
        }
    }

    /// `_checkReopen`: só em classe, com a superclasse da mesma biblioteca
    /// e os modificadores que a anotação reabre.
    fn reabrir(&mut self, m: &ast::Annotation, classe: Option<ClassId>) {
        let Some(c) = classe else { return };
        let e = self.program.class(c);
        let Some(s) = e.supertype_class.map(|s| self.program.class(s)) else { return };
        if s.kind != ClassKind::Class {
            return;
        }
        let (k, sm) = (e.modifiers, s.modifiers);
        let invalido = if k.final_ || k.mixin || k.sealed || e.library != s.library {
            true
        } else if k.base {
            !sm.final_ && !sm.interface
        } else if !k.interface {
            !sm.interface
        } else {
            false
        };
        if invalido {
            let (span, _) = self.nome_da_anotacao(m);
            self.relatar(w::INVALID_REOPEN_ANNOTATION, span, &[]);
        }
    }

    /// `_checkUseResult`: o parâmetro de `UseResult.unless` existe.
    fn uso_de_resultado(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        if m.name.len() != 2 || self.texto(m.name[1]) != "unless" {
            return;
        }
        let Some(args) = &m.arguments else { return };
        let Some(arg) = args.args.iter().find(|x| x.name.is_some_and(|n| self.texto(n) == "parameterDefined")) else { return };
        let ExprKind::String(lit) = &self.a.expr(arg.value).kind else { return };
        let Some(pedido) = dartforge_elements::load::string_lit_value(lit) else { return };
        let funcao = match alvo {
            Alvo::Topo(d) => match &d.kind {
                DeclKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Alvo::Membro { membro, .. } => match &membro.kind {
                MemberKind::Method(f) => Some(self.a.function(*f)),
                _ => None,
            },
            _ => None,
        };
        let Some(func) = funcao else { return };
        let (Some(nome), Some(parametros)) = (func.name, &func.parameters) else { return };
        if parametros.iter().any(|p| p.name.is_some_and(|n| self.texto(n) == pedido)) {
            return;
        }
        let span = self.a.expr(arg.value).span;
        self.relatar(w::UNDEFINED_REFERENCED_PARAMETER, span, &[pedido.as_str(), self.texto(nome)]);
    }

    /// `_checkVisibility`.
    fn visibilidade(&mut self, m: &ast::Annotation, alvo: Alvo<'a>, para_sobrescrita: bool) {
        let (no_nome, texto) = self.nome_da_anotacao(m);
        if let Some((lista, de_topo)) = self.variaveis(alvo) {
            for v in lista.variables.iter() {
                let nome = self.texto(v.name);
                if de_topo {
                    if nome.starts_with('_') {
                        self.relatar(w::INVALID_VISIBILITY_ANNOTATION, no_nome, &[nome, texto.as_str()]);
                    }
                    // Uma variável de topo não se sobrescreve.
                    if para_sobrescrita {
                        self.relatar(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, no_nome, &[]);
                    }
                } else {
                    if lista.static_ && para_sobrescrita {
                        self.relatar(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, no_nome, &[]);
                    }
                    if nome.starts_with('_') {
                        self.relatar(w::INVALID_VISIBILITY_ANNOTATION, no_nome, &[nome, texto.as_str()]);
                    }
                }
            }
            return;
        }
        // Uma diretiva não é declaração: nada.
        let Some(nome) = self.nome_do_alvo(alvo) else { return };
        let de_instancia = match alvo {
            Alvo::Membro { membro, dono } => match &membro.kind {
                MemberKind::Method(f) => !self.a.function(*f).static_ && !matches!(dono.kind, DeclKind::ExtensionType(_)),
                _ => false,
            },
            _ => false,
        };
        if para_sobrescrita && !de_instancia {
            self.relatar(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, no_nome, &[]);
        }
        if nome.starts_with('_') {
            self.relatar(w::INVALID_VISIBILITY_ANNOTATION, no_nome, &[nome, texto.as_str()]);
        }
    }

    /// `_checkVisibleOutsideTemplate`: num membro de classe, enum ou mixin
    /// anotado com `@visibleForTemplate`.
    fn fora_do_template(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        let dono = match alvo {
            Alvo::Membro { dono, .. } | Alvo::Constante { dono, .. } => Some(dono),
            _ => None,
        };
        let valido = dono.is_some_and(|d| {
            matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_)) && d.metadata.iter().any(|x| self.tem(x, "visibleForTemplate"))
        });
        if !valido {
            let (span, _) = self.nome_da_anotacao(m);
            self.relatar(w::INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION, span, &[]);
        }
    }

    /// O nome de exibição de um `TargetKind` (`meta_meta.dart`).
    fn exibicao_da_especie(nome: &str) -> Option<&'static str> {
        Some(match nome {
            "classType" => "classes",
            "constructor" => "constructors",
            "directive" => "directives",
            "enumType" => "enums",
            "enumValue" => "enum values",
            "extension" => "extensions",
            "extensionType" => "extension types",
            "field" => "fields",
            "function" => "top-level functions",
            "library" => "libraries",
            "getter" => "getters",
            "method" => "methods",
            "mixinType" => "mixins",
            "optionalParameter" => "optional parameters",
            "overridableMember" => "overridable members",
            "parameter" => "parameters",
            "setter" => "setters",
            "topLevelVariable" => "top-level variables",
            "type" => "types (classes, enums, mixins, or typedefs)",
            "typedefType" => "typedefs",
            "typeParameter" => "type parameters",
            _ => return None,
        })
    }

    /// Os `TargetKind` do `@Target({...})` da classe `c` (a classe de uma
    /// anotação): os nomes das constantes, na ordem escrita.
    fn especies_da_classe(&self, c: ClassId) -> Vec<&'a str> {
        let Some(d) = self.program.class(c).decl else { return Vec::new() };
        let a = &self.program.unit(d.unit).ast;
        let mut saida = Vec::new();
        for m in a.decl(d.decl).metadata.iter() {
            if !m.name.last().is_some_and(|n| self.texto(*n) == "Target") {
                continue;
            }
            let Some(primeiro) = m.arguments.as_ref().and_then(|args| args.args.iter().find(|x| x.name.is_none())) else { continue };
            let ExprKind::SetOrMap { elements, .. } = &a.expr(primeiro.value).kind else { continue };
            for el in elements.iter() {
                if let ast::CollectionElement::Expression(e) = el
                    && let ExprKind::Property { name, .. } = &a.expr(*e).kind
                {
                    saida.push(self.texto(*name));
                }
            }
        }
        saida
    }

    /// A classe de uma anotação e o nome com que ela é citada na mensagem:
    /// a classe do construtor chamado, ou a do valor de uma constante.
    fn classe_da_anotacao(&self, m: &ast::Annotation) -> Option<(ClassId, String)> {
        let program = self.program;
        let u = self.unidade;
        let simples = |n: ast::Name| program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter);
        let (elemento, construtor) = match &m.name[..] {
            [n] => (simples(*n)?, None),
            [p, n] => match program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter) {
                Some(e) => (e, None),
                // `Classe.construtor`.
                None => (simples(*p)?, Some(*n)),
            },
            [p, n, k] => (program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter)?, Some(*k)),
            _ => return None,
        };
        match elemento {
            Element::Class(c) => {
                let classe = self.interner.resolve(program.class(c).name);
                let nome = match construtor {
                    Some(k) => format!("{classe}.{}", self.texto(k)),
                    None => classe.to_string(),
                };
                Some((c, nome))
            }
            Element::Variable(_) | Element::Function(_) => {
                // Uma constante (`immutable`): a classe do tipo escrito, ou a
                // da criação no inicializador.
                let v = match elemento {
                    Element::Variable(v) => v,
                    Element::Function(f) => program.function(f).variable?,
                    _ => return None,
                };
                let variavel = program.variable(v);
                let VariableRef::TopLevel { unit, decl, index } = variavel.node else { return None };
                let a = &program.unit(unit).ast;
                let DeclKind::Variables(lista) = &a.decl(decl).kind else { return None };
                let nome_do_tipo: ast::Name = match lista.ty.map(|t| &a.types[t.0 as usize].kind) {
                    Some(TypeKind::Named { name, .. }) => *name.last()?,
                    _ => {
                        let inicializador = lista.variables.get(index)?.initializer?;
                        match &a.expr(inicializador).kind {
                            ExprKind::InstanceCreation { ty, .. } => match &a.types[ty.0 as usize].kind {
                                TypeKind::Named { name, .. } => *name.last()?,
                                _ => return None,
                            },
                            ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                                ExprKind::Identifier(n) => *n,
                                ExprKind::Property { target, .. } => match &a.expr(*target).kind {
                                    ExprKind::Identifier(n) => *n,
                                    _ => return None,
                                },
                                _ => return None,
                            },
                            _ => return None,
                        }
                    }
                };
                match program.lookup_na_unidade(unit, nome_do_tipo.sym).and_then(|b| b.getter)? {
                    Element::Class(c) => Some((c, self.interner.resolve(variavel.name).to_string())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `_checkKinds` e `_isValidTarget` (`annotation_verifier.dart:160-185`,
    /// `:505-568`): a anotação cuja classe tem `@Target({...})` num alvo que
    /// não é de nenhuma das espécies. Parâmetros e parâmetros de tipo não são
    /// visitados aqui.
    fn especies(&mut self, m: &ast::Annotation, alvo: Alvo<'a>) {
        let Some((classe, nome)) = self.classe_da_anotacao(m) else { return };
        let especies = self.especies_da_classe(classe);
        if especies.is_empty() {
            return;
        }
        let tem = |k: &str| especies.contains(&k);
        // `overridableMember`: campo ou método de instância de classe,
        // extension type ou mixin.
        let sobrescrevivel = match alvo {
            Alvo::Membro { membro, dono } if matches!(dono.kind, DeclKind::Class(_) | DeclKind::ExtensionType(_) | DeclKind::Mixin(_)) => {
                match &membro.kind {
                    MemberKind::Field(l) => !l.static_,
                    MemberKind::Method(f) => !self.a.function(*f).static_,
                    MemberKind::Constructor(_) => false,
                }
            }
            _ => false,
        };
        let valido = (tem("overridableMember") && sobrescrevivel)
            || match alvo {
                Alvo::Topo(d) => match &d.kind {
                    DeclKind::Class(_) => tem("classType") || tem("type"),
                    DeclKind::Enum(_) => tem("enumType") || tem("type"),
                    DeclKind::Mixin(_) => tem("mixinType") || tem("type"),
                    DeclKind::Typedef(_) => tem("typedefType") || tem("type"),
                    DeclKind::ExtensionType(_) => tem("extensionType"),
                    DeclKind::Extension(_) => tem("extension"),
                    DeclKind::Variables(_) => tem("topLevelVariable"),
                    DeclKind::Function(f) => match self.a.function(*f).kind {
                        FunctionKind::Getter => tem("getter"),
                        FunctionKind::Setter => tem("setter"),
                        _ => tem("function"),
                    },
                },
                Alvo::Membro { membro, .. } => match &membro.kind {
                    MemberKind::Constructor(_) => tem("constructor"),
                    MemberKind::Field(_) => tem("field"),
                    MemberKind::Method(f) => match self.a.function(*f).kind {
                        FunctionKind::Getter => tem("getter"),
                        FunctionKind::Setter => tem("setter"),
                        _ => tem("method"),
                    },
                },
                Alvo::Constante { .. } => tem("enumValue"),
                Alvo::Diretiva => tem("directive") || (self.primeira_diretiva && tem("library")),
            };
        if valido {
            return;
        }
        // Os nomes de exibição, em ordem, juntados com vírgulas e `or`
        // (`commaSeparatedWithOr`).
        let mut nomes: Vec<&str> = especies.iter().filter_map(|k| Self::exibicao_da_especie(k)).collect();
        nomes.sort_unstable();
        let lista = match nomes.len() {
            0 => String::new(),
            1 => nomes[0].to_string(),
            2 => format!("{} or {}", nomes[0], nomes[1]),
            n => format!("{}, or {}", nomes[..n - 1].join(", "), nomes[n - 1]),
        };
        let (span, _) = self.nome_da_anotacao(m);
        self.relatar(w::INVALID_ANNOTATION_TARGET, span, &[nome.as_str(), lista.as_str()]);
    }

    /// `checkAnnotation`: uma anotação, pelo primeiro papel que casa, e
    /// depois as espécies de alvo (`_checkKinds`).
    fn anotacao(&mut self, m: &ast::Annotation, alvo: Alvo<'a>, classe: Option<ClassId>) {
        if self.tem(m, "factory") {
            self.fabrica(alvo);
        } else if self.tem(m, "internal") {
            self.interno(m, alvo);
        } else if self.tem(m, "literal") {
            // `_checkLiteral`: só em construtor `const`.
            let valido = matches!(alvo, Alvo::Membro { membro, .. } if matches!(&membro.kind, MemberKind::Constructor(k) if k.const_));
            if !valido {
                let (span, _) = self.nome_da_anotacao(m);
                self.relatar(w::INVALID_LITERAL_ANNOTATION, span, &[]);
            }
        } else if self.tem(m, "nonVirtual") {
            self.nao_virtual(m, alvo);
        } else if self.tem(m, "reopen") {
            if matches!(alvo, Alvo::Topo(d) if matches!(d.kind, DeclKind::Class(_))) {
                self.reabrir(m, classe);
            }
        } else if self.tem(m, "redeclare") {
            self.redeclarar(m, alvo);
        } else if self.tem(m, "useResult") || self.tem(m, "UseResult") {
            self.uso_de_resultado(m, alvo);
        } else if self.tem(m, "visibleForTemplate") || self.tem(m, "visibleForTesting") {
            self.visibilidade(m, alvo, false);
        } else if self.tem(m, "visibleForOverriding") {
            self.visibilidade(m, alvo, true);
        } else if self.tem(m, "visibleOutsideTemplate") {
            self.visibilidade(m, alvo, false);
            self.fora_do_template(m, alvo);
        }
        self.especies(m, alvo);
    }

    /// `_checkRequiredParameter`: `@required` onde não faz sentido.
    fn parametros(&mut self, lista: &[ast::Parameter]) {
        for p in lista {
            if !p.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| self.texto(*n) == "required")) {
                continue;
            }
            let nome = p.name.map_or("", |n| self.texto(n));
            let codigo = match p.kind {
                ParameterKind::Optional => w::INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM,
                ParameterKind::Required => w::INVALID_REQUIRED_POSITIONAL_PARAM,
                ParameterKind::Named if p.default_value.is_some() => w::INVALID_REQUIRED_NAMED_PARAM,
                ParameterKind::Named => continue,
            };
            self.relatar(codigo, p.span, &[nome]);
        }
    }
}

fn membros_de(d: &ast::Decl) -> &[ast::MemberId] {
    match &d.kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Extension(x) => &x.members,
        DeclKind::ExtensionType(x) => &x.members,
        _ => &[],
    }
}

/// As anotações da declaração de um membro: as do método, ou as do campo de
/// um acessor implícito.
fn anotacoes_do_membro(program: &Program, f: FunctionElementId) -> &[ast::Annotation] {
    let e = program.function(f);
    if let Some(v) = e.variable {
        return match program.variable(v).node {
            VariableRef::Field { unit, member, .. } => &program.unit(unit).ast.member(member).metadata[..],
            _ => &[],
        };
    }
    match e.node {
        FunctionRef::Function { unit, function } => program
            .unit(unit)
            .ast
            .members
            .iter()
            .find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function))
            .map_or(&[][..], |m| &m.metadata[..]),
        _ => &[],
    }
}

/// `getMember2(…, forSuper: true)` aproximado: o primeiro membro de chave
/// `chave` pelos mixins (do último ao primeiro), as restrições `on` e a
/// cadeia de superclasses, sem a própria classe. Devolve a classe dona.
fn herdado(program: &Program, classe: ClassId, chave: SymbolId) -> Option<(ClassId, FunctionElementId)> {
    let mut vistos: Vec<ClassId> = vec![classe];
    let mut atual = classe;
    loop {
        let e = program.class(atual);
        for m in e.mixin_classes.iter().rev().chain(e.on_classes.iter()) {
            if let Some(f) = program.class(*m).instance_members.get(&chave) {
                return Some((*m, *f));
            }
        }
        let s = e.supertype_class?;
        if vistos.contains(&s) {
            return None;
        }
        vistos.push(s);
        if let Some(f) = program.class(s).instance_members.get(&chave) {
            return Some((s, *f));
        }
        atual = s;
    }
}

/// A classe, ou algo de que ela herda (mixins, interfaces, superclasse), é
/// `@immutable` (`isOrInheritsImmutable`).
fn imutavel(program: &Program, interner: &Interner, c: ClassId, vistos: &mut Vec<ClassId>) -> bool {
    if vistos.contains(&c) {
        return false;
    }
    vistos.push(c);
    let e = program.class(c);
    let anotada = e.decl.is_some_and(|d| {
        program.unit(d.unit).ast.decl(d.decl).metadata.iter().any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == "immutable"))
    });
    anotada
        || e.mixin_classes.iter().any(|m| imutavel(program, interner, *m, vistos))
        || e.interface_classes.iter().any(|m| imutavel(program, interner, *m, vistos))
        || e.supertype_class.is_some_and(|s| imutavel(program, interner, s, vistos))
}

/// `Classe.campo` de cada campo de instância não final da classe.
fn campos_nao_finais(program: &Program, interner: &Interner, c: ClassId, saida: &mut Vec<String>) {
    let e = program.class(c);
    let nome = interner.resolve(e.name);
    for v in e.fields.iter().map(|v| program.variable(*v)) {
        if !v.final_ && !v.const_ && !v.static_ {
            saida.push(format!("{nome}.{}", interner.resolve(v.name)));
        }
    }
}

/// As anotações da declaração de um elemento de topo (as da variável, para
/// um acessor implícito).
fn anotacoes_do_topo(program: &Program, e: Element) -> &[ast::Annotation] {
    let da_declaracao = |unit: UnitId, decl: ast::DeclId| &program.unit(unit).ast.decl(decl).metadata[..];
    match e {
        Element::Class(c) => program.class(c).decl.map_or(&[][..], |d| da_declaracao(d.unit, d.decl)),
        Element::Typedef(t) => {
            let d = program.typedef(t).decl;
            da_declaracao(d.unit, d.decl)
        }
        Element::Extension(x) => {
            let d = program.extension(x).decl;
            da_declaracao(d.unit, d.decl)
        }
        Element::Variable(v) => match program.variable(v).node {
            VariableRef::TopLevel { unit, decl, .. } => da_declaracao(unit, decl),
            _ => &[],
        },
        Element::Function(f) => {
            let elemento = program.function(f);
            if let Some(v) = elemento.variable {
                return anotacoes_do_topo(program, Element::Variable(v));
            }
            match elemento.node {
                FunctionRef::Function { unit, function } => program
                    .unit(unit)
                    .ast
                    .decls
                    .iter()
                    .find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function))
                    .map_or(&[][..], |d| &d.metadata[..]),
                _ => &[],
            }
        }
        Element::Prefix(..) => &[],
    }
}

/// `_checkForInternalExport` (`best_practices_verifier.dart:952-990`): numa
/// biblioteca da API pública, o `export` que traz um elemento `@internal`
/// (`invalid_export_of_internal_element`), ou uma função cuja assinatura
/// usa um `typedef` `@internal`
/// (`invalid_export_of_internal_element_indirectly`). Os limites dos
/// parâmetros de tipo da função não são olhados.
fn exports_de_internos(program: &Program, lib: LibraryId, interner: &Interner, out: &mut Vec<(UnitId, Diagnostic)>) {
    let Some(sym_interno) = interner.lookup("internal") else { return };
    let interno = |m: &[ast::Annotation]| m.iter().any(|x| x.arguments.is_none() && x.name.last().is_some_and(|n| n.sym == sym_interno));
    let biblioteca = program.library(lib);
    for exp in biblioteca.exports.iter() {
        let Some(diretiva) = program.unit(exp.unit).unit.directives.get(exp.directive) else { continue };
        let alvo = program.library(exp.library);
        // A própria biblioteca exportada é `@internal`.
        let da_biblioteca = alvo
            .units
            .first()
            .and_then(|u| program.unit(*u).unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. })));
        if let Some(d) = da_biblioteca
            && interno(&d.metadata)
        {
            let nome = alvo.name.as_ref().map(|partes| partes.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default();
            out.push((exp.unit, Diagnostic::com_codigo(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT, diretiva.span, [nome.as_str()])));
        }
        // O namespace que a diretiva exporta, em ordem de nome.
        let mut nomes: Vec<SymbolId> = alvo
            .exported
            .keys()
            .copied()
            .filter(|k| {
                exp.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == *k),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == *k),
                })
            })
            .collect();
        nomes.sort_by_key(|k| interner.resolve(*k).to_string());
        for k in nomes {
            let Some(ligacao) = alvo.exported.get(&k) else { continue };
            let texto = interner.resolve(k);
            for e in ligacao.getter.into_iter().chain(ligacao.setter.filter(|s| Some(*s) != ligacao.getter)) {
                if interno(anotacoes_do_topo(program, e)) {
                    out.push((exp.unit, Diagnostic::com_codigo(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT, diretiva.span, [texto])));
                    continue;
                }
                // Uma função: os tipos escritos da assinatura que são um
                // `typedef` `@internal`.
                let Element::Function(f) = e else { continue };
                let FunctionRef::Function { unit, function } = program.function(f).node else { continue };
                let a = &program.unit(unit).ast;
                let func = a.function(function);
                let tipos = func.parameters.iter().flat_map(|ps| ps.iter()).filter_map(|p| p.ty).chain(func.return_type);
                for t in tipos {
                    let ast::TypeKind::Named { name, .. } = &a.types[t.0 as usize].kind else { continue };
                    let achado = match &name[..] {
                        [n] => program.lookup_na_unidade(unit, n.sym),
                        [p, n] => program.lookup_prefixed_na_unidade(unit, p.sym, n.sym),
                        _ => None,
                    };
                    if let Some(alias @ Element::Typedef(td)) = achado.and_then(|b| b.getter)
                        && interno(anotacoes_do_topo(program, alias))
                    {
                        let nome_do_alias = interner.resolve(program.typedef(td).name);
                        out.push((
                            exp.unit,
                            Diagnostic::com_codigo(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY, diretiva.span, [nome_do_alias, texto]),
                        ));
                    }
                }
            }
        }
    }
}

/// Os relatos das anotações do `package:meta` na biblioteca `lib`.
/// `em_api_publica`: a biblioteca é da API pública do pacote (está em
/// `lib/`, fora de `lib/src/`).
pub fn verificar(program: &Program, lib: LibraryId, interner: &Interner, em_api_publica: bool) -> Vec<(UnitId, Diagnostic)> {
    let biblioteca = program.library(lib);
    let mut out: Vec<(UnitId, Diagnostic)> = Vec::new();
    // As classes desta biblioteca pela declaração.
    let classe_de = |u: UnitId, d: ast::DeclId| {
        program.classes.iter().position(|c| c.library == lib && c.decl.is_some_and(|x| x.unit == u && x.decl == d)).map(|i| ClassId(i as u32))
    };
    for &u in &biblioteca.units {
        let unidade = program.unit(u);
        let a = &unidade.ast;
        let mut ctx = Ctx { program, interner, a, unidade: u, em_api_publica, primeira_diretiva: false, out: Vec::new() };
        for (indice, diretiva) in unidade.unit.directives.iter().enumerate() {
            ctx.primeira_diretiva = indice == 0;
            for m in diretiva.metadata.iter() {
                ctx.anotacao(m, Alvo::Diretiva, None);
            }
        }
        for &d in unidade.unit.declarations.iter() {
            let decl = a.decl(d);
            let classe = classe_de(u, d);
            for m in decl.metadata.iter() {
                ctx.anotacao(m, Alvo::Topo(decl), classe);
            }
            if let DeclKind::Enum(x) = &decl.kind {
                for constante in x.constants.iter() {
                    for m in constante.metadata.iter() {
                        ctx.anotacao(m, Alvo::Constante { constante, dono: decl }, None);
                    }
                }
            }
            for &mid in membros_de(decl) {
                let membro = a.member(mid);
                for m in membro.metadata.iter() {
                    ctx.anotacao(m, Alvo::Membro { membro, dono: decl }, None);
                }
                if let MemberKind::Constructor(k) = &membro.kind {
                    ctx.parametros(&k.parameters);
                }
            }
            // `must_be_immutable`: classe, aplicação de mixin ou mixin.
            let nome = match &decl.kind {
                DeclKind::Class(x) => Some(x.name),
                DeclKind::Mixin(x) => Some(x.name),
                _ => None,
            };
            if let (Some(nome), Some(c)) = (nome, classe)
                && imutavel(program, interner, c, &mut Vec::new())
            {
                let mut campos: Vec<String> = Vec::new();
                let mut vistos: Vec<ClassId> = Vec::new();
                let mut atual = Some(c);
                while let Some(x) = atual {
                    if vistos.contains(&x) {
                        break;
                    }
                    vistos.push(x);
                    campos_nao_finais(program, interner, x, &mut campos);
                    for m in program.class(x).mixin_classes.iter() {
                        campos_nao_finais(program, interner, *m, &mut campos);
                    }
                    atual = program.class(x).supertype_class;
                }
                if !campos.is_empty() {
                    let lista = campos.join(", ");
                    ctx.relatar(w::MUST_BE_IMMUTABLE, nome.span, &[lista.as_str()]);
                }
            }
            // `invalid_override_of_non_virtual_member`.
            if let Some(c) = classe
                && matches!(decl.kind, DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_))
            {
                let nao_virtual = |chave: Option<SymbolId>| {
                    let (dona, f) = herdado(program, c, chave?)?;
                    anotacoes_do_membro(program, f)
                        .iter()
                        .any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == "nonVirtual"))
                        .then(|| interner.resolve(program.class(dona).name))
                };
                for &mid in membros_de(decl) {
                    match &a.member(mid).kind {
                        MemberKind::Field(l) if !l.static_ => {
                            for v in l.variables.iter() {
                                let texto = interner.resolve(v.name.sym);
                                let do_setter = if l.final_ || l.const_ { None } else { interner.lookup(&format!("{texto}_=")) };
                                if let Some(dona) = nao_virtual(Some(v.name.sym)).or_else(|| nao_virtual(do_setter)) {
                                    ctx.relatar(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, v.name.span, &[texto, dona]);
                                }
                            }
                        }
                        MemberKind::Method(f) => {
                            let func = a.function(*f);
                            let Some(n) = func.name else { continue };
                            if func.static_ {
                                continue;
                            }
                            let texto = interner.resolve(n.sym);
                            let chave = match func.kind {
                                FunctionKind::Setter => interner.lookup(&format!("{texto}_=")),
                                _ => Some(n.sym),
                            };
                            if let Some(dona) = nao_virtual(chave) {
                                ctx.relatar(w::INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER, n.span, &[texto, dona]);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        // `@required` nas listas de parâmetros de funções e métodos.
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                ctx.parametros(ps);
            }
        }
        out.append(&mut ctx.out);
    }
    if em_api_publica {
        exports_de_internos(program, lib, interner, &mut out);
    }
    // `import_deferred_library_with_load_function`: o import adiado de uma
    // biblioteca que exporta `loadLibrary`, na diretiva inteira.
    if let Some(sym) = interner.lookup("loadLibrary") {
        for imp in biblioteca.imports.iter().filter(|i| i.deferred && i.prefix.is_some()) {
            let exporta = program.library(imp.library).exported.contains_key(&sym)
                && imp.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == sym),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == sym),
                });
            let diretiva = program.unit(imp.unit).unit.directives.get(imp.directive);
            if let (true, Some(d)) = (exporta, diretiva)
                && matches!(d.kind, DirectiveKind::Import { .. })
            {
                out.push((imp.unit, Diagnostic::com_codigo(h::IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION, d.span, std::iter::empty::<&str>())));
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Só as checagens que não dependem do programa: um contexto com a
    /// árvore de uma unidade solta não se monta sem `Program`, então os
    /// testes daqui ficam nas funções puras.
    #[test]
    fn membros_de_declaracao_sem_membros() {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse("void f() {}\n", &mut nomes);
        assert!(membros_de(p.ast.decl(p.unit.declarations[0])).is_empty());
    }
}

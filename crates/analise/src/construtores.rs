//! A inicialização de campos nos construtores (docs/ANALYZER-ESPECIFICACAO.md,
//! parte C, construtores):
//!
//! * o `ConstructorFieldsVerifier`
//!   (`analyzer/lib/src/error/constructor_fields_verifier.dart`): o estado
//!   de cada campo por construtor gerador não redirecionador e não
//!   `external` — `field_initialized_by_multiple_initializers`,
//!   `field_initialized_in_initializer_and_declaration`,
//!   `field_initialized_in_parameter_and_initializer` e
//!   `final_initialized_in_declaration_and_constructor`;
//! * o `ErrorVerifier._checkForValidField` (`this.x`, em todo construtor) e
//!   o `_checkForInvalidField` (`x = e`):
//!   `initializing_formal_for_non_existent_field`,
//!   `initializer_for_static_field` e `initializer_for_non_existent_field`.
//!
//! O campo de um nome é o `augmented.getField(nome)`: o primeiro campo
//! declarado com ele; sem campo, o sintético de um getter/setter (e o
//! `values` de enum), que não conta como campo. A comparação de tipos
//! (`field_initializing_formal_not_assignable`) fica em `crates/types`.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, FunctionKind, Initializer, MemberKind};
use dartforge_intern::{Interner, SymbolId};

/// Um campo declarado (não sintético).
struct Campo {
    nome: SymbolId,
    estatico: bool,
    final_ou_const: bool,
    com_inicializador: bool,
}

/// O que `getField(nome)` dá.
enum Achado {
    Declarado(usize),
    Sintetico,
    Nenhum,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Estado {
    NaoIniciado,
    NaDeclaracao,
    NoParametro,
    NoInicializador,
}

struct Dono<'a> {
    campos: Vec<Campo>,
    /// Nomes de getters/setters (campos sintéticos) e o `values` do enum.
    sinteticos: Vec<SymbolId>,
    membros: &'a [ast::MemberId],
    /// Enum: o `index` não entra no mapa do verificador.
    enum_: bool,
}

impl Dono<'_> {
    fn achar(&self, nome: SymbolId) -> Achado {
        if let Some(i) = self.campos.iter().position(|f| f.nome == nome) {
            return Achado::Declarado(i);
        }
        if self.sinteticos.contains(&nome) {
            return Achado::Sintetico;
        }
        Achado::Nenhum
    }
}

fn dono<'a>(u: &Unidade<'a>, d: &'a ast::Decl, nomes: &Interner) -> Option<Dono<'a>> {
    let a = u.ast;
    let mut campos = Vec::new();
    let mut sinteticos = Vec::new();
    let (membros, enum_): (&'a [ast::MemberId], bool) = match &d.kind {
        DeclKind::Class(k) if !k.mixin_application => (&k.members, false),
        DeclKind::Mixin(k) => (&k.members, false),
        DeclKind::Enum(k) => {
            // As constantes são campos estáticos `const` com inicializador;
            // o `values` é sintético.
            for k in k.constants.iter() {
                campos.push(Campo { nome: k.name.sym, estatico: true, final_ou_const: true, com_inicializador: true });
            }
            if let Some(v) = nomes.lookup("values") {
                sinteticos.push(v);
            }
            (&k.members, true)
        }
        DeclKind::ExtensionType(k) => {
            campos.push(Campo { nome: k.representation_name.sym, estatico: false, final_ou_const: true, com_inicializador: false });
            (&k.members, false)
        }
        _ => return None,
    };
    for &mid in membros {
        let m = a.member(mid);
        match &m.kind {
            MemberKind::Field(l) => {
                for v in l.variables.iter() {
                    campos.push(Campo { nome: v.name.sym, estatico: l.static_, final_ou_const: l.final_ || l.const_, com_inicializador: v.initializer.is_some() });
                }
            }
            MemberKind::Method(fid) => {
                let f = a.function(*fid);
                if matches!(f.kind, FunctionKind::Getter | FunctionKind::Setter)
                    && let Some(n) = f.name
                {
                    sinteticos.push(n.sym);
                }
            }
            MemberKind::Constructor(_) => {}
        }
    }
    Some(Dono { campos, sinteticos, membros, enum_ })
}

/// O nó `FieldFormalParameter`: dos metadados (ou do primeiro modificador)
/// ao nome, ou ao fim da lista de um parâmetro-função (com o `?`).
fn span_do_parametro(fonte: &str, p: &ast::Parameter) -> Span {
    let inicio = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
    let mut fim = p.name.map_or(p.span.end, |n| n.span.end);
    if p.function_parameters.is_some() {
        let b = fonte.as_bytes();
        let mut i = fim;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        // Parâmetros de tipo e a lista: até o `)` casado.
        let mut nivel = 0i32;
        while i < b.len() {
            match b[i] {
                b'(' | b'<' => nivel += 1,
                b')' | b'>' => {
                    nivel -= 1;
                    if nivel == 0 && b[i] == b')' {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        fim = i;
        if p.function_nullable && b.get(fim) == Some(&b'?') {
            fim += 1;
        }
    }
    Span { start: inicio, end: fim.min(p.span.end).max(inicio) }
}

pub fn verificar(unidades: &[Unidade<'_>], nomes: &Interner) -> Vec<(usize, Diagnostic)> {
    let mut saida = Vec::new();
    for (iu, u) in unidades.iter().enumerate() {
        let a = u.ast;
        for &did in &u.unit.declarations {
            let decl = a.decl(did);
            let Some(dono) = dono(u, decl, nomes) else { continue };
            // O mapa do verificador: os campos declarados (sem o `index` do
            // enum), com o estado de partida.
            let index = nomes.lookup("index");
            let inicial: Vec<Option<Estado>> = dono
                .campos
                .iter()
                .map(|f| {
                    if dono.enum_ && Some(f.nome) == index {
                        None
                    } else if f.com_inicializador {
                        Some(Estado::NaDeclaracao)
                    } else {
                        Some(Estado::NaoIniciado)
                    }
                })
                .collect();
            for &mid in dono.membros {
                let m = a.member(mid);
                let MemberKind::Constructor(k) = &m.kind else { continue };
                // `_checkForValidField`: todo construtor.
                for p in k.parameters.iter().filter(|p| p.this_ && !p.super_) {
                    let nome = p.name.map(|n| nomes.resolve(n.sym).to_string()).unwrap_or_default();
                    let alvo = p.name.map_or(Achado::Nenhum, |n| dono.achar(n.sym));
                    let sp = span_do_parametro(u.fonte, p);
                    match alvo {
                        Achado::Nenhum | Achado::Sintetico => {
                            saida.push((iu, Diagnostic::com_codigo(c::INITIALIZING_FORMAL_FOR_NON_EXISTENT_FIELD, sp, [nome.as_str()])));
                        }
                        Achado::Declarado(i) if dono.campos[i].estatico => {
                            saida.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_STATIC_FIELD, sp, [nome.as_str()])));
                        }
                        Achado::Declarado(_) => {}
                    }
                }
                // `_checkForInvalidField`: todo `x = e`.
                for ini in k.initializers.iter() {
                    if let Initializer::Field { span, name, .. } = ini {
                        let texto = nomes.resolve(name.sym).to_string();
                        match dono.achar(name.sym) {
                            Achado::Nenhum | Achado::Sintetico => {
                                saida.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_NON_EXISTENT_FIELD, *span, [texto.as_str()])));
                            }
                            Achado::Declarado(i) if dono.campos[i].estatico => {
                                saida.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_STATIC_FIELD, *span, [texto.as_str()])));
                            }
                            Achado::Declarado(_) => {}
                        }
                    }
                }
                // O `ConstructorFieldsVerifier`: só o gerador não
                // redirecionador (`= B`) e não `external`.
                if k.factory || k.redirect.is_some() || k.external {
                    continue;
                }
                let mut estados = inicial.clone();
                if !m.augment {
                    for p in k.parameters.iter().filter(|p| p.this_ && !p.super_) {
                        let Some(n) = p.name else { continue };
                        let Achado::Declarado(i) = dono.achar(n.sym) else { continue };
                        match estados[i] {
                            Some(Estado::NaoIniciado) => estados[i] = Some(Estado::NoParametro),
                            Some(Estado::NaDeclaracao) if dono.campos[i].final_ou_const => {
                                saida.push((
                                    iu,
                                    Diagnostic::com_codigo(c::FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR, n.span, [nomes.resolve(n.sym)]),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                for ini in k.initializers.iter() {
                    let Initializer::Field { name, .. } = ini else { continue };
                    let Achado::Declarado(i) = dono.achar(name.sym) else { continue };
                    match estados[i] {
                        Some(Estado::NaoIniciado) => estados[i] = Some(Estado::NoInicializador),
                        Some(Estado::NaDeclaracao) => {
                            if dono.campos[i].final_ou_const {
                                saida.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_INITIALIZER_AND_DECLARATION, name.span, Vec::<&str>::new())));
                            }
                        }
                        Some(Estado::NoParametro) => {
                            saida.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_PARAMETER_AND_INITIALIZER, name.span, Vec::<&str>::new())));
                        }
                        Some(Estado::NoInicializador) => {
                            saida.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_BY_MULTIPLE_INITIALIZERS, name.span, [nomes.resolve(name.sym)])));
                        }
                        None => {}
                    }
                }
            }
        }
    }
    saida
}

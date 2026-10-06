//! O trigésimo terceiro lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `use_build_context_synchronously`: fora dos diretórios de teste, o uso
//!   de um `BuildContext` (o alvo de uma invocação de método, o prefixo de
//!   um `PrefixedIdentifier` que não é `.mounted`, o argumento de uma
//!   invocação ou criação) depois de uma lacuna assíncrona sem a guarda do
//!   `mounted` associado (`State.mounted` para o `context` de um `State`,
//!   senão o `mounted` do tipo), pelo `AsyncStateVisitor` inteiro sobre a
//!   árvore no formato do analyzer ([`crate::arvore_analyzer`], na forma
//!   resolvida): do nó para cima até o corpo da função, cada pai dá o estado
//!   em relação ao filho; o `await` já visto como pai fica no cache. No corpo
//!   de uma closure passada a um construtor ou método protegido (`Future`,
//!   `Stream`, `StreamController`, `StreamSubscription`), o uso é relatado
//!   também. O `mounted` de outro elemento troca o código para
//!   `wrong_mounted`. O `terminatesControl` usa o `ExitDetector`
//!   ([`crate::saida`]); o `constantBoolValue`, os literais `true`/`false`,
//!   os parênteses, o `!` e as constantes de topo com inicializador
//!   booleano.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::arvore_analyzer::{Arvore, Ligacao, Marca};
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, VariableId, VariableRef};
use dartforge_frontend::ast::{DeclKind, ExprId, ExprKind, MemberKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Estado {
    Assincrono,
    Montado,
    NaoMontado,
}

/// `asynchronousOrNull`.
fn so_assincrono(e: Option<Estado>) -> Option<Estado> {
    e.filter(|x| *x == Estado::Assincrono)
}

const CORPOS: &[&str] = &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"];

/// A árvore, o texto e a semântica.
struct Cx<'a> {
    arv: &'a Arvore,
    fonte: &'a str,
    s: &'a super::Semantica<'a>,
    interner: &'a Interner,
}

impl<'a> Cx<'a> {
    fn especie(&self, n: usize) -> &'static str {
        self.arv.nos[n].especie
    }

    fn filhos(&self, n: usize) -> &'a [usize] {
        &self.arv.nos[n].filhos
    }

    fn pai(&self, n: usize) -> Option<usize> {
        self.arv.nos[n].pai
    }

    fn texto(&self, n: usize) -> &'a str {
        let no = &self.arv.nos[n];
        &self.fonte[no.inicio..no.fim]
    }

    fn span(&self, n: usize) -> Span {
        Span { start: self.arv.nos[n].inicio, end: self.arv.nos[n].fim }
    }

    /// O operador entre dois filhos (`BinaryExpression`,
    /// `AssignmentExpression`).
    fn operador_entre(&self, l: usize, r: usize) -> &'a str {
        self.fonte[self.arv.nos[l].fim..self.arv.nos[r].inicio].trim()
    }

    /// A expressão do DartForge que o nó representa.
    fn expr(&self, n: usize) -> Option<ExprId> {
        match self.arv.nos[n].ligacao {
            Ligacao::Expr(e) => Some(e),
            _ => match self.arv.nos[n].marca {
                Marca::Expr(e) => Some(e),
                _ => None,
            },
        }
    }

    /// O `staticType` do nó.
    fn tipo(&self, n: usize) -> Option<TypeId> {
        self.s.corpo.get_type(self.expr(n)?)
    }

    /// O elemento resolvido de um identificador (a marca dele).
    fn resolvido(&self, n: usize) -> Option<&'a Resolved> {
        match self.arv.nos[n].marca {
            Marca::Expr(e) => self.s.corpo.get_resolved(e),
            _ => None,
        }
    }

    /// O `methodName` de uma `MethodInvocation`: o último `SimpleIdentifier`.
    fn nome_do_metodo(&self, n: usize) -> Option<usize> {
        self.filhos(n).iter().rev().copied().find(|&f| self.especie(f) == "SimpleIdentifier")
    }

    /// O `target` de uma `MethodInvocation` (sem o da cascata).
    fn alvo_do_metodo(&self, n: usize) -> Option<usize> {
        let nome = self.nome_do_metodo(n);
        self.filhos(n).first().copied().filter(|&a| Some(a) != nome && self.especie(a) != "ArgumentList" && self.especie(a) != "TypeArgumentList")
    }

    /// Os argumentos (filhos do `ArgumentList` filho).
    fn argumentos(&self, n: usize) -> Vec<usize> {
        self.filhos(n).iter().copied().find(|&k| self.especie(k) == "ArgumentList").map(|l| self.filhos(l).to_vec()).unwrap_or_default()
    }

    /// `realTarget` de uma seção de cascata sem alvo escrito.
    fn alvo_da_cascata(&self, n: usize) -> Option<usize> {
        let mut k = n;
        while let Some(p) = self.pai(k) {
            if self.especie(p) == "CascadeExpression" {
                return self.filhos(p).first().copied().filter(|&a| a != k);
            }
            if !matches!(self.especie(p), "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "AssignmentExpression") {
                return None;
            }
            k = p;
        }
        None
    }

    /// `constantBoolValue`.
    fn booleano_constante(&self, n: usize) -> Option<bool> {
        match self.especie(n) {
            "BooleanLiteral" => Some(self.texto(n) == "true"),
            "ParenthesizedExpression" => self.filhos(n).first().and_then(|&f| self.booleano_constante(f)),
            "PrefixExpression" if self.texto(n).trim_start().starts_with('!') => self.filhos(n).first().and_then(|&f| self.booleano_constante(f)).map(|b| !b),
            "SimpleIdentifier" | "PrefixedIdentifier" => {
                let alvo = if self.especie(n) == "PrefixedIdentifier" { *self.filhos(n).last()? } else { n };
                let v = match self.resolvido(alvo)? {
                    Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => *v,
                    Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => self.s.program.function(*f).variable?,
                    _ => return None,
                };
                booleano_da_constante(self.s, v, 0)
            }
            _ => None,
        }
    }

    /// `terminatesControl`.
    fn termina(&self, n: usize) -> bool {
        match self.especie(n) {
            "Block" => self.filhos(n).last().is_some_and(|&u| self.termina(u)),
            "ReturnStatement" | "BreakStatement" | "ContinueStatement" => true,
            _ => {
                let elemento = |m: usize| elemento_sai(self, m);
                crate::saida::Saida::novo(self.arv, self.fonte, &elemento).sai(n).unwrap_or(false)
            }
        }
    }
}

/// O valor de uma constante booleana de topo ou estática pelo inicializador
/// literal.
fn booleano_da_constante(s: &super::Semantica<'_>, v: VariableId, prof: u32) -> Option<bool> {
    if prof > 8 {
        return None;
    }
    let x = s.program.variable(v);
    if !x.const_ {
        return None;
    }
    let (unit, init) = match x.node {
        VariableRef::TopLevel { unit, decl, index } => match &s.program.unit(unit).ast.decl(decl).kind {
            DeclKind::Variables(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        VariableRef::Field { unit, member, index } => match &s.program.unit(unit).ast.member(member).kind {
            MemberKind::Field(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        _ => return None,
    };
    let a = &s.program.unit(unit).ast;
    let mut e = init;
    while let ExprKind::Parenthesized(i) = &a.expr(e).kind {
        e = *i;
    }
    match &a.expr(e).kind {
        ExprKind::Bool(b) => Some(*b),
        _ => None,
    }
}

/// `_elementExits`: o executável retorna `Never` ou tem `@alwaysThrows`.
fn elemento_sai(cx: &Cx<'_>, m: usize) -> bool {
    let s = cx.s;
    match cx.resolvido(m) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
            let nunca = s.outline.functions.get(f.0 as usize).is_some_and(|d| matches!(s.table.get(d.return_type), Type::Never));
            nunca || anotacoes_da_funcao(s, *f).is_some_and(|(meta, u)| meta.iter().any(|a| dartforge_types::anotacoes::e_getter_de(s.program, cx.interner, u, a, "meta", "alwaysThrows")))
        }
        Some(Resolved::Local(_)) => {
            let Marca::Expr(e) = cx.arv.nos[m].marca else { return false };
            let Some(d) = s.corpo.declaracao_local(e) else { return false };
            matches!(s.corpo.tipo_local(d).map(|t| s.table.get(t)), Some(Type::Function { ret, .. }) if matches!(s.table.get(*ret), Type::Never))
        }
        _ => false,
    }
}

/// As anotações da declaração de uma função, método ou construtor.
fn anotacoes_da_funcao<'p>(s: &super::Semantica<'p>, f: FunctionElementId) -> Option<(&'p [dartforge_frontend::ast::Annotation], dartforge_elements::model::UnitId)> {
    let program: &'p dartforge_elements::model::Program = s.program;
    match program.function(f).node {
        FunctionRef::Constructor { unit, member } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return Some((&m.metadata[..], unit));
            }
            a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| (&d.metadata[..], unit))
        }
        FunctionRef::None => None,
    }
}

/// O `AsyncStateVisitor`.
struct Visitante<'c, 'a> {
    cx: &'c Cx<'a>,
    referencia: usize,
    montado: FunctionElementId,
    cache: HashMap<usize, Option<Estado>>,
    nao_relacionado: bool,
}

impl Visitante<'_, '_> {
    /// `_asynchronousIfAnyIsAsync`.
    fn algum_assincrono(&mut self, nos: &[usize]) -> Option<Estado> {
        let ate = nos.iter().position(|&n| n == self.referencia).unwrap_or(nos.len());
        for &n in &nos[..ate] {
            if self.visitar(n) == Some(Estado::Assincrono) {
                return Some(Estado::Assincrono);
            }
        }
        None
    }

    /// `_inOrderAsyncState`.
    fn em_ordem(&mut self, nos: &[(Option<usize>, bool)]) -> Option<Estado> {
        if nos.is_empty() {
            return None;
        }
        if nos[0].0 == Some(self.referencia) {
            return None;
        }
        let indice = nos.iter().position(|(n, _)| *n == Some(self.referencia));
        let inicio = match indice {
            Some(i) if i > 0 => i - 1,
            _ => nos.len() - 1,
        };
        for i in (0..=inicio).rev() {
            let (n, guarda) = nos[i];
            let Some(n) = n else { continue };
            let estado = self.visitar(n);
            if estado == Some(Estado::Assincrono) {
                return Some(Estado::Assincrono);
            }
            if guarda && estado.is_some() {
                return estado;
            }
        }
        None
    }

    fn em_ordem_guardavel(&mut self, nos: &[usize]) -> Option<Estado> {
        let v: Vec<(Option<usize>, bool)> = nos.iter().map(|&n| (Some(n), true)).collect();
        self.em_ordem(&v)
    }

    /// `_visitBlockLike`.
    fn como_bloco(&mut self, comandos: &[usize], pai: Option<usize>) -> Option<Estado> {
        let r = self.referencia;
        if crate::arvore_analyzer::e_comando(self.cx.especie(r))
            && let Some(i) = comandos.iter().position(|&c| c == r)
        {
            let anterior = self.em_ordem_guardavel(comandos);
            if anterior.is_some() {
                return anterior;
            }
            if pai.is_some_and(|p| matches!(self.cx.especie(p), "DoStatement" | "ForStatement" | "WhileStatement")) {
                return so_assincrono(self.em_ordem_guardavel(&comandos[i + 1..]));
            }
            return None;
        }
        for &c in comandos.iter().rev() {
            let e = self.visitar(c);
            if e.is_some() {
                return e;
            }
        }
        None
    }

    /// `_visitIdentifier`.
    fn identificador(&mut self, n: usize) -> Option<Estado> {
        if self.cx.texto(n) != "mounted" {
            return None;
        }
        let f = match self.cx.resolvido(n) {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                Some(self.cx.s.program.publico(*f))
            }
            _ => None,
        };
        if f == Some(self.cx.s.program.publico(self.montado)) {
            return Some(Estado::Montado);
        }
        self.nao_relacionado = true;
        None
    }

    /// `_constantEquality`.
    fn igualdade_constante(estado: Option<Estado>, constante: bool) -> Option<Estado> {
        match (estado, constante) {
            (Some(Estado::Montado), true) | (Some(Estado::NaoMontado), false) => Some(Estado::Montado),
            (Some(Estado::NaoMontado), true) | (Some(Estado::Montado), false) => Some(Estado::NaoMontado),
            _ => None,
        }
    }

    /// `_visitIfLike`.
    fn como_if(&mut self, expressao: usize, caso: Option<usize>, entao: usize, senao: Option<usize>) -> Option<Estado> {
        if self.referencia == expressao {
            return None;
        }
        let e = self.visitar(expressao);
        if Some(self.referencia) == caso {
            return match e {
                Some(Estado::Assincrono) => Some(Estado::Assincrono),
                Some(Estado::Montado) => Some(Estado::Montado),
                _ => None,
            };
        }
        let c = caso.and_then(|k| self.visitar(k));
        let condicao = match (e, c) {
            (None, _) => c,
            (_, None) => e,
            (_, Some(Estado::Assincrono)) => Some(Estado::Assincrono),
            (Some(Estado::Assincrono), _) => c,
            (Some(Estado::Montado), _) => Some(Estado::Montado),
            (Some(Estado::NaoMontado), _) => Some(Estado::NaoMontado),
        };
        if self.referencia == entao {
            return match condicao {
                Some(Estado::Assincrono) => Some(Estado::Assincrono),
                Some(Estado::Montado) => Some(Estado::Montado),
                _ => None,
            };
        }
        if Some(self.referencia) == senao {
            return match condicao {
                Some(Estado::Assincrono) => Some(Estado::Assincrono),
                Some(Estado::NaoMontado) => Some(Estado::Montado),
                _ => None,
            };
        }
        let estado_entao = self.visitar(entao);
        let estado_senao = senao.and_then(|k| self.visitar(k));
        let entao_termina = self.cx.termina(entao);
        let senao_termina = senao.is_some_and(|k| self.cx.termina(k));
        if estado_entao == Some(Estado::NaoMontado) && (estado_senao == Some(Estado::NaoMontado) || senao_termina) {
            return Some(Estado::NaoMontado);
        }
        if estado_senao == Some(Estado::NaoMontado) && entao_termina {
            return Some(Estado::NaoMontado);
        }
        if estado_entao == Some(Estado::Assincrono) && !entao_termina {
            return Some(Estado::Assincrono);
        }
        if estado_senao == Some(Estado::Assincrono) && !senao_termina {
            return Some(Estado::Assincrono);
        }
        if condicao == Some(Estado::Assincrono) {
            return Some(Estado::Assincrono);
        }
        if condicao == Some(Estado::Montado) && senao_termina {
            return Some(Estado::NaoMontado);
        }
        if condicao == Some(Estado::NaoMontado) && entao_termina {
            return Some(Estado::NaoMontado);
        }
        None
    }

    /// As partes de `for`, de comando ou de elemento.
    fn para(&mut self, n: usize) -> Option<Estado> {
        let cx = self.cx;
        let f = cx.filhos(n);
        let (Some(&partes), corpo) = (f.first(), f.get(1).copied()) else { return None };
        let corpo_e_referencia = corpo == Some(self.referencia);
        match cx.especie(partes) {
            "ForPartsWithDeclarations" | "ForPartsWithExpression" => {
                let p = cx.arv.partes_de_for.get(&partes).cloned().unwrap_or_default();
                let mut nos: Vec<(Option<usize>, bool)> = Vec::new();
                if cx.especie(partes) == "ForPartsWithDeclarations" {
                    if let Some(lista) = p.inicio {
                        for &v in cx.filhos(lista).iter().filter(|&&v| cx.especie(v) == "VariableDeclaration") {
                            nos.push((Some(v), false));
                        }
                    }
                } else {
                    nos.push((p.inicio, false));
                }
                nos.push((p.condicao, corpo_e_referencia));
                for &u in &p.atualizacoes {
                    nos.push((Some(u), false));
                }
                nos.push((corpo, false));
                self.em_ordem(&nos)
            }
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let iteravel = cx.filhos(partes).last().copied();
                self.em_ordem(&[(iteravel, false), (corpo, false)])
            }
            _ => None,
        }
    }

    /// `node.accept(AsyncStateVisitor)`.
    fn visitar(&mut self, n: usize) -> Option<Estado> {
        let cx = self.cx;
        let f = cx.filhos(n);
        let r = self.referencia;
        let primeiro = f.first().copied();
        match cx.especie(n) {
            "AdjacentStrings" | "CascadeExpression" | "RecordLiteral" | "StringInterpolation" => self.algum_assincrono(f),
            "AsExpression" | "IsExpression" | "SpreadElement" | "InterpolationExpression" | "PostfixExpression" | "YieldStatement" => {
                let e = primeiro.and_then(|k| self.visitar(k));
                so_assincrono(e)
            }
            "AssignmentExpression" => {
                let (Some(&l), Some(&d)) = (f.first(), f.get(1)) else { return None };
                self.em_ordem(&[(Some(l), false), (Some(d), true)])
            }
            "AwaitExpression" => {
                if let Some(e) = self.cache.get(&n) {
                    return *e;
                }
                if primeiro == Some(r) { None } else { Some(Estado::Assincrono) }
            }
            "BinaryExpression" => {
                let (Some(&l), Some(&d)) = (f.first(), f.get(1)) else { return None };
                let op = cx.operador_entre(l, d);
                if l == r {
                    return None;
                }
                if d == r {
                    let e = self.visitar(l);
                    return match e {
                        Some(Estado::Assincrono) => Some(Estado::Assincrono),
                        Some(Estado::Montado) if op == "&&" => Some(Estado::Montado),
                        Some(Estado::NaoMontado) if op == "||" => Some(Estado::NaoMontado),
                        _ => None,
                    };
                }
                match op {
                    "&&" => {
                        let a = self.visitar(l);
                        let b = self.visitar(d);
                        match (a, b) {
                            (None, _) => b,
                            (_, None) => a,
                            (_, Some(Estado::Assincrono)) => Some(Estado::Assincrono),
                            (Some(Estado::Assincrono), _) => b,
                            (Some(Estado::Montado), _) => Some(Estado::Montado),
                            (Some(Estado::NaoMontado), _) => Some(Estado::NaoMontado),
                        }
                    }
                    "||" => {
                        let a = self.visitar(l);
                        let b = self.visitar(d);
                        match (a, b) {
                            (_, Some(Estado::Assincrono)) => Some(Estado::Assincrono),
                            (_, Some(Estado::NaoMontado)) => Some(Estado::NaoMontado),
                            (Some(Estado::Assincrono), _) => Some(Estado::Assincrono),
                            (Some(Estado::Montado), Some(Estado::Montado)) => Some(Estado::Montado),
                            (Some(Estado::NaoMontado), _) => Some(Estado::NaoMontado),
                            _ => None,
                        }
                    }
                    "==" | "!=" => {
                        let negado = op == "!=";
                        let a = self.visitar(l);
                        let b = self.visitar(d);
                        if a == Some(Estado::Assincrono) || b == Some(Estado::Assincrono) {
                            return Some(Estado::Assincrono);
                        }
                        if matches!(a, Some(Estado::Montado | Estado::NaoMontado)) {
                            let v = cx.booleano_constante(d)?;
                            return Self::igualdade_constante(a, if negado { !v } else { v });
                        }
                        if matches!(b, Some(Estado::Montado | Estado::NaoMontado)) {
                            let v = cx.booleano_constante(l)?;
                            return Self::igualdade_constante(b, if negado { !v } else { v });
                        }
                        None
                    }
                    _ => {
                        let a = so_assincrono(self.visitar(l));
                        if a.is_some() {
                            return a;
                        }
                        so_assincrono(self.visitar(d))
                    }
                }
            }
            "Block" => {
                let pai = cx.pai(n);
                self.como_bloco(f, pai)
            }
            "BlockFunctionBody" | "ExpressionFunctionBody" => None,
            "CaseClause" => f.iter().copied().find(|&k| cx.especie(k) == "GuardedPattern").and_then(|k| self.visitar(k)),
            "CatchClause" => {
                let e = f.last().copied().and_then(|k| self.visitar(k));
                so_assincrono(e)
            }
            "ConditionalExpression" => {
                let (Some(&c), Some(&t), Some(&e)) = (f.first(), f.get(1), f.get(2)) else { return None };
                self.como_if(c, None, t, Some(e))
            }
            "DoStatement" => {
                let (Some(&corpo), Some(&cond)) = (f.first(), f.get(1)) else { return None };
                if corpo == r {
                    so_assincrono(self.visitar(cond))
                } else if cond == r {
                    so_assincrono(self.visitar(corpo))
                } else {
                    let a = so_assincrono(self.visitar(cond));
                    if a.is_some() {
                        return a;
                    }
                    so_assincrono(self.visitar(corpo))
                }
            }
            "ExpressionStatement" => {
                let e = primeiro?;
                if e == r { None } else { so_assincrono(self.visitar(e)) }
            }
            "ExtensionOverride" => {
                let args = cx.argumentos(n);
                self.algum_assincrono(&args)
            }
            "ForElement" | "ForStatement" => self.para(n),
            "FunctionExpressionInvocation" => {
                let mut nos: Vec<usize> = primeiro.into_iter().filter(|&k| cx.especie(k) != "ArgumentList").collect();
                nos.extend(cx.argumentos(n));
                self.algum_assincrono(&nos)
            }
            "GuardedPattern" => f.iter().copied().find(|&k| cx.especie(k) == "WhenClause").and_then(|k| self.visitar(k)),
            "IfElement" | "IfStatement" => {
                let mut it = f.iter().copied();
                let expressao = it.next()?;
                let mut resto: Vec<usize> = it.collect();
                let caso = if resto.first().is_some_and(|&k| cx.especie(k) == "CaseClause") { Some(resto.remove(0)) } else { None };
                let entao = *resto.first()?;
                let senao = resto.get(1).copied();
                self.como_if(expressao, caso, entao, senao)
            }
            "IndexExpression" => {
                let nos: Vec<usize> = f.to_vec();
                self.algum_assincrono(&nos)
            }
            "InstanceCreationExpression" => {
                let args = cx.argumentos(n);
                self.algum_assincrono(&args)
            }
            "LabeledStatement" => f.last().copied().and_then(|k| self.visitar(k)),
            "ListLiteral" | "SetOrMapLiteral" => {
                let nos: Vec<usize> = f.iter().copied().filter(|&k| cx.especie(k) != "TypeArgumentList").collect();
                self.algum_assincrono(&nos)
            }
            "MapLiteralEntry" => self.algum_assincrono(f),
            "MethodInvocation" => {
                let mut nos: Vec<usize> = cx.alvo_do_metodo(n).into_iter().collect();
                nos.extend(cx.argumentos(n));
                self.algum_assincrono(&nos)
            }
            "NamedExpression" => {
                let e = f.get(1).copied().and_then(|k| self.visitar(k));
                so_assincrono(e)
            }
            "ParenthesizedExpression" => primeiro.and_then(|k| self.visitar(k)),
            "PrefixedIdentifier" => {
                let id = *f.last()?;
                self.identificador(id)
            }
            "PrefixExpression" => {
                if cx.texto(n).trim_start().starts_with('!') {
                    match primeiro.and_then(|k| self.visitar(k)) {
                        Some(Estado::Montado) => Some(Estado::NaoMontado),
                        Some(Estado::NaoMontado) => Some(Estado::Montado),
                        e => e,
                    }
                } else {
                    None
                }
            }
            "PropertyAccess" => {
                let nome = *f.last()?;
                let alvo = if f.len() >= 2 { Some(f[0]) } else { None };
                let a = so_assincrono(alvo.and_then(|k| self.visitar(k)));
                if cx.texto(nome) == "mounted" {
                    if a.is_some() {
                        return a;
                    }
                    return self.identificador(nome);
                }
                a
            }
            "SimpleIdentifier" => self.identificador(n),
            "SwitchCase" => {
                let nos: Vec<usize> = f.iter().copied().filter(|&k| cx.especie(k) != "Label").collect();
                self.em_ordem_guardavel(&nos)
            }
            "SwitchDefault" => {
                let nos: Vec<usize> = f.iter().copied().filter(|&k| cx.especie(k) != "Label").collect();
                self.em_ordem_guardavel(&nos)
            }
            "SwitchExpression" => self.algum_assincrono(f),
            "SwitchExpressionCase" => {
                let (Some(&gp), Some(&expr)) = (f.first(), f.get(1)) else { return None };
                if gp == r {
                    return None;
                }
                let quando = cx.filhos(gp).iter().copied().find(|&k| cx.especie(k) == "WhenClause").and_then(|k| self.visitar(k));
                if expr == r {
                    if matches!(quando, Some(Estado::Assincrono | Estado::Montado)) {
                        return quando;
                    }
                    return None;
                }
                let a = so_assincrono(quando);
                if a.is_some() {
                    return a;
                }
                so_assincrono(self.visitar(expr))
            }
            "SwitchPatternCase" => {
                let gp = f.iter().copied().find(|&k| cx.especie(k) == "GuardedPattern")?;
                if gp == r {
                    return None;
                }
                let comandos: Vec<usize> = f.iter().copied().filter(|&k| crate::arvore_analyzer::e_comando(cx.especie(k))).collect();
                let pai = cx.pai(n);
                let e = self.como_bloco(&comandos, pai);
                if e.is_some() {
                    return e;
                }
                if comandos.contains(&r) {
                    return None;
                }
                let quando = cx.filhos(gp).iter().copied().find(|&k| cx.especie(k) == "WhenClause").and_then(|k| self.visitar(k));
                so_assincrono(quando)
            }
            "SwitchStatement" => {
                let expressao = primeiro?;
                let membros: Vec<usize> = f.iter().copied().skip(1).filter(|&k| matches!(cx.especie(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                // O primeiro cálculo do emissor é descartado, mas visita (e
                // pode marcar o `mounted` não relacionado).
                if so_assincrono(self.visitar(expressao)).is_none() {
                    let _ = self.algum_assincrono(&membros);
                }
                if let Some(indice) = membros.iter().position(|&m| m == r) {
                    let mut caem = true;
                    let mut todos_montados = true;
                    for i in (0..=indice).rev() {
                        let caso = membros[i];
                        if cx.especie(caso) != "SwitchPatternCase" {
                            continue;
                        }
                        let gp = cx.filhos(caso).iter().copied().find(|&k| cx.especie(k) == "GuardedPattern");
                        let quando = gp.and_then(|g| cx.filhos(g).iter().copied().find(|&k| cx.especie(k) == "WhenClause")).and_then(|k| self.visitar(k));
                        if quando == Some(Estado::Assincrono) {
                            return Some(Estado::Assincrono);
                        }
                        if caem {
                            let vazio = !cx.filhos(caso).iter().any(|&k| crate::arvore_analyzer::e_comando(cx.especie(k)));
                            let cai = i == indice || vazio;
                            if cai {
                                todos_montados &= quando == Some(Estado::Montado);
                            } else if todos_montados {
                                return Some(Estado::Montado);
                            }
                            caem &= cai;
                        }
                    }
                    if caem && todos_montados {
                        return Some(Estado::Montado);
                    }
                    return None;
                }
                let a = so_assincrono(self.visitar(expressao));
                if a.is_some() {
                    return a;
                }
                self.algum_assincrono(&membros)
            }
            "TryStatement" => {
                let corpo = primeiro?;
                let capturas: Vec<usize> = f.iter().copied().filter(|&k| cx.especie(k) == "CatchClause").collect();
                let finalmente = f.last().copied().filter(|&k| f.len() > 1 && cx.especie(k) == "Block");
                if corpo == r {
                    return None;
                }
                let mut corpo_e_capturas = vec![corpo];
                corpo_e_capturas.extend(capturas.iter().copied());
                if capturas.contains(&r) {
                    return so_assincrono(self.visitar(corpo));
                }
                if finalmente == Some(r) {
                    return self.algum_assincrono(&corpo_e_capturas);
                }
                if let Some(fin) = finalmente {
                    let e = self.visitar(fin);
                    if e.is_some() {
                        return e;
                    }
                }
                self.algum_assincrono(&corpo_e_capturas)
            }
            "VariableDeclaration" => {
                let init = f.iter().copied().find(|&k| crate::arvore_analyzer::e_expressao(cx.especie(k)));
                so_assincrono(init.and_then(|k| self.visitar(k)))
            }
            "VariableDeclarationStatement" => {
                let lista = f.iter().copied().find(|&k| cx.especie(k) == "VariableDeclarationList")?;
                let inits: Vec<usize> = cx
                    .filhos(lista)
                    .iter()
                    .copied()
                    .filter(|&v| cx.especie(v) == "VariableDeclaration")
                    .filter_map(|v| cx.filhos(v).iter().copied().find(|&k| crate::arvore_analyzer::e_expressao(cx.especie(k))))
                    .collect();
                self.algum_assincrono(&inits)
            }
            "WhenClause" => primeiro.and_then(|k| self.visitar(k)),
            "WhileStatement" => {
                let (Some(&cond), Some(&corpo)) = (f.first(), f.get(1)) else { return None };
                let a = so_assincrono(self.visitar(cond));
                if a.is_some() {
                    return a;
                }
                so_assincrono(self.visitar(corpo))
            }
            _ => None,
        }
    }
}

/// Uma função com parâmetros de callback protegidos.
struct Protegida {
    tipo: &'static str,
    nome: Option<&'static str>,
    posicionais: &'static [usize],
    nomeados: &'static [&'static str],
}

const fn p(tipo: &'static str, nome: Option<&'static str>, posicionais: &'static [usize], nomeados: &'static [&'static str]) -> Protegida {
    Protegida { tipo, nome, posicionais, nomeados }
}

const CONSTRUTORES: &[Protegida] = &[
    p("Future", None, &[0], &[]),
    p("Future", Some("new"), &[0], &[]),
    p("Future", Some("delayed"), &[1], &[]),
    p("Future", Some("microtask"), &[0], &[]),
    p("Stream", Some("eventTransformed"), &[1], &[]),
    p("Stream", Some("multi"), &[0], &[]),
    p("Stream", Some("periodic"), &[1], &[]),
    p("StreamController", None, &[], &["onListen", "onPause", "onResume", "onCancel"]),
    p("StreamController", Some("new"), &[], &["onListen", "onPause", "onResume", "onCancel"]),
    p("StreamController", Some("broadcast"), &[], &["onListen", "onCancel"]),
];

const METODOS_DE_INSTANCIA: &[Protegida] = &[
    p("Future", Some("catchError"), &[0], &["test"]),
    p("Future", Some("onError"), &[0], &["test"]),
    p("Future", Some("then"), &[0], &["onError"]),
    p("Future", Some("timeout"), &[], &["onTimeout"]),
    p("Future", Some("whenComplete"), &[0], &[]),
    p("Stream", Some("any"), &[0], &[]),
    p("Stream", Some("asBroadcastStream"), &[], &["onListen", "onCancel"]),
    p("Stream", Some("asyncExpand"), &[0], &[]),
    p("Stream", Some("asyncMap"), &[0], &[]),
    p("Stream", Some("distinct"), &[0], &[]),
    p("Stream", Some("expand"), &[0], &[]),
    p("Stream", Some("firstWhere"), &[0], &["orElse"]),
    p("Stream", Some("fold"), &[1], &[]),
    p("Stream", Some("forEach"), &[0], &[]),
    p("Stream", Some("handleError"), &[0], &["test"]),
    p("Stream", Some("lastWhere"), &[0], &["orElse"]),
    p("Stream", Some("listen"), &[0], &["onError", "onDone"]),
    p("Stream", Some("map"), &[0], &[]),
    p("Stream", Some("reduce"), &[0], &[]),
    p("Stream", Some("singleWhere"), &[0], &["orElse"]),
    p("Stream", Some("skipWhile"), &[0], &[]),
    p("Stream", Some("takeWhile"), &[0], &[]),
    p("Stream", Some("timeout"), &[], &["onTimeout"]),
    p("Stream", Some("where"), &[0], &[]),
    p("StreamSubscription", Some("onData"), &[0], &[]),
    p("StreamSubscription", Some("onDone"), &[0], &[]),
    p("StreamSubscription", Some("onError"), &[0], &[]),
];

const METODOS_ESTATICOS: &[Protegida] = &[
    p("Future", Some("doWhile"), &[0], &[]),
    p("Future", Some("forEach"), &[1], &[]),
    p("Future", Some("wait"), &[], &["cleanUp"]),
];

/// O elemento de `BuildContext` (a declaração) de uma expressão
/// (`buildContextTypedElement`): o getter, a variável ou o parâmetro, e o
/// tipo dele.
#[derive(Clone, Copy)]
enum Contexto {
    /// Um acessor (de campo ou escrito) de uma classe.
    Acessor { classe: Option<ClassId>, tipo: TypeId },
    /// Variável, parâmetro ou função.
    Outro { tipo: TypeId },
}

fn elemento_de_contexto(cx: &Cx<'_>, interner: &Interner, mut n: usize) -> Option<Contexto> {
    let s = cx.s;
    if cx.especie(n) == "NamedExpression" {
        n = *cx.filhos(n).get(1)?;
    }
    if cx.especie(n) == "PropertyAccess" {
        n = *cx.filhos(n).last()?;
    }
    match cx.especie(n) {
        "SimpleIdentifier" | "PrefixedIdentifier" => {
            let id = if cx.especie(n) == "PrefixedIdentifier" { *cx.filhos(n).last()? } else { n };
            let Marca::Expr(e) = cx.arv.nos[id].marca else { return None };
            let r = s.corpo.get_resolved(e)?;
            let funcao = match r {
                Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) | Resolved::ExtensionMember { member: f, .. } => Some(*f),
                _ => None,
            };
            let (tipo, acessor, classe) = match r {
                _ if funcao.is_some_and(|f| s.program.function(f).kind == FunctionKind::Getter) => {
                    let f = funcao?;
                    let classe = match r {
                        Resolved::Member { class, .. } => Some(*class),
                        _ => None,
                    };
                    (s.outline.functions.get(f.0 as usize)?.return_type, true, s.program.function(f).class.or(classe))
                }
                _ if funcao.is_some() => (s.outline.functions.get(funcao?.0 as usize)?.return_type, false, None),
                Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => {
                    let i = v.0 as usize;
                    let d = s.outline.variables.get(i)?;
                    // O identificador lido resolve ao getter sintético.
                    (d.declared_type.or(d.inferred)?, true, s.program.variable(*v).class)
                }
                Resolved::Local(_) => (s.corpo.declaracao_local(e).and_then(|d| s.corpo.tipo_local(d)).or_else(|| s.corpo.get_type(e))?, false, None),
                Resolved::Parameter { .. } => (s.corpo.get_type(e)?, false, None),
                _ => return None,
            };
            if !super::flutter::e_build_context(s, interner, tipo, acessor) {
                return None;
            }
            Some(if acessor { Contexto::Acessor { classe, tipo } } else { Contexto::Outro { tipo } })
        }
        "ParenthesizedExpression" => elemento_de_contexto(cx, interner, *cx.filhos(n).first()?),
        "PostfixExpression" if cx.texto(n).trim_end().ends_with('!') => elemento_de_contexto(cx, interner, *cx.filhos(n).first()?),
        _ => None,
    }
}

/// `lookUpGetter('mounted')`: na classe, nos mixins e na cadeia de
/// superclasses.
fn getter_mounted(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> Option<FunctionElementId> {
    let nome = interner.lookup("mounted")?;
    let mut vistos: Vec<ClassId> = Vec::new();
    let mut atual = Some(c);
    while let Some(k) = atual {
        if vistos.contains(&k) {
            return None;
        }
        vistos.push(k);
        let x = s.program.class(k);
        if let Some(f) = x.instance_members.get(&nome)
            && s.program.function(*f).kind == FunctionKind::Getter
        {
            return Some(*f);
        }
        for m in x.mixin_classes.iter().rev() {
            if let Some(f) = s.program.class(*m).instance_members.get(&nome)
                && s.program.function(*f).kind == FunctionKind::Getter
            {
                return Some(*f);
            }
        }
        atual = x.supertype_class;
    }
    None
}

/// `associatedMountedGetter`.
fn getter_associado(s: &super::Semantica<'_>, interner: &Interner, c: Contexto) -> Option<FunctionElementId> {
    if let Contexto::Acessor { classe: Some(k), .. } = c
        && super::flutter::e_state(s, interner, k)
    {
        return getter_mounted(s, interner, k);
    }
    let tipo = match c {
        Contexto::Acessor { tipo, .. } | Contexto::Outro { tipo } => tipo,
    };
    match s.table.get(tipo) {
        Type::Interface { class, .. } => getter_mounted(s, interner, *class),
        _ => None,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if !ligada("use_build_context_synchronously") {
        return out;
    }
    let program = s.program;
    // `inTestDir`, pela unidade que define a biblioteca.
    let lib = program.library(program.unit(s.unidade).library);
    if let Some(caminho) = lib.units.first().and_then(|x| program.unit(*x).path.as_ref()) {
        let texto = caminho.to_string_lossy();
        let sep = std::path::MAIN_SEPARATOR;
        if ["test", "integration_test", "test_driver", "testing"].iter().any(|d| texto.contains(&format!("{sep}{d}{sep}"))) {
            return out;
        }
    }
    let a = u.ast;
    let corpo = s.corpo;
    // A árvore resolvida: as criações sem `new` e os prefixos.
    let mut construtores: HashSet<ExprId> = HashSet::new();
    let mut prefixos: HashSet<ExprId> = HashSet::new();
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        match &e.kind {
            ExprKind::Call { .. } if matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) => {
                construtores.insert(id);
            }
            ExprKind::InstanceCreation { keyword: None, constructor: None, .. } if !matches!(corpo.get_resolved(id), Some(r) if !matches!(r, Resolved::Constructor(_))) => {
                construtores.insert(id);
            }
            ExprKind::Identifier(_) if matches!(corpo.get_resolved(id), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) => {
                prefixos.insert(id);
            }
            _ => {}
        }
    }
    let antes_de_3 = lib.features.versao().major < 3;
    let arv = crate::arvore_analyzer::construir_resolvida(u.fonte, a, u.unit, antes_de_3, &construtores, &prefixos);
    let cx = Cx { arv: &arv, fonte: u.fonte, s, interner };
    let mut achados: Vec<(Span, &'static CodigoLint)> = Vec::new();

    // `check`.
    let checar = |no: usize, montado: FunctionElementId, achados: &mut Vec<(Span, &'static CodigoLint)>| {
        let mut v = Visitante { cx: &cx, referencia: no, montado, cache: HashMap::new(), nao_relacionado: false };
        let mut filho = no;
        loop {
            if CORPOS.contains(&cx.especie(filho)) {
                break;
            }
            let Some(pai) = cx.pai(filho) else { break };
            v.referencia = filho;
            let estado = v.visitar(pai);
            v.cache.insert(pai, estado);
            if matches!(estado, Some(Estado::Montado | Estado::NaoMontado)) {
                return;
            }
            if estado == Some(Estado::Assincrono) {
                let codigo = if v.nao_relacionado { &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_WRONG_MOUNTED } else { &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE };
                achados.push((cx.span(no), codigo));
                return;
            }
            filho = pai;
        }
        if !CORPOS.contains(&cx.especie(filho)) {
            return;
        }
        // O corpo de uma closure passada a uma função protegida.
        let Some(funcao) = cx.pai(filho) else { return };
        if cx.especie(funcao) != "FunctionExpression" {
            return;
        }
        let Some(mut avo) = cx.pai(funcao) else { return };
        if cx.especie(avo) == "NamedExpression" {
            let Some(x) = cx.pai(avo) else { return };
            avo = x;
        }
        if cx.especie(avo) != "ArgumentList" {
            return;
        }
        let Some(invocacao) = cx.pai(avo) else { return };
        let argumentos = cx.filhos(avo);
        let posicionais: Vec<usize> = argumentos.iter().copied().filter(|&k| cx.especie(k) != "NamedExpression").collect();
        let nomeados: Vec<usize> = argumentos.iter().copied().filter(|&k| cx.especie(k) == "NamedExpression").collect();
        let protegida = |p: &Protegida, achados: &mut Vec<(Span, &'static CodigoLint)>| {
            for &i in p.posicionais {
                if posicionais.get(i) == Some(&funcao) {
                    achados.push((cx.span(no), &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE));
                }
            }
            for nome in p.nomeados {
                let arg = nomeados.iter().copied().find(|&k| cx.filhos(k).first().is_some_and(|&l| cx.texto(l).trim_end_matches(':').trim() == *nome));
                if arg.is_some_and(|k| cx.filhos(k).get(1) == Some(&funcao)) {
                    achados.push((cx.span(no), &c::USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE));
                }
            }
        };
        let classe_do_async = |t: TypeId, nome: &str| -> bool {
            matches!(s.table.get(t), Type::Interface { class, .. }
                if interner.resolve(program.class(*class).name) == nome
                    && program.library(program.class(*class).library).name.as_ref().is_some_and(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join(".") == "dart.async"))
        };
        match cx.especie(invocacao) {
            "InstanceCreationExpression" => {
                let Some(tipo) = cx.tipo(invocacao) else { return };
                let nome_do_construtor = cx
                    .filhos(invocacao)
                    .iter()
                    .copied()
                    .find(|&k| cx.especie(k) == "ConstructorName")
                    .and_then(|k| cx.filhos(k).iter().copied().find(|&i| cx.especie(i) == "SimpleIdentifier"))
                    .map(|k| cx.texto(k));
                for k in CONSTRUTORES {
                    if nome_do_construtor == k.nome && classe_do_async(tipo, k.tipo) {
                        protegida(k, achados);
                    }
                }
            }
            "MethodInvocation" => {
                let Some(nome) = cx.nome_do_metodo(invocacao).map(|k| cx.texto(k)) else { return };
                let alvo = cx.alvo_do_metodo(invocacao).or_else(|| cx.alvo_da_cascata(invocacao));
                let classe_do_alvo = alvo.filter(|&k| matches!(cx.especie(k), "SimpleIdentifier" | "PrefixedIdentifier")).and_then(|k| {
                    let id = if cx.especie(k) == "PrefixedIdentifier" { *cx.filhos(k).last()? } else { k };
                    match cx.resolvido(id) {
                        Some(Resolved::Element(Element::Class(c))) if program.class(*c).kind == dartforge_elements::model::ClassKind::Class => Some(*c),
                        _ => None,
                    }
                });
                if let Some(c) = classe_do_alvo {
                    for m in METODOS_ESTATICOS {
                        if Some(nome) == m.nome && interner.resolve(program.class(c).name) == m.tipo {
                            protegida(m, achados);
                        }
                    }
                } else {
                    let Some(tipo) = alvo.and_then(|k| cx.tipo(k)) else { return };
                    let nome_do_tipo = match s.table.get(tipo) {
                        Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(interner.resolve(program.class(*class).name)),
                        Type::TypeParameter { param, .. } => Some(interner.resolve(s.table.param(*param).name)),
                        _ => None,
                    };
                    for m in METODOS_DE_INSTANCIA {
                        if Some(nome) == m.nome && nome_do_tipo == Some(m.tipo) {
                            protegida(m, achados);
                        }
                    }
                }
            }
            _ => {}
        }
    };

    // Os nós visitados: `MethodInvocation`, `PrefixedIdentifier` e as listas
    // de argumentos de invocação e criação.
    for n in 0..arv.nos.len() {
        match cx.especie(n) {
            "MethodInvocation" => {
                if let Some(alvo) = cx.alvo_do_metodo(n)
                    && cx.tipo(alvo).is_some_and(|t| super::flutter::e_build_context(s, interner, t, true))
                    && let Some(el) = elemento_de_contexto(&cx, interner, alvo)
                    && let Some(m) = getter_associado(s, interner, el)
                {
                    checar(alvo, m, &mut achados);
                }
                for arg in cx.argumentos(n) {
                    if let Some(el) = elemento_de_contexto(&cx, interner, arg)
                        && let Some(m) = getter_associado(s, interner, el)
                    {
                        checar(arg, m, &mut achados);
                    }
                }
            }
            "InstanceCreationExpression" | "FunctionExpressionInvocation" => {
                for arg in cx.argumentos(n) {
                    if let Some(el) = elemento_de_contexto(&cx, interner, arg)
                        && let Some(m) = getter_associado(s, interner, el)
                    {
                        checar(arg, m, &mut achados);
                    }
                }
            }
            "PrefixedIdentifier" => {
                let (Some(&prefixo), Some(&id)) = (cx.filhos(n).first(), cx.filhos(n).last()) else { continue };
                if cx.texto(id) == "mounted" {
                    continue;
                }
                if cx.tipo(prefixo).is_some_and(|t| super::flutter::e_build_context(s, interner, t, true))
                    && let Some(el) = elemento_de_contexto(&cx, interner, prefixo)
                    && let Some(m) = getter_associado(s, interner, el)
                {
                    checar(prefixo, m, &mut achados);
                }
            }
            _ => {}
        }
    }
    achados.sort_by_key(|(sp, c)| (sp.start, sp.end, c.unico));
    achados.dedup_by(|x, y| x.0 == y.0 && x.1.unico == y.1.unico);
    for (span, codigo) in achados {
        out.push(RelatoDeLint { codigo, span, args: Vec::new() });
    }
    out
}

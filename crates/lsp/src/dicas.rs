//! Dicas embutidas (`textDocument/inlayHint`) no formato do
//! `DartInlayHintComputer` do Dart 3.6.2
//! (`pkg/analysis_server/lib/src/computer/computer_inlay_hint.dart`):
//!
//! * tipo (espécie 1) antes do nome de variável declarada sem tipo (`var`,
//!   `final`, `const`, campos e variáveis de topo), de variável de `for-in`
//!   sem tipo, de variável de padrão sem tipo e de parâmetro sem tipo (o de
//!   *closure* inclusive; `this.x`, `super.x` e parâmetros-função não);
//! * tipo de retorno antes do nome (ou do `get`) de função ou método
//!   declarado sem tipo de retorno (setter não);
//! * nome do parâmetro (espécie 2, `nome:`) antes de cada argumento
//!   posicional cuja chamada resolve para uma função com parâmetros
//!   declarados;
//! * argumentos de tipo inferidos (`<int>`) antes do `[`/`{` de literal de
//!   coleção sem argumentos escritos, e depois do nome da classe numa
//!   criação de instância sem argumentos escritos.
//!
//! O analyzer devolve as dicas do arquivo inteiro, sem olhar o intervalo
//! pedido; aqui também. Tipos vêm da inferência comum (`tipo_local`,
//! `get_type`, outline), nunca de uma inferência própria.

use crate::projeto::Projeto;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, ParameterKind, StmtKind};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::HashMap;

/// Uma dica: offset (bytes) onde aparece, texto, espécie LSP (1 tipo, 2
/// parâmetro) e se leva espaço à direita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dica {
    pub offset: usize,
    pub rotulo: String,
    pub especie: u8,
    pub espaco_depois: bool,
}

impl Projeto {
    /// As dicas de `unidade`.
    pub(crate) fn dicas(&self, unidade: UnitId) -> Vec<Dica> {
        let p = self.programa();
        let u = p.unit(unidade);
        let ast = &u.ast;
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let mut saida = Vec::new();
        let tipo = |offset: usize, t: TypeId, saida: &mut Vec<Dica>| {
            saida.push(Dica { offset, rotulo: self.consulta.formatar(t), especie: 1, espaco_depois: true });
        };
        // Variáveis de topo e campos: pelo elemento.
        let mut variaveis: HashMap<(bool, u32, usize), dartforge_elements::model::VariableId> = HashMap::new();
        for (i, v) in p.variables.iter().enumerate() {
            let id = dartforge_elements::model::VariableId(i as u32);
            match v.node {
                VariableRef::TopLevel { unit, decl, index } if unit == unidade => {
                    variaveis.insert((true, decl.0, index), id);
                }
                VariableRef::Field { unit, member, index } if unit == unidade => {
                    variaveis.insert((false, member.0, index), id);
                }
                _ => {}
            }
        }
        // Função da árvore -> elemento (retornos sem tipo escrito).
        let funcoes: HashMap<u32, FunctionElementId> = p
            .functions
            .iter()
            .enumerate()
            .filter_map(|(i, x)| match x.node {
                FunctionRef::Function { unit, function } if unit == unidade => Some((function.0, FunctionElementId(i as u32))),
                _ => None,
            })
            .collect();
        for (i, d) in ast.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Variables(vl) if vl.ty.is_none() => {
                    for (k, var) in vl.variables.iter().enumerate() {
                        if let Some(t) = variaveis.get(&(true, i as u32, k)).and_then(|v| self.consulta.tipo_da_variavel(*v)) {
                            tipo(var.name.span.start, t, &mut saida);
                        }
                    }
                }
                DeclKind::Function(f) => self.retorno(unidade, *f, funcoes.get(&f.0).copied(), &mut saida),
                _ => {}
            }
        }
        for (i, m) in ast.members.iter().enumerate() {
            match &m.kind {
                MemberKind::Field(vl) if vl.ty.is_none() => {
                    for (k, var) in vl.variables.iter().enumerate() {
                        if let Some(t) = variaveis.get(&(false, i as u32, k)).and_then(|v| self.consulta.tipo_da_variavel(*v)) {
                            tipo(var.name.span.start, t, &mut saida);
                        }
                    }
                }
                MemberKind::Method(f) => self.retorno(unidade, *f, funcoes.get(&f.0).copied(), &mut saida),
                _ => {}
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::Variables(vl) if vl.ty.is_none() => {
                    for var in vl.variables.iter() {
                        if let Some(t) = corpos.tipo_local(var.name.span.start) {
                            tipo(var.name.span.start, t, &mut saida);
                        }
                    }
                }
                StmtKind::ForIn { target: ast::ForInTarget::Declared { ty: None, name, .. }, .. } => {
                    if let Some(t) = corpos.tipo_local(name.span.start) {
                        tipo(name.span.start, t, &mut saida);
                    }
                }
                StmtKind::Function(f) => {
                    let func = ast.function(*f);
                    if func.return_type.is_none()
                        && func.kind != ast::FunctionKind::Setter
                        && let Some(n) = func.name
                        && let Some(t) = corpos.tipo_local(n.span.start)
                        && let Type::Function { ret, .. } = self.consulta.tabela.get(t)
                    {
                        tipo(n.span.start, *ret, &mut saida);
                    }
                }
                _ => {}
            }
        }
        for pt in &ast.patterns {
            if let ast::PatternKind::Variable { ty: None, name, .. } = &pt.kind
                && let Some(t) = corpos.tipo_local(name.span.start)
            {
                tipo(name.span.start, t, &mut saida);
            }
        }
        // Parâmetros sem tipo (de funções, métodos e closures).
        for f in &ast.functions {
            for prm in f.parameters.iter().flatten() {
                if prm.ty.is_none()
                    && !prm.this_
                    && !prm.super_
                    && prm.function_parameters.is_none()
                    && let Some(n) = prm.name
                    && let Some(t) = corpos.tipo_local(n.span.start)
                {
                    tipo(n.span.start, t, &mut saida);
                }
            }
        }
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    let f = match corpos.get_resolved(id) {
                        Some(Resolved::Constructor(f)) => Some(*f),
                        _ => match corpos.get_resolved(*target) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. })
                            | Some(Resolved::ExtensionMember { member: f, .. })
                            | Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                            | Some(Resolved::Constructor(f)) => Some(*f),
                            _ => None,
                        },
                    };
                    let nomes = match f {
                        Some(f) => self.nomes_posicionais(f),
                        None => corpos.declaracao_local(*target).map_or_else(Vec::new, |d| nomes_de_funcao_local(ast, &u.source, d)),
                    };
                    self.nomes_de_argumentos(ast, &u.source, arguments, &nomes, &mut saida);
                    // `Caixa('a')` sem `new`: os argumentos de tipo inferidos
                    // depois do nome da classe.
                    if let Some(Resolved::Constructor(_)) = corpos.get_resolved(id) {
                        let classe = match &ast.expr(*target).kind {
                            ExprKind::Identifier(n) => Some(n.span.end),
                            ExprKind::Property { target: t, .. } => match &ast.expr(*t).kind {
                                ExprKind::Identifier(n) => Some(n.span.end),
                                ExprKind::Property { name, .. } => Some(name.span.end),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(fim) = classe {
                            self.argumentos_de_tipo(corpos.get_type(id), fim, &mut saida);
                        }
                    }
                }
                ExprKind::InstanceCreation { ty, arguments, .. } => {
                    if let Some(Resolved::Constructor(f)) = corpos.get_resolved(id) {
                        let nomes = self.nomes_posicionais(*f);
                        self.nomes_de_argumentos(ast, &u.source, arguments, &nomes, &mut saida);
                    }
                    if let ast::TypeKind::Named { args, name } = &ast.ty(*ty).kind
                        && args.is_empty()
                        && let Some(n) = name.last()
                    {
                        self.argumentos_de_tipo(corpos.get_type(id), n.span.end, &mut saida);
                    }
                }
                ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } if type_args.is_empty() => {
                    let abre = if matches!(e.kind, ExprKind::List { .. }) { '[' } else { '{' };
                    if let Some(k) = u.source[e.span.start..e.span.end].find(abre) {
                        self.argumentos_de_tipo(corpos.get_type(id), e.span.start + k, &mut saida);
                    }
                }
                _ => {}
            }
        }
        saida.sort_by_key(|d| d.offset);
        saida
    }

    /// Tipo de retorno de uma função ou método sem tipo escrito.
    fn retorno(&self, unidade: UnitId, f: ast::FunctionId, elemento: Option<FunctionElementId>, saida: &mut Vec<Dica>) {
        let p = self.programa();
        let Some(fe) = elemento else { return };
        let func = p.unit(unidade).ast.function(f);
        if func.return_type.is_some() || func.kind == ast::FunctionKind::Setter || p.function(fe).kind == FunctionKind::Setter {
            return;
        }
        let Some(n) = func.name else { return };
        // Getter: antes do `get`.
        let fonte = &p.unit(unidade).source;
        let offset = if func.kind == ast::FunctionKind::Getter {
            fonte[..n.span.start].trim_end().strip_suffix("get").map_or(n.span.start, str::len)
        } else {
            n.span.start
        };
        let t = self.consulta.outline.functions[fe.0 as usize].return_type;
        saida.push(Dica { offset, rotulo: self.consulta.formatar(t), especie: 1, espaco_depois: true });
    }

    /// Nomes dos parâmetros posicionais de `f`, na ordem.
    fn nomes_posicionais(&self, f: FunctionElementId) -> Vec<String> {
        let p = self.programa();
        let (unit, ps): (UnitId, &[ast::Parameter]) = match p.function(f).node {
            FunctionRef::Function { unit, function } => (unit, p.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[])),
            FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (unit, &k.parameters),
                _ => return Vec::new(),
            },
            FunctionRef::None => return Vec::new(),
        };
        let fonte = &p.unit(unit).source;
        ps.iter()
            .filter(|x| x.kind != ParameterKind::Named)
            .map(|x| x.name.map_or(String::new(), |n| fonte[n.span.start..n.span.end].to_string()))
            .collect()
    }

    /// `nome:` antes de cada argumento posicional.
    fn nomes_de_argumentos(&self, ast: &ast::Ast, _fonte: &str, argumentos: &ast::Arguments, nomes: &[String], saida: &mut Vec<Dica>) {
        let mut k = 0;
        for a in argumentos.args.iter() {
            if a.name.is_some() {
                continue;
            }
            if let Some(n) = nomes.get(k).filter(|n| !n.is_empty()) {
                saida.push(Dica { offset: ast.expr(a.value).span.start, rotulo: format!("{n}:"), especie: 2, espaco_depois: true });
            }
            k += 1;
        }
    }

    /// `<A, B>` dos argumentos de um tipo de interface, em `offset`.
    fn argumentos_de_tipo(&self, tipo: Option<TypeId>, offset: usize, saida: &mut Vec<Dica>) {
        let Some(t) = tipo else { return };
        let args = match self.consulta.tabela.get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return,
        };
        if args.is_empty() {
            return;
        }
        let textos: Vec<String> = args.iter().map(|a| self.consulta.formatar(*a)).collect();
        saida.push(Dica { offset, rotulo: format!("<{}>", textos.join(", ")), especie: 1, espaco_depois: false });
    }
}

/// Nomes dos parâmetros posicionais da função local declarada em `decl`.
fn nomes_de_funcao_local(ast: &ast::Ast, fonte: &str, decl: usize) -> Vec<String> {
    let Some(f) = ast.functions.iter().find(|f| f.name.is_some_and(|n| n.span.start == decl)) else {
        return Vec::new();
    };
    f.parameters
        .iter()
        .flatten()
        .filter(|x| x.kind != ParameterKind::Named)
        .map(|x| x.name.map_or(String::new(), |n| fonte[n.span.start..n.span.end].to_string()))
        .collect()
}

//! Padrões (P3): o casamento como árvore de decisão sobre os testes que o
//! lowering já tem (`testar_tipo`, `==` do Dart, comparações).
//!
//! A semântica é a da especificação de padrões (`patterns-feature-
//! specification.md`, "Pattern matching"): cada subpadrão é testado da
//! esquerda para a direita; o primeiro que falha desvia para o bloco de
//! falha; as variáveis são ligadas à medida que o casamento avança; `||`
//! tenta o segundo lado só se o primeiro falhar; o padrão constante casa se
//! `constante == valor` (o `==` da constante); o relacional é `valor op
//! operando`; o de lista confere o comprimento exato (ou mínimo, com `...`);
//! o de mapa confere `containsKey` antes de ler; o de objeto testa o tipo e
//! lê os getters pelo nome.
//!
//! Os comandos e expressões com padrão (`switch`, `if-case`, declaração e
//! atribuição por padrão, `for-in` com padrão) estão no fim.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind};
use dartforge_frontend::ast::{
    self, BinaryOp, ExprId, ExprKind, ListPatternElement, PatternId, PatternKind, StmtId,
};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

/// Como um padrão liga os nomes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ligacao {
    /// Padrão de declaração (`var (a, b) = …`, `case`): declara.
    Declarar,
    /// Padrão de atribuição (`(a, b) = …`): grava em variáveis existentes.
    Atribuir,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Segue adiante se `cond` (um `i1`), senão desvia para `falha`.
    fn exigir(&mut self, cond: Operand, falha: BlockId) {
        let cond = self.para_bool(cond);
        let ok = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond,
            then_block: ok,
            else_block: falha,
        });
        self.set_block(ok);
    }

    /// Liga o nome de um padrão de variável ao valor.
    fn ligar_padrao(
        &mut self,
        nome: ast::Name,
        valor: Operand,
        ligacao: Ligacao,
        ligados: &mut HashSet<SymbolId>,
        span: Span,
    ) {
        if ligacao == Ligacao::Atribuir || ligados.contains(&nome.sym) {
            if self.gravar_local(nome.sym, valor.clone()).is_none() {
                // Atribuição por padrão a um nome que não é local: pelo
                // escopo léxico (campo, global).
                match self.resolver_por_nome(nome.sym) {
                    Some(dartforge_types::resolved::Resolved::Element(Element::Variable(vid))) => {
                        self.gravar_global(vid, valor, span);
                    }
                    Some(dartforge_types::resolved::Resolved::Member {
                        member: dartforge_types::resolved::MemberRef::Variable(vid),
                        ..
                    }) if !super::e_global(self.ctx, vid) => {
                        if let Some(this) = self.this_param.clone() {
                            self.gravar_campo(this, vid, valor, span);
                        }
                    }
                    _ => {
                        let n = self.ctx.symbol_name(nome.sym).to_string();
                        self.nao_suportado(&format!("atribuição por padrão a `{n}`"), span);
                    }
                }
            }
            return;
        }
        let ty = self.repr_do_local(nome.span.start as usize);
        self.declarar_variavel(nome.sym, nome.span.start as usize, ty, valor);
        ligados.insert(nome.sym);
    }

    /// A classe do programa nomeada por um tipo anotado, se for uma.
    fn classe_do_tipo(&self, ty: &ast::TypeAnnotation) -> Option<ClassId> {
        let ast::TypeKind::Named { name, .. } = &ty.kind else {
            return None;
        };
        let ultimo = name.last()?;
        let lib = self.ctx.program.unit(self.unit_id).library;
        let binding = match &name[..] {
            [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
            _ => self.ctx.program.lookup(lib, ultimo.sym),
        };
        match binding?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    }

    /// Lê o membro `nome` de `valor` para um padrão de objeto: pela classe
    /// do padrão quando ela é do programa, senão pelo despacho dinâmico.
    fn ler_membro_de_padrao(
        &mut self,
        valor: Operand,
        classe: Option<ClassId>,
        nome: SymbolId,
        origem: ExprId,
        span: Span,
    ) -> Operand {
        let texto = self.ctx.symbol_name(nome).to_string();
        if let Some(c) = classe
            && self.ctx.biblioteca_compilada(self.ctx.program.classes[c.0 as usize].library)
        {
            // `index`/`name` de enum.
            if let Some(op) = self.membro_de_enum(c, &texto, valor.clone()) {
                return op;
            }
            for k in crate::lower::membros::linearizacao(self.ctx, c) {
                let cl = &self.ctx.program.classes[k.0 as usize];
                if !self.ctx.biblioteca_compilada(cl.library) {
                    break;
                }
                if let Some(&f) = cl.instance_members.get(&nome) {
                    let fid = f.0 as usize;
                    if let Some(vid) = self.ctx.program.functions[fid].variable {
                        return self.ler_campo_com_late(valor, vid, span);
                    }
                    if self.ctx.program.functions[fid].kind == FunctionKind::Getter {
                        return self.chamar_membro(valor, fid, &[], span);
                    }
                    return self.tearoff_de_metodo(valor, fid, span);
                }
                if let Some(&v) = cl.fields.iter().find(|&&v| {
                    let var = &self.ctx.program.variables[v.0 as usize];
                    !var.static_ && var.name == nome
                }) {
                    return self.ler_campo_com_late(valor, v, span);
                }
            }
        }
        let alvos = self.alvos_por_nome(&texto);
        let tem_alvos = !alvos.is_empty();
        let v2 = valor.clone();
        self.despachar(
            valor,
            &alvos,
            super::despacho::Uso::Ler,
            &mut |_s: &mut Self| Vec::new(),
            &mut |s: &mut Self| {
                let n = s.erros.len();
                let r = s.propriedade_sdk_por_nome(v2.clone(), &texto, origem, span);
                if s.erros.len() > n && tem_alvos {
                    s.erros.truncate(n);
                    return s.lancar_nsm(&texto);
                }
                r
            },
            span,
        )
    }

    /// Casa `valor` com o padrão `p`; em falha desvia para `falha`. `origem`
    /// é a expressão do valor casado (para os membros do SDK por nome).
    pub fn casar(
        &mut self,
        ast: &ast::Ast,
        p: PatternId,
        valor: Operand,
        falha: BlockId,
        ligacao: Ligacao,
        ligados: &mut HashSet<SymbolId>,
        origem: ExprId,
    ) {
        let padrao = ast.pattern(p);
        let span = padrao.span;
        match &padrao.kind {
            PatternKind::Wildcard { ty } => {
                if let Some(t) = ty {
                    let v = self.coagir(valor, Type::Ref);
                    let ok = self.testar_tipo(ast.ty(*t), v);
                    self.exigir(ok, falha);
                }
            }
            PatternKind::Variable {
                ty: None,
                name,
                var_: false,
                final_: false,
            } if self.padrao_refutavel && ligacao == Ligacao::Declarar && self.ctx.symbol_name(name.sym) != "_" => {
                // Num padrão de casamento, um nome solto é um padrão
                // constante (especificação de padrões, "Constant patterns"):
                // casa se `constante == valor`.
                let c = match self.ler_local_por_nome(name.sym) {
                    Some(v) => v,
                    None => match self.ler_nome_sem_resolucao(name.sym, span) {
                        Some(v) => v,
                        None => {
                            let n = self.ctx.symbol_name(name.sym).to_string();
                            self.nao_suportado(&format!("padrão constante `{n}`"), span)
                        }
                    },
                };
                let ok = self.operar(BinaryOp::Eq, c, valor, false, span);
                self.exigir(ok, falha);
            }
            PatternKind::Variable { ty, name, .. } => {
                if let Some(t) = ty {
                    let v = self.coagir(valor.clone(), Type::Ref);
                    let ok = self.testar_tipo(ast.ty(*t), v);
                    self.exigir(ok, falha);
                }
                self.ligar_padrao(*name, valor, ligacao, ligados, span);
            }
            PatternKind::Constant(e) => {
                // `const (e)`: expressão constante entre parênteses (o
                // parser a lê como record de um campo; sem a vírgula final
                // ela é só `e`).
                let e = match &ast.expr(*e).kind {
                    ExprKind::Record { positional, named, .. } if positional.len() == 1 && named.is_empty() => {
                        let s = ast.expr(*e).span;
                        let texto = &self.source()[s.start as usize..s.end as usize];
                        if texto.trim_end().trim_end_matches(')').trim_end().ends_with(',') {
                            e
                        } else {
                            &positional[0]
                        }
                    }
                    _ => e,
                };
                let c = self.lower_expr(ast, *e);
                let ok = self.operar(BinaryOp::Eq, c, valor, false, span);
                self.exigir(ok, falha);
            }
            PatternKind::Relational { op, value } => {
                let c = self.lower_expr(ast, *value);
                let ok = match op {
                    BinaryOp::Eq | BinaryOp::NotEq => self.operar(*op, valor, c, false, span),
                    _ => {
                        let v = self.coagir(valor, Type::Ref);
                        let c = self.coagir(c, Type::Ref);
                        self.operar_dinamico(*op, v, c, span)
                    }
                };
                self.exigir(ok, falha);
            }
            PatternKind::Or(a, b) => {
                let tenta_b = self.new_block();
                let fim = self.new_block();
                self.casar(ast, *a, valor.clone(), tenta_b, ligacao, ligados, origem);
                self.terminate(Terminator::Branch(fim));
                self.set_block(tenta_b);
                self.casar(ast, *b, valor, falha, ligacao, ligados, origem);
                self.terminate(Terminator::Branch(fim));
                self.set_block(fim);
            }
            PatternKind::And(a, b) => {
                self.casar(ast, *a, valor.clone(), falha, ligacao, ligados, origem);
                self.casar(ast, *b, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::Parenthesized(x) => self.casar(ast, *x, valor, falha, ligacao, ligados, origem),
            PatternKind::NullCheck(x) => {
                if self.operand_type(&valor) == Type::Ref {
                    let nn = self.emit(
                        Instruction::ICmp(ICmpOp::Ne, valor.clone(), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    );
                    self.exigir(nn, falha);
                }
                self.casar(ast, *x, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::NullAssert(x) => {
                if self.operand_type(&valor) == Type::Ref {
                    let nulo = self.emit(
                        Instruction::ICmp(ICmpOp::Eq, valor.clone(), Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    );
                    let b_erro = self.new_block();
                    let b_ok = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: nulo,
                        then_block: b_erro,
                        else_block: b_ok,
                    });
                    self.set_block(b_erro);
                    let e = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_type_error_new".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.emit_throw_op(e);
                    self.set_block(b_ok);
                }
                self.casar(ast, *x, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::Cast { pattern, ty } => {
                let v = self.coagir(valor.clone(), Type::Ref);
                self.checar_tipo_ou_lancar(ast.ty(*ty), v);
                self.casar(ast, *pattern, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::List { type_args, .. } | PatternKind::Map { type_args, .. } if !type_args.is_empty() => {
                // `<int>[…]`: o tipo do elemento em tempo de execução (RTI)
                // ainda não existe.
                let _ = (valor, falha, ligados, origem, ligacao);
                self.nao_suportado("padrão de coleção com argumento de tipo (RTI)", span);
            }
            PatternKind::List { elements, .. } if self.ctx.sdk_da_fonte => {
                self.casar_lista_fonte(ast, elements, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::Map { entries, .. } if self.ctx.sdk_da_fonte => {
                self.casar_mapa_fonte(ast, entries, valor, falha, ligacao, ligados, origem);
            }
            PatternKind::List { elements, .. } => {
                let v = self.coagir(valor, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let e_lista = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(-3))),
                    Type::I1,
                );
                self.exigir(e_lista, falha);
                let len = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_list_len".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let resto = elements
                    .iter()
                    .position(|e| matches!(e, ListPatternElement::Rest(_)));
                let n_fixos = elements.len() - usize::from(resto.is_some());
                let ok = self.emit(
                    Instruction::ICmp(
                        if resto.is_some() { ICmpOp::Sge } else { ICmpOp::Eq },
                        len.clone(),
                        Operand::Constant(Constant::Int(n_fixos as i64)),
                    ),
                    Type::I1,
                );
                self.exigir(ok, falha);
                for (i, el) in elements.iter().enumerate() {
                    match (el, resto) {
                        (ListPatternElement::Pattern(sp), Some(r)) if i > r => {
                            // Depois do resto: contado do fim.
                            let depois = (elements.len() - i) as i64;
                            let idx = self.emit(
                                Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(depois))),
                                Type::I64,
                            );
                            let x = self.ler_elemento_lista(v.clone(), idx, Type::Ref);
                            self.casar(ast, *sp, x, falha, ligacao, ligados, origem);
                        }
                        (ListPatternElement::Pattern(sp), _) => {
                            let x = self.ler_elemento_lista(
                                v.clone(),
                                Operand::Constant(Constant::Int(i as i64)),
                                Type::Ref,
                            );
                            self.casar(ast, *sp, x, falha, ligacao, ligados, origem);
                        }
                        (ListPatternElement::Rest(Some(sp)), _) => {
                            let depois = (elements.len() - i - 1) as i64;
                            let fim = self.emit(
                                Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(depois))),
                                Type::I64,
                            );
                            let sub = self.emit_call_with_check(
                                Instruction::CallRuntime {
                                    name: "dartforge_list_sublist".to_string(),
                                    args: vec![
                                        (v.clone(), Type::Ref),
                                        (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                                        (fim, Type::I64),
                                    ],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            );
                            self.casar(ast, *sp, sub, falha, ligacao, ligados, origem);
                        }
                        (ListPatternElement::Rest(None), _) => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                let v = self.coagir(valor, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let e_mapa = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(-4))),
                    Type::I1,
                );
                self.exigir(e_mapa, falha);
                for en in entries.iter() {
                    let k = self.lower_expr(ast, en.key);
                    let ktag = self.operand_tag(&k);
                    let (kbits, _) = self.para_bits(k);
                    let tem = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_map_contains".to_string(),
                            args: vec![
                                (v.clone(), Type::Ref),
                                (kbits.clone(), Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                            ],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                    let tem = self.emit(
                        Instruction::ICmp(ICmpOp::Ne, tem, Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    );
                    self.exigir(tem, falha);
                    let x = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_map_get_ref".to_string(),
                            args: vec![
                                (v.clone(), Type::Ref),
                                (kbits, Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.casar(ast, en.value, x, falha, ligacao, ligados, origem);
                }
            }
            PatternKind::Record { fields } if fields.iter().any(|f| f.name.is_some() || self.nome_implicito(ast, f).is_some()) => {
                // Forma com campo nomeado (`registros.rs`): a classe do
                // valor tem de ser a da forma do padrão.
                let mut npos = 0usize;
                let mut nomes = Vec::new();
                let mut campos = Vec::new();
                for f in fields.iter() {
                    match f.name.map(|n| n.sym).or_else(|| self.nome_implicito(ast, f)) {
                        Some(s) => {
                            let n = self.ctx.symbol_name(s).to_string();
                            nomes.push(n.clone());
                            campos.push((Some(n), f.pattern, f.name.is_none()));
                        }
                        None => {
                            npos += 1;
                            campos.push((None, f.pattern, false));
                        }
                    }
                }
                let mut ordenados = nomes.clone();
                ordenados.sort();
                let Some(id) = self.ctx.id_da_forma(npos, &ordenados) else {
                    self.nao_suportado("padrão de record com forma desconhecida", span);
                    return;
                };
                let v = self.coagir(valor, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let ok = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(i64::from(id)))),
                    Type::I1,
                );
                self.exigir(ok, falha);
                let mut k_pos = 0usize;
                for (nome, sp, implicito) in campos {
                    let idx = match nome {
                        None => {
                            k_pos += 1;
                            k_pos - 1
                        }
                        Some(n) => npos + ordenados.iter().position(|x| *x == n).expect("nome"),
                    };
                    let x = self.campo_de_forma(v.clone(), idx);
                    // `:x` declara `x` mesmo num padrão de casamento.
                    let salvo = self.padrao_refutavel;
                    if implicito {
                        self.padrao_refutavel = false;
                    }
                    self.casar(ast, sp, x, falha, ligacao, ligados, origem);
                    self.padrao_refutavel = salvo;
                }
            }
            PatternKind::Record { fields } => {
                let v = self.coagir(valor, Type::Ref);
                let cls = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_value_class".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let e_rec = if self.ctx.sdk_da_fonte {
                    let _ = cls;
                    self.e_instancia_do_core(v.clone(), "Record")
                } else {
                    self.emit(Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(-7))), Type::I1)
                };
                self.exigir(e_rec, falha);
                let n = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_record_len".to_string(),
                        args: vec![(v.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let ok = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, n, Operand::Constant(Constant::Int(fields.len() as i64))),
                    Type::I1,
                );
                self.exigir(ok, falha);
                for (i, f) in fields.iter().enumerate() {
                    let x = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_record_get_ref".to_string(),
                            args: vec![
                                (v.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.casar(ast, f.pattern, x, falha, ligacao, ligados, origem);
                }
            }
            PatternKind::Object { ty, fields } => {
                let t = ast.ty(*ty);
                let v = self.coagir(valor, Type::Ref);
                let ok = self.testar_tipo(t, v.clone());
                self.exigir(ok, falha);
                let classe = self.classe_do_tipo(t);
                for f in fields.iter() {
                    let nome = match f.name {
                        Some(n) => Some(n.sym),
                        None => self.nome_implicito(ast, f),
                    };
                    let Some(nome) = nome else {
                        self.nao_suportado("campo de padrão de objeto sem nome", f.span);
                        continue;
                    };
                    let x = self.ler_membro_de_padrao(v.clone(), classe, nome, origem, f.span);
                    // `:x` declara `x` mesmo num padrão de casamento.
                    let salvo = self.padrao_refutavel;
                    if f.name.is_none() {
                        self.padrao_refutavel = false;
                    }
                    self.casar(ast, f.pattern, x, falha, ligacao, ligados, origem);
                    self.padrao_refutavel = salvo;
                }
            }
        }
    }

    /// O nome de um campo `:x` (o do padrão de variável dentro dele).
    fn nome_implicito(&self, ast: &ast::Ast, f: &ast::PatternField) -> Option<SymbolId> {
        let mut p = f.pattern;
        loop {
            match &ast.pattern(p).kind {
                PatternKind::Variable { name, .. } => {
                    // `:x` tem o nome do campo vazio no texto: o `name` do
                    // campo é `None` só nesse caso quando o padrão é variável
                    // e o texto começa com `:`.
                    let fonte = self.source();
                    let ini = f.span.start as usize;
                    return fonte[ini..].trim_start().starts_with(':').then_some(name.sym);
                }
                PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => {
                    p = *x;
                }
                _ => return None,
            }
        }
    }

    // ------------------------------------------------------------------
    // Construções com padrão

    /// `switch` como comando: casos em ordem, `when`, corpo vazio que cai no
    /// seguinte (vários `case` rotulando o mesmo corpo), `default`, `break`
    /// e `continue rótulo` para o rótulo de um `case`.
    pub fn lower_switch_comando(&mut self, ast: &ast::Ast, value: ExprId, cases: &[ast::SwitchCase]) {
        let v = self.lower_expr(ast, value);
        let saida = self.new_block();
        let rotulos_externos = std::mem::take(&mut self.pending_labels);
        for &l in &rotulos_externos {
            self.labeled_break_targets.insert(l, saida);
        }
        // O bloco do corpo de cada caso (o dos casos vazios é o do próximo
        // caso não vazio: eles compartilham o corpo).
        let mut corpos: Vec<BlockId> = cases.iter().map(|_| self.new_block()).collect();
        for i in (0..cases.len()).rev() {
            if cases[i].body.is_empty() && i + 1 < cases.len() {
                corpos[i] = corpos[i + 1];
            }
        }
        // `continue rótulo` para o rótulo de um caso vai ao corpo dele.
        let mut rotulos_de_caso = Vec::new();
        for (i, c) in cases.iter().enumerate() {
            for l in c.labels.iter() {
                self.labeled_continue_targets.insert(l.sym, corpos[i]);
                rotulos_de_caso.push(l.sym);
            }
        }
        self.break_targets.push(saida);
        // Testes, em ordem; as variáveis de cada caso vivem no escopo dele.
        let mut escopos_dos_casos = Vec::with_capacity(cases.len());
        for c in cases.iter() {
            let proximo = self.new_block();
            self.abrir_escopo();
            let mut ligados = HashSet::new();
            if let Some(p) = c.pattern {
                let salvo = std::mem::replace(&mut self.padrao_refutavel, true);
                self.casar(ast, p, v.clone(), proximo, Ligacao::Declarar, &mut ligados, value);
                self.padrao_refutavel = salvo;
            }
            if let Some(g) = c.guard {
                let ok = self.lower_expr(ast, g);
                self.exigir(ok, proximo);
            }
            let corpo_i = corpos[escopos_dos_casos.len()];
            self.terminate(Terminator::Branch(corpo_i));
            escopos_dos_casos.push(self.escopos.pop().expect("escopo do caso"));
            self.set_block(proximo);
        }
        // Nenhum caso casou: sai.
        self.terminate(Terminator::Branch(saida));
        // Os corpos. O escopo de um corpo compartilhado é o do último caso
        // que o rotula (as variáveis de padrão em casos compartilhados têm
        // de ser as mesmas; aqui cada caso declarou as suas nos testes).
        let mut feitos = HashSet::new();
        for (i, c) in cases.iter().enumerate() {
            if c.body.is_empty() || !feitos.insert(corpos[i]) {
                continue;
            }
            self.set_block(corpos[i]);
            self.escopos.push(escopos_dos_casos[i].clone());
            for &s in c.body.iter() {
                self.lower_stmt(ast, s);
            }
            self.fechar_escopo();
            // Dart 3: um caso não vazio não cai no seguinte.
            self.terminate(Terminator::Branch(saida));
        }
        // Um caso vazio sem caso seguinte (o último): o corpo dele é vazio.
        for (i, c) in cases.iter().enumerate() {
            if c.body.is_empty() && !feitos.contains(&corpos[i]) {
                feitos.insert(corpos[i]);
                self.set_block(corpos[i]);
                self.terminate(Terminator::Branch(saida));
            }
        }
        self.break_targets.pop();
        for l in rotulos_de_caso {
            self.labeled_continue_targets.remove(&l);
        }
        for l in rotulos_externos {
            self.labeled_break_targets.remove(&l);
        }
        self.set_block(saida);
    }

    /// `switch` como expressão: o primeiro caso que casa dá o valor; nenhum
    /// casando é erro em tempo de execução (a exaustividade é do
    /// front-end; aqui o caminho inalcançável lança `StateError`).
    pub fn lower_switch_expressao(
        &mut self,
        ast: &ast::Ast,
        expr_id: ExprId,
        value: ExprId,
        cases: &[ast::SwitchExprCase],
    ) -> Operand {
        let v = self.lower_expr(ast, value);
        let ty = self.repr_da_expressao(expr_id).unwrap_or(Type::Ref);
        let juncao = self.new_block();
        let mut entradas = Vec::new();
        for c in cases.iter() {
            let proximo = self.new_block();
            self.abrir_escopo();
            let mut ligados = HashSet::new();
            let salvo = std::mem::replace(&mut self.padrao_refutavel, true);
            self.casar(ast, c.pattern, v.clone(), proximo, Ligacao::Declarar, &mut ligados, value);
            self.padrao_refutavel = salvo;
            if let Some(g) = c.guard {
                let ok = self.lower_expr(ast, g);
                self.exigir(ok, proximo);
            }
            let r = self.lower_expr(ast, c.body);
            if !self.is_terminated() {
                let r = if matches!(self.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    self.coagir(r, ty)
                };
                if !self.is_terminated() {
                    entradas.push((self.current_block, r));
                    self.terminate(Terminator::Branch(juncao));
                }
            }
            self.fechar_escopo();
            self.set_block(proximo);
        }
        let msg = self.emit(
            Instruction::Const(Constant::String("switch sem caso correspondente".to_string())),
            Type::Ref,
        );
        let e = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_state_error_new".to_string(),
                args: vec![(msg, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(e);
        self.terminate(Terminator::Unreachable);
        self.set_block(juncao);
        if entradas.is_empty() {
            return Self::valor_zero(ty);
        }
        self.emit(Instruction::Phi { incoming: entradas, ty }, ty)
    }

    /// `if (e case p when g) S else T`.
    pub fn lower_if_case(
        &mut self,
        ast: &ast::Ast,
        condition: ExprId,
        pattern: PatternId,
        guard: Option<ExprId>,
        then: StmtId,
        else_: Option<StmtId>,
    ) {
        let v = self.lower_expr(ast, condition);
        let b_senao = self.new_block();
        let fim = self.new_block();
        self.abrir_escopo();
        let mut ligados = HashSet::new();
        let salvo = std::mem::replace(&mut self.padrao_refutavel, true);
        self.casar(ast, pattern, v, b_senao, Ligacao::Declarar, &mut ligados, condition);
        self.padrao_refutavel = salvo;
        if let Some(g) = guard {
            let ok = self.lower_expr(ast, g);
            self.exigir(ok, b_senao);
        }
        self.lower_stmt(ast, then);
        self.fechar_escopo();
        self.terminate(Terminator::Branch(fim));
        self.set_block(b_senao);
        if let Some(e) = else_ {
            self.lower_stmt(ast, e);
        }
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
    }

    /// Um padrão irrefutável (declaração, atribuição, `for-in`): a falha é
    /// um erro em tempo de execução (só o `as` e o `!` podem falhar, e esses
    /// lançam por si; um padrão refutável aqui é recusado pelo front-end).
    pub fn casar_irrefutavel(
        &mut self,
        ast: &ast::Ast,
        p: PatternId,
        valor: Operand,
        ligacao: Ligacao,
        origem: ExprId,
    ) {
        let falha = self.new_block();
        let mut ligados = HashSet::new();
        let salvo = std::mem::replace(&mut self.padrao_refutavel, false);
        self.casar(ast, p, valor, falha, ligacao, &mut ligados, origem);
        self.padrao_refutavel = salvo;
        let segue = self.current_block;
        self.set_block(falha);
        let msg = self.emit(
            Instruction::Const(Constant::String("padrão irrefutável não casou".to_string())),
            Type::Ref,
        );
        let e = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_state_error_new".to_string(),
                args: vec![(msg, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(e);
        self.terminate(Terminator::Unreachable);
        self.set_block(segue);
    }
}


//! Expressões: literais, identificadores, operadores, curto-circuito,
//! condicional, cadeias `?.`, propriedades, índices, coleções, `is`/`as`.
//! As chamadas estão em `calls.rs`; o membro do SDK sem elemento útil vai
//! pela classe dinâmica do receptor (`FnBuilder::property_by_selector`).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Baixa os tres operadores de curto-circuito do Dart: &&, || e ??.
    ///
    /// A semantica esta na especificacao da linguagem: em `a && b` o `b` so e
    /// avaliado quando `a` e verdadeiro, em `a || b` so quando `a` e falso, e
    /// em `a ?? b` so quando `a` e nulo. Nao e otimizacao: o lado direito pode
    /// ter efeito colateral (ou lancar), e avaliar cedo muda o programa.
    ///
    /// Antes desta funcao, && e || caiam no ramo `_` do match de operadores
    /// binarios e viravam a constante 0 — ou seja, toda condicao composta do
    /// corpus era falsa.
    fn lower_short_circuit(
        &mut self,
        ast: &ast::Ast,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
        expr_id: ExprId,
    ) -> Operand {
        let lop = self.lower_expr(ast, left);
        if op == BinaryOp::IfNull {
            return self.lower_if_null(ast, lop, right, expr_id);
        }
        let cond = self.to_bool(lop);
        // O bloco de origem é o que termina com o desvio — `to_bool` pode
        // ter aberto blocos (o `Unbox` de um `bool` encaixotado).
        let left_block = self.current_block;
        let right_block = self.new_block();
        let end_block = self.new_block();
        if op == BinaryOp::And {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: right_block,
                else_block: end_block,
            });
        } else {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: end_block,
                else_block: right_block,
            });
        }

        self.set_block(right_block);
        let rop = self.lower_expr(ast, right);
        let rop = self.to_bool(rop);
        let right_block_end = self.current_block;
        let right_reaches = !self.is_terminated();
        if right_reaches {
            self.terminate(Terminator::Branch(end_block));
        }

        self.set_block(end_block);
        // O valor que vem do lado esquerdo quando ele decide sozinho: em `&&`
        // a esquerda so pula o direito sendo falsa, em `||` sendo verdadeira.
        let from_left = Operand::Constant(Constant::Bool(op != BinaryOp::And));
        if !right_reaches {
            return from_left;
        }
        self.emit(
            Instruction::Phi {
                incoming: vec![(left_block, from_left), (right_block_end, rop)],
                ty: Type::I1,
            },
            Type::I1,
        )
    }

    /// `a ?? b`: `b` só é avaliado quando `a` é null. As duas entradas do
    /// phi saem na representação do resultado (R4) — a da esquerda num
    /// bloco próprio, porque o `Unbox` de `a` só vale quando ele não é null.
    fn lower_if_null(
        &mut self,
        ast: &ast::Ast,
        lop: Operand,
        right: ExprId,
        expr_id: ExprId,
    ) -> Operand {
        if self.operand_type(&lop) != Type::Ref {
            // Escalar nunca é null: a direita é código morto.
            return lop;
        }
        let ty = self.expression_repr(expr_id).unwrap_or(Type::Ref);
        let is_null = self.emit(
            Instruction::ICmp(ICmpOp::Eq, lop.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let right_block = self.new_block();
        let nonnull_block = self.new_block();
        let end_block = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: is_null,
            then_block: right_block,
            else_block: nonnull_block,
        });
        let mut entries = Vec::new();
        self.set_block(nonnull_block);
        let v = self.coerce(lop, ty);
        entries.push((self.current_block, v));
        self.terminate(Terminator::Branch(end_block));
        self.set_block(right_block);
        let r = self.lower_expr(ast, right);
        if !self.is_terminated() {
            let r = self.coerce(r, ty);
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(end_block));
            }
        }
        self.set_block(end_block);
        if entries.len() == 1 {
            return entries.pop().expect("uma entrada").1;
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty,
            },
            ty,
        )
    }

    /// Elo de cadeia de acesso: `a.b`, `a[i]`, `a.m()`.
    fn is_link(ast: &ast::Ast, e: ExprId) -> bool {
        match &ast.expr(e).kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } => true,
            // `a?.b!.c`: o `!` é seletor da cadeia (§17.23), não a encerra.
            ExprKind::Unary { op: UnaryOp::NullAssert, .. } => true,
            ExprKind::Call { target, .. } => {
                matches!(ast.expr(*target).kind, ExprKind::Property { .. })
            }
            _ => false,
        }
    }

    /// A cadeia que termina em `e` tem algum `?.`/`?[`?
    pub(crate) fn chain_has_null_aware(ast: &ast::Ast, e: ExprId) -> bool {
        let mut current = e;
        loop {
            match &ast.expr(current).kind {
                ExprKind::Property {
                    target, null_aware, ..
                }
                | ExprKind::Index {
                    target, null_aware, ..
                } => {
                    if *null_aware {
                        return true;
                    }
                    current = *target;
                }
                ExprKind::Call { target, .. }
                    if matches!(ast.expr(*target).kind, ExprKind::Property { .. }) =>
                {
                    current = *target;
                }
                ExprKind::Unary { op: UnaryOp::NullAssert, operand } => {
                    current = *operand;
                }
                _ => return false,
            }
        }
    }

    /// Baixa o receptor de um elo mantendo a cadeia `?.` corrente (N3).
    pub fn lower_target(&mut self, ast: &ast::Ast, alvo: ExprId) -> Operand {
        self.continue_chain = true;
        self.lower_expr(ast, alvo)
    }

    /// `?.`: se o receptor é null, a cadeia inteira vale null (N3).
    pub fn branch_if_null(&mut self, recv: &Operand) {
        let Some((exit, _)) = self.null_chain.as_ref() else {
            return;
        };
        let exit = *exit;
        let is_null = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                recv.clone(),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        );
        let follows = self.new_block();
        let origin = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: is_null,
            then_block: exit,
            else_block: follows,
        });
        if let Some((_, entries)) = self.null_chain.as_mut() {
            entries.push((origin, Operand::Constant(Constant::Null)));
        }
        self.set_block(follows);
    }

    pub fn lower_expr(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        self.lower_expr_with(ast, expr_id, false)
    }

    /// A expressão de um comando (`e;`): o valor se descarta, então não se
    /// encaixota no fim (um `acc = acc + v` SIMD ou um `i++` não alocam a
    /// caixa que ninguém lê). As demais coerções ficam, pois podem falhar.
    pub fn lower_expr_discarded(&mut self, ast: &ast::Ast, expr_id: ExprId) {
        self.lower_expr_with(ast, expr_id, true);
    }

    /// J05 e o rastro simbólico (§13.14): as instruções da expressão levam a
    /// posição que a VM dá a ela (`expression_position`); ao sair, volta a
    /// de fora (as coerções de quem a usa são de quem a usa).
    fn lower_expr_with(&mut self, ast: &ast::Ast, expr_id: ExprId, discarded: bool) -> Operand {
        if self.ctx.debug_info.is_none() {
            return self.lower_expr_with_in_place(ast, expr_id, discarded);
        }
        let previous = self.position;
        if let Some(offset) = expression_position(ast, &self.ctx.program.unit(self.unit_id).source, expr_id) {
            self.mark_position(ast, offset);
        }
        let r = self.lower_expr_with_in_place(ast, expr_id, discarded);
        self.position = previous;
        r
    }

    fn lower_expr_with_in_place(&mut self, ast: &ast::Ast, expr_id: ExprId, discarded: bool) -> Operand {
        if self.receiver_ready.as_ref().is_some_and(|(e, _)| *e == expr_id) {
            self.continue_chain = false;
            return self.receiver_ready.take().expect("verificado acima").1;
        }
        // Tear-off de função genérica num contexto de função não genérica
        // (`int Function(int) f = id;`): a instanciação implícita que a
        // inferência escolheu, como a de `id<int>` — a closure tem o tipo
        // instanciado e leva os argumentos de tipo.
        if matches!(ast.expr(expr_id).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
            && let Some(args) = self.ctx.bodies.units[self.unit_id.0 as usize]
                .instanciacao_de_tearoff(expr_id)
                .map(<[_]>::to_vec)
            && let Some(fid) = self.target_generic_function(ast, expr_id)
        {
            let mut r = super::rti::Recipe { text: "L<".to_string(), variables: false };
            for (i, t) in args.iter().enumerate() {
                if i > 0 {
                    r.text.push(',');
                }
                let x = self.type_recipe(*t);
                r.text.push_str(&x.text);
                r.variables |= x.variables;
            }
            r.text.push('>');
            let tuple = self.recipe_rti(&r);
            let typ = self.ctx.get_type(self.unit_id, expr_id);
            return self.instantiated_tearoff(fid, tuple, typ);
        }
        let proceed = std::mem::replace(&mut self.continue_chain, false);
        let saved = if proceed {
            None
        } else {
            self.null_chain.take()
        };
        let root =
            !proceed && Self::is_link(ast, expr_id) && Self::chain_has_null_aware(ast, expr_id);
        if root {
            let exit = self.new_block();
            self.null_chain = Some((exit, Vec::new()));
        }
        let mut v = self.lower_expr_internal(ast, expr_id);
        if root {
            let (exit, mut entries) = self.null_chain.take().expect("cadeia aberta");
            // A cadeia `?.` vale null ou o valor: sempre `Ref` na junção.
            let ty = Type::Ref;
            if !self.is_terminated() {
                if self.operand_type(&v) != Type::Void {
                    v = self.coerce(v, Type::Ref);
                }
                entries.push((self.current_block, v.clone()));
                self.terminate(Terminator::Branch(exit));
            }
            self.set_block(exit);
            v = if entries.is_empty() {
                Operand::Constant(Constant::Null)
            } else {
                self.emit(
                    Instruction::Phi {
                        incoming: entries,
                        ty,
                    },
                    ty,
                )
            };
        }
        if !proceed {
            self.null_chain = saved;
        }
        // R5: a expressão sai na representação do seu tipo estático. Tipo
        // `dynamic` (ou ausente) não força nada: o operando fica na
        // representação em que foi produzido, e cada fronteira coage pelo
        // tipo real do operando.
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
            if t != self.ctx.core.dynamic_ && !self.ctx.is_void(t) && !self.is_terminated() {
                let r = self.ctx.to_hir_type(t);
                if !matches!(r, Type::Void)
                    && !matches!(self.operand_type(&v), Type::Void)
                    && !(discarded && r == Type::Ref)
                {
                    v = self.coerce(v, r);
                }
            }
        }
        v
    }

    /// A expressão sem a coerção final ao tipo estático (um local SIMD sem
    /// caixa sai como vetor): para `simd.rs`.
    pub(super) fn lower_expr_raw(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        self.lower_expr_internal(ast, expr_id)
    }

    fn lower_expr_internal(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let expr = ast.expr(expr_id);
        // `const` canônico (P3).
        if let Some(op) = self.canonical_constant(ast, expr_id) {
            return op;
        }
        // Operador, método, getter ou construtor SIMD com forma sem caixa
        // (`simd.rs`): o vetor sai na caixa do tipo estático (fim de
        // `lower_expr`), o escalar direto.
        if let Some(op) = self.simd_expression(ast, expr_id) {
            return op;
        }
        // Membro de `int`/`double` em linha (`intrinsics.rs`).
        if let Some(op) = self.intrinsic_expression(ast, expr_id) {
            return op;
        }
        match &expr.kind {
            ExprKind::Int(span) => {
                let raw = &self.source()[span.start as usize..span.end as usize];
                let text = raw.replace('_', "");
                let val: i64 = if let Some(hex) =
                    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
                {
                    u64::from_str_radix(hex, 16).unwrap_or(0) as i64
                } else {
                    // Como `u64`: `9223372036854775808` só é válido sob o
                    // menos unário (o front-end recusa fora dele), e o
                    // complemento de dois dá `-9223372036854775808`.
                    text.parse::<u64>().map_or(0, |v| v as i64)
                };
                self.emit(Instruction::Const(Constant::Int(val)), Type::I64)
            }
            ExprKind::Double(span) => {
                let raw = &self.source()[span.start as usize..span.end as usize];
                let val: f64 = raw.replace('_', "").parse().unwrap_or(0.0);
                self.emit(Instruction::Const(Constant::Double(val)), Type::F64)
            }
            ExprKind::Bool(b) => self.emit(Instruction::Const(Constant::Bool(*b)), Type::I1),
            ExprKind::Null => self.emit(Instruction::Const(Constant::Null), Type::Ref),
            ExprKind::Symbol(names) => {
                let name = self.symbol_literal_name(names);
                let Some(class) = self.ctx.sdk_class("_internal", "Symbol") else {
                    return self.unsupported("literal de símbolo sem dart:_internal", expr.span);
                };
                let Some(empty) = self.ctx.interner.lookup("") else {
                    return self.unsupported("construtor de Symbol ausente", expr.span);
                };
                let Some(&ctor) = self.ctx.program.classes[class.0 as usize].constructors.get(&empty) else {
                    return self.unsupported("construtor de Symbol ausente", expr.span);
                };
                let text = self.emit(Instruction::Const(Constant::String(name)), Type::Ref);
                self.instantiate_evaluated(ctor, &[(None, text)], expr.span)
            }
            ExprKind::String(str_lit) => {
                if let Some(text) = str_lit.constant_value() {
                    self.emit(Instruction::Const(Constant::StringWtf8(text.as_bytes().to_vec())), Type::Ref)
                } else {
                    let mut parts: Vec<Operand> = Vec::with_capacity(str_lit.parts.len());
                    for part in &str_lit.parts {
                        let part_op = match part {
                            ast::StringPart::Text(t) => {
                                self.emit(Instruction::Const(Constant::StringWtf8(t.as_bytes().to_vec())), Type::Ref)
                            }
                            ast::StringPart::Interpolation(sub_expr) => {
                                let raw_op = self.lower_expr(ast, *sub_expr);
                                let raw_ty = self.operand_type(&raw_op);
                                match raw_ty {
                                    // O `int` entra sem caixa em `ConcatStrings`: os
                                    // dígitos vão direto para o texto junto, sem a
                                    // string intermediária (a `_interpolate` da VM
                                    // também não a guarda).
                                    Type::I64 if str_lit.parts.len() > 1 => raw_op,
                                    Type::I64 => self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_to_string_i64".to_string(),
                                            args: vec![(raw_op, Type::I64)],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    ),
                                    Type::F64 => self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_to_string_f64".to_string(),
                                            args: vec![(raw_op, Type::F64)],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    ),
                                    Type::I1 | Type::I8 => {
                                        let b_i8 = if raw_ty == Type::I1 {
                                            self.emit(
                                                Instruction::ZExt {
                                                    op: raw_op,
                                                    from: Type::I1,
                                                    to: Type::I8,
                                                },
                                                Type::I8,
                                            )
                                        } else {
                                            raw_op
                                        };
                                        self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_to_string_bool".to_string(),
                                                args: vec![(b_i8, Type::I8)],
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        )
                                    }
                                    // Um vetor SIMD sem caixa nunca sai de
                                    // `lower_expr` (ele coage ao tipo estático).
                                    Type::V4F32 | Type::V4I32 | Type::V2F64 => {
                                        let boxed = self.coerce(raw_op, Type::Ref);
                                        self.string_by_selector(boxed)
                                    }
                                    Type::Ref => self.string_by_selector(raw_op),
                                    Type::Void | Type::Ptr => self.emit(
                                        Instruction::Const(Constant::String("null".to_string())),
                                        Type::Ref,
                                    ),
                                }
                            }
                        };
                        parts.push(part_op);
                    }
                    match parts.len() {
                        0 => self.emit(Instruction::Const(Constant::String(String::new())), Type::Ref),
                        1 => parts.pop().expect("uma parte"),
                        _ => self.emit(Instruction::ConcatStrings { parts }, Type::Ref),
                    }
                }
            }
            ExprKind::Identifier(name) => {
                let sym = name.sym;
                let span = expr.span;
                // A resolução manda (R6/N1): um local sombreia um campo, um
                // campo sombreia um global. O mapa por nome só vale para o
                // que a resolução diz que é local ou parâmetro.
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) | None => {
                        if let Some(op) = self.read_local_by_name(sym) {
                            return op;
                        }
                    }
                    r @ Some(Resolved::Member { .. }) if self.resolved_member_te(r.as_ref()).is_some() => {
                        let m = self.resolved_member_te(r.as_ref()).expect("membro de tipo de extensão");
                        let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                        let receiver = self.implicit_receiver();
                        return self.read_member_te(this, m, receiver, span);
                    }
                    Some(Resolved::Member { member, .. }) => {
                        if let Some(op) = self.read_sdk_implicit_member(member, sym, expr_id, span) {
                            return op;
                        }
                        return self.read_implicit_member(member, span);
                    }
                    Some(Resolved::Element(el)) => {
                        return self.read_element(el, span);
                    }
                    Some(Resolved::ExtensionMember { member, .. }) => {
                        let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                        let receiver = self.implicit_receiver();
                        return self.read_extension(this, member.0 as usize, receiver, span);
                    }
                    _ => {}
                }
                if let Some(op) = self.read_local_by_name(sym) {
                    return op;
                }
                // Uma `const` local vista de dentro do getter de uma
                // constante canônica: o inicializador dela, de novo.
                if let Some((_, init)) = self.local_const_keys.get(&sym).cloned() {
                    return self.lower_in_const_context(ast, init);
                }
                // `$this` numa interpolação chega como identificador.
                if self.ctx.symbol_name(sym) == "this"
                    && let Some(t) = self.this_param.clone()
                {
                    return t;
                }
                // `$1` sem receptor, no corpo de uma extensão sobre um record;
                // também um campo nomeado do record do `on` (`sqlPrefix` na
                // extensão sobre `({String? sqlPrefix, bool isNullable})` do
                // drift): o `this` implícito lê o campo (a inferência deixa
                // o identificador como `dynamic`).
                let on_field = self.this_extension.is_some_and(|(_, on)| {
                    matches!(self.ctx.table.get(self.ctx.erase(on)),
                        dartforge_types::table::Type::Record { named, .. } if named.iter().any(|(n, _)| *n == sym))
                });
                if (on_field || self.ctx.symbol_name(sym).starts_with('$'))
                    && let Some(t) = self.this_param.clone()
                    && let Some(op) = {
                        let name_str = self.ctx.symbol_name(sym).to_string();
                        let mut pat = |s: &mut Self| s.throw_nsm(&name_str);
                        self.read_record_field(t, self.ctx.symbol_name(sym), &mut pat)
                    }
                {
                    return op;
                }
                // `x` no corpo de um construtor com `this.x`: a inferência o
                // resolve como parâmetro, mas o parâmetro de inicialização não
                // está no escopo do corpo — ali `x` é o campo.
                if let (Some(this), Some(cid)) = (self.this_param.clone(), self.enclosing_class) {
                    let field = super::members::layout(self.ctx, cid)
                        .into_iter()
                        .find(|v| self.ctx.program.variables[v.0 as usize].name == sym);
                    if let Some(vid) = field {
                        return self.read_field_with_late(this, vid, span);
                    }
                }
                // Sem resolução (corpo de closure, que a inferência ainda não
                // visita): o nome pelo escopo léxico — membro da classe
                // envolvente, depois o escopo da biblioteca.
                if let Some(op) = self.read_unresolved_name(sym, span) {
                    return op;
                }
                // Uma variável de tipo como expressão (`T`): o objeto `Type`
                // do argumento de tipo corrente.
                if let Some(r) = self.type_variable_recipe(sym) {
                    let typ = self.recipe_rti(&r);
                    return self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_rti_objeto_tipo".to_string(),
                            args: vec![(typ, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                }
                let name_str = self.ctx.symbol_name(sym).to_string();
                // `dynamic`, `Never` e `Null` também são expressões que
                // denotam objetos `Type`. Os dois últimos podem vir pela
                // resolução normal; esta via cobre os nomes especiais sem
                // elemento no outline, como já faz o emissor JS.
                let recipe = match name_str.as_str() {
                    "dynamic" => Some("D"),
                    "Never" => Some("N"),
                    "Null" => Some("U"),
                    _ => None,
                };
                if let Some(text_str) = recipe {
                    let typ = self.recipe_rti(&super::rti::Recipe {
                        text: text_str.to_string(),
                        variables: false,
                    });
                    return self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_rti_objeto_tipo".to_string(),
                            args: vec![(typ, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                }
                // Nada no escopo léxico, nas importações nem no outline tem o
                // nome: o programa é inválido (o `Undefined name` da VM). No
                // código do SDK da fonte, construto que falta.
                if self.ctx.from_source.contains(&self.ctx.program.unit(self.unit_id).library) {
                    return self.unsupported(&format!("identificador `{name_str}`"), span);
                }
                self.language_error(&format!("Undefined name '{name_str}'."), span)
            }
            ExprKind::Parenthesized(sub) => self.lower_expr(ast, *sub),
            ExprKind::Binary { op, left, right } => {
                // Curto-circuito vem antes do lowering dos dois lados: && e ||
                // nao podem avaliar a direita quando a esquerda ja decide, e ??
                // so avalia a direita quando a esquerda e nula. O caminho
                // aritmetico abaixo avalia os dois de uma vez, entao esses tres
                // nao podem passar por ele.
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull) {
                    return self.lower_short_circuit(ast, *op, *left, *right, expr_id);
                }
                // `super op e` (P4).
                if matches!(ast.expr(*left).kind, ExprKind::Super) {
                    let rop = self.lower_expr(ast, *right);
                    let name_str = match op {
                        BinaryOp::Eq | BinaryOp::NotEq => "==",
                        _ => super::dispatch::operator(*op).map_or("?", |(n, _)| n),
                    };
                    let r = self.super_operator(name_str, rop, expr.span);
                    if *op == BinaryOp::NotEq {
                        let b = self.to_bool(r);
                        return self.emit(Instruction::LNot(b), Type::I1);
                    }
                    return r;
                }
                // Operador de extensão (P4): chamada direta ao membro.
                if let Some(fid) = self.extension_operator(expr_id) {
                    let lop = self.lower_expr(ast, *left);
                    let rop = self.lower_expr(ast, *right);
                    let receiver = self.ctx.get_type_raw(self.unit_id, *left);
                    let r = self.call_extension(lop, fid, &[(None, rop)], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                // Operador de classe do programa: chamada direta pelo membro
                // (o `==`, com a regra do `null`, fica com o caminho geral).
                if let Some((name_str, _)) = super::dispatch::operator(*op)
                    && let Some(fid) = self.program_operator(*left, name_str)
                {
                    let lop = self.lower_expr(ast, *left);
                    let rop = self.lower_expr(ast, *right);
                    let r = self.call_member(lop, fid, &[(None, rop)], expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                let lop = self.lower_expr(ast, *left);
                let rop = self.lower_expr(ast, *right);
                self.mark_if_typed(*left, &lop);
                if let Some((name_str, _)) = super::dispatch::operator(*op)
                    && !matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                {
                    self.implicit_operator_cast(*right, &rop, *left, name_str, 0);
                } else {
                    self.mark_if_typed(*right, &rop);
                }
                let l_ty = self.ctx.get_type(self.unit_id, *left);
                let r_ty = self.ctx.get_type(self.unit_id, *right);
                let unknown = |t: Option<dartforge_types::table::TypeId>| {
                    t.is_none_or(|t| t == self.ctx.core.dynamic_)
                };
                // `String` pelo tipo estático; com o tipo desconhecido (corpo
                // não inferido, parâmetro de closure sem tipo) um `Ref` à
                // esquerda de `+`/`*` é texto, como era antes.
                // Com o tipo desconhecido, o operador vai ao despacho
                // dinâmico (`dispatch.rs`), que sabe concatenar texto.
                let _ = unknown;
                let text_str = l_ty.is_some_and(|t| self.ctx.is_string(t))
                    || (matches!(op, BinaryOp::Add) && r_ty.is_some_and(|t| self.ctx.is_string(t)));
                // `int?`/`double?` promovido pelo fluxo (`if (x != null) x + 1`):
                // a inferência ainda não grava a promoção no tipo da leitura,
                // então o operando chega `Ref` e volta ao escalar aqui.
                // (`==`/`!=` fica de fora: comparar com null é legítimo.)
                // `String` não anulável à esquerda: o `==` da `String` (que não
                // é sobrescrevível) em linha, `@df.texto_igual_a`.
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && l_ty.is_some_and(|t| {
                        self.ctx.is_string(t)
                            && matches!(self.ctx.table.get(t), dartforge_types::table::Type::Interface { nullable: false, .. })
                    })
                    && self.operand_type(&lop) == Type::Ref
                {
                    let r = self.string_equality(lop, rop);
                    if matches!(op, BinaryOp::NotEq) {
                        return self.emit(Instruction::LNot(r), Type::I1);
                    }
                    return r;
                }
                if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                    && let Some(r) =
                        self.closed_class_equality(l_ty, &lop, &rop, matches!(op, BinaryOp::NotEq))
                {
                    return r;
                }
                let (lop, rop) = if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) || text_str {
                    (lop, rop)
                } else {
                    let l = self.denullify_numeric(*left, lop);
                    let r = self.denullify_numeric(*right, rop);
                    (l, r)
                };
                let admits_smi = self.type_admits_smi(l_ty) && self.type_admits_smi(r_ty);
                self.operate_with_smi(*op, lop, rop, text_str, admits_smi, expr.span)
            }
            ExprKind::Unary { op, operand } => {
                match op {
                    UnaryOp::PrefixInc
                    | UnaryOp::PostfixInc
                    | UnaryOp::PrefixDec
                    | UnaryOp::PostfixDec => {
                        // `x++` = `x += 1` devolvendo o valor antigo; vale para
                        // local, campo, global e índice (antes só local, e um
                        // campo virava a constante 0).
                        let bin = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) {
                            BinaryOp::Add
                        } else {
                            BinaryOp::Sub
                        };
                        let new = self.lower_assignment(
                            ast,
                            ast::AssignOp::Compound(bin),
                            *operand,
                            super::assignment::Rhs::One,
                            expr.span,
                        );
                        let old = self.old_value.take();
                        return match op {
                            UnaryOp::PrefixInc | UnaryOp::PrefixDec => new,
                            _ => old.unwrap_or(new),
                        };
                    }
                    _ => {}
                }
                // `-x`/`~x` de extensão (P4): chamada direta ao membro.
                if matches!(op, UnaryOp::Neg | UnaryOp::BitNot)
                    && let Some(fid) = self.extension_operator(expr_id)
                {
                    let v = self.lower_expr(ast, *operand);
                    let receiver = self.ctx.get_type_raw(self.unit_id, *operand);
                    let r = self.call_extension(v, fid, &[], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                if *op == UnaryOp::NullAssert {
                    let sub_op = self.lower_target(ast, *operand);
                    if self.operand_type(&sub_op) != Type::Ref {
                        // Escalar não é null (R1): nada a checar.
                        return sub_op;
                    }
                    let is_null = self.emit(
                        Instruction::ICmp(
                            ICmpOp::Eq,
                            sub_op.clone(),
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    );
                    let null_block = self.new_block();
                    let cont_block = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: is_null,
                        then_block: null_block,
                        else_block: cont_block,
                    });
                    self.set_block(null_block);
                    let err_op = self.new_runtime_error("dartforge_null_check_error_new", Vec::new());
                    self.emit_throw_op(err_op);
                    self.set_block(cont_block);
                    return sub_op;
                }
                let sub_op = self.lower_expr(ast, *operand);
                let sub_op = if matches!(op, UnaryOp::Neg | UnaryOp::BitNot) {
                    self.denullify_numeric(*operand, sub_op)
                } else {
                    sub_op
                };
                match op {
                    UnaryOp::Neg | UnaryOp::BitNot if self.operand_type(&sub_op) == Type::Ref => {
                        self.dynamic_unary(*op, sub_op)
                    }
                    UnaryOp::Neg if self.operand_type(&sub_op) == Type::F64 => {
                        self.emit(Instruction::FNeg(sub_op), Type::F64)
                    }
                    UnaryOp::Neg => self.emit(Instruction::Neg(sub_op), Type::I64),
                    UnaryOp::Not => {
                        let b = self.to_bool(sub_op);
                        self.emit(Instruction::LNot(b), Type::I1)
                    }
                    UnaryOp::BitNot => self.emit(Instruction::Not(sub_op), Type::I64),
                    _ => self.unsupported("incremento/decremento de não-local", expr.span),
                }
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.to_bool(cond_op);
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block,
                    else_block,
                });
                // As duas entradas do phi na representação do resultado
                // (R4), coagidas no bloco de origem de cada uma.
                let ty = self.expression_repr(expr_id);
                self.set_block(then_block);
                let then_op = self.lower_expr(ast, *then);
                let then_op = match ty {
                    Some(t) => self.coerce(then_op, t),
                    None => then_op,
                };
                let then_end = self.current_block;
                let then_reaches = !self.is_terminated();

                // O `then` só fecha depois do `else`: sem tipo estático, a
                // representação do resultado vem dos dois ramos (iguais, ou
                // `Ref`), e o `then` é coagido no bloco dele.
                self.set_block(else_block);
                let else_op = self.lower_expr(ast, *else_);
                let ty = ty.unwrap_or_else(|| {
                    let (a, b) = (self.operand_type(&then_op), self.operand_type(&else_op));
                    if a == b { a } else { Type::Ref }
                });
                let else_op = self.coerce(else_op, ty);
                let else_end = self.current_block;
                let else_reaches = !self.is_terminated();
                if else_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }
                let (then_op, then_end) = if then_reaches {
                    self.set_block(then_end);
                    let op = self.coerce(then_op, ty);
                    let end = self.current_block;
                    self.terminate(Terminator::Branch(merge_block));
                    (op, end)
                } else {
                    (then_op, then_end)
                };

                self.set_block(merge_block);
                if then_reaches && else_reaches {
                    self.emit(
                        Instruction::Phi {
                            incoming: vec![(then_end, then_op), (else_end, else_op)],
                            ty,
                        },
                        ty,
                    )
                } else if then_reaches {
                    then_op
                } else if else_reaches {
                    else_op
                } else {
                    self.default_return_operand()
                }
            }
            ExprKind::Property {
                target,
                name,
                null_aware,
            } => {
                let prop_name = self.ctx.symbol_name(name.sym);

                let span = expr.span;
                let resolved = self.ctx.get_resolved(self.unit_id, expr_id).cloned();

                // `E.x`: campo ou getter estático de extensão.
                if matches!(
                    self.ctx.get_resolved(self.unit_id, *target),
                    Some(Resolved::Element(dartforge_elements::model::Element::Extension(_)))
                ) {
                    return match resolved {
                        Some(Resolved::Element(el)) => self.read_element(el, span),
                        Some(Resolved::ExtensionMember { member, .. }) => {
                            self.read_static_member(MemberRef::Function(member), span)
                        }
                        _ => self.unsupported(&format!("membro estático de extensão `{prop_name}`"), span),
                    };
                }
                // `C<T…>.new` / `C<T…>.name`: tear-off de construtor
                // instanciado (os argumentos de tipo vão no ambiente).
                if let ExprKind::TypeArguments { target: cls, type_args } = &ast.expr(*target).kind
                    && let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                        self.resolution_or_name(ast, *cls)
                    && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied())
                    else {
                        return self.unsupported(&format!("construtor `{prop_name}`"), span);
                    };
                    let args = self.type_argument_recipes(type_args);
                    let object = self.recipe_rti(&super::rti::Recipe {
                        text: format!("C{}<{}>", self.ctx.id_rti(c), args.text),
                        variables: args.variables,
                    });
                    let tuple = self.recipe_rti(&super::rti::Recipe {
                        text: format!("L<{}>", args.text),
                        variables: args.variables,
                    });
                    let typ = self.ctx.get_type(self.unit_id, expr_id);
                    return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, typ, span);
                }
                // `E.new`/`E.name` de tipo de extensão: tear-off de
                // construtor (o primário também).
                if let Some(c) = self.named_extension_type(ast, *target) {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    if let Some(k) = key
                        && (super::extension_types::primary_name(self.ctx, c) == Some(k)
                            || self.ctx.program.classes[c.0 as usize].constructors.contains_key(&k))
                        && !matches!(resolved, Some(Resolved::Member { .. }))
                    {
                        return self.constructor_tearoff_te(expr_id, c, k, span);
                    }
                }
                // `Alias.new`/`Alias.name` com `typedef Alias<…> = C<…>`:
                // tear-off do construtor de `C` (o `NotifierFamilyProvider
                // .internal` do riverpod). Com o tipo estático instanciado
                // pela inferência, o objeto nasce com os argumentos; sem ele,
                // só quando o alias repassa os parâmetros na ordem (então o
                // tear-off é o genérico de `C`).
                if let Some(Resolved::Element(dartforge_elements::model::Element::Typedef(td))) =
                    self.ctx.get_resolved(self.unit_id, *target).cloned()
                    && let Some(data) = self.ctx.outline.typedefs.get(td.0 as usize)
                    && let dartforge_types::table::Type::Interface { class: c, args, .. } =
                        self.ctx.table.get(data.target_type).clone()
                    && self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                    && !matches!(resolved, Some(Resolved::Member { .. }))
                {
                    let key = if prop_name == "new" { self.ctx.interner.lookup("") } else { Some(name.sym) };
                    if let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied()) {
                        if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                            && let Some((object, tuple)) = self.tearoff_instantiation(c, typ)
                        {
                            return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, Some(typ), span);
                        }
                        // Tear-off genérico por um alias que não só repassa os
                        // parâmetros (`typedef F<T> = C<int, T>`): a função
                        // genérica do tipo estático, com o objeto escrito
                        // nos parâmetros dela — ou, com a instanciação
                        // implícita que a inferência escolheu (`C<int,
                        // String> Function() g = F.nome;`), já instanciado.
                        if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                            && let dartforge_types::table::Type::Function { type_params, ret, .. } = self.ctx.table.get(typ).clone()
                            && !type_params.is_empty()
                            && let dartforge_types::table::Type::Interface { class: c2, args: a2, .. } = self.ctx.table.get(ret).clone()
                            && c2 == c
                        {
                            if let Some(inst) = self.ctx.bodies.units[self.unit_id.0 as usize]
                                .instanciacao_de_tearoff(expr_id)
                                .map(<[_]>::to_vec)
                                .filter(|i| i.len() == type_params.len())
                            {
                                let mut variables = false;
                                let real_set: Vec<String> = inst
                                    .iter()
                                    .map(|t| {
                                        let r = self.type_recipe(*t);
                                        variables |= r.variables;
                                        r.text
                                    })
                                    .collect();
                                let strings: Vec<String> =
                                    a2.iter().map(|a| self.type_recipe_with(*a, &type_params, &real_set).text).collect();
                                let object = self.recipe_rti(&super::rti::Recipe {
                                    text: format!("C{}<{}>", self.ctx.id_rti(c), strings.join(",")),
                                    variables,
                                });
                                let tuple = self.recipe_rti(&super::rti::Recipe {
                                    text: format!("L<{}>", strings.join(",")),
                                    variables,
                                });
                                return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, None, span);
                            }
                            return self.generic_constructor_tearoff(f.0 as usize, c, &type_params, &a2, typ, span);
                        }
                        let forwards = args.len() == data.type_params.len()
                            && args.iter().zip(data.type_params.iter()).all(|(a, p)| {
                                matches!(self.ctx.table.get(*a),
                                    dartforge_types::table::Type::TypeParameter { param, nullable: false } if param == p)
                            });
                        if forwards {
                            let t = self.constructor_tearoff(f.0 as usize, span);
                            self.tearoff_generic_signature(&t, expr_id);
                            return t;
                        }
                    }
                }
                // `C.x`: membro estático (o alvo é um literal de classe, ou
                // um `typedef A = C<…>` — especificação §15 "Type Aliases":
                // `A.x` denota o membro estático `x` de `C`).
                if let Some(c) = self.static_target_class(*target) {
                    if prop_name == "values" && super::enums::is_enum(self.ctx, c) {
                        return self.enum_values(c, span);
                    }
                    // `C.new`/`C.name`: tear-off de construtor (P4).
                    if self.ctx.compiled_library(self.ctx.program.classes[c.0 as usize].library)
                        && matches!(
                            self.ctx.get_resolved(self.unit_id, *target),
                            Some(Resolved::Element(dartforge_elements::model::Element::Class(_)))
                        )
                    {
                        let key = if prop_name == "new" {
                            self.ctx.interner.lookup("")
                        } else {
                            Some(name.sym)
                        };
                        if let Some(f) = key.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied())
                            && !matches!(resolved, Some(Resolved::Member { .. }))
                        {
                            // Classe genérica com os argumentos de tipo
                            // inferidos pelo contexto (`putIfAbsent(k,
                            // HashSet.new)` é `HashSet<V> Function()`): o
                            // tear-off é o instanciado, como `C<T…>.new`.
                            if let Some(typ) = self.ctx.get_type(self.unit_id, expr_id)
                                && let Some((object, tuple)) = self.tearoff_instantiation(c, typ)
                            {
                                return self.instantiated_constructor_tearoff(f.0 as usize, object, tuple, Some(typ), span);
                            }
                            let t = self.constructor_tearoff(f.0 as usize, span);
                            self.tearoff_generic_signature(&t, expr_id);
                            return t;
                        }
                    }
                    return match resolved {
                        Some(Resolved::Member { member, .. }) => {
                            self.read_static_member(member, span)
                        }
                        _ => self.unsupported(&format!("membro estático `{prop_name}`"), span),
                    };
                }

                // `prefix.x`: o elemento importado com prefixo.
                if let Some(el) = self.prefixed_element(ast, *target, name.sym) {
                    return self.read_element(el, span);
                }
                // `super.x` (P4).
                if matches!(ast.expr(*target).kind, ExprKind::Super) {
                    return self.read_super(name.sym, span);
                }

                let target_op = self.lower_target(ast, *target);
                if *null_aware {
                    self.branch_if_null(&target_op);
                }

                // Membro de tipo de extensão (`extension_types.rs`): antes
                // dos caminhos pelo tipo apagado (um `length` do tipo de
                // extensão não é o da lista que ele representa).
                if let Some(m) = self.resolved_member_te(resolved.as_ref()) {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    let r = self.read_member_te(target_op, m, receiver, span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }

                // `length` de uma lista tipada numérica ou `List<E>` (`typed_items.rs`).
                if prop_name == "length"
                    && let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *target))
                {
                    self.access_fixed = self.fixed_list_of(ast, *target);
                    self.access_proven = self.proven_of(ast, *target, None);
                    let n = self.indexed_length(target_op, l);
                    self.access_fixed = None;
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::I64);
                    return self.coerce(n, repr);
                }

                // `index`/`name` de um valor de enum do programa (os
                // campos implícitos, que o elemento não compila).
                if let Some(c) = self.user_class_of(*target)
                    && let Some(op) = self.enum_member(c, prop_name, target_op.clone())
                {
                    return op;
                }

                // Membro de classe do usuário: pelo elemento resolvido (R7),
                // nunca pelo nome.
                if let Some((_, member)) = self.user_member(expr_id, *target, name.sym, false)
                {
                    {
                        let vid = match member {
                            MemberRef::Variable(v) => Some(v),
                            MemberRef::Function(f) => {
                                self.ctx.program.functions[f.0 as usize].variable
                            }
                        };
                        if let Some(vid) = vid {
                            if crate::lower::is_global(self.ctx, vid) {
                                return self.read_global(vid, span);
                            }
                            return self.read_field_with_late(target_op, vid, span);
                        }
                        let MemberRef::Function(f) = member else {
                            unreachable!()
                        };
                        if self.ctx.program.functions[f.0 as usize].kind
                            == dartforge_elements::model::FunctionKind::Getter
                        {
                            return self.call_member(target_op, f.0 as usize, &[], span);
                        }
                        return self.method_tearoff(target_op, f.0 as usize, span);
                    }
                }

                // Getter de extensão (P4).
                if let Some(Resolved::ExtensionMember { member, .. }) = resolved.clone()
                    && crate::lower::user_function(self.ctx, member.0 as usize)
                {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    return self.read_extension(target_op, member.0 as usize, receiver, span);
                }
                // `r.$1`/`r.name`: campo de um record (posicional do
                // runtime, ou de uma forma com campo nomeado).
                let positional_arg = prop_name
                    .strip_prefix('$')
                    .and_then(|d| d.parse::<i64>().ok())
                    .is_some_and(|k| k >= 1);
                if positional_arg || !self.shapes_with_field(prop_name).is_empty() {
                    let name_str = prop_name.to_string();
                    let alvo = *target;
                    let t2 = target_op.clone();
                    let mut rest = |s: &mut Self| s.property_without_member(t2.clone(), alvo, &name_str, expr_id, span);
                    if let Some(op) = self.read_record_field(target_op.clone(), prop_name, &mut rest) {
                        return op;
                    }
                }
                self.property_without_member(target_op, *target, prop_name, expr_id, span)
            }
            // `super[i]` (P4): o `operator []` da superclasse (ou do mixin
            // anterior na linearização).
            ExprKind::Index { target, index, .. } if matches!(ast.expr(*target).kind, ExprKind::Super) => {
                let i = self.lower_expr(ast, *index);
                let Some(name_str) = self.ctx.interner.lookup("[]") else {
                    return self.unsupported("`super[]` sem operador", expr.span);
                };
                let r = self.call_super_method(name_str, &[(None, i)], expr.span);
                let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                self.coerce(r, repr)
            }
            ExprKind::Index {
                target,
                index,
                null_aware,
            } => {
                let target_op = self.lower_target(ast, *target);
                if *null_aware {
                    self.branch_if_null(&target_op);
                }
                let idx_op = self.lower_expr(ast, *index);
                self.implicit_operator_cast(*index, &idx_op, *target, "[]", 0);
                if let Some(fid) = self.extension_operator(expr_id) {
                    let receiver = self.ctx.get_type_raw(self.unit_id, *target);
                    let r = self.call_extension(target_op, fid, &[(None, idx_op)], receiver, None, expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                if let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *target)) {
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    self.access_fixed = self.fixed_list_of(ast, *target);
                    self.access_proven = self.proven_of(ast, *target, Some(*index));
                    let read_done = self.read_indexed(target_op.clone(), idx_op.clone(), l, repr);
                    self.access_proven = None;
                    self.access_fixed = None;
                    if let Some(r) = read_done {
                        return r;
                    }
                }
                // `operator []` de classe do programa: pelo membro.
                if let Some(fid) = self.program_operator(*target, "[]") {
                    let r = self.call_member(target_op, fid, &[(None, idx_op)], expr.span);
                    let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                    return self.coerce(r, repr);
                }
                // `[]` do SDK pela classe dinâmica.
                let r = self.call_by_name_typed(target_op, *target, super::sdk_source::AccessKind::Call, "[]", &[(None, idx_op)]);
                let repr = self.expression_repr(expr_id).unwrap_or(Type::Ref);
                self.coerce(r, repr)
            }
            ExprKind::Record { positional, named, .. } if !named.is_empty() => {
                self.lower_named_record(ast, positional, named, expr.span)
            }
            // O record posicional (`_Record`, `REFS`): os campos encaixotados
            // na HIR e o `AllocRecord` só com `Ref` (`records.rs`).
            ExprKind::Record { positional, .. } => self.lower_positional_record(ast, positional),
            ExprKind::Call { target, arguments } => {
                self.lower_call(ast, expr_id, expr, target, arguments)
            }
            // Com elementos de controle, ou grande e constante (o caminho
            // em pedaços de `literals.rs`, que não põe um buffer de `n`
            // palavras e `n` valores vivos no quadro).
            ExprKind::List { elements, .. }
                if !elements.iter().all(|e| matches!(e, ast::CollectionElement::Expression(_)))
                    || self.large_literal_is_constant(ast, elements) =>
            {
                let l = self.lower_collection_literal(ast, super::literals::Collection::Lista, elements, expr.span);
                self.literal_rti(l, expr_id)
            }
            // Mapas e conjuntos são o `_Map`/`_Set` da fonte do SDK.
            ExprKind::SetOrMap { elements, .. } => {
                let typ = if self.literal_is_set(expr_id, elements) {
                    super::literals::Collection::Set
                } else {
                    super::literals::Collection::Mapa
                };
                let collection = self.lower_collection_literal(ast, typ, elements, expr.span);
                self.literal_rti(collection, expr_id)
            }
            ExprKind::List { elements, .. } => {
                let mut elem_ops = Vec::new();
                for el in elements.iter() {
                    if let ast::CollectionElement::Expression(e) = el {
                        let op = self.lower_expr(ast, *e);
                        let tag = self.operand_tag(&op);
                        elem_ops.push((op, tag));
                    }
                }
                let l = self.emit(Instruction::AllocList { elements: elem_ops }, Type::Ref);
                // RTI: `<int>[…]` é `List<int>` (o tipo do literal).
                if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
                    self.define_rti_if_generic(l.clone(), t);
                }
                l
            }
            ExprKind::InstanceCreation { arguments, constructor, ty, .. } => {
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Constructor(fid)) => {
                        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                        self.instantiate(ast, fid, &arguments.args, expr.span)
                    }
                    // `new A<T>(…)` de uma aplicação de mixin nomeada
                    // (`class A = B with M;`, o `_SyncStreamController` do
                    // `Stream.fromFutures`): sem construtor declarado, o da
                    // superclasse que ela encaminha (`constructor_of`).
                    _ if let Some(dartforge_types::table::Type::Interface { class: c, .. }) =
                        self.ctx.get_type_raw(self.unit_id, expr_id).map(|t| self.ctx.table.get(t).clone())
                        && self.ctx.program.classes[c.0 as usize].kind == dartforge_elements::model::ClassKind::MixinApplication
                        && let Some(f) = constructor
                            .map(|n| n.sym)
                            .or_else(|| match &ast.ty(*ty).kind {
                                ast::TypeKind::Named { name, .. } if name.len() == 2 => Some(name[1].sym),
                                _ => self.ctx.interner.lookup(""),
                            })
                            .and_then(|n| self.constructor_of(c, n)) =>
                    {
                        self.creation_type = self.ctx.get_type_raw(self.unit_id, expr_id);
                        self.instantiate(ast, f, &arguments.args, expr.span)
                    }
                    // `new E(…)`/`const E.name(…)` de tipo de extensão: o
                    // primário não é elemento (`extension_types.rs`).
                    _ => match self.ctx.get_type_raw(self.unit_id, expr_id).map(|t| self.ctx.table.get(t).clone()) {
                        Some(dartforge_types::table::Type::ExtensionType { decl, .. }) => {
                            // `new E.name(…)` sem prefixo chega do parser
                            // como o tipo de duas partes `E.name` (ele não
                            // distingue `p.T` de `T.n`): quando a primeira
                            // parte nomeia o próprio tipo, a segunda é o
                            // construtor.
                            let name_str = match constructor {
                                Some(n) => Some(n.sym),
                                None => match &ast.ty(*ty).kind {
                                    ast::TypeKind::Named { name, .. }
                                        if name.len() == 2
                                            && matches!(
                                                self.ctx
                                                    .program
                                                    .lookup(self.ctx.program.unit(self.unit_id).library, name[0].sym)
                                                    .and_then(|b| b.getter),
                                                Some(dartforge_elements::model::Element::Class(c)) if c == decl
                                            ) =>
                                    {
                                        Some(name[1].sym)
                                    }
                                    _ => self.ctx.interner.lookup(""),
                                },
                            };
                            match name_str {
                                Some(n) => self.build_te(ast, expr_id, decl, n, &arguments.args, expr.span),
                                None => self.unsupported("instanciação não resolvida", expr.span),
                            }
                        }
                        _ => self.unsupported("instanciação não resolvida", expr.span),
                    },
                }
            }
            ExprKind::Throw(inner) => self.emit_throw(ast, *inner),
            ExprKind::Rethrow => self.emit_rethrow(),
            ExprKind::Is { value, ty, negated } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                let is_m = self.test_type(ast_ty, val_op);
                let is_m = self.to_bool(is_m);
                if *negated {
                    self.emit(Instruction::LNot(is_m), Type::I1)
                } else {
                    is_m
                }
            }
            ExprKind::As { value, ty } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                self.check_type_or_throw(ast_ty, val_op.clone(), super::rti::CastContext::How);
                val_op
            }
            ExprKind::Assign { op, target, value } => self.lower_assignment(
                ast,
                *op,
                *target,
                super::assignment::Rhs::Expr(*value),
                expr.span,
            ),
            ExprKind::FunctionExpression(fid) => {
                let typ = self.literal_function_type(ast, *fid, Some(expr_id));
                let c = self.lower_closure(ast, *fid, expr.span, typ);
                // RTI: a assinatura da closure (`f is R Function(P)`).
                self.define_closure_rti(c.clone(), ast, *fid, Some(expr_id));
                c
            }
            ExprKind::Switch { value, cases } => self.lower_switch_expression(ast, expr_id, *value, cases),
            ExprKind::Cascade {
                target,
                sections,
                null_aware,
            } => self.lower_cascade(ast, *target, sections, *null_aware),
            ExprKind::CascadeTarget => match self.current_cascade_target.clone() {
                Some(t) => t,
                None => self.unsupported("cascata", expr.span),
            },
            ExprKind::PatternAssign { pattern, value } => {
                let v = self.lower_expr(ast, *value);
                self.match_irrefutable(ast, *pattern, v.clone(), super::patterns::Linkage::Assign, *value);
                v
            }
            ExprKind::Await(inner) => self.lower_await(ast, *inner, expr.span),
            ExprKind::This => match self.this_param.clone() {
                Some(t) => t,
                None => self.unsupported("`this` fora de membro de instância", expr.span),
            },
            // `f<T…>`: tear-off instanciado de função genérica de topo ou
            // estática (os argumentos de tipo vão no ambiente da closure).
            ExprKind::TypeArguments { target, type_args }
                if let Some(fid) = self.target_generic_function(ast, *target) =>
            {
                let args = self.type_argument_recipes(type_args);
                let tuple = self.recipe_rti(&super::rti::Recipe {
                    text: format!("L<{}>", args.text),
                    variables: args.variables,
                });
                let typ = self.ctx.get_type(self.unit_id, expr_id);
                self.instantiated_tearoff(fid, tuple, typ)
            }
            // `C<T…>` como expressão: o objeto `Type` canônico do tipo
            // instanciado (o literal cru `C` está em `members.rs`).
            ExprKind::TypeArguments { target, type_args }
                if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                    self.resolution_or_name(ast, *target) =>
            {
                let args = self.type_argument_recipes(type_args);
                let t = self.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<{}>", self.ctx.id_rti(c), args.text),
                    variables: args.variables,
                });
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_objeto_tipo".to_string(),
                        args: vec![(t, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
            other => {
                let what = match other {
                    ExprKind::Super => "`super` como valor",

                    ExprKind::TypeArguments { .. } => "instanciação de tipo genérico",


                    ExprKind::Await(_) => "await",

                    _ => "expressão",
                };
                self.unsupported(what, expr.span)
            }
        }
    }

    /// `alvo.name` quando o nome não é membro estático do tipo do alvo:
    /// `index`/`name` de enum, o membro pela classe dinâmica (receptor sem
    /// tipo útil) ou o membro do SDK casado pelo nome.
    pub fn property_without_member(
        &mut self,
        target_op: Operand,
        target: ExprId,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        // `index`/`name` de um valor de enum do programa.
        if let Some(c) = self.user_class_of(target)
            && let Some(op) = self.enum_member(c, prop_name, target_op.clone())
        {
            return op;
        }
        self.property_by_selector(target_op, prop_name, expr_id, span)
    }

    /// Propriedade de valor do SDK (`length`, `isEmpty`, `first`, `message`…):
    /// o membro pela classe dinâmica do receptor, por seletor (o SDK vem
    /// sempre da fonte).
    pub(super) fn property_by_selector(
        &mut self,
        target_op: Operand,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        let _ = (expr_id, span);
        self.call_by_name(target_op, super::sdk_source::AccessKind::Read, prop_name, &[])
    }

    /// Membro implícito (`x` = `this.x`) que é do SDK (a extensão sobre um
    /// tipo do SDK, o `name`/`index` do enum dentro dele): pelo caminho do
    /// SDK com `this` como receptor.
    pub fn read_sdk_implicit_member(
        &mut self,
        member: dartforge_types::resolved::MemberRef,
        sym: dartforge_intern::SymbolId,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Option<Operand> {
        let lib = match member {
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].library,
            MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].library,
        };
        if self.ctx.compiled_library(lib) {
            return None;
        }
        let this = self.this_param.clone()?;
        let name_str = self.ctx.symbol_name(sym).to_string();
        if let Some(c) = self.enclosing_class
            && let Some(op) = self.enum_member(c, &name_str, this.clone())
        {
            return Some(op);
        }
        if name_str.starts_with('$') {
            let n2 = name_str.clone();
            let mut pat = |s: &mut Self| s.throw_nsm(&n2);
            if let Some(op) = self.read_record_field(this.clone(), &name_str, &mut pat) {
                return Some(op);
            }
        }
        Some(self.property_by_selector(this, &name_str, expr_id, span))
    }
}

/// O deslocamento que a VM dá à expressão no rastro (o `fileOffset` do
/// front_end, `offsetForToken(selector.token)`): o nome chamado em `f()`,
/// `o.m()` e `o.p`; o operador em `a + b`, `a += b` e `e as T`; o `[` em
/// `a[i]`; o `(` na chamada de uma expressão (`f()()`); o começo em
/// `throw`, `await`, criação de instância, unário e identificador (o
/// getter de topo). `None`: a expressão não chama nada por si, e fica a
/// posição de fora. `source` é a da unidade da árvore (`mark_position`
/// descarta a posição de outra árvore).
fn expression_position(ast: &ast::Ast, source: &str, expr_id: ExprId) -> Option<usize> {
    // O primeiro byte não branco a partir de `end`: o operador depois do
    // operando da esquerda.
    let after = |end: usize| -> usize {
        source
            .as_bytes()
            .get(end..)
            .and_then(|rest| rest.iter().position(|b| !b.is_ascii_whitespace()))
            .map_or(end, |k| end + k)
    };
    let e = ast.expr(expr_id);
    match &e.kind {
        ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => Some(name.span.start),
            _ => Some(after(ast.expr(*target).span.end)),
        },
        ExprKind::Property { name, .. } => Some(name.span.start),
        ExprKind::Index { target, .. } => Some(after(ast.expr(*target).span.end)),
        ExprKind::Binary { left, .. } | ExprKind::Assign { target: left, .. } | ExprKind::As { value: left, .. } => {
            Some(after(ast.expr(*left).span.end))
        }
        ExprKind::InstanceCreation { .. }
        | ExprKind::Throw(_)
        | ExprKind::Rethrow
        | ExprKind::Await(_)
        | ExprKind::Unary { .. }
        | ExprKind::Identifier(_) => Some(e.span.start),
        _ => None,
    }
}

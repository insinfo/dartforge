//! Expressões: literais, identificadores, operadores, curto-circuito,
//! condicional, cadeias `?.`, propriedades, índices, coleções, `is`/`as`.
//! As chamadas estão em `chamadas.rs`; os membros do SDK casados pelo nome,
//! em `sdk_por_nome.rs`.

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
    fn lower_curto_circuito(
        &mut self,
        ast: &ast::Ast,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
        expr_id: ExprId,
    ) -> Operand {
        let lop = self.lower_expr(ast, left);
        if op == BinaryOp::IfNull {
            return self.lower_se_nulo(ast, lop, right, expr_id);
        }
        let cond = self.para_bool(lop);
        // O bloco de origem é o que termina com o desvio — `para_bool` pode
        // ter aberto blocos (o `Unbox` de um `bool` encaixotado).
        let bloco_esq = self.current_block;
        let bloco_dir = self.new_block();
        let bloco_fim = self.new_block();
        if op == BinaryOp::And {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: bloco_dir,
                else_block: bloco_fim,
            });
        } else {
            self.terminate(Terminator::CondBranch {
                cond,
                then_block: bloco_fim,
                else_block: bloco_dir,
            });
        }

        self.set_block(bloco_dir);
        let rop = self.lower_expr(ast, right);
        let rop = self.para_bool(rop);
        let bloco_dir_fim = self.current_block;
        let direita_alcanca = !self.is_terminated();
        if direita_alcanca {
            self.terminate(Terminator::Branch(bloco_fim));
        }

        self.set_block(bloco_fim);
        // O valor que vem do lado esquerdo quando ele decide sozinho: em `&&`
        // a esquerda so pula o direito sendo falsa, em `||` sendo verdadeira.
        let de_esquerda = Operand::Constant(Constant::Bool(op != BinaryOp::And));
        if !direita_alcanca {
            return de_esquerda;
        }
        self.emit(
            Instruction::Phi {
                incoming: vec![(bloco_esq, de_esquerda), (bloco_dir_fim, rop)],
                ty: Type::I1,
            },
            Type::I1,
        )
    }

    /// `a ?? b`: `b` só é avaliado quando `a` é null. As duas entradas do
    /// phi saem na representação do resultado (R4) — a da esquerda num
    /// bloco próprio, porque o `Unbox` de `a` só vale quando ele não é null.
    fn lower_se_nulo(
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
        let ty = self.repr_da_expressao(expr_id).unwrap_or(Type::Ref);
        let e_nulo = self.emit(
            Instruction::ICmp(ICmpOp::Eq, lop.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let bloco_dir = self.new_block();
        let bloco_nn = self.new_block();
        let bloco_fim = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: e_nulo,
            then_block: bloco_dir,
            else_block: bloco_nn,
        });
        let mut entradas = Vec::new();
        self.set_block(bloco_nn);
        let v = self.coagir(lop, ty);
        entradas.push((self.current_block, v));
        self.terminate(Terminator::Branch(bloco_fim));
        self.set_block(bloco_dir);
        let r = self.lower_expr(ast, right);
        if !self.is_terminated() {
            let r = self.coagir(r, ty);
            if !self.is_terminated() {
                entradas.push((self.current_block, r));
                self.terminate(Terminator::Branch(bloco_fim));
            }
        }
        self.set_block(bloco_fim);
        if entradas.len() == 1 {
            return entradas.pop().expect("uma entrada").1;
        }
        self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty,
            },
            ty,
        )
    }

    /// Elo de cadeia de acesso: `a.b`, `a[i]`, `a.m()`.
    fn e_elo(ast: &ast::Ast, e: ExprId) -> bool {
        match &ast.expr(e).kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } => true,
            ExprKind::Call { target, .. } => {
                matches!(ast.expr(*target).kind, ExprKind::Property { .. })
            }
            _ => false,
        }
    }

    /// A cadeia que termina em `e` tem algum `?.`/`?[`?
    fn cadeia_tem_null_aware(ast: &ast::Ast, e: ExprId) -> bool {
        let mut atual = e;
        loop {
            match &ast.expr(atual).kind {
                ExprKind::Property {
                    target, null_aware, ..
                }
                | ExprKind::Index {
                    target, null_aware, ..
                } => {
                    if *null_aware {
                        return true;
                    }
                    atual = *target;
                }
                ExprKind::Call { target, .. }
                    if matches!(ast.expr(*target).kind, ExprKind::Property { .. }) =>
                {
                    atual = *target;
                }
                _ => return false,
            }
        }
    }

    /// Baixa o receptor de um elo mantendo a cadeia `?.` corrente (N3).
    pub fn lower_alvo(&mut self, ast: &ast::Ast, alvo: ExprId) -> Operand {
        self.continuar_cadeia = true;
        self.lower_expr(ast, alvo)
    }

    /// `?.`: se o receptor é null, a cadeia inteira vale null (N3).
    pub fn desviar_se_nulo(&mut self, recv: &Operand) {
        let Some((saida, _)) = self.cadeia_nula.as_ref() else {
            return;
        };
        let saida = *saida;
        let e_nulo = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                recv.clone(),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        );
        let segue = self.new_block();
        let origem = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: e_nulo,
            then_block: saida,
            else_block: segue,
        });
        if let Some((_, entradas)) = self.cadeia_nula.as_mut() {
            entradas.push((origem, Operand::Constant(Constant::Null)));
        }
        self.set_block(segue);
    }

    pub fn lower_expr(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let continuar = std::mem::replace(&mut self.continuar_cadeia, false);
        let salvo = if continuar {
            None
        } else {
            self.cadeia_nula.take()
        };
        let raiz =
            !continuar && Self::e_elo(ast, expr_id) && Self::cadeia_tem_null_aware(ast, expr_id);
        if raiz {
            let saida = self.new_block();
            self.cadeia_nula = Some((saida, Vec::new()));
        }
        let mut v = self.lower_expr_interno(ast, expr_id);
        if raiz {
            let (saida, mut entradas) = self.cadeia_nula.take().expect("cadeia aberta");
            // A cadeia `?.` vale null ou o valor: sempre `Ref` na junção.
            let ty = Type::Ref;
            if !self.is_terminated() {
                if self.operand_type(&v) != Type::Void {
                    v = self.coagir(v, Type::Ref);
                }
                entradas.push((self.current_block, v.clone()));
                self.terminate(Terminator::Branch(saida));
            }
            self.set_block(saida);
            v = if entradas.is_empty() {
                Operand::Constant(Constant::Null)
            } else {
                self.emit(
                    Instruction::Phi {
                        incoming: entradas,
                        ty,
                    },
                    ty,
                )
            };
        }
        if !continuar {
            self.cadeia_nula = salvo;
        }
        // R5: a expressão sai na representação do seu tipo estático. Tipo
        // `dynamic` (ou ausente) não força nada: o operando fica na
        // representação em que foi produzido, e cada fronteira coage pelo
        // tipo real do operando.
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
            if t != self.ctx.core.dynamic_ && !self.ctx.is_void(t) && !self.is_terminated() {
                let r = self.ctx.to_hir_type(t);
                if !matches!(r, Type::Void) && !matches!(self.operand_type(&v), Type::Void) {
                    v = self.coagir(v, r);
                }
            }
        }
        v
    }

    fn lower_expr_interno(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let expr = ast.expr(expr_id);
        // `const` canônico (P3).
        if let Some(op) = self.constante_canonica(ast, expr_id) {
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
                    text.parse().unwrap_or(0)
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
            ExprKind::String(str_lit) => {
                if let Some(text) = str_lit.constant_value() {
                    let s = String::from_utf8_lossy(text.as_bytes()).to_string();
                    self.emit(Instruction::Const(Constant::String(s)), Type::Ref)
                } else {
                    let mut current_str: Option<Operand> = None;
                    for part in &str_lit.parts {
                        let part_op = match part {
                            ast::StringPart::Text(t) => {
                                let s = String::from_utf8_lossy(t.as_bytes()).to_string();
                                self.emit(Instruction::Const(Constant::String(s)), Type::Ref)
                            }
                            ast::StringPart::Interpolation(sub_expr) => {
                                let raw_op = self.lower_expr(ast, *sub_expr);
                                let raw_ty = self.operand_type(&raw_op);
                                match raw_ty {
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
                                    Type::Ref if self.ctx.sdk_da_fonte => self.texto_por_seletor(raw_op),
                                    Type::Ref => self.emit(
                                        Instruction::CallStatic {
                                            symbol: "dartforge_dispatch_toString".to_string(),
                                            args: vec![raw_op],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    ),
                                    Type::Void | Type::Ptr => self.emit(
                                        Instruction::Const(Constant::String("null".to_string())),
                                        Type::Ref,
                                    ),
                                }
                            }
                        };
                        current_str = match current_str {
                            None => Some(part_op),
                            Some(prev) => {
                                let concat = self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_string_concat".to_string(),
                                        args: vec![(prev, Type::Ref), (part_op, Type::Ref)],
                                        ret_ty: Type::Ref,
                                    },
                                    Type::Ref,
                                );
                                Some(concat)
                            }
                        };
                    }
                    current_str.unwrap_or_else(|| {
                        self.emit(
                            Instruction::Const(Constant::String("".to_string())),
                            Type::Ref,
                        )
                    })
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
                        if let Some(op) = self.ler_local_por_nome(sym) {
                            return op;
                        }
                    }
                    Some(Resolved::Member { member, .. }) => {
                        if let Some(op) = self.ler_membro_implicito_do_sdk(member, sym, expr_id, span) {
                            return op;
                        }
                        return self.ler_membro_implicito(member, span);
                    }
                    Some(Resolved::Element(el)) => {
                        return self.ler_elemento(el, span);
                    }
                    Some(Resolved::ExtensionMember { member, .. }) => {
                        let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                        return self.ler_extensao(this, member.0 as usize, span);
                    }
                    _ => {}
                }
                if let Some(op) = self.ler_local_por_nome(sym) {
                    return op;
                }
                // Uma `const` local vista de dentro do getter de uma
                // constante canônica: o inicializador dela, de novo.
                if let Some((_, init)) = self.chaves_de_const_locais.get(&sym).cloned() {
                    return self.lower_em_contexto_const(ast, init);
                }
                // `$this` numa interpolação chega como identificador.
                if self.ctx.symbol_name(sym) == "this"
                    && let Some(t) = self.this_param.clone()
                {
                    return t;
                }
                // `$1` sem receptor, no corpo de uma extensão sobre um record.
                if self.ctx.symbol_name(sym).starts_with('$')
                    && let Some(t) = self.this_param.clone()
                    && let Some(op) = {
                        let nome = self.ctx.symbol_name(sym).to_string();
                        let mut padrao = |s: &mut Self| s.lancar_nsm(&nome);
                        self.ler_campo_de_registro(t, self.ctx.symbol_name(sym), &mut padrao)
                    }
                {
                    return op;
                }
                // `x` no corpo de um construtor com `this.x`: a inferência o
                // resolve como parâmetro, mas o parâmetro de inicialização não
                // está no escopo do corpo — ali `x` é o campo.
                if let (Some(this), Some(cid)) = (self.this_param.clone(), self.enclosing_class) {
                    let campo = super::membros::layout(self.ctx, cid)
                        .into_iter()
                        .find(|v| self.ctx.program.variables[v.0 as usize].name == sym);
                    if let Some(vid) = campo {
                        return self.ler_campo_com_late(this, vid, span);
                    }
                }
                // Sem resolução (corpo de closure, que a inferência ainda não
                // visita): o nome pelo escopo léxico — membro da classe
                // envolvente, depois o escopo da biblioteca.
                if let Some(op) = self.ler_nome_sem_resolucao(sym, span) {
                    return op;
                }
                let nome = self.ctx.symbol_name(sym).to_string();
                self.nao_suportado(&format!("identificador `{nome}`"), span)
            }
            ExprKind::Parenthesized(sub) => self.lower_expr(ast, *sub),
            ExprKind::Binary { op, left, right } => {
                // Curto-circuito vem antes do lowering dos dois lados: && e ||
                // nao podem avaliar a direita quando a esquerda ja decide, e ??
                // so avalia a direita quando a esquerda e nula. O caminho
                // aritmetico abaixo avalia os dois de uma vez, entao esses tres
                // nao podem passar por ele.
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull) {
                    return self.lower_curto_circuito(ast, *op, *left, *right, expr_id);
                }
                // `super op e` (P4).
                if matches!(ast.expr(*left).kind, ExprKind::Super) {
                    let rop = self.lower_expr(ast, *right);
                    let nome = match op {
                        BinaryOp::Eq | BinaryOp::NotEq => "==",
                        _ => super::despacho::operador(*op).map_or("?", |(n, _)| n),
                    };
                    let r = self.operador_super(nome, rop, expr.span);
                    if *op == BinaryOp::NotEq {
                        let b = self.para_bool(r);
                        return self.emit(Instruction::LNot(b), Type::I1);
                    }
                    return r;
                }
                let lop = self.lower_expr(ast, *left);
                let rop = self.lower_expr(ast, *right);
                let l_ty = self.ctx.get_type(self.unit_id, *left);
                let r_ty = self.ctx.get_type(self.unit_id, *right);
                let desconhecido = |t: Option<dartforge_types::table::TypeId>| {
                    t.is_none_or(|t| t == self.ctx.core.dynamic_)
                };
                // `String` pelo tipo estático; com o tipo desconhecido (corpo
                // não inferido, parâmetro de closure sem tipo) um `Ref` à
                // esquerda de `+`/`*` é texto, como era antes.
                // Com o tipo desconhecido, o operador vai ao despacho
                // dinâmico (`despacho.rs`), que sabe concatenar texto.
                let _ = desconhecido;
                let texto = l_ty.is_some_and(|t| self.ctx.is_string(t))
                    || (matches!(op, BinaryOp::Add) && r_ty.is_some_and(|t| self.ctx.is_string(t)));
                // `int?`/`double?` promovido pelo fluxo (`if (x != null) x + 1`):
                // a inferência ainda não grava a promoção no tipo da leitura,
                // então o operando chega `Ref` e volta ao escalar aqui.
                // (`==`/`!=` fica de fora: comparar com null é legítimo.)
                let (lop, rop) = if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) || texto {
                    (lop, rop)
                } else {
                    let l = self.desnulificar_numerico(*left, lop);
                    let r = self.desnulificar_numerico(*right, rop);
                    (l, r)
                };
                self.operar(*op, lop, rop, texto, expr.span)
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
                        let novo = self.lower_atribuicao(
                            ast,
                            ast::AssignOp::Compound(bin),
                            *operand,
                            super::atribuicao::Rhs::Um,
                            expr.span,
                        );
                        let antigo = self.valor_antigo.take();
                        return match op {
                            UnaryOp::PrefixInc | UnaryOp::PrefixDec => novo,
                            _ => antigo.unwrap_or(novo),
                        };
                    }
                    _ => {}
                }
                if *op == UnaryOp::NullAssert {
                    let sub_op = self.lower_expr(ast, *operand);
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
                    let err_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_type_error_new".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.emit_throw_op(err_op);
                    self.set_block(cont_block);
                    return sub_op;
                }
                let sub_op = self.lower_expr(ast, *operand);
                let sub_op = if matches!(op, UnaryOp::Neg | UnaryOp::BitNot) {
                    self.desnulificar_numerico(*operand, sub_op)
                } else {
                    sub_op
                };
                match op {
                    UnaryOp::Neg | UnaryOp::BitNot if self.operand_type(&sub_op) == Type::Ref => {
                        self.unario_dinamico(*op, sub_op)
                    }
                    UnaryOp::Neg if self.operand_type(&sub_op) == Type::F64 => {
                        self.emit(Instruction::FNeg(sub_op), Type::F64)
                    }
                    UnaryOp::Neg => self.emit(Instruction::Neg(sub_op), Type::I64),
                    UnaryOp::Not => {
                        let b = self.para_bool(sub_op);
                        self.emit(Instruction::LNot(b), Type::I1)
                    }
                    UnaryOp::BitNot => self.emit(Instruction::Not(sub_op), Type::I64),
                    _ => self.nao_suportado("incremento/decremento de não-local", expr.span),
                }
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.para_bool(cond_op);
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
                let ty = self.repr_da_expressao(expr_id);
                self.set_block(then_block);
                let then_op = self.lower_expr(ast, *then);
                let then_op = match ty {
                    Some(t) => self.coagir(then_op, t),
                    None => then_op,
                };
                let then_end = self.current_block;
                let then_reaches = !self.is_terminated();
                if then_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }

                self.set_block(else_block);
                let else_op = self.lower_expr(ast, *else_);
                let ty = ty.unwrap_or_else(|| {
                    let (a, b) = (self.operand_type(&then_op), self.operand_type(&else_op));
                    if a == b { a } else { Type::Ref }
                });
                let else_op = self.coagir(else_op, ty);
                let else_end = self.current_block;
                let else_reaches = !self.is_terminated();
                if else_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }
                // Sem tipo estático, o `then` pode ter ficado noutra
                // representação; o phi exige as duas iguais.
                if then_reaches && self.operand_type(&then_op) != ty {
                    self.set_block(merge_block);
                    self.set_block(then_end);
                    return self.nao_suportado(
                        "condicional com ramos de representações diferentes",
                        expr.span,
                    );
                }

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

                if let Some(op) = self.propriedade_estatica_sdk_por_nome(ast, target, prop_name) {
                    return op;
                }

                let span = expr.span;
                let resolved = self.ctx.get_resolved(self.unit_id, expr_id).cloned();

                // `C.x`: membro estático (o alvo é um literal de classe).
                let alvo_e_classe = matches!(
                    self.ctx.get_resolved(self.unit_id, *target),
                    Some(Resolved::Element(
                        dartforge_elements::model::Element::Class(_)
                    ))
                );
                if alvo_e_classe {
                    if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                        self.ctx.get_resolved(self.unit_id, *target).cloned()
                        && prop_name == "values"
                        && super::enums::e_enum(self.ctx, c)
                    {
                        return self.valores_do_enum(c, span);
                    }
                    // `C.new`/`C.nome`: tear-off de construtor (P4).
                    if let Some(Resolved::Element(dartforge_elements::model::Element::Class(c))) =
                        self.ctx.get_resolved(self.unit_id, *target).cloned()
                        && self.ctx.biblioteca_compilada(self.ctx.program.classes[c.0 as usize].library)
                    {
                        let chave = if prop_name == "new" {
                            self.ctx.interner.lookup("")
                        } else {
                            Some(name.sym)
                        };
                        if let Some(f) = chave.and_then(|k| self.ctx.program.classes[c.0 as usize].constructors.get(&k).copied())
                            && !matches!(resolved, Some(Resolved::Member { .. }))
                        {
                            return self.tearoff_de_construtor(f.0 as usize, span);
                        }
                    }
                    return match resolved {
                        Some(Resolved::Member { member, .. }) => {
                            self.ler_membro_estatico(member, span)
                        }
                        _ => self.nao_suportado(&format!("membro estático `{prop_name}`"), span),
                    };
                }

                // `prefixo.x`: o elemento importado com prefixo.
                if let Some(el) = self.elemento_prefixado(ast, *target, name.sym) {
                    return self.ler_elemento(el, span);
                }
                // `super.x` (P4).
                if matches!(ast.expr(*target).kind, ExprKind::Super) {
                    return self.ler_super(name.sym, span);
                }

                let target_op = self.lower_alvo(ast, *target);
                if *null_aware {
                    self.desviar_se_nulo(&target_op);
                }

                // `index`/`name` de um valor de enum do programa (os
                // campos implícitos, que o elemento não compila).
                if let Some(c) = self.classe_do_usuario_de(*target)
                    && let Some(op) = self.membro_de_enum(c, prop_name, target_op.clone())
                {
                    return op;
                }

                // Membro de classe do usuário: pelo elemento resolvido (R7),
                // nunca pelo nome.
                if let Some((_, member)) = self.membro_do_usuario(expr_id, *target, name.sym, false)
                {
                    {
                        let vid = match member {
                            MemberRef::Variable(v) => Some(v),
                            MemberRef::Function(f) => {
                                self.ctx.program.functions[f.0 as usize].variable
                            }
                        };
                        if let Some(vid) = vid {
                            if crate::lower::e_global(self.ctx, vid) {
                                return self.ler_global(vid, span);
                            }
                            return self.ler_campo_com_late(target_op, vid, span);
                        }
                        let MemberRef::Function(f) = member else {
                            unreachable!()
                        };
                        if self.ctx.program.functions[f.0 as usize].kind
                            == dartforge_elements::model::FunctionKind::Getter
                        {
                            return self.chamar_membro(target_op, f.0 as usize, &[], span);
                        }
                        return self.tearoff_de_metodo(target_op, f.0 as usize, span);
                    }
                }

                // Getter de extensão (P4).
                if let Some(Resolved::ExtensionMember { member, .. }) = resolved.clone()
                    && crate::lower::funcao_do_usuario(self.ctx, member.0 as usize)
                {
                    return self.ler_extensao(target_op, member.0 as usize, span);
                }
                // `r.$1`/`r.nome`: campo de um record (posicional do
                // runtime, ou de uma forma com campo nomeado).
                let posicional = prop_name
                    .strip_prefix('$')
                    .and_then(|d| d.parse::<i64>().ok())
                    .is_some_and(|k| k >= 1);
                if posicional || !self.formas_com_campo(prop_name).is_empty() {
                    let nome = prop_name.to_string();
                    let alvo = *target;
                    let t2 = target_op.clone();
                    let mut resto = |s: &mut Self| s.propriedade_sem_membro(t2.clone(), alvo, &nome, expr_id, span);
                    if let Some(op) = self.ler_campo_de_registro(target_op.clone(), prop_name, &mut resto) {
                        return op;
                    }
                }
                self.propriedade_sem_membro(target_op, *target, prop_name, expr_id, span)
            }
            ExprKind::Index {
                target,
                index,
                null_aware,
            } => {
                let target_op = self.lower_alvo(ast, *target);
                if *null_aware {
                    self.desviar_se_nulo(&target_op);
                }
                let idx_op = self.lower_expr(ast, *index);
                if self.ctx.sdk_da_fonte {
                    // SDK da fonte: `[]` pela classe dinâmica.
                    let r = self.chamar_por_nome(target_op, super::sdk_fonte::Tipo::Chamar, "[]", &[(None, idx_op)]);
                    let repr = self.repr_da_expressao(expr_id).unwrap_or(Type::Ref);
                    return self.coagir(r, repr);
                }
                // `operator []` de classe do usuário, pela classe estática.
                if let Some(cid) = self.classe_do_usuario_de(*target) {
                    let Some(fid) = self.membro_na_classe(cid, "[]") else {
                        return self.nao_suportado("operador [] ausente", expr.span);
                    };
                    return self.chamar_membro(target_op, fid, &[(None, idx_op)], expr.span);
                }
                let target_ty = self.ctx.get_type(self.unit_id, *target);
                let is_map = target_ty.map_or(false, |t| self.ctx.is_map(t))
                    || self.operand_type(&idx_op) == Type::Ref;
                let is_string_or_match = target_ty.map_or(false, |t| self.ctx.is_string(t))
                    || matches!(ast.expr(*target).kind, ExprKind::String(_))
                    || (if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                        self.ctx.symbol_name(id.sym) == "m"
                    } else {
                        false
                    });
                if is_map {
                    // E2: a chave com a tag real (antes, 3 fixo). R5: o
                    // valor de `mapa[k]` é `V?`, sempre `Ref`.
                    let ktag = self.operand_tag(&idx_op);
                    let (kbits, _) = self.para_bits(idx_op);
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_map_get_ref".to_string(),
                            args: vec![
                                (target_op, Type::Ref),
                                (kbits, Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                            ],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if is_string_or_match {
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_list_get_bits".to_string(),
                            args: vec![(target_op, Type::Ref), (idx_op, Type::I64)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else {
                    let repr = self.repr_da_expressao(expr_id).unwrap_or(Type::Ref);
                    self.ler_elemento_lista(target_op, idx_op, repr)
                }
            }
            ExprKind::Record { positional, named, .. } if !named.is_empty() => {
                self.lower_registro_nomeado(ast, positional, named, expr.span)
            }
            ExprKind::Record { positional, .. } => {
                let mut ops = Vec::new();
                for p in positional.iter() {
                    let op = self.lower_expr(ast, *p);
                    let tag = self.operand_tag(&op);
                    ops.push((op, tag));
                }
                let vid = ValueId(self.next_value);
                self.next_value += 1;
                self.value_types.insert(vid, Type::Ref);
                let block_idx = self
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.id == self.current_block)
                    .unwrap();
                self.func.blocks[block_idx].instructions.push((
                    vid,
                    Instruction::AllocRecord { elements: ops },
                    Type::Ref,
                ));
                Operand::Val(vid)
            }
            ExprKind::Call { target, arguments } => {
                self.lower_chamada(ast, expr_id, expr, target, arguments)
            }
            ExprKind::List { elements, .. }
                if !elements.iter().all(|e| matches!(e, ast::CollectionElement::Expression(_))) =>
            {
                let l = self.lower_literal_de_colecao(ast, super::literais::Colecao::Lista, elements, expr.span);
                self.rti_do_literal(l, expr_id)
            }
            // SDK da fonte: mapas e conjuntos são o `_Map`/`_Set` da fonte.
            ExprKind::SetOrMap { elements, .. } if self.ctx.sdk_da_fonte => {
                let tipo = if self.literal_e_conjunto(expr_id, elements) {
                    super::literais::Colecao::Conjunto
                } else {
                    super::literais::Colecao::Mapa
                };
                self.lower_literal_de_colecao(ast, tipo, elements, expr.span)
            }
            ExprKind::SetOrMap { elements, .. } if self.literal_e_conjunto(expr_id, elements) => {
                let l = self.lower_literal_de_colecao(ast, super::literais::Colecao::Conjunto, elements, expr.span);
                self.rti_do_literal(l, expr_id)
            }
            ExprKind::SetOrMap { elements, .. }
                if !elements.iter().all(|e| {
                    matches!(
                        e,
                        ast::CollectionElement::MapEntry {
                            null_aware_key: false,
                            null_aware_value: false,
                            ..
                        }
                    )
                }) =>
            {
                let l = self.lower_literal_de_colecao(ast, super::literais::Colecao::Mapa, elements, expr.span);
                self.rti_do_literal(l, expr_id)
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
                    self.definir_rti_se_generico(l.clone(), t);
                }
                l
            }
            ExprKind::SetOrMap { elements, .. } => {
                let mut entries = Vec::new();
                for el in elements.iter() {
                    if let ast::CollectionElement::MapEntry { key, value, .. } = el {
                        let kop = self.lower_expr(ast, *key);
                        let ktag = self.operand_tag(&kop);
                        let vop = self.lower_expr(ast, *value);
                        let vtag = self.operand_tag(&vop);
                        entries.push(((kop, ktag), (vop, vtag)));
                    }
                }
                let m = self.emit(Instruction::AllocMap { entries }, Type::Ref);
                if let Some(t) = self.ctx.get_type(self.unit_id, expr_id) {
                    self.definir_rti_se_generico(m.clone(), t);
                }
                m
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Constructor(fid)) => {
                        self.tipo_da_criacao = self.ctx.get_type(self.unit_id, expr_id);
                        self.instanciar(ast, fid, &arguments.args, expr.span)
                    }
                    _ => self.nao_suportado("instanciação não resolvida", expr.span),
                }
            }
            ExprKind::Throw(inner) => self.emit_throw(ast, *inner),
            ExprKind::Rethrow => self.emit_rethrow(),
            ExprKind::Is { value, ty, negated } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                let is_m = self.testar_tipo(ast_ty, val_op);
                let is_m = self.para_bool(is_m);
                if *negated {
                    self.emit(Instruction::LNot(is_m), Type::I1)
                } else {
                    is_m
                }
            }
            ExprKind::As { value, ty } => {
                let val_op = self.lower_expr(ast, *value);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                self.checar_tipo_ou_lancar(ast_ty, val_op.clone());
                val_op
            }
            ExprKind::Assign { op, target, value } => self.lower_atribuicao(
                ast,
                *op,
                *target,
                super::atribuicao::Rhs::Expr(*value),
                expr.span,
            ),
            ExprKind::FunctionExpression(fid) => {
                let c = self.lower_closure(ast, *fid, expr.span);
                // RTI: a assinatura da closure (`f is R Function(P)`).
                self.definir_rti_de_closure(c.clone(), ast, *fid, Some(expr_id));
                c
            }
            ExprKind::Switch { value, cases } => self.lower_switch_expressao(ast, expr_id, *value, cases),
            ExprKind::Cascade {
                target,
                sections,
                null_aware,
            } => self.lower_cascata(ast, *target, sections, *null_aware),
            ExprKind::CascadeTarget => match self.current_cascade_target.clone() {
                Some(t) => t,
                None => self.nao_suportado("cascata", expr.span),
            },
            ExprKind::PatternAssign { pattern, value } => {
                let v = self.lower_expr(ast, *value);
                self.casar_irrefutavel(ast, *pattern, v.clone(), super::padroes::Ligacao::Atribuir, *value);
                v
            }
            ExprKind::Await(inner) => self.lower_await(ast, *inner, expr.span),
            ExprKind::This => match self.this_param.clone() {
                Some(t) => t,
                None => self.nao_suportado("`this` fora de membro de instância", expr.span),
            },
            outro => {
                let oque = match outro {
                    ExprKind::Super => "`super` como valor",
                    ExprKind::Symbol(_) => "literal de símbolo",

                    ExprKind::TypeArguments { .. } => "instanciação de tipo genérico",


                    ExprKind::Await(_) => "await",

                    _ => "expressão",
                };
                self.nao_suportado(oque, expr.span)
            }
        }
    }

    /// `alvo.nome` quando o nome não é membro estático do tipo do alvo:
    /// `index`/`name` de enum, o membro pela classe dinâmica (receptor sem
    /// tipo útil) ou o membro do SDK casado pelo nome.
    pub fn propriedade_sem_membro(
        &mut self,
        target_op: Operand,
        target: ExprId,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        // `index`/`name` de um valor de enum do programa.
        if let Some(c) = self.classe_do_usuario_de(target)
            && let Some(op) = self.membro_de_enum(c, prop_name, target_op.clone())
        {
            return op;
        }
        // Receptor sem tipo útil: o membro pela classe dinâmica.
        if self.receptor_dinamico(target) {
            let alvos = self.alvos_por_nome(prop_name);
            if !alvos.is_empty() {
                let nome = prop_name.to_string();
                let r2 = target_op.clone();
                return self.despachar(
                    target_op,
                    &alvos,
                    super::despacho::Uso::Ler,
                    &mut |_s: &mut Self| Vec::new(),
                    &mut |s: &mut Self| {
                        let n = s.erros.len();
                        let r = s.propriedade_sdk_por_nome(r2.clone(), &nome, expr_id, span);
                        if s.erros.len() > n {
                            s.erros.truncate(n);
                            return s.lancar_nsm(&nome);
                        }
                        r
                    },
                    span,
                );
            }
        }
        self.propriedade_sdk_por_nome(target_op, prop_name, expr_id, span)
    }

    /// Membro implícito (`x` = `this.x`) que é do SDK (a extensão sobre um
    /// tipo do SDK, o `name`/`index` do enum dentro dele): pelo caminho do
    /// SDK com `this` como receptor.
    pub fn ler_membro_implicito_do_sdk(
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
        if self.ctx.biblioteca_compilada(lib) {
            return None;
        }
        let this = self.this_param.clone()?;
        let nome = self.ctx.symbol_name(sym).to_string();
        if let Some(c) = self.enclosing_class
            && let Some(op) = self.membro_de_enum(c, &nome, this.clone())
        {
            return Some(op);
        }
        if nome.starts_with('$') {
            let n2 = nome.clone();
            let mut padrao = |s: &mut Self| s.lancar_nsm(&n2);
            if let Some(op) = self.ler_campo_de_registro(this.clone(), &nome, &mut padrao) {
                return Some(op);
            }
        }
        Some(self.propriedade_sdk_por_nome(this, &nome, expr_id, span))
    }
}

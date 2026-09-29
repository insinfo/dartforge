//! Comandos: blocos, declarações, `if`, laços, `break`/`continue` com
//! rótulo, `return`, `try`/`catch`/`finally` e `assert`.

use super::fn_builder::{FinallyScope, FnBuilder, ListaFixa};
use dartforge_intern::SymbolId;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind, StmtId, StmtKind};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `assert(cond, msg)`: lança `AssertionError` capturável (asserts
    /// ligados, como no oráculo `dart run --enable-asserts`).
    pub fn lower_assert(&mut self, ast: &ast::Ast, condition: ExprId, message: Option<ExprId>) {
        let c = self.lower_expr(ast, condition);
        let c = self.para_bool(c);
        let fail_b = self.new_block();
        let cont_b = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: c,
            then_block: cont_b,
            else_block: fail_b,
        });
        self.set_block(fail_b);
        let msg = match message {
            Some(m) => self.lower_expr(ast, m),
            None => Operand::Constant(Constant::Null),
        };
        let (bits, is_ref) = self.para_bits(msg);
        let err = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_assertion_error_new".to_string(),
                args: vec![
                    (bits, Type::I64),
                    (
                        Operand::Constant(Constant::Int(i64::from(is_ref))),
                        Type::I8,
                    ),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(err);
        self.set_block(cont_b);
    }

    pub fn lower_stmt(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
        // J05: as instruções do comando levam a posição dele; ao sair, volta
        // a do comando de fora (a atualização e a condição de um laço são
        // dele, não do último comando do corpo).
        let anterior = self.posicao;
        if !matches!(ast.stmt(stmt_id).kind, StmtKind::Block(_)) {
            self.marcar_posicao(ast, ast.stmt(stmt_id).span.start);
        }
        self.lower_stmt_no_lugar(ast, stmt_id);
        self.posicao = anterior;
    }

    fn lower_stmt_no_lugar(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
        let stmt = ast.stmt(stmt_id);
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.abrir_escopo();
                for &s in stmts.iter() {
                    self.lower_stmt(ast, s);
                }
                self.fechar_escopo();
            }
            StmtKind::Expression(expr_id) => {
                self.lower_expr_descartada(ast, *expr_id);
                // Dentro de um `try`: uma extern que não confere a exceção
                // pendente (as do SDK casado pelo nome, congeladas) não pode
                // deixar o `catch` para depois — confere no fim do comando.
                if !self.is_terminated() && (!self.exception_targets.is_empty() || !self.finally_scopes.is_empty()) {
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_exception_pending".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                }
            }
            StmtKind::Variables(var_list) => {
                let var_ty_opt = var_list.ty;
                for var in &var_list.variables {
                    let sym = var.name.sym;
                    // R6: o local guarda a representação do tipo declarado
                    // (ou inferido do inicializador), não a do valor.
                    let mut ty = self.repr_do_local(var.name.span.start);
                    if var_list.late && ty.e_vetor() {
                        // O `late` guarda o estado ao lado do valor: a caixa.
                        ty = Type::Ref;
                    }
                    let late_local = var_list.late
                        && (var.initializer.is_none()
                            || !self.celulas.contains(&(var.name.span.start as usize)));
                    if var_list.const_
                        && let Some(init_id) = var.initializer
                        && let Some(k) = self.chave_constante(ast, init_id, true)
                    {
                        self.chaves_de_const_locais.insert(sym, (k, init_id));
                    }
                    let init_op = if late_local {
                        Self::valor_zero(ty)
                    } else if ty.e_vetor() {
                        // SIMD sem caixa: o inicializador como vetor, ou o zero
                        // (a leitura antes da atribuição não compila).
                        match var.initializer {
                            Some(init_id) => self.lower_simd(ast, init_id, ty),
                            None => self.emit(Instruction::Simd { op: OpSimd::Zero, args: Vec::new() }, ty),
                        }
                    } else if let Some(init_id) = var.initializer {
                        let op = if var_list.const_ {
                            self.lower_em_contexto_const(ast, init_id)
                        } else {
                            self.lower_expr(ast, init_id)
                        };
                        // Checagem na declaração só quando o estático não a
                        // garante: inicializador `dynamic` num tipo declarado.
                        // Antes ela rodava sempre, e `Foo? x = null` chamava
                        // `dartforge_value_class(0)` (H6).
                        let init_dinamico = self.ctx.get_type(self.unit_id, init_id)
                            == Some(self.ctx.core.dynamic_);
                        if let Some(tid) = var_ty_opt.filter(|_| init_dinamico) {
                            self.checar_tipo_ou_lancar(ast.ty(tid), op.clone(), super::rti::ContextoDoCast::Implicito);
                        }
                        op
                    } else {
                        Self::valor_zero(ty)
                    };
                    self.declarar_variavel(sym, var.name.span.start as usize, ty, init_op);
                    if late_local {
                        self.configurar_local_late(sym, var_list.final_, var.initializer);
                    }
                }
            }
            StmtKind::If {
                condition,
                case_pattern: Some(p),
                guard,
                then,
                else_,
            } => {
                self.lower_if_case(ast, *condition, *p, *guard, *then, *else_);
            }
            StmtKind::If {
                condition,
                then,
                else_,
                ..
            } => {
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.para_bool(cond_op);
                let then_b = self.new_block();
                let else_b = self.new_block();
                let merge_b = self.new_block();

                let false_target = if else_.is_some() { else_b } else { merge_b };
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: then_b,
                    else_block: false_target,
                });

                self.set_block(then_b);
                self.lower_stmt(ast, *then);
                self.terminate(Terminator::Branch(merge_b));

                if let Some(e) = else_ {
                    self.set_block(else_b);
                    self.lower_stmt(ast, *e);
                    self.terminate(Terminator::Branch(merge_b));
                }

                self.set_block(merge_b);
            }
            StmtKind::While { condition, body } => {
                let loop_header = self.new_block();
                let loop_body = self.new_block();
                let exit_block = self.new_block();

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_header);
                }

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.para_bool(cond_op);
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_header);

                self.set_block(loop_body);
                self.emitir_ponto_seguro();
                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_header));

                self.break_targets.pop();
                self.continue_targets.pop();

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(exit_block);
            }
            StmtKind::DoWhile { body, condition } => {
                let loop_body = self.new_block();
                let loop_cond = self.new_block();
                let exit_block = self.new_block();

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_cond);
                }

                self.terminate(Terminator::Branch(loop_body));
                self.set_block(loop_body);
                self.emitir_ponto_seguro();

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_cond);

                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_cond));

                self.break_targets.pop();
                self.continue_targets.pop();

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(loop_cond);
                let cond_op = self.lower_expr(ast, *condition);
                let cond_op = self.para_bool(cond_op);
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.set_block(exit_block);
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                // Um laço contado termina sozinho: sem ponto seguro nas
                // voltas (o que ficar em laço infinito está num chamado, que
                // tem os dele), e o LLVM continua livre para vetorizá-lo.
                let contado = self.laco_contado(ast, init.as_ref(), *condition, updates, *body);
                // A variável do `for` vive num `alloca` (R6): antes ela era só
                // um valor SSA no mapa por nome, e a condição, baixada antes
                // do `i++`, lia para sempre o valor inicial.
                self.abrir_escopo();
                if let Some(init) = init {
                    match init {
                        ast::ForInit::Variables(var_list) => {
                            for var in &var_list.variables {
                                let sym = var.name.sym;
                                let ty = self.repr_do_local(var.name.span.start);
                                let late_local = var_list.late
                                    && (var.initializer.is_none()
                                        || !self.celulas.contains(&(var.name.span.start as usize)));
                                let init_op = if late_local {
                                    Self::valor_zero(ty)
                                } else if let Some(init_id) = var.initializer {
                                    self.lower_expr(ast, init_id)
                                } else {
                                    Self::valor_zero(ty)
                                };
                                self.declarar_variavel(sym, var.name.span.start as usize, ty, init_op);
                                if late_local {
                                    self.configurar_local_late(sym, var_list.final_, var.initializer);
                                }
                            }
                        }
                        ast::ForInit::Expression(e) => {
                            self.lower_expr_descartada(ast, *e);
                        }
                    }
                }

                let rotulos = std::mem::take(&mut self.pending_labels);
                match contado {
                    // N13: a lista do limite é do runtime (o cabeçalho tem
                    // comprimento) — as voltas usam o comprimento e os dados
                    // lidos uma vez, e as conferências de limite do `v[i]`
                    // ficam provadas pela condição; senão (vazia, ou uma
                    // classe do programa que implementa `List`), as voltas de
                    // sempre, que leem o `length` a cada volta. N14: as
                    // voltas rápidas exigem também a forma dos elementos que
                    // o tipo estático pede (compacta para `List<int>`,
                    // `List<double>`, `List<bool>`; geral para os outros), e
                    // a leem na emissão; a lista de outra forma vai às voltas
                    // de sempre, que conferem a forma a cada acesso.
                    Some(Contado::Lista { alvo, local }) => {
                        let forma = match self.indexavel(self.ctx.get_type(self.unit_id, alvo)) {
                            Some(super::tipados::Indexavel::Nucleo { gravacao: Some(t) }) => super::tipados::codigo_da_forma(t),
                            _ => 0,
                        };
                        let lista = self.lower_expr(ast, alvo);
                        let lista = self.coagir(lista, Type::Ref);
                        let cab = self.cabecalho_da_lista(&lista);
                        let n = self.campo_do_cabecalho(&cab, 1);
                        let dados = self.campo_do_cabecalho(&cab, 0);
                        let do_runtime = self.emit(Instruction::ICmp(ICmpOp::Ne, n.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                        let conferir_forma = self.new_block();
                        let rapido = self.new_block();
                        let geral = self.new_block();
                        let juncao = self.new_block();
                        self.terminate(Terminator::CondBranch { cond: do_runtime, then_block: conferir_forma, else_block: geral });
                        self.set_block(conferir_forma);
                        let f = self.campo_do_cabecalho(&cab, 3);
                        let da_forma = self.emit(Instruction::ICmp(ICmpOp::Eq, f, Operand::Constant(Constant::Int(forma))), Type::I1);
                        self.terminate(Terminator::CondBranch { cond: da_forma, then_block: rapido, else_block: geral });
                        self.set_block(rapido);
                        let comprimento_gravavel = (forma != 0).then(|| {
                            self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_lista_len_gravavel".to_string(),
                                    args: vec![(lista.clone(), Type::Ref), (Operand::Constant(Constant::Int(forma)), Type::I64)],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            )
                        });
                        let chave = match self.buscar_local(local).map(|l| l.modo) {
                            Some(super::locais::Modo::Memoria(p)) => p,
                            _ => unreachable!("o laço de lista exige um local em memória"),
                        };
                        self.listas_fixas.push(ListaFixa { chave, comprimento: n, dados, forma, comprimento_gravavel });
                        self.voltas_do_for(ast, init.as_ref(), *condition, updates, *body, true, &rotulos);
                        self.listas_fixas.pop();
                        self.terminate(Terminator::Branch(juncao));
                        self.set_block(geral);
                        self.voltas_do_for(ast, init.as_ref(), *condition, updates, *body, false, &rotulos);
                        self.terminate(Terminator::Branch(juncao));
                        self.set_block(juncao);
                    }
                    _ => {
                        self.voltas_do_for(ast, init.as_ref(), *condition, updates, *body, contado.is_some(), &rotulos);
                    }
                }
                self.fechar_escopo();
            }
            StmtKind::Return(expr_opt) => {
                let op = expr_opt.map(|e| self.lower_expr(ast, e));
                self.route_return(op);
            }
            StmtKind::Break(lbl) => {
                let sym = lbl.as_ref().map(|n| n.sym);
                self.route_break_to(sym);
            }
            StmtKind::Continue(lbl) => {
                let sym = lbl.as_ref().map(|n| n.sym);
                self.route_continue_to(sym);
            }
            StmtKind::Assert { condition, message } => {
                self.lower_assert(ast, *condition, *message);
            }
            StmtKind::ForIn {
                await_: true,
                target,
                iterable,
                body,
            } => {
                self.lower_await_for(ast, target, *iterable, *body, stmt.span);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } if self.ctx.sdk_da_fonte => {
                // SDK da fonte: o protocolo do `Iterator` (§17.7.3).
                self.lower_for_in_fonte(ast, target, *iterable, *body, stmt.span);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                let iterable_op = self.lower_expr(ast, *iterable);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_iteration_begin".to_string(),
                        args: vec![(iterable_op.clone(), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
                let len_op = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_generic_len".to_string(),
                        args: vec![(iterable_op.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );

                let pre_block = self.current_block;
                let loop_header = self.new_block();
                let loop_body = self.new_block();
                let loop_update = self.new_block();
                let exit_block = self.new_block();

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_update);
                }

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                let phi_vid = ValueId(self.next_value);
                self.next_value += 1;
                self.value_types.insert(phi_vid, Type::I64);
                let phi_op = Operand::Val(phi_vid);

                let header_idx = self
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.id == loop_header)
                    .unwrap();
                let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
                self.func.blocks[header_idx].instructions.push((
                    phi_vid,
                    Instruction::Phi {
                        incoming: Vec::new(),
                        ty: Type::I64,
                    },
                    Type::I64,
                ));

                let cmp = self.emit(
                    Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), len_op),
                    Type::I1,
                );
                self.terminate(Terminator::CondBranch {
                    cond: cmp,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_update);

                self.set_block(loop_body);
                self.emitir_ponto_seguro();
                self.abrir_escopo();
                match target {
                    ast::ForInTarget::Declared { name, .. } => {
                        // O elemento é lido na representação da variável (R5):
                        // `for (Cell c in cells)` quer o handle, `for (int n
                        // in ns)` quer os bits.
                        let ty = self.repr_do_local(name.span.start);
                        let item_val =
                            self.ler_elemento_iteravel(iterable_op.clone(), phi_op.clone(), ty);
                        self.declarar_variavel(name.sym, name.span.start as usize, ty, item_val);
                    }
                    ast::ForInTarget::Expression(e) => {
                        if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                            let ty = self.buscar_local(id.sym).map_or(Type::Ref, |l| l.ty);
                            let item_val =
                                self.ler_elemento_iteravel(iterable_op.clone(), phi_op.clone(), ty);
                            self.gravar_local(id.sym, item_val);
                        } else {
                            self.nao_suportado("alvo de for-in", stmt.span);
                        }
                    }
                    ast::ForInTarget::Pattern { pattern, .. } => {
                        let item_val =
                            self.ler_elemento_iteravel(iterable_op.clone(), phi_op.clone(), Type::Ref);
                        self.casar_irrefutavel(ast, *pattern, item_val, super::padroes::Ligacao::Declarar, *iterable);
                    }
                }

                self.lower_stmt(ast, *body);
                self.fechar_escopo();
                self.terminate(Terminator::Branch(loop_update));

                self.break_targets.pop();
                self.continue_targets.pop();

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(loop_update);
                let next_idx = self.emit(
                    Instruction::Add(phi_op, Operand::Constant(Constant::Int(1))),
                    Type::I64,
                );
                self.terminate(Terminator::Branch(loop_header));

                let header_idx = self
                    .func
                    .blocks
                    .iter()
                    .position(|b| b.id == loop_header)
                    .unwrap();
                self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                    incoming: vec![
                        (pre_block, Operand::Constant(Constant::Int(0))),
                        (loop_update, next_idx),
                    ],
                    ty: Type::I64,
                };

                self.set_block(exit_block);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_iteration_end".to_string(),
                        args: vec![(iterable_op, Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
            StmtKind::Function(fid) => {
                if self.funcao_local_direta(*fid) {
                    self.declarar_funcao_direta(ast, *fid, stmt.span);
                } else {
                    self.declarar_funcao_local(ast, *fid, stmt.span);
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.lower_try_stmt(ast, *body, catches, *finally_);
            }
            StmtKind::Labeled { labels, body } => {
                let laco = matches!(
                    ast.stmt(*body).kind,
                    StmtKind::While { .. }
                        | StmtKind::DoWhile { .. }
                        | StmtKind::For { .. }
                        | StmtKind::ForIn { .. }
                        | StmtKind::Switch { .. }
                        | StmtKind::Labeled { .. }
                );
                if laco {
                    for l in labels.iter() {
                        self.pending_labels.push(l.sym);
                    }
                    self.lower_stmt(ast, *body);
                } else {
                    // `rótulo: { … break rótulo; … }`: o `break` sai do
                    // comando rotulado (§18.13 "Labels").
                    let saida = self.new_block();
                    for l in labels.iter() {
                        self.labeled_break_targets.insert(l.sym, saida);
                    }
                    self.lower_stmt(ast, *body);
                    self.terminate(Terminator::Branch(saida));
                    for l in labels.iter() {
                        self.labeled_break_targets.remove(&l.sym);
                    }
                    self.set_block(saida);
                }
            }
            StmtKind::Empty => {}
            StmtKind::PatternVariables { pattern, value, .. } => {
                let v = self.lower_expr(ast, *value);
                self.casar_irrefutavel(ast, *pattern, v, super::padroes::Ligacao::Declarar, *value);
            }
            StmtKind::Switch { value, cases } => {
                self.lower_switch_comando(ast, *value, cases);
            }
            StmtKind::Yield { star, value } => {
                self.lower_yield(ast, *star, *value, stmt.span);
            }
        }
    }

    fn lower_try_stmt(
        &mut self,
        ast: &ast::Ast,
        body: StmtId,
        catches: &[ast::CatchClause],
        finally_: Option<StmtId>,
    ) {
        let mut fin = finally_.map(|f| move |b: &mut Self| b.lower_stmt(ast, f));
        self.lower_try_com(
            ast,
            &mut |b: &mut Self| b.lower_stmt(ast, body),
            catches,
            fin.as_mut().map(|f| f as &mut dyn FnMut(&mut Self)),
        );
    }

    /// O `try` com o corpo e o `finally` dados por quem chama: o do comando
    /// (`lower_try_stmt`) e os sintetizados pelo lowering (o `finally` que
    /// cancela a inscrição de um `await for`).
    pub(super) fn lower_try_com(
        &mut self,
        ast: &ast::Ast,
        corpo: &mut dyn FnMut(&mut Self),
        catches: &[ast::CatchClause],
        mut finally_: Option<&mut dyn FnMut(&mut Self)>,
    ) {
        let try_body_block = self.new_block();
        let merge_block = self.new_block();

        let has_catches = !catches.is_empty();
        let catch_dispatch_block = if has_catches {
            Some(self.new_block())
        } else {
            None
        };

        let fin_info = if finally_.is_some() {
            let entry = self.new_block();
            let resume = self.new_block();
            let reason_phi = ValueId(self.next_value);
            self.next_value += 1;
            self.value_types.insert(reason_phi, Type::I64);

            let ret_val_phi = ValueId(self.next_value);
            self.next_value += 1;
            let ret_ty = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
            self.value_types.insert(ret_val_phi, ret_ty);

            Some((entry, resume, reason_phi, ret_val_phi))
        } else {
            None
        };

        if let Some((entry, resume, reason_phi, ret_val_phi)) = fin_info {
            self.finally_scopes.push(FinallyScope {
                entry_block: entry,
                resume_block: resume,
                reason_phi,
                ret_val_phi,
                incoming: Vec::new(),
                prof_break: self.break_targets.len(),
                prof_continue: self.continue_targets.len(),
                rotulos_break: self.labeled_break_targets.keys().copied().collect(),
                rotulos_continue: self.labeled_continue_targets.keys().copied().collect(),
                saltos: Vec::new(),
            });
        }

        // Pouso das exceções que vão para o `finally`: um bloco só, que
        // registra a entrada "exceção" (2) dos phis do `finally`. Os desvios
        // de exceção (`emit_call_with_check` com um alvo de exceção) não
        // registram entrada de phi; ir direto ao `finally` deixava o phi sem
        // a entrada de cada um deles (o Clang recusa o módulo). Também é o
        // alvo das exceções dentro dos `catch`: elas passam pelo `finally`
        // antes de subir.
        let pouso = fin_info.map(|(entry, _, _, _)| {
            let p = self.new_block();
            let prev = self.current_block;
            self.set_block(p);
            let default_ret = self.default_return_operand();
            self.finally_scopes
                .last_mut()
                .expect("escopo do finally")
                .incoming
                .push((p, 2, default_ret));
            self.terminate(Terminator::Branch(entry));
            self.set_block(prev);
            p
        });

        let try_exc_target = if let Some(cb) = catch_dispatch_block {
            cb
        } else if let Some(p) = pouso {
            p
        } else if let Some(&parent_target) = self.exception_targets.last() {
            parent_target
        } else {
            let u = self.new_block();
            let prev = self.current_block;
            self.set_block(u);
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
            self.set_block(prev);
            u
        };

        self.exception_targets.push(try_exc_target);
        self.terminate(Terminator::Branch(try_body_block));

        self.set_block(try_body_block);
        corpo(self);

        if !self.is_terminated() {
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope
                    .incoming
                    .push((self.current_block, 0, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else {
                self.terminate(Terminator::Branch(merge_block));
            }
        }

        self.exception_targets.pop();

        if let Some(dispatch_block) = catch_dispatch_block {
            self.set_block(dispatch_block);
            // A exceção como referência (R5): `throw 42` chega encaixotado, e
            // os testes `on T` e a variável do `catch` são sobre um `Ref`.
            let ex_bits = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_peek_ref".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );

            // O desenrolar do isolado (`Isolate.exit`) não é capturável:
            // vai direto ao caminho de "nenhuma cláusula casou".
            let capturavel = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_capturavel".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let capturavel = self.emit(
                Instruction::ICmp(ICmpOp::Ne, capturavel, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let primeiro_teste = self.new_block();
            let sem_captura = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: capturavel,
                then_block: primeiro_teste,
                else_block: sem_captura,
            });
            let mut current_test_block = primeiro_teste;

            for clause in catches.iter() {
                self.set_block(current_test_block);
                let body_b = self.new_block();
                let next_test_b = self.new_block();

                if let Some(on_tid) = clause.on_type {
                    let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(on_tid);
                    let is_match = self.testar_tipo(ast_ty, ex_bits.clone());
                    self.terminate(Terminator::CondBranch {
                        cond: is_match,
                        then_block: body_b,
                        else_block: next_test_b,
                    });
                } else {
                    self.terminate(Terminator::Branch(body_b));
                }

                self.set_block(body_b);
                // O rastro explícito pertence à exceção pendente. Leia-o
                // antes de `clear`, que libera o slot e a raiz do runtime.
                let st_val = clause.stack_trace.as_ref().map(|_| self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_stack_trace_get".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_clear".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );

                self.abrir_escopo();
                if let Some(ex_name) = &clause.exception {
                    self.declarar_variavel(ex_name.sym, ex_name.span.start as usize, Type::Ref, ex_bits.clone());
                }
                if let (Some(st_name), Some(st_val)) = (&clause.stack_trace, st_val) {
                    self.declarar_variavel(st_name.sym, st_name.span.start as usize, Type::Ref, st_val);
                }

                self.active_catch_stack.push((ex_bits.clone(), 3));
                if let Some(p) = pouso {
                    self.exception_targets.push(p);
                }
                self.lower_stmt(ast, clause.body);
                if pouso.is_some() {
                    self.exception_targets.pop();
                }
                self.fechar_escopo();
                self.active_catch_stack.pop();

                if !self.is_terminated() {
                    if let Some((entry, _, _, _)) = fin_info {
                        let default_ret = self.default_return_operand();
                        let last_scope = self.finally_scopes.last_mut().unwrap();
                        last_scope
                            .incoming
                            .push((self.current_block, 0, default_ret));
                        self.terminate(Terminator::Branch(entry));
                    } else {
                        self.terminate(Terminator::Branch(merge_block));
                    }
                }

                current_test_block = next_test_b;
            }

            self.set_block(sem_captura);
            self.terminate(Terminator::Branch(current_test_block));
            self.set_block(current_test_block);
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope
                    .incoming
                    .push((self.current_block, 2, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else if let Some(&parent_target) = self.exception_targets.last() {
                self.terminate(Terminator::Branch(parent_target));
            } else {
                self.terminate(Terminator::Return(self.default_return_operand_opt()));
            }
        }

        if let Some(fin_corpo) = finally_.as_mut() {
            let (entry, _resume, reason_phi, ret_val_phi) = fin_info.unwrap();
            let scope = self.finally_scopes.pop().unwrap();

            self.set_block(entry);

            let header_idx = self.func.blocks.iter().position(|b| b.id == entry).unwrap();
            let mut reason_incoming = Vec::new();
            let mut ret_val_incoming = Vec::new();
            for (b, reason, ret_op) in &scope.incoming {
                reason_incoming.push((*b, Operand::Constant(Constant::Int(*reason))));
                ret_val_incoming.push((*b, ret_op.clone()));
            }

            self.func.blocks[header_idx].instructions.push((
                reason_phi,
                Instruction::Phi {
                    incoming: reason_incoming,
                    ty: Type::I64,
                },
                Type::I64,
            ));
            let ret_ty = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
            self.func.blocks[header_idx].instructions.push((
                ret_val_phi,
                Instruction::Phi {
                    incoming: ret_val_incoming,
                    ty: ret_ty,
                },
                ret_ty,
            ));

            // A exceção que entrou no `finally` (razão 2) fica guardada e sai
            // da pendência enquanto o corpo do `finally` roda: senão a
            // primeira chamada dele vê a exceção pendente e desvia, e o
            // `finally` não roda (é a semântica da VM: o corpo do `finally`
            // executa normalmente e a exceção volta ao fim, a menos que ele
            // saia por `return`/`break`/`continue`/`throw`, que a descartam).
            let guardada = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_peek_ref".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_clear".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::Void,
                },
                Type::Void,
            );

            // O desenrolar do isolado (`kill`, `Isolate.exit`: o `UnwindError`
            // da VM) não roda o corpo do `finally`, como a VM: a pendência
            // segue direto para o tratador de fora. Antes isso dependia de a
            // primeira chamada conferida do corpo ver a pendência — e o efeito
            // de uma chamada que acontece antes da conferência escapava.
            let capturavel = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_capturavel".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let desenrolando = self.emit(
                Instruction::ICmp(ICmpOp::Eq, capturavel, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_corpo = self.new_block();
            let b_desenrolar = self.new_block();
            self.terminate(Terminator::CondBranch { cond: desenrolando, then_block: b_desenrolar, else_block: b_corpo });
            self.set_block(b_desenrolar);
            if let Some(&parent_target) = self.exception_targets.last() {
                self.terminate(Terminator::Branch(parent_target));
            } else if !self.finally_scopes.is_empty() {
                let default_ret = self.default_return_operand();
                let parent_fin = self.finally_scopes.last_mut().unwrap();
                parent_fin.incoming.push((b_desenrolar, 2, default_ret));
                let p_entry = parent_fin.entry_block;
                self.terminate(Terminator::Branch(p_entry));
            } else {
                self.terminate(Terminator::Return(self.default_return_operand_opt()));
            }
            self.set_block(b_corpo);

            fin_corpo(self);

            if !self.is_terminated() {
                let b_norm = merge_block;
                let b_ret = self.new_block();
                let b_exc = self.new_block();
                let b_saltos: Vec<BlockId> = scope.saltos.iter().map(|_| self.new_block()).collect();
                let mut casos = vec![(0, b_norm), (1, b_ret), (2, b_exc)];
                for (k, b) in b_saltos.iter().enumerate() {
                    casos.push((5 + k as i64, *b));
                }
                self.terminate(Terminator::Switch {
                    val: Operand::Val(reason_phi),
                    default: b_norm,
                    cases: casos,
                });

                self.set_block(b_ret);
                let ret_op = if self.func.return_ty == Type::Void {
                    None
                } else {
                    Some(Operand::Val(ret_val_phi))
                };
                self.route_return(ret_op);

                self.set_block(b_exc);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_throw".to_string(),
                        args: vec![
                            (guardada, Type::I64),
                            (Operand::Constant(Constant::Int(3)), Type::I8),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
                // A exceção sobe para o tratador mais interno: o alvo de
                // exceção corrente (um `catch` de fora, ou o pouso do
                // `finally` de fora, que registra a entrada do phi) vem antes
                // de um `finally` de fora — este só é o mais interno quando
                // não há alvo.
                if let Some(&parent_target) = self.exception_targets.last() {
                    self.terminate(Terminator::Branch(parent_target));
                } else if !self.finally_scopes.is_empty() {
                    let default_ret = self.default_return_operand();
                    let parent_fin = self.finally_scopes.last_mut().unwrap();
                    parent_fin.incoming.push((b_exc, 2, default_ret));
                    let p_entry = parent_fin.entry_block;
                    self.terminate(Terminator::Branch(p_entry));
                } else {
                    self.terminate(Terminator::Return(self.default_return_operand_opt()));
                }

                // Os saltos que atravessaram este `finally` continuam.
                for (k, b) in b_saltos.into_iter().enumerate() {
                    self.set_block(b);
                    let (e_continue, rotulo) = scope.saltos[k];
                    self.saltar(e_continue, rotulo);
                }
            }
        }

        self.set_block(merge_block);
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um laço sobre a lista local `lista` (N13) não chama nada:
    /// só locais, literais, aritmética e comparação de `int`/`double`/`bool`,
    /// `if`/blocos/`break`/`continue`, e índice ou `length` da própria lista
    /// ou de listas de `dart:typed_data` (classes finais do SDK). Nenhum
    /// caminho dele executa código do programa, então o comprimento e os
    /// dados da lista não mudam nas voltas (o heap não move objetos).
    fn corpo_sem_chamadas(&self, ast: &ast::Ast, corpo: StmtId, lista: SymbolId, contador: SymbolId) -> bool {
        let mut declarados: std::collections::HashSet<SymbolId> = [contador].into_iter().collect();
        self.comando_sem_chamadas(ast, corpo, lista, &mut declarados)
    }

    fn comando_sem_chamadas(
        &self,
        ast: &ast::Ast,
        s: StmtId,
        lista: SymbolId,
        declarados: &mut std::collections::HashSet<SymbolId>,
    ) -> bool {
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => ss.iter().all(|&x| self.comando_sem_chamadas(ast, x, lista, declarados)),
            StmtKind::Expression(e) => self.expressao_sem_chamadas(ast, *e, lista, declarados),
            StmtKind::Variables(l) => {
                if l.late {
                    return false;
                }
                l.variables.iter().all(|v| {
                    let ok = v.initializer.is_none_or(|e| self.expressao_sem_chamadas(ast, e, lista, declarados))
                        && self.escalar(self.ctx.tipo_local(self.unit_id, v.name.span.start as usize));
                    declarados.insert(v.name.sym);
                    ok
                })
            }
            StmtKind::If { condition, then, else_, .. } => {
                self.expressao_sem_chamadas(ast, *condition, lista, declarados)
                    && self.comando_sem_chamadas(ast, *then, lista, declarados)
                    && else_.is_none_or(|x| self.comando_sem_chamadas(ast, x, lista, declarados))
            }
            StmtKind::Break(None) | StmtKind::Continue(None) => true,
            _ => false,
        }
    }

    /// `int`, `double` ou `bool` (não anuláveis): os operadores deles são do
    /// compilador, não chamados.
    fn escalar(&self, t: Option<dartforge_types::TypeId>) -> bool {
        let c = &self.ctx.core;
        t.is_some_and(|t| t == c.int || t == c.double || t == c.bool_)
    }

    fn expressao_sem_chamadas(
        &self,
        ast: &ast::Ast,
        e: ExprId,
        lista: SymbolId,
        declarados: &std::collections::HashSet<SymbolId>,
    ) -> bool {
        use super::tipados::Indexavel;
        let tipo = |x: ExprId| self.ctx.get_type(self.unit_id, x);
        let local = |n: &ast::Name| declarados.contains(&n.sym) || self.buscar_local(n.sym).is_some();
        // A própria lista, ou uma de `dart:typed_data`.
        let lista_ok = |x: ExprId| match &ast.expr(x).kind {
            ExprKind::Identifier(n) if n.sym == lista => true,
            ExprKind::Identifier(n) if local(n) => {
                matches!(self.indexavel(tipo(x)), Some(Indexavel::Tipada(_) | Indexavel::Simd { .. }))
            }
            _ => false,
        };
        let sub = |x: ExprId| self.expressao_sem_chamadas(ast, x, lista, declarados);
        match &ast.expr(e).kind {
            ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) => true,
            ExprKind::Identifier(n) => local(n) && (self.escalar(tipo(e)) || n.sym == lista),
            ExprKind::Parenthesized(x) => sub(*x),
            ExprKind::Binary { op, left, right } => {
                !matches!(op, ast::BinaryOp::IfNull)
                    && self.escalar(tipo(*left))
                    && self.escalar(tipo(*right))
                    && sub(*left)
                    && sub(*right)
            }
            ExprKind::Unary { operand, .. } => self.escalar(tipo(*operand)) && sub(*operand),
            ExprKind::Conditional { condition, then, else_ } => sub(*condition) && sub(*then) && sub(*else_),
            ExprKind::Index { target, index, null_aware: false } => {
                lista_ok(*target) && self.escalar(tipo(*index)) && sub(*index)
            }
            ExprKind::Property { target, name, null_aware: false } => {
                self.ctx.symbol_name(name.sym) == "length" && lista_ok(*target)
            }
            ExprKind::Assign { target, value, .. } => {
                let alvo_ok = match &ast.expr(*target).kind {
                    // O tipo do alvo de uma atribuição fica na própria
                    // atribuição (o do resultado: o do local, em `s += x`).
                    ExprKind::Identifier(n) => local(n) && self.escalar(tipo(e)),
                    ExprKind::Index { target: t, index, null_aware: false } => {
                        lista_ok(*t) && self.escalar(tipo(*index)) && sub(*index)
                    }
                    _ => false,
                };
                alvo_ok && self.escalar(tipo(*value)) && sub(*value)
            }
            _ => false,
        }
    }

    /// As voltas de um `for` (a condição, o corpo e as atualizações), depois
    /// da inicialização; termina no bloco de saída. `contado`: o laço termina
    /// sozinho e as voltas não levam ponto seguro (J01).
    #[allow(clippy::too_many_arguments)]
    fn voltas_do_for(
        &mut self,
        ast: &ast::Ast,
        init: Option<&ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
        contado: bool,
        rotulos: &[SymbolId],
    ) {
        let loop_header = self.new_block();
        let loop_body = self.new_block();
        let loop_update = self.new_block();
        let exit_block = self.new_block();

        for &lbl in rotulos {
            self.labeled_break_targets.insert(lbl, exit_block);
            self.labeled_continue_targets.insert(lbl, loop_update);
        }

        self.terminate(Terminator::Branch(loop_header));
        self.set_block(loop_header);

        if let Some(cond_id) = &condition {
            let cond_op = self.lower_expr(ast, *cond_id);
            let cond_op = self.para_bool(cond_op);
            self.terminate(Terminator::CondBranch {
                cond: cond_op,
                then_block: loop_body,
                else_block: exit_block,
            });
        } else {
            self.terminate(Terminator::Branch(loop_body));
        }

        self.break_targets.push(exit_block);
        self.continue_targets.push(loop_update);

        self.set_block(loop_body);
        if !contado {
            self.emitir_ponto_seguro();
        }
        self.lower_stmt(ast, body);
        self.terminate(Terminator::Branch(loop_update));

        self.set_block(loop_update);
        // Uma variável nova por volta (a especificação do `for`):
        // só muda algo para a que mora numa célula (P1).
        if let Some(ast::ForInit::Variables(var_list)) = init {
            for var in var_list.variables.iter() {
                self.renovar_celula(var.name.sym);
            }
        }
        for &u in updates.iter() {
            self.lower_expr_descartada(ast, u);
        }
        self.terminate(Terminator::Branch(loop_header));

        self.break_targets.pop();
        self.continue_targets.pop();

        for &lbl in rotulos {
            self.labeled_break_targets.remove(&lbl);
            self.labeled_continue_targets.remove(&lbl);
        }

        self.set_block(exit_block);
    }

    /// `for (int i = a; i < L; i++)` e as variações (`<=`, `>`, `>=`;
    /// `i += c`, `i -= c` com `c` literal positivo no sentido da condição),
    /// com `i` sem outra gravação no corpo e na condição, e o limite `L`
    /// invariante: literal, local sem célula que o laço não grava, ou
    /// `v.length` de um local `v` assim cuja lista tem comprimento fixo (as
    /// listas de `dart:typed_data`). `i` também não pode transbordar ao
    /// passar do último valor: com `L` qualquer, só `<`/`>` com passo 1.
    /// Um laço assim termina: o ponto seguro de volta (J01) é dispensado,
    /// como o HotSpot dispensa o dos laços contados.
    fn laco_contado(
        &self,
        ast: &ast::Ast,
        init: Option<&ast::ForInit>,
        condition: Option<ExprId>,
        updates: &[ExprId],
        body: StmtId,
    ) -> Option<Contado> {
        let Some(ast::ForInit::Variables(lista)) = init else { return None };
        let [var] = &lista.variables[..] else { return None };
        if lista.late || var.initializer.is_none() {
            return None;
        }
        let i = var.name.sym;
        let int = self.ctx.core.int;
        if self.ctx.tipo_local(self.unit_id, var.name.span.start as usize) != Some(int) {
            return None;
        }
        let Some(cond) = condition else { return None };
        let ExprKind::Binary { op, left, right } = &ast.expr(cond).kind else { return None };
        let e_i = |e: ExprId| matches!(&ast.expr(e).kind, ExprKind::Identifier(n) if n.sym == i);
        // `i < L` ou `L > i` (e as demais): o sentido em que `i` anda e se a
        // comparação é estrita.
        let (limite, crescente, estrita) = match op {
            ast::BinaryOp::Lt if e_i(*left) => (*right, true, true),
            ast::BinaryOp::LtEq if e_i(*left) => (*right, true, false),
            ast::BinaryOp::Gt if e_i(*left) => (*right, false, true),
            ast::BinaryOp::GtEq if e_i(*left) => (*right, false, false),
            ast::BinaryOp::Gt if e_i(*right) => (*left, true, true),
            ast::BinaryOp::GtEq if e_i(*right) => (*left, true, false),
            ast::BinaryOp::Lt if e_i(*right) => (*left, false, true),
            ast::BinaryOp::LtEq if e_i(*right) => (*left, false, false),
            _ => return None,
        };
        let [u] = updates else { return None };
        let fonte = self.source();
        let literal = |e: ExprId| match &ast.expr(e).kind {
            ExprKind::Int(span) => fonte.get(span.start as usize..span.end as usize).and_then(|t| {
                let t = t.replace('_', "");
                match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).ok(),
                    None => t.parse::<i64>().ok(),
                }
            }),
            _ => None,
        };
        // O passo, positivo, no sentido da comparação.
        let passo = match &ast.expr(*u).kind {
            ExprKind::Unary { op: ast::UnaryOp::PostfixInc | ast::UnaryOp::PrefixInc, operand } if crescente && e_i(*operand) => 1,
            ExprKind::Unary { op: ast::UnaryOp::PostfixDec | ast::UnaryOp::PrefixDec, operand } if !crescente && e_i(*operand) => 1,
            ExprKind::Assign { op: ast::AssignOp::Compound(ast::BinaryOp::Add), target, value } if crescente && e_i(*target) => {
                literal(*value).unwrap_or(0)
            }
            ExprKind::Assign { op: ast::AssignOp::Compound(ast::BinaryOp::Sub), target, value } if !crescente && e_i(*target) => {
                literal(*value).unwrap_or(0)
            }
            _ => 0,
        };
        if passo <= 0 {
            return None;
        }
        let gravados = super::captura::gravados_de_fora(self.ctx, self.unit_id, ast, body, &[cond]);
        if gravados.contains(&i) {
            return None;
        }
        // Um local que nem o corpo, nem a condição, nem closure alguma
        // grava: sem célula (`captura.rs`), o valor não muda no laço.
        let local_fixo = |e: ExprId| match &ast.expr(e).kind {
            ExprKind::Identifier(n) => {
                !gravados.contains(&n.sym)
                    && self.buscar_local(n.sym).is_some_and(|l| {
                        matches!(l.modo, super::locais::Modo::Memoria(_) | super::locais::Modo::Valor(_) | super::locais::Modo::Ambiente { celula: false, .. })
                    })
            }
            _ => false,
        };
        // O último valor que `i` pode ter sem sair do laço é `L - 1` (estrita)
        // ou `L`; o laço termina se somar o passo a ele não transborda — o
        // `int` do nativo dá a volta, e aí o laço não terminaria.
        let sem_transbordar = |l: i64| {
            let ultimo = if estrita { if crescente { l.checked_sub(1) } else { l.checked_add(1) } } else { Some(l) };
            ultimo.is_some_and(|u| if crescente { u.checked_add(passo).is_some() } else { u.checked_sub(passo).is_some() })
        };
        let simples = match &ast.expr(limite).kind {
            ExprKind::Int(_) => literal(limite).is_some_and(sem_transbordar),
            // Um valor qualquer: só o passo 1 estrito nunca transborda.
            ExprKind::Identifier(_) => {
                estrita && passo == 1 && local_fixo(limite) && self.ctx.get_type(self.unit_id, limite) == Some(int)
            }
            // O comprimento de uma lista tipada fica em `0..=2^60` (cabe na
            // memória endereçável); o passo não o faz transbordar.
            ExprKind::Property { target, name, null_aware: false } if self.ctx.symbol_name(name.sym) == "length" => {
                let alvo = self.indexavel(self.ctx.get_type(self.unit_id, *target));
                // N13: `List<E>` só termina sozinho se o comprimento não muda
                // nas voltas — nenhum chamado no corpo — e a lista for do
                // runtime, o que só a execução diz (as voltas são duplicadas).
                if crescente
                    && estrita
                    && passo <= 1 << 60
                    && local_fixo(*target)
                    && matches!(alvo, Some(super::tipados::Indexavel::Nucleo { .. }))
                    && let ExprKind::Identifier(n) = &ast.expr(*target).kind
                    && self.buscar_local(n.sym).is_some_and(|l| matches!(l.modo, super::locais::Modo::Memoria(_)))
                    && self.corpo_sem_chamadas(ast, body, n.sym, i)
                {
                    return Some(Contado::Lista { alvo: *target, local: n.sym });
                }
                crescente
                    && passo <= 1 << 60
                    && local_fixo(*target)
                    && matches!(alvo, Some(super::tipados::Indexavel::Tipada(_) | super::tipados::Indexavel::Simd { .. }))
            }
            _ => false,
        };
        simples.then_some(Contado::Simples)
    }
}

/// Um `for` que termina sozinho (J01): o limite é fixo, ou (N13) é o
/// `length` de uma lista do núcleo que o corpo não pode mudar — aí as voltas
/// valem para a lista do runtime, e a classe do programa que implementa
/// `List` segue pelas voltas de sempre.
#[derive(Debug, Clone, Copy)]
pub(super) enum Contado {
    Simples,
    Lista { alvo: ExprId, local: SymbolId },
}

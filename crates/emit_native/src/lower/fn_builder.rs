//! Construtor de funções HIR durante o lowering.

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionElementId};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, FunctionBody, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{LocalId, MemberRef, Resolved};
use std::collections::HashMap;

pub struct FnBuilder<'a, 'c> {
    pub ctx: &'c Context<'a>,
    pub unit_id: dartforge_elements::model::UnitId,
    pub func: Function,
    pub current_block: BlockId,
    pub next_value: u32,
    pub next_block: u32,
    pub locals: HashMap<LocalId, Operand>,
    pub named_locals: HashMap<SymbolId, Operand>,
    pub value_types: HashMap<ValueId, Type>,
    pub break_targets: Vec<BlockId>,
    pub continue_targets: Vec<BlockId>,
    pub this_param: Option<Operand>,
    pub enclosing_class: Option<dartforge_elements::model::ClassId>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn new(
        ctx: &'c Context<'a>,
        unit_id: dartforge_elements::model::UnitId,
        symbol: String,
        name: String,
        return_ty: Type,
    ) -> Self {
        let entry_block = BlockId(0);
        let func = Function {
            symbol,
            name,
            params: Vec::new(),
            return_ty,
            blocks: vec![BasicBlock {
                id: entry_block,
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
        };

        Self {
            ctx,
            unit_id,
            func,
            current_block: entry_block,
            next_value: 0,
            next_block: 1,
            locals: HashMap::new(),
            named_locals: HashMap::new(),
            value_types: HashMap::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            this_param: None,
            enclosing_class: None,
        }
    }

    pub fn add_param(&mut self, name: String, ty: Type) -> ValueId {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty);
        self.func.params.push((vid, name, ty));
        vid
    }

    pub fn operand_type(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.value_types.get(v).cloned().unwrap_or(Type::Ref),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::String(_)) => Type::Ref,
            Operand::Constant(Constant::Null) => Type::Ref,
        }
    }

    pub fn operand_tag(&self, op: &Operand) -> u8 {
        match self.operand_type(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::Ref => 3,
            Type::F64 => 4,
            _ => 1,
        }
    }

    pub fn source(&self) -> &str {
        &self.ctx.program.unit(self.unit_id).source
    }

    pub fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        self.func.blocks.push(BasicBlock {
            id,
            instructions: Vec::new(),
            terminator: Terminator::Return(None),
        });
        id
    }

    pub fn set_block(&mut self, block: BlockId) {
        self.current_block = block;
    }

    pub fn emit(&mut self, inst: Instruction, ty: Type) -> Operand {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty.clone());
        let idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
        self.func.blocks[idx].instructions.push((vid, inst, ty));
        Operand::Val(vid)
    }

    pub fn terminate(&mut self, term: Terminator) {
        let idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
        self.func.blocks[idx].terminator = term;
    }

    pub fn lower_stmt(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
        let stmt = ast.stmt(stmt_id);
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                for &s in stmts.iter() {
                    self.lower_stmt(ast, s);
                }
            }
            StmtKind::Expression(expr_id) => {
                self.lower_expr(ast, *expr_id);
            }
            StmtKind::Variables(var_list) => {
                for var in &var_list.variables {
                    let sym = var.name.sym;
                    let init_op = if let Some(init_id) = var.initializer {
                        self.lower_expr(ast, init_id)
                    } else {
                        self.emit(Instruction::Const(Constant::Null), Type::Ref)
                    };
                    self.named_locals.insert(sym, init_op);
                }
            }
            StmtKind::If {
                condition,
                then,
                else_,
                ..
            } => {
                let cond_op = self.lower_expr(ast, *condition);
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

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                let cond_op = self.lower_expr(ast, *condition);
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_header);

                self.set_block(loop_body);
                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_header));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.set_block(exit_block);
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                if let Some(init) = init {
                    match init {
                        ast::ForInit::Variables(var_list) => {
                            for var in &var_list.variables {
                                let sym = var.name.sym;
                                let init_op = if let Some(init_id) = var.initializer {
                                    self.lower_expr(ast, init_id)
                                } else {
                                    self.emit(Instruction::Const(Constant::Null), Type::Ref)
                                };
                                self.named_locals.insert(sym, init_op);
                            }
                        }
                        ast::ForInit::Expression(e) => {
                            self.lower_expr(ast, *e);
                        }
                    }
                }

                let loop_header = self.new_block();
                let loop_body = self.new_block();
                let loop_update = self.new_block();
                let exit_block = self.new_block();

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                if let Some(cond_id) = condition {
                    let cond_op = self.lower_expr(ast, *cond_id);
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
                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_update));

                self.set_block(loop_update);
                for &u in updates.iter() {
                    self.lower_expr(ast, u);
                }
                self.terminate(Terminator::Branch(loop_header));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.set_block(exit_block);
            }
            StmtKind::Return(expr_opt) => {
                if let Some(e) = expr_opt {
                    let op = self.lower_expr(ast, *e);
                    self.terminate(Terminator::Return(Some(op)));
                } else {
                    self.terminate(Terminator::Return(None));
                }
                let dead_b = self.new_block();
                self.set_block(dead_b);
            }
            StmtKind::Break(_) => {
                if let Some(&target) = self.break_targets.last() {
                    self.terminate(Terminator::Branch(target));
                    let dead_b = self.new_block();
                    self.set_block(dead_b);
                }
            }
            StmtKind::Continue(_) => {
                if let Some(&target) = self.continue_targets.last() {
                    self.terminate(Terminator::Branch(target));
                    let dead_b = self.new_block();
                    self.set_block(dead_b);
                }
            }
            StmtKind::Assert { condition, .. } => {
                let cond_op = self.lower_expr(ast, *condition);
                let fail_b = self.new_block();
                let cont_b = self.new_block();

                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block: cont_b,
                    else_block: fail_b,
                });

                self.set_block(fail_b);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_null_assert_fail".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
                self.terminate(Terminator::Unreachable);

                self.set_block(cont_b);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                let iterable_op = self.lower_expr(ast, *iterable);
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

                self.terminate(Terminator::Branch(loop_header));
                self.set_block(loop_header);

                let phi_vid = ValueId(self.next_value);
                self.next_value += 1;
                self.value_types.insert(phi_vid, Type::I64);
                let phi_op = Operand::Val(phi_vid);

                let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
                let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
                self.func.blocks[header_idx].instructions.push((
                    phi_vid,
                    Instruction::Phi { incoming: Vec::new(), ty: Type::I64 },
                    Type::I64,
                ));

                let cmp = self.emit(Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), len_op), Type::I1);
                self.terminate(Terminator::CondBranch {
                    cond: cmp,
                    then_block: loop_body,
                    else_block: exit_block,
                });

                self.break_targets.push(exit_block);
                self.continue_targets.push(loop_update);

                self.set_block(loop_body);
                let item_val = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_list_get_bits".to_string(),
                        args: vec![(iterable_op.clone(), Type::Ref), (phi_op.clone(), Type::I64)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );

                match target {
                    ast::ForInTarget::Declared { name, .. } => {
                        self.named_locals.insert(name.sym, item_val);
                    }
                    ast::ForInTarget::Expression(e) => {
                        if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                            self.named_locals.insert(id.sym, item_val);
                        }
                    }
                    _ => {}
                }

                self.lower_stmt(ast, *body);
                self.terminate(Terminator::Branch(loop_update));

                self.break_targets.pop();
                self.continue_targets.pop();

                self.set_block(loop_update);
                let next_idx = self.emit(
                    Instruction::Add(phi_op, Operand::Constant(Constant::Int(1))),
                    Type::I64,
                );
                self.terminate(Terminator::Branch(loop_header));

                let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
                self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                    incoming: vec![
                        (pre_block, Operand::Constant(Constant::Int(0))),
                        (loop_update, next_idx),
                    ],
                    ty: Type::I64,
                };

                self.set_block(exit_block);
            }
            StmtKind::Empty => {}
            _ => {}
        }
    }

    pub fn lower_expr(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let expr = ast.expr(expr_id);
        match &expr.kind {
            ExprKind::Int(span) => {
                let raw = &self.source()[span.start as usize..span.end as usize];
                let text = raw.replace('_', "");
                let val: i64 = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
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
            ExprKind::Bool(b) => {
                self.emit(Instruction::Const(Constant::Bool(*b)), Type::I1)
            }
            ExprKind::Null => {
                self.emit(Instruction::Const(Constant::Null), Type::Ref)
            }
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
                                if let ExprKind::Index { target: idx_target, index: idx_index, .. } = &ast.expr(*sub_expr).kind {
                                    let t_op = self.lower_expr(ast, *idx_target);
                                    let i_op = self.lower_expr(ast, *idx_index);
                                    let target_ty = self.ctx.get_type(self.unit_id, *idx_target);
                                    let is_map = target_ty.map_or(false, |t| self.ctx.is_map(t))
                                        || self.operand_type(&i_op) == Type::Ref;
                                    if is_map {
                                        let itag = self.operand_tag(&i_op);
                                        self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_map_get_to_string".to_string(),
                                                args: vec![
                                                    (t_op, Type::Ref),
                                                    (i_op, Type::I64),
                                                    (Operand::Constant(Constant::Int(itag as i64)), Type::I8),
                                                ],
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        )
                                    } else {
                                        let item_op = self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_list_get_bits".to_string(),
                                                args: vec![(t_op, Type::Ref), (i_op, Type::I64)],
                                                ret_ty: Type::I64,
                                            },
                                            Type::I64,
                                        );
                                        self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_to_string_i64".to_string(),
                                                args: vec![(item_op, Type::I64)],
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        )
                                    }
                                } else {
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
                                        Type::Ref => self.emit(
                                            Instruction::CallStatic {
                                                symbol: "dartforge_dispatch_toString".to_string(),
                                                args: vec![raw_op],
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        ),
                                        Type::Void => self.emit(
                                            Instruction::Const(Constant::String("null".to_string())),
                                            Type::Ref,
                                        ),
                                    }
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
                        self.emit(Instruction::Const(Constant::String("".to_string())), Type::Ref)
                    })
                }
            }
            ExprKind::Identifier(name) => {
                let sym = name.sym;
                if let Some(op) = self.named_locals.get(&sym) {
                    return op.clone();
                }
                if let Some(resolved) = self.ctx.get_resolved(self.unit_id, expr_id) {
                    match resolved {
                        Resolved::Local(lid) => {
                            if let Some(op) = self.locals.get(lid) {
                                return op.clone();
                            }
                        }
                        Resolved::Parameter { name, .. } => {
                            if let Some(op) = self.named_locals.get(name) {
                                return op.clone();
                            }
                        }
                        Resolved::Member { class, member, .. } => {
                            if let Some(this_op) = &self.this_param {
                                let field_idx_opt = match member {
                                    MemberRef::Variable(var_id) => {
                                        self.ctx.program.classes[class.0 as usize]
                                            .fields
                                            .iter()
                                            .position(|&v| v == *var_id)
                                    }
                                    MemberRef::Function(fid) => {
                                        let func = &self.ctx.program.functions[fid.0 as usize];
                                        if let Some(var_id) = func.variable {
                                            self.ctx.program.classes[class.0 as usize]
                                                .fields
                                                .iter()
                                                .position(|&v| v == var_id)
                                        } else {
                                            self.ctx.program.classes[class.0 as usize]
                                                .fields
                                                .iter()
                                                .position(|&v| self.ctx.program.variables[v.0 as usize].name == func.name)
                                        }
                                    }
                                };
                                if let Some(field_idx) = field_idx_opt {
                                    return self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_object_get".to_string(),
                                            args: vec![
                                                (this_op.clone(), Type::Ref),
                                                (Operand::Constant(Constant::Int(field_idx as i64)), Type::I64),
                                            ],
                                            ret_ty: Type::I64,
                                        },
                                        Type::I64,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
                self.emit(Instruction::Const(Constant::Int(0)), Type::I64)
            }
            ExprKind::Parenthesized(sub) => self.lower_expr(ast, *sub),
            ExprKind::Binary { op, left, right } => {
                let lop = self.lower_expr(ast, *left);
                let rop = self.lower_expr(ast, *right);

                let l_ty = self.ctx.get_type(self.unit_id, *left);
                let is_double = l_ty.map_or(false, |t| self.ctx.is_double(t))
                    || self.operand_type(&lop) == Type::F64
                    || self.operand_type(&rop) == Type::F64;
                let r_ty = self.ctx.get_type(self.unit_id, *right);
                let is_string = l_ty.map_or(false, |t| self.ctx.is_string(t))
                    || r_ty.map_or(false, |t| self.ctx.is_string(t))
                    || matches!(ast.expr(*left).kind, ExprKind::String(_))
                    || matches!(ast.expr(*right).kind, ExprKind::String(_));

                match op {
                    BinaryOp::Add => {
                        if is_string {
                            self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_string_concat".to_string(),
                                    args: vec![(lop, Type::Ref), (rop, Type::Ref)],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            )
                        } else if is_double {
                            self.emit(Instruction::FAdd(lop, rop), Type::F64)
                        } else {
                            self.emit(Instruction::Add(lop, rop), Type::I64)
                        }
                    }
                    BinaryOp::Sub => {
                        if is_double {
                            self.emit(Instruction::FSub(lop, rop), Type::F64)
                        } else {
                            self.emit(Instruction::Sub(lop, rop), Type::I64)
                        }
                    }
                    BinaryOp::Mul => {
                        if is_string {
                            self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_string_repeat".to_string(),
                                    args: vec![(lop, Type::Ref), (rop, Type::I64)],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            )
                        } else if is_double {
                            self.emit(Instruction::FMul(lop, rop), Type::F64)
                        } else {
                            self.emit(Instruction::Mul(lop, rop), Type::I64)
                        }
                    }
                    BinaryOp::Div => self.emit(Instruction::FDiv(lop, rop), Type::F64),
                    BinaryOp::TruncDiv => self.emit(Instruction::SDiv(lop, rop), Type::I64),
                    BinaryOp::Rem => self.emit(Instruction::SRem(lop, rop), Type::I64),
                    BinaryOp::Shl => self.emit(Instruction::Shl(lop, rop), Type::I64),
                    BinaryOp::Shr => self.emit(Instruction::AShr(lop, rop), Type::I64),
                    BinaryOp::BitAnd => self.emit(Instruction::And(lop, rop), Type::I64),
                    BinaryOp::BitOr => self.emit(Instruction::Or(lop, rop), Type::I64),
                    BinaryOp::BitXor => self.emit(Instruction::Xor(lop, rop), Type::I64),
                    BinaryOp::Eq => {
                        let is_ref_cmp = self.operand_type(&lop) == Type::Ref && self.operand_type(&rop) == Type::Ref;
                        if is_string || is_ref_cmp {
                            let eq_i8 = self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_equal".to_string(),
                                    args: vec![(lop, Type::Ref), (rop, Type::Ref)],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            );
                            self.emit(
                                Instruction::Trunc {
                                    op: eq_i8,
                                    from: Type::I8,
                                    to: Type::I1,
                                },
                                Type::I1,
                            )
                        } else if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Eq, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Eq, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::NotEq => {
                        let is_ref_cmp = self.operand_type(&lop) == Type::Ref && self.operand_type(&rop) == Type::Ref;
                        if is_string || is_ref_cmp {
                            let eq_i8 = self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_equal".to_string(),
                                    args: vec![(lop, Type::Ref), (rop, Type::Ref)],
                                    ret_ty: Type::I8,
                                },
                                Type::I8,
                            );
                            let eq_i1 = self.emit(
                                Instruction::Trunc {
                                    op: eq_i8,
                                    from: Type::I8,
                                    to: Type::I1,
                                },
                                Type::I1,
                            );
                            self.emit(Instruction::LNot(eq_i1), Type::I1)
                        } else if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Ne, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Ne, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::Lt => {
                        if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Lt, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Slt, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::LtEq => {
                        if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Le, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Sle, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::Gt => {
                        if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Gt, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Sgt, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::GtEq => {
                        if is_double {
                            self.emit(Instruction::FCmp(FCmpOp::Ge, lop, rop), Type::I1)
                        } else {
                            self.emit(Instruction::ICmp(ICmpOp::Sge, lop, rop), Type::I1)
                        }
                    }
                    BinaryOp::IfNull => {
                        let left_block = self.current_block;
                        let null_block = self.new_block();
                        let merge_block = self.new_block();
                        let is_null = self.emit(
                            Instruction::ICmp(
                                ICmpOp::Eq,
                                lop.clone(),
                                Operand::Constant(Constant::Int(0)),
                            ),
                            Type::I1,
                        );
                        self.terminate(Terminator::CondBranch {
                            cond: is_null,
                            then_block: null_block,
                            else_block: merge_block,
                        });
                        self.set_block(null_block);
                        let rop = self.lower_expr(ast, *right);
                        let rop_block = self.current_block;
                        self.terminate(Terminator::Branch(merge_block));

                        self.set_block(merge_block);
                        let ty = self.operand_type(&rop);
                        self.emit(
                            Instruction::Phi {
                                incoming: vec![(left_block, lop), (rop_block, rop)],
                                ty,
                            },
                            ty,
                        )
                    }
                    _ => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                }
            }
            ExprKind::Unary { op, operand } => {
                let sub_op = self.lower_expr(ast, *operand);
                match op {
                    UnaryOp::Neg => self.emit(Instruction::Neg(sub_op), Type::I64),
                    UnaryOp::Not => self.emit(Instruction::LNot(sub_op), Type::I1),
                    UnaryOp::BitNot => self.emit(Instruction::Not(sub_op), Type::I64),
                    _ => sub_op,
                }
            }
            ExprKind::Conditional { condition, then, else_ } => {
                let cond_op = self.lower_expr(ast, *condition);
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: cond_op,
                    then_block,
                    else_block,
                });
                self.set_block(then_block);
                let then_op = self.lower_expr(ast, *then);
                let then_end = self.current_block;
                self.terminate(Terminator::Branch(merge_block));

                self.set_block(else_block);
                let else_op = self.lower_expr(ast, *else_);
                let else_end = self.current_block;
                self.terminate(Terminator::Branch(merge_block));

                self.set_block(merge_block);
                let ty = self.operand_type(&then_op);
                self.emit(
                    Instruction::Phi {
                        incoming: vec![(then_end, then_op), (else_end, else_op)],
                        ty,
                    },
                    ty,
                )
            }
            ExprKind::Property { target, name, .. } => {
                let target_op = self.lower_expr(ast, *target);
                let prop_name = self.ctx.symbol_name(name.sym);

                if let Some(resolved) = self.ctx.get_resolved(self.unit_id, expr_id) {
                    if let Resolved::Member { class, member, .. } = resolved {
                        let field_idx_opt = match member {
                            MemberRef::Variable(var_id) => {
                                self.ctx.program.classes[class.0 as usize]
                                    .fields
                                    .iter()
                                    .position(|&v| v == *var_id)
                            }
                            MemberRef::Function(fid) => {
                                let func = &self.ctx.program.functions[fid.0 as usize];
                                if let Some(var_id) = func.variable {
                                    self.ctx.program.classes[class.0 as usize]
                                        .fields
                                        .iter()
                                        .position(|&v| v == var_id)
                                } else {
                                    self.ctx.program.classes[class.0 as usize]
                                        .fields
                                        .iter()
                                        .position(|&v| self.ctx.program.variables[v.0 as usize].name == func.name)
                                }
                            }
                        };
                        if let Some(field_idx) = field_idx_opt {
                            return self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_object_get".to_string(),
                                    args: vec![
                                        (target_op, Type::Ref),
                                        (Operand::Constant(Constant::Int(field_idx as i64)), Type::I64),
                                    ],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            );
                        }
                    }
                }

                if prop_name == "length" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_generic_len".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "isEmpty" {
                    let len_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_generic_len".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    );
                    self.emit(
                        Instruction::ICmp(
                            ICmpOp::Eq,
                            len_op,
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    )
                } else if prop_name == "isNotEmpty" {
                    let len_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_generic_len".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    );
                    self.emit(
                        Instruction::ICmp(
                            ICmpOp::Ne,
                            len_op,
                            Operand::Constant(Constant::Int(0)),
                        ),
                        Type::I1,
                    )
                } else if prop_name == "last" {
                    let len_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_len".to_string(),
                            args: vec![(target_op.clone(), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    );
                    let last_idx = self.emit(
                        Instruction::Sub(
                            len_op,
                            Operand::Constant(Constant::Int(1)),
                        ),
                        Type::I64,
                    );
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_get_bits".to_string(),
                            args: vec![(target_op, Type::Ref), (last_idx, Type::I64)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "first" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_get_bits".to_string(),
                            args: vec![
                                (target_op, Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                            ],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "codeUnits" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_string_code_units".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "runes" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_string_runes".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "reversed" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_reversed".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else {
                    self.emit(Instruction::Const(Constant::Int(0)), Type::I64)
                }
            }
            ExprKind::Index { target, index, .. } => {
                let target_op = self.lower_expr(ast, *target);
                let idx_op = self.lower_expr(ast, *index);
                let target_ty = self.ctx.get_type(self.unit_id, *target);
                let is_map = target_ty.map_or(false, |t| self.ctx.is_map(t))
                    || self.operand_type(&idx_op) == Type::Ref;
                if is_map {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_map_get_bits".to_string(),
                            args: vec![
                                (target_op, Type::Ref),
                                (idx_op, Type::Ref),
                                (Operand::Constant(Constant::Int(3)), Type::I8),
                            ],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_get_bits".to_string(),
                            args: vec![(target_op, Type::Ref), (idx_op, Type::I64)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                }
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
                let block_idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
                self.func.blocks[block_idx].instructions.push((
                    vid,
                    Instruction::AllocRecord { elements: ops },
                    Type::Ref,
                ));
                Operand::Val(vid)
            }
            ExprKind::Call { target, arguments } => {
                if let ExprKind::Identifier(name) = &ast.expr(*target).kind {
                    if self.ctx.symbol_name(name.sym) == "print" {
                        if let Some(first_arg) = arguments.args.first() {
                            let arg_op = self.lower_expr(ast, first_arg.value);
                            let op_ty = self.operand_type(&arg_op);

                            if op_ty == Type::F64 {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_f64".to_string(),
                                        args: vec![(arg_op, Type::F64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else if op_ty == Type::I64 {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_i64".to_string(),
                                        args: vec![(arg_op, Type::I64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else if op_ty == Type::I1 || op_ty == Type::I8 {
                                let arg_i8 = if op_ty == Type::I1 {
                                    self.emit(
                                        Instruction::ZExt {
                                            op: arg_op,
                                            from: Type::I1,
                                            to: Type::I8,
                                        },
                                        Type::I8,
                                    )
                                } else {
                                    arg_op
                                };
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_bool".to_string(),
                                        args: vec![(arg_i8, Type::I8)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else if matches!(ast.expr(first_arg.value).kind, ExprKind::Null) {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_null".to_string(),
                                        args: Vec::new(),
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_handle".to_string(),
                                        args: vec![(arg_op, Type::I64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            }
                        }
                    }
                }

                // Verifica construtor via Identifier de classe especial (Object, StringBuffer)
                if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                    let id_str = self.ctx.symbol_name(id.sym);
                    if id_str == "Object" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_object_new".to_string(),
                                args: vec![
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                    (Operand::Constant(Constant::Int(0)), Type::I64),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if id_str == "StringBuffer" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_buffer_new".to_string(),
                                args: Vec::new(),
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    }
                }

                // Verifica métodos estáticos/construtores nomeados de String (String.fromCharCode, String.fromCharCodes)
                if let ExprKind::Property { target: inner_target, name: method_name, .. } = &ast.expr(*target).kind {
                    let m_name = self.ctx.symbol_name(method_name.sym);
                    if let ExprKind::Identifier(id) = &ast.expr(*inner_target).kind {
                        let id_str = self.ctx.symbol_name(id.sym);
                        if id_str == "String" {
                            if m_name == "fromCharCode" {
                                let code_op = self.lower_expr(ast, arguments.args[0].value);
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_string_from_char_code".to_string(),
                                        args: vec![(code_op, Type::I64)],
                                        ret_ty: Type::Ref,
                                    },
                                    Type::Ref,
                                );
                            } else if m_name == "fromCharCodes" {
                                let list_op = self.lower_expr(ast, arguments.args[0].value);
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_string_from_char_codes".to_string(),
                                        args: vec![(list_op, Type::Ref)],
                                        ret_ty: Type::Ref,
                                    },
                                    Type::Ref,
                                );
                            }
                        }
                    }
                }

                // Verifica construtor via Resolved
                if let Some(resolved) = self.ctx.get_resolved(self.unit_id, expr_id) {
                    if let Resolved::Constructor(fid) = resolved {
                        return self.lower_constructor(ast, *fid, &arguments.args);
                    }
                }

                // Verifica construtor via Identifier de classe de usuário
                if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                    let id_str = self.ctx.symbol_name(id.sym);
                    for (c_idx, c) in self.ctx.program.classes.iter().enumerate() {
                        if self.ctx.symbol_name(c.name) == id_str {
                            let empty_sym = self.ctx.interner.lookup("");
                            let cfid_opt = empty_sym.and_then(|sym| c.constructors.get(&sym).copied());
                            if let Some(cfid) = cfid_opt {
                                return self.lower_constructor(ast, cfid, &arguments.args);
                            } else {
                                let class_id = (c_idx + 1) as u32;
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_object_new".to_string(),
                                        args: vec![
                                            (Operand::Constant(Constant::Int(class_id as i64)), Type::I64),
                                            (Operand::Constant(Constant::Int(c.fields.len() as i64)), Type::I64),
                                        ],
                                        ret_ty: Type::Ref,
                                    },
                                    Type::Ref,
                                );
                            }
                        }
                    }
                }

                // Chamadas de método sobre objeto / coleção
                if let ExprKind::Property { target: inner_target, name: method_name, .. } = &ast.expr(*target).kind {
                    let m_name = self.ctx.symbol_name(method_name.sym);
                    let recv_op = self.lower_expr(ast, *inner_target);

                    if m_name == "toUpperCase" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_to_upper".to_string(),
                                args: vec![(recv_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "codeUnitAt" {
                        let idx_op = if let Some(first_arg) = arguments.args.first() {
                            self.lower_expr(ast, first_arg.value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_code_unit_at".to_string(),
                                args: vec![(recv_op, Type::Ref), (idx_op, Type::I64)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                    } else if m_name == "toRadixString" {
                        let radix_op = if let Some(first_arg) = arguments.args.first() {
                            self.lower_expr(ast, first_arg.value)
                        } else {
                            Operand::Constant(Constant::Int(10))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_int_to_radix_string".to_string(),
                                args: vec![(recv_op, Type::I64), (radix_op, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "substring" {
                        let start_op = if let Some(first_arg) = arguments.args.first() {
                            self.lower_expr(ast, first_arg.value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        let end_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            Operand::Constant(Constant::Int(-1))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_substring".to_string(),
                                args: vec![(recv_op, Type::Ref), (start_op, Type::I64), (end_op, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "indexOf" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        let start_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_index_of".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref), (start_op, Type::I64)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                    } else if m_name == "lastIndexOf" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_last_index_of".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                    } else if m_name == "split" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_split".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "contains" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        let c_i8 = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_contains".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        );
                        return self.emit(
                            Instruction::Trunc {
                                op: c_i8,
                                from: Type::I8,
                                to: Type::I1,
                            },
                            Type::I1,
                        );
                    } else if m_name == "replaceAll" {
                        let from_op = self.lower_expr(ast, arguments.args[0].value);
                        let to_op = self.lower_expr(ast, arguments.args[1].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_replace_all".to_string(),
                                args: vec![(recv_op, Type::Ref), (from_op, Type::Ref), (to_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "padLeft" {
                        let width_op = self.lower_expr(ast, arguments.args[0].value);
                        let pad_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            self.emit(Instruction::Const(Constant::String(" ".to_string())), Type::Ref)
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_pad_left".to_string(),
                                args: vec![(recv_op, Type::Ref), (width_op, Type::I64), (pad_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "padRight" {
                        let width_op = self.lower_expr(ast, arguments.args[0].value);
                        let pad_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            self.emit(Instruction::Const(Constant::String(" ".to_string())), Type::Ref)
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_pad_right".to_string(),
                                args: vec![(recv_op, Type::Ref), (width_op, Type::I64), (pad_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "startsWith" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        let start_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        let c_i8 = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_starts_with".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref), (start_op, Type::I64)],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        );
                        return self.emit(
                            Instruction::Trunc {
                                op: c_i8,
                                from: Type::I8,
                                to: Type::I1,
                            },
                            Type::I1,
                        );
                    } else if m_name == "endsWith" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        let c_i8 = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_ends_with".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                                ret_ty: Type::I8,
                            },
                            Type::I8,
                        );
                        return self.emit(
                            Instruction::Trunc {
                                op: c_i8,
                                from: Type::I8,
                                to: Type::I1,
                            },
                            Type::I1,
                        );
                    } else if m_name == "trim" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_trim".to_string(),
                                args: vec![(recv_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "trimLeft" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_trim_left".to_string(),
                                args: vec![(recv_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "trimRight" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_trim_right".to_string(),
                                args: vec![(recv_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "toLowerCase" {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_to_lower".to_string(),
                                args: vec![(recv_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "compareTo" {
                        let other_op = self.lower_expr(ast, arguments.args[0].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_compare_to".to_string(),
                                args: vec![(recv_op, Type::Ref), (other_op, Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                    } else if m_name == "replaceFirst" {
                        let from_op = self.lower_expr(ast, arguments.args[0].value);
                        let to_op = self.lower_expr(ast, arguments.args[1].value);
                        let start_op = if arguments.args.len() > 2 {
                            self.lower_expr(ast, arguments.args[2].value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_replace_first".to_string(),
                                args: vec![(recv_op, Type::Ref), (from_op, Type::Ref), (to_op, Type::Ref), (start_op, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "replaceRange" {
                        let start_op = self.lower_expr(ast, arguments.args[0].value);
                        let end_op = self.lower_expr(ast, arguments.args[1].value);
                        let rep_op = self.lower_expr(ast, arguments.args[2].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_replace_range".to_string(),
                                args: vec![(recv_op, Type::Ref), (start_op, Type::I64), (end_op, Type::I64), (rep_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "write" {
                        let arg_op = self.lower_expr(ast, arguments.args[0].value);
                        let arg_ty = self.operand_type(&arg_op);
                        let str_op = match arg_ty {
                            Type::I64 => self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_to_string_i64".to_string(),
                                    args: vec![(arg_op, Type::I64)],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            ),
                            Type::Ref => self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_to_string_handle".to_string(),
                                    args: vec![(arg_op, Type::I64)],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            ),
                            _ => arg_op,
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_buffer_write".to_string(),
                                args: vec![(recv_op, Type::Ref), (str_op, Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        );
                    } else if m_name == "toString" {
                        return self.emit(
                            Instruction::CallStatic {
                                symbol: "dartforge_dispatch_toString".to_string(),
                                args: vec![recv_op],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "join" {
                        let sep_op = if let Some(first_arg) = arguments.args.first() {
                            self.lower_expr(ast, first_arg.value)
                        } else {
                            self.emit(Instruction::Const(Constant::String("".to_string())), Type::Ref)
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_list_join".to_string(),
                                args: vec![(recv_op, Type::Ref), (sep_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "toList" {
                        return recv_op;
                    } else if m_name == "map" {
                        if let Some(first_arg) = arguments.args.first() {
                            if let ExprKind::FunctionExpression(func_id) = ast.expr(first_arg.value).kind {
                                let closure = &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                                let param_sym = closure.parameters.as_ref()
                                    .and_then(|p| p.first())
                                    .and_then(|p| p.name.as_ref())
                                    .map(|n| n.sym);

                                let len_op = self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_list_len".to_string(),
                                        args: vec![(recv_op.clone(), Type::Ref)],
                                        ret_ty: Type::I64,
                                    },
                                    Type::I64,
                                );
                                let res_list = self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_list_new_empty".to_string(),
                                        args: Vec::new(),
                                        ret_ty: Type::Ref,
                                    },
                                    Type::Ref,
                                );

                                let pre_block = self.current_block;
                                let loop_header = self.new_block();
                                let loop_body = self.new_block();
                                let exit_block = self.new_block();

                                self.terminate(Terminator::Branch(loop_header));
                                self.set_block(loop_header);

                                let phi_vid = ValueId(self.next_value);
                                self.next_value += 1;
                                self.value_types.insert(phi_vid, Type::I64);
                                let phi_op = Operand::Val(phi_vid);

                                let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
                                let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
                                self.func.blocks[header_idx].instructions.push((
                                    phi_vid,
                                    Instruction::Phi { incoming: Vec::new(), ty: Type::I64 },
                                    Type::I64,
                                ));

                                let cmp = self.emit(Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), len_op), Type::I1);
                                self.terminate(Terminator::CondBranch {
                                    cond: cmp,
                                    then_block: loop_body,
                                    else_block: exit_block,
                                });

                                self.set_block(loop_body);
                                let item_val = self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_list_get_bits".to_string(),
                                        args: vec![(recv_op.clone(), Type::Ref), (phi_op.clone(), Type::I64)],
                                        ret_ty: Type::I64,
                                    },
                                    Type::I64,
                                );

                                if let Some(sym) = param_sym {
                                    self.named_locals.insert(sym, item_val);
                                }

                                let mapped_val = match &closure.body {
                                    FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                                    _ => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                                };

                                let mapped_is_ref = if self.operand_type(&mapped_val) == Type::Ref { 1 } else { 0 };
                                let mapped_tag = if mapped_is_ref == 1 { 3 } else { 1 };
                                self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_list_push".to_string(),
                                        args: vec![
                                            (res_list.clone(), Type::Ref),
                                            (mapped_val, Type::I64),
                                            (Operand::Constant(Constant::Int(mapped_tag)), Type::I8),
                                        ],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );

                                let next_idx = self.emit(
                                    Instruction::Add(phi_op, Operand::Constant(Constant::Int(1))),
                                    Type::I64,
                                );
                                let body_end_block = self.current_block;
                                self.terminate(Terminator::Branch(loop_header));

                                let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
                                self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                                    incoming: vec![
                                        (pre_block, Operand::Constant(Constant::Int(0))),
                                        (body_end_block, next_idx),
                                    ],
                                    ty: Type::I64,
                                };

                                self.set_block(exit_block);
                                return res_list;
                            }
                        }
                    }
                }

                let mut args_ops = Vec::new();
                for a in &arguments.args {
                    args_ops.push(self.lower_expr(ast, a.value));
                }

                self.emit(Instruction::Const(Constant::Int(0)), Type::I64)
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
                self.emit(Instruction::AllocList { elements: elem_ops }, Type::Ref)
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
                self.emit(Instruction::AllocMap { entries }, Type::Ref)
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                if let Some(resolved) = self.ctx.get_resolved(self.unit_id, expr_id) {
                    if let Resolved::Constructor(fid) = resolved {
                        return self.lower_constructor(ast, *fid, &arguments.args);
                    }
                }
                let mut args_ops = Vec::new();
                for a in &arguments.args {
                    args_ops.push(self.lower_expr(ast, a.value));
                }
                self.emit(
                    Instruction::AllocObject {
                        class_id: 0,
                        fields: args_ops,
                    },
                    Type::Ref,
                )
            }
            _ => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
        }
    }

    fn lower_constructor(&mut self, ast: &ast::Ast, fid: FunctionElementId, args: &[ast::Argument]) -> Operand {
        let cid = self.ctx.program.functions[fid.0 as usize].class.unwrap_or(dartforge_elements::model::ClassId(0));
        let class_id = (cid.0 + 1) as u32;
        let field_count = self.ctx.program.classes.get(cid.0 as usize).map_or(0, |c| c.fields.len());
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(class_id as i64)), Type::I64),
                    (Operand::Constant(Constant::Int(field_count as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        for (i, arg) in args.iter().enumerate() {
            let arg_op = self.lower_expr(ast, arg.value);
            let is_ref = if self.operand_type(&arg_op) == Type::Ref { 1 } else { 0 };
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_object_set".to_string(),
                    args: vec![
                        (obj.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                        (arg_op, Type::I64),
                        (Operand::Constant(Constant::Int(is_ref)), Type::I8),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        obj
    }
}

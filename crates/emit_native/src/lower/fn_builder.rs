//! Construtor de funções HIR durante o lowering.

use crate::context::Context;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{LocalId, Resolved};
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
        }
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
                                self.lower_expr(ast, *sub_expr)
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
                let is_string = l_ty.map_or(false, |t| self.ctx.is_string(t))
                    || self.operand_type(&lop) == Type::Ref && matches!(ast.expr(*left).kind, ExprKind::String(_));

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
                        if is_double {
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
                        if is_string {
                            let eq_i8 = self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_string_equal".to_string(),
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
                        if is_string {
                            let eq_i8 = self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_string_equal".to_string(),
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
            ExprKind::Call { target, arguments } => {
                if let ExprKind::Identifier(name) = &ast.expr(*target).kind {
                    if self.ctx.symbol_name(name.sym) == "print" {
                        if let Some(first_arg) = arguments.args.first() {
                            let arg_op = self.lower_expr(ast, first_arg.value);
                            let op_ty = self.operand_type(&arg_op);
                            let arg_ty = self.ctx.get_type(self.unit_id, first_arg.value);

                            if op_ty == Type::I64 || arg_ty.map_or(false, |t| self.ctx.is_int(t)) {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_i64".to_string(),
                                        args: vec![(arg_op, Type::I64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else if op_ty == Type::F64 || arg_ty.map_or(false, |t| self.ctx.is_double(t)) {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_f64".to_string(),
                                        args: vec![(arg_op, Type::F64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                );
                            } else if op_ty == Type::I1 || op_ty == Type::I8 || arg_ty.map_or(false, |t| self.ctx.is_bool(t)) {
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
                            } else if arg_ty.map_or(false, |t| self.ctx.is_string(t))
                                || matches!(ast.expr(first_arg.value).kind, ExprKind::String(_))
                            {
                                return self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_string".to_string(),
                                        args: vec![(arg_op, Type::Ref)],
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

                let mut args_ops = Vec::new();
                for a in &arguments.args {
                    args_ops.push(self.lower_expr(ast, a.value));
                }

                if let ExprKind::Identifier(name) = &ast.expr(*target).kind {
                    let sname = self.ctx.symbol_name(name.sym);
                    if sname == "Object" {
                        return self.emit(
                            Instruction::AllocObject {
                                class_id: 0,
                                fields: Vec::new(),
                            },
                            Type::Ref,
                        );
                    }
                }

                self.emit(Instruction::Const(Constant::Int(0)), Type::I64)
            }
            ExprKind::List { elements, .. } => {
                let mut elem_ops = Vec::new();
                for el in elements.iter() {
                    if let ast::CollectionElement::Expression(e) = el {
                        elem_ops.push(self.lower_expr(ast, *e));
                    }
                }
                self.emit(Instruction::AllocList { elements: elem_ops }, Type::Ref)
            }
            ExprKind::SetOrMap { elements, .. } => {
                let mut entries = Vec::new();
                for el in elements.iter() {
                    if let ast::CollectionElement::MapEntry { key, value, .. } = el {
                        let kop = self.lower_expr(ast, *key);
                        let vop = self.lower_expr(ast, *value);
                        entries.push((kop, vop));
                    }
                }
                self.emit(Instruction::AllocMap { entries }, Type::Ref)
            }
            ExprKind::InstanceCreation { arguments, .. } => {
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
}

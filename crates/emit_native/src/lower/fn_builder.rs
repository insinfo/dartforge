//! Construtor de funções HIR durante o lowering.

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionElementId};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, FunctionBody, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{LocalId, MemberRef, Resolved};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FinallyScope {
    pub entry_block: BlockId,
    pub resume_block: BlockId,
    pub reason_phi: ValueId,
    pub ret_val_phi: ValueId,
    pub incoming: Vec<(BlockId, i64, Operand)>,
}

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
    pub exception_targets: Vec<BlockId>,
    pub finally_scopes: Vec<FinallyScope>,
    pub active_catch_stack: Vec<(Operand, u8)>,
    pub terminated_blocks: std::collections::HashSet<BlockId>,
    pub local_ptrs: HashMap<SymbolId, Operand>,
    pub local_functions: HashMap<SymbolId, (String, Type, Vec<Type>)>,
    pub extra_functions: Vec<Function>,
    pub labeled_break_targets: HashMap<SymbolId, BlockId>,
    pub labeled_continue_targets: HashMap<SymbolId, BlockId>,
    pub pending_labels: Vec<SymbolId>,
    pub current_cascade_target: Option<Operand>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Inicializador de uma variável/campo declarado no AST.
    ///
    /// O `VariableElement` não guarda a expressão; ela mora no AST da unidade
    /// que declarou a variável (`VariableRef`). Só devolvemos a expressão
    /// quando a unidade é a mesma que este builder está baixando, porque
    /// `lower_expr` recebe o `ast` corrente — uma `ExprId` de outra unidade
    /// indexaria a árvore errada e produziria código silenciosamente errado.
    /// Converte um operando qualquer para i1, para servir de condicao.
    ///
    /// Locais passam por alloca/store/load como i64, entao um bool guardado
    /// numa variavel volta como i64 0/1; comparar com zero recupera o i1 sem
    /// supor nada sobre a largura de origem.
    pub fn para_bool(&mut self, op: Operand) -> Operand {
        if self.operand_type(&op) == Type::I1 {
            return op;
        }
        self.emit(
            Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))),
            Type::I1,
        )
    }

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
    ) -> Operand {
        let lop = self.lower_expr(ast, left);
        let bloco_esq = self.current_block;
        let bloco_dir = self.new_block();
        let bloco_fim = self.new_block();

        match op {
            BinaryOp::IfNull => {
                let e_nulo = self.emit(
                    Instruction::ICmp(
                        ICmpOp::Eq,
                        lop.clone(),
                        Operand::Constant(Constant::Int(0)),
                    ),
                    Type::I1,
                );
                self.terminate(Terminator::CondBranch {
                    cond: e_nulo,
                    then_block: bloco_dir,
                    else_block: bloco_fim,
                });
            }
            BinaryOp::And => {
                let cond = self.para_bool(lop.clone());
                self.terminate(Terminator::CondBranch {
                    cond,
                    then_block: bloco_dir,
                    else_block: bloco_fim,
                });
            }
            _ => {
                let cond = self.para_bool(lop.clone());
                self.terminate(Terminator::CondBranch {
                    cond,
                    then_block: bloco_fim,
                    else_block: bloco_dir,
                });
            }
        }

        self.set_block(bloco_dir);
        let rop = self.lower_expr(ast, right);
        let rop = if op == BinaryOp::IfNull { rop } else { self.para_bool(rop) };
        let bloco_dir_fim = self.current_block;
        let direita_alcanca = !self.is_terminated();
        if direita_alcanca {
            self.terminate(Terminator::Branch(bloco_fim));
        }

        self.set_block(bloco_fim);
        if !direita_alcanca {
            // O lado direito nao volta (lancou ou retornou): o valor que chega
            // ao fim so pode vir da esquerda.
            return match op {
                BinaryOp::IfNull => lop,
                BinaryOp::And => Operand::Constant(Constant::Bool(false)),
                _ => Operand::Constant(Constant::Bool(true)),
            };
        }

        // O valor que vem do lado esquerdo quando ele decide sozinho: em `&&`
        // a esquerda so pula o direito sendo falsa, em `||` sendo verdadeira.
        let (de_esquerda, ty) = match op {
            BinaryOp::IfNull => (lop, self.operand_type(&rop)),
            BinaryOp::And => (Operand::Constant(Constant::Bool(false)), Type::I1),
            _ => (Operand::Constant(Constant::Bool(true)), Type::I1),
        };
        self.emit(
            Instruction::Phi {
                incoming: vec![(bloco_esq, de_esquerda), (bloco_dir_fim, rop)],
                ty,
            },
            ty,
        )
    }

    /// Indice do elemento de funcao de um membro de instancia que de fato
    /// vira simbolo no modulo.
    ///
    /// `lower_program` so emite funcoes cujo `node` e `FunctionRef::Function`.
    /// Acessor implicito de campo (um `final int codigo;` gera um getter
    /// `codigo`) tem `node: FunctionRef::None` e nunca vira simbolo; emitir
    /// `call @df_fn_N_codigo` da "use of undefined value" e o Clang recusa o
    /// modulo inteiro — nao so aquela chamada. Quem procura um metodo por nome
    /// tem de pular esses, e cair no acesso a campo que vem logo depois.
    pub fn metodo_de_instancia(&self, nome: SymbolId) -> Option<usize> {
        self.ctx.program.functions.iter().position(|f| {
            f.class.is_some()
                && !f.static_
                && f.name == nome
                && matches!(f.node, dartforge_elements::model::FunctionRef::Function { .. })
        })
    }

    /// Mesma regra para funcoes de topo.
    pub fn funcao_de_topo(&self, nome: SymbolId) -> Option<usize> {
        self.ctx.program.functions.iter().position(|f| {
            f.class.is_none()
                && f.name == nome
                && matches!(f.node, dartforge_elements::model::FunctionRef::Function { .. })
        })
    }

    pub fn variable_initializer(
        &self,
        var_id: dartforge_elements::model::VariableId,
    ) -> Option<dartforge_frontend::ast::ExprId> {
        use dartforge_elements::model::VariableRef;
        use dartforge_frontend::ast::{DeclKind, MemberKind};
        let v_elem = &self.ctx.program.variables[var_id.0 as usize];
        match v_elem.node {
            VariableRef::Field { unit, member, index } => {
                if unit != self.unit_id {
                    return None;
                }
                let ast = &self.ctx.program.unit(unit).ast;
                match &ast.member(member).kind {
                    MemberKind::Field(list) => list.variables.get(index)?.initializer,
                    _ => None,
                }
            }
            VariableRef::TopLevel { unit, decl, index } => {
                if unit != self.unit_id {
                    return None;
                }
                let ast = &self.ctx.program.unit(unit).ast;
                match &ast.decl(decl).kind {
                    DeclKind::Variables(list) => list.variables.get(index)?.initializer,
                    _ => None,
                }
            }
            _ => None,
        }
    }
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
            exception_targets: Vec::new(),
            finally_scopes: Vec::new(),
            active_catch_stack: Vec::new(),
            terminated_blocks: std::collections::HashSet::new(),
            local_ptrs: HashMap::new(),
            local_functions: HashMap::new(),
            extra_functions: Vec::new(),
            labeled_break_targets: HashMap::new(),
            labeled_continue_targets: HashMap::new(),
            pending_labels: Vec::new(),
            current_cascade_target: None,
        }
    }

    pub fn class_all_fields(&self, cid: dartforge_elements::model::ClassId) -> Vec<dartforge_elements::model::VariableId> {
        let mut fields = Vec::new();
        let class = &self.ctx.program.classes[cid.0 as usize];
        if let Some(sup) = class.supertype_class {
            fields.extend(self.class_all_fields(sup));
        }
        fields.extend(class.fields.iter().copied());
        fields
    }

    pub fn find_field_index(&self, cid: dartforge_elements::model::ClassId, sym: SymbolId) -> Option<usize> {
        let all = self.class_all_fields(cid);
        all.iter().position(|&vid| self.ctx.program.variables[vid.0 as usize].name == sym)
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

    pub fn is_terminated(&self) -> bool {
        self.terminated_blocks.contains(&self.current_block)
    }

    pub fn emit(&mut self, inst: Instruction, ty: Type) -> Operand {
        if self.is_terminated() {
            let dead = self.new_block();
            self.set_block(dead);
        }
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty.clone());
        let idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
        self.func.blocks[idx].instructions.push((vid, inst, ty));
        Operand::Val(vid)
    }

    pub fn terminate(&mut self, term: Terminator) {
        if self.is_terminated() {
            return;
        }
        self.terminated_blocks.insert(self.current_block);
        let idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
        self.func.blocks[idx].terminator = term;
    }

    pub fn default_return_operand(&self) -> Operand {
        match self.func.return_ty {
            Type::I64 => Operand::Constant(Constant::Int(0)),
            Type::I1 | Type::I8 => Operand::Constant(Constant::Bool(false)),
            Type::F64 => Operand::Constant(Constant::Double(0.0)),
            _ => Operand::Constant(Constant::Null),
        }
    }

    pub fn default_return_operand_opt(&self) -> Option<Operand> {
        if self.func.return_ty == Type::Void {
            None
        } else {
            Some(self.default_return_operand())
        }
    }

    pub fn route_return(&mut self, ret_val: Option<Operand>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if self.finally_scopes.is_empty() {
            self.terminate(Terminator::Return(ret_val));
        } else {
            let default_val = self.default_return_operand();
            let val = ret_val.unwrap_or(default_val);
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((self.current_block, 1, val));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        }
        let dead = self.new_block();
        self.set_block(dead);
    }

    pub fn route_break(&mut self) {
        self.route_break_to(None);
    }

    pub fn route_break_to(&mut self, label: Option<SymbolId>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let target_opt = label.and_then(|sym| self.labeled_break_targets.get(&sym).copied())
            .or_else(|| self.break_targets.last().copied());
        if let Some(target) = target_opt {
            if self.finally_scopes.is_empty() {
                self.terminate(Terminator::Branch(target));
            } else {
                let default_ret = self.default_return_operand();
                let fin = self.finally_scopes.last_mut().unwrap();
                fin.incoming.push((self.current_block, 3, default_ret));
                let fin_entry = fin.entry_block;
                self.terminate(Terminator::Branch(fin_entry));
            }
        }
        let dead = self.new_block();
        self.set_block(dead);
    }

    pub fn route_continue(&mut self) {
        self.route_continue_to(None);
    }

    pub fn route_continue_to(&mut self, label: Option<SymbolId>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let target_opt = label.and_then(|sym| self.labeled_continue_targets.get(&sym).copied())
            .or_else(|| self.continue_targets.last().copied());
        if let Some(target) = target_opt {
            if self.finally_scopes.is_empty() {
                self.terminate(Terminator::Branch(target));
            } else {
                let default_ret = self.default_return_operand();
                let fin = self.finally_scopes.last_mut().unwrap();
                fin.incoming.push((self.current_block, 4, default_ret));
                let fin_entry = fin.entry_block;
                self.terminate(Terminator::Branch(fin_entry));
            }
        }
        let dead = self.new_block();
        self.set_block(dead);
    }

    pub fn emit_call_with_check(&mut self, inst: Instruction, ret_ty: Type) -> Operand {
        let res_op = self.emit(inst, ret_ty);
        if self.is_terminated() {
            return res_op;
        }
        let pending = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".to_string(),
                args: Vec::new(),
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let is_exc = self.emit(
            Instruction::ICmp(ICmpOp::Ne, pending, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );

        let cont_b = self.new_block();
        let curr_b = self.current_block;

        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: exc_target,
                else_block: cont_b,
            });
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: fin_entry,
                else_block: cont_b,
            });
        } else {
            let unwind_b = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: unwind_b,
                else_block: cont_b,
            });
            let prev = self.current_block;
            self.set_block(unwind_b);
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
            self.set_block(prev);
        }

        self.set_block(cont_b);
        res_op
    }

    pub fn emit_throw(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let ex_op = self.lower_expr(ast, expr_id);
        self.emit_throw_op(ex_op)
    }

    pub fn emit_throw_op(&mut self, ex_op: Operand) -> Operand {
        let tag = match self.operand_type(&ex_op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::Ref => 3,
            Type::F64 => 4,
            _ => 3,
        };
        let bits_op = match self.operand_type(&ex_op) {
            Type::I1 => self.emit(Instruction::ZExt { op: ex_op, from: Type::I1, to: Type::I8 }, Type::I8),
            _ => ex_op,
        };
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_throw".to_string(),
                args: vec![
                    (bits_op, Type::I64),
                    (Operand::Constant(Constant::Int(tag as i64)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );

        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }

        let dead = self.new_block();
        self.set_block(dead);
        self.default_return_operand()
    }

    pub fn emit_rethrow(&mut self) -> Operand {
        if let Some(&(ref ex_op, tag)) = self.active_catch_stack.last() {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_throw".to_string(),
                    args: vec![
                        (ex_op.clone(), Type::I64),
                        (Operand::Constant(Constant::Int(tag as i64)), Type::I8),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }

        let dead = self.new_block();
        self.set_block(dead);
        self.default_return_operand()
    }

    pub fn lower_type_match(&mut self, ast_ty: &ast::TypeAnnotation, ex_bits: Operand, ex_tag: Operand) -> Operand {
        if let ast::TypeKind::Named { name, .. } = &ast_ty.kind {
            if let Some(first) = name.first() {
                let name_str = self.ctx.symbol_name(first.sym);
                match name_str {
                    "int" => {
                        return self.emit(Instruction::ICmp(ICmpOp::Eq, ex_tag, Operand::Constant(Constant::Int(1))), Type::I1);
                    }
                    "bool" => {
                        return self.emit(Instruction::ICmp(ICmpOp::Eq, ex_tag, Operand::Constant(Constant::Int(2))), Type::I1);
                    }
                    "double" => {
                        return self.emit(Instruction::ICmp(ICmpOp::Eq, ex_tag, Operand::Constant(Constant::Int(4))), Type::I1);
                    }
                    "String" => {
                        let is_ref = self.emit(Instruction::ICmp(ICmpOp::Eq, ex_tag, Operand::Constant(Constant::Int(3))), Type::I1);
                        let cls = self.emit(Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(ex_bits, Type::I64)], ret_ty: Type::I64 }, Type::I64);
                        let is_str_cls = self.emit(Instruction::ICmp(ICmpOp::Eq, cls, Operand::Constant(Constant::Int(-2))), Type::I1);
                        return self.emit(Instruction::And(is_ref, is_str_cls), Type::I1);
                    }
                    "Object" | "dynamic" => {
                        let not_null_tag = self.emit(Instruction::ICmp(ICmpOp::Ne, ex_tag, Operand::Constant(Constant::Int(0))), Type::I1);
                        let not_null_bits = self.emit(Instruction::ICmp(ICmpOp::Ne, ex_bits, Operand::Constant(Constant::Int(0))), Type::I1);
                        return self.emit(Instruction::And(not_null_tag, not_null_bits), Type::I1);
                    }
                    _ => {
                        let target_cid = match name_str {
                            "Exception" => 1000,
                            "FormatException" => 1001,
                            "StateError" => 1002,
                            "ArgumentError" => 1003,
                            "RangeError" => 1004,
                            "UnsupportedError" => 1005,
                            "StackTrace" => 1006,
                            "Error" => 1007,
                            "UnimplementedError" => 1008,
                            "AssertionError" => 1009,
                            "ConcurrentModificationError" => 1010,
                            "TypeError" => 1011,
                            "NoSuchMethodError" => 1012,
                            _ => {
                                self.ctx.program.classes.iter().position(|c| self.ctx.symbol_name(c.name) == name_str)
                                    .map(|idx| (idx + 1) as i64)
                                    .unwrap_or(0)
                            }
                        };
                        let cls = self.emit(Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(ex_bits, Type::I64)], ret_ty: Type::I64 }, Type::I64);
                        let is_sub = self.emit(Instruction::CallRuntime {
                            name: "dartforge_is_subclass".to_string(),
                            args: vec![(cls, Type::I64), (Operand::Constant(Constant::Int(target_cid)), Type::I64)],
                            ret_ty: Type::I8,
                        }, Type::I8);
                        return self.emit(Instruction::ICmp(ICmpOp::Eq, is_sub, Operand::Constant(Constant::Int(1))), Type::I1);
                    }
                }
            }
        }
        self.emit(Instruction::Const(Constant::Bool(true)), Type::I1)
    }

    pub fn emit_trunc_div(&mut self, lop: Operand, rop: Operand) -> Operand {
        let is_zero = self.emit(
            Instruction::ICmp(ICmpOp::Eq, rop.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let div_zero_block = self.new_block();
        let normal_div_block = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: is_zero,
            then_block: div_zero_block,
            else_block: normal_div_block,
        });

        self.set_block(div_zero_block);
        let msg = self.emit(Instruction::Const(Constant::String("IntegerDivisionByZeroException".to_string())), Type::Ref);
        let err_obj = self.emit(
            Instruction::AllocObject {
                class_id: 1005, // UnsupportedError
                fields: vec![msg],
            },
            Type::Ref,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_throw".to_string(),
                args: vec![
                    (err_obj, Type::I64),
                    (Operand::Constant(Constant::Int(3)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }

        self.set_block(normal_div_block);
        self.emit(Instruction::SDiv(lop, rop), Type::I64)
    }

    pub fn lower_binary_op_helper(&mut self, op: BinaryOp, lop: Operand, rop: Operand) -> Operand {
        let lop_ty = self.operand_type(&lop);
        let rop_ty = self.operand_type(&rop);
        let is_float = lop_ty == Type::F64 || rop_ty == Type::F64;
        let is_string = lop_ty == Type::Ref || rop_ty == Type::Ref;

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
                } else if is_float {
                    self.emit(Instruction::FAdd(lop, rop), Type::F64)
                } else {
                    self.emit(Instruction::Add(lop, rop), Type::I64)
                }
            }
            BinaryOp::Sub => {
                if is_float {
                    self.emit(Instruction::FSub(lop, rop), Type::F64)
                } else {
                    self.emit(Instruction::Sub(lop, rop), Type::I64)
                }
            }
            BinaryOp::Mul => {
                if is_float {
                    self.emit(Instruction::FMul(lop, rop), Type::F64)
                } else {
                    self.emit(Instruction::Mul(lop, rop), Type::I64)
                }
            }
            BinaryOp::Div => {
                self.emit(Instruction::FDiv(lop, rop), Type::F64)
            }
            BinaryOp::TruncDiv => {
                self.emit_trunc_div(lop, rop)
            }
            BinaryOp::Rem => {
                self.emit(Instruction::SRem(lop, rop), Type::I64)
            }
            BinaryOp::Shl => {
                self.emit(Instruction::Shl(lop, rop), Type::I64)
            }
            BinaryOp::Shr => {
                self.emit(Instruction::AShr(lop, rop), Type::I64)
            }
            BinaryOp::BitAnd => {
                self.emit(Instruction::And(lop, rop), Type::I64)
            }
            BinaryOp::BitOr => {
                self.emit(Instruction::Or(lop, rop), Type::I64)
            }
            BinaryOp::BitXor => {
                self.emit(Instruction::Xor(lop, rop), Type::I64)
            }
            _ => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
        }
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
                let var_ty_opt = var_list.ty;
                for var in &var_list.variables {
                    let sym = var.name.sym;
                    let init_op = if let Some(init_id) = var.initializer {
                        let op = self.lower_expr(ast, init_id);
                        if let Some(tid) = var_ty_opt {
                            let ast_ty = ast.ty(tid);
                            if let ast::TypeKind::Named { name, .. } = &ast_ty.kind {
                                if let Some(first) = name.first() {
                                    let ty_name = self.ctx.symbol_name(first.sym);
                                    if ty_name != "dynamic" && ty_name != "var" && ty_name != "Object" {
                                        let tag = self.operand_tag(&op);
                                        let is_m = self.lower_type_match(ast_ty, op.clone(), Operand::Constant(Constant::Int(tag as i64)));
                                        let fail_b = self.new_block();
                                        let pass_b = self.new_block();
                                        self.terminate(Terminator::CondBranch {
                                            cond: is_m,
                                            then_block: pass_b,
                                            else_block: fail_b,
                                        });
                                        self.set_block(fail_b);
                                        let err_op = self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_type_error_new".to_string(),
                                                args: Vec::new(),
                                                ret_ty: Type::Ref,
                                            },
                                            Type::Ref,
                                        );
                                        self.emit_throw_op(err_op);
                                        self.set_block(pass_b);
                                    }
                                }
                            }
                        }
                        op
                    } else {
                        self.emit(Instruction::Const(Constant::Null), Type::Ref)
                    };
                    let ptr = self.emit(Instruction::Alloca(Type::I64), Type::Ref);
                    self.emit(Instruction::Store { ptr: ptr.clone(), val: init_op.clone() }, Type::Void);
                    self.local_ptrs.insert(sym, ptr);
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

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_header);
                }

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

                let attached_labels = std::mem::take(&mut self.pending_labels);
                for &lbl in &attached_labels {
                    self.labeled_break_targets.insert(lbl, exit_block);
                    self.labeled_continue_targets.insert(lbl, loop_update);
                }

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

                for &lbl in &attached_labels {
                    self.labeled_break_targets.remove(&lbl);
                    self.labeled_continue_targets.remove(&lbl);
                }

                self.set_block(exit_block);
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

                let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
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
                let f = self.ctx.program.unit(self.unit_id).ast.function(*fid);
                if let Some(fn_name) = f.name {
                    let sym = fn_name.sym;
                    let local_sym_name = format!("{}_local_{}", self.func.symbol, fid.0);
                    let name_str = self.ctx.symbol_name(sym).to_string();
                    let ret_ty = Type::Ref;

                    let mut b = FnBuilder::new(
                        self.ctx,
                        self.unit_id,
                        local_sym_name.clone(),
                        name_str,
                        ret_ty,
                    );
                    b.local_functions = self.local_functions.clone();

                    if let Some(params) = &f.parameters {
                        for p in params.iter() {
                            let p_name = p.name.map(|n| self.ctx.symbol_name(n.sym).to_string()).unwrap_or_else(|| "arg".to_string());
                            let vid = b.add_param(p_name, Type::Ref);
                            if let Some(n) = p.name {
                                b.named_locals.insert(n.sym, Operand::Val(vid));
                            }
                        }
                    }

                    match &f.body {
                        FunctionBody::Block(stmt_id) => {
                            b.lower_stmt(ast, *stmt_id);
                        }
                        FunctionBody::Expression(expr_id) => {
                            let r = b.lower_expr(ast, *expr_id);
                            b.terminate(Terminator::Return(Some(r)));
                        }
                        _ => {}
                    }

                    self.local_functions.insert(sym, (local_sym_name, ret_ty, Vec::new()));
                    self.extra_functions.push(b.func);
                    self.extra_functions.extend(b.extra_functions);
                }
            }
            StmtKind::Try { body, catches, finally_ } => {
                self.lower_try_stmt(ast, *body, catches, *finally_);
            }
            StmtKind::Labeled { labels, body } => {
                for l in labels.iter() {
                    self.pending_labels.push(l.sym);
                }
                self.lower_stmt(ast, *body);
            }
            StmtKind::Empty => {}
            _ => {}
        }
    }

    fn lower_try_stmt(
        &mut self,
        ast: &ast::Ast,
        body: StmtId,
        catches: &[ast::CatchClause],
        finally_: Option<StmtId>,
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
            let ret_ty = if self.func.return_ty == Type::Void { Type::Ref } else { self.func.return_ty };
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
            });
        }

        let try_exc_target = if let Some(cb) = catch_dispatch_block {
            cb
        } else if let Some((entry, _, _, _)) = fin_info {
            entry
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
        self.lower_stmt(ast, body);

        if !self.is_terminated() {
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope.incoming.push((self.current_block, 0, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else {
                self.terminate(Terminator::Branch(merge_block));
            }
        }

        self.exception_targets.pop();

        if let Some(dispatch_block) = catch_dispatch_block {
            self.set_block(dispatch_block);
            let ex_bits = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_peek_bits".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            let ex_tag = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_exception_peek_tag".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::I8,
                },
                Type::I8,
            );

            let mut current_test_block = dispatch_block;

            for clause in catches.iter() {
                self.set_block(current_test_block);
                let body_b = self.new_block();
                let next_test_b = self.new_block();

                if let Some(on_tid) = clause.on_type {
                    let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(on_tid);
                    let is_match = self.lower_type_match(ast_ty, ex_bits.clone(), ex_tag.clone());
                    self.terminate(Terminator::CondBranch {
                        cond: is_match,
                        then_block: body_b,
                        else_block: next_test_b,
                    });
                } else {
                    self.terminate(Terminator::Branch(body_b));
                }

                self.set_block(body_b);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_clear".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );

                if let Some(ex_name) = &clause.exception {
                    let ptr = self.emit(Instruction::Alloca(Type::I64), Type::Ref);
                    self.emit(Instruction::Store { ptr: ptr.clone(), val: ex_bits.clone() }, Type::Void);
                    self.local_ptrs.insert(ex_name.sym, ptr);
                    self.named_locals.insert(ex_name.sym, ex_bits.clone());
                }
                if let Some(st_name) = &clause.stack_trace {
                    let st_val = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_stack_trace_get".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    let ptr = self.emit(Instruction::Alloca(Type::I64), Type::Ref);
                    self.emit(Instruction::Store { ptr: ptr.clone(), val: st_val.clone() }, Type::Void);
                    self.local_ptrs.insert(st_name.sym, ptr);
                    self.named_locals.insert(st_name.sym, st_val);
                }

                self.active_catch_stack.push((ex_bits.clone(), 3));
                self.lower_stmt(ast, clause.body);
                self.active_catch_stack.pop();

                if !self.is_terminated() {
                    if let Some((entry, _, _, _)) = fin_info {
                        let default_ret = self.default_return_operand();
                        let last_scope = self.finally_scopes.last_mut().unwrap();
                        last_scope.incoming.push((self.current_block, 0, default_ret));
                        self.terminate(Terminator::Branch(entry));
                    } else {
                        self.terminate(Terminator::Branch(merge_block));
                    }
                }

                current_test_block = next_test_b;
            }

            self.set_block(current_test_block);
            if let Some((entry, _, _, _)) = fin_info {
                let default_ret = self.default_return_operand();
                let last_scope = self.finally_scopes.last_mut().unwrap();
                last_scope.incoming.push((self.current_block, 2, default_ret));
                self.terminate(Terminator::Branch(entry));
            } else if let Some(&parent_target) = self.exception_targets.last() {
                self.terminate(Terminator::Branch(parent_target));
            } else {
                self.terminate(Terminator::Return(self.default_return_operand_opt()));
            }
        }

        if let Some(fin_stmt) = finally_ {
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
                Instruction::Phi { incoming: reason_incoming, ty: Type::I64 },
                Type::I64,
            ));
            let ret_ty = if self.func.return_ty == Type::Void { Type::Ref } else { self.func.return_ty };
            self.func.blocks[header_idx].instructions.push((
                ret_val_phi,
                Instruction::Phi { incoming: ret_val_incoming, ty: ret_ty },
                ret_ty,
            ));

            self.lower_stmt(ast, fin_stmt);

            if !self.is_terminated() {
                let b_norm = merge_block;
                let b_ret = self.new_block();
                let b_exc = self.new_block();
                let b_brk = self.new_block();
                let b_cont = self.new_block();

                self.terminate(Terminator::Switch {
                    val: Operand::Val(reason_phi),
                    default: b_norm,
                    cases: vec![
                        (0, b_norm),
                        (1, b_ret),
                        (2, b_exc),
                        (3, b_brk),
                        (4, b_cont),
                    ],
                });

                self.set_block(b_ret);
                let ret_op = if self.func.return_ty == Type::Void {
                    None
                } else {
                    Some(Operand::Val(ret_val_phi))
                };
                self.route_return(ret_op);

                self.set_block(b_exc);
                if !self.finally_scopes.is_empty() {
                    let default_ret = self.default_return_operand();
                    let parent_fin = self.finally_scopes.last_mut().unwrap();
                    parent_fin.incoming.push((b_exc, 2, default_ret));
                    let p_entry = parent_fin.entry_block;
                    self.terminate(Terminator::Branch(p_entry));
                } else if let Some(&parent_target) = self.exception_targets.last() {
                    self.terminate(Terminator::Branch(parent_target));
                } else {
                    self.terminate(Terminator::Return(self.default_return_operand_opt()));
                }

                self.set_block(b_brk);
                self.route_break();

                self.set_block(b_cont);
                self.route_continue();
            }
        }

        self.set_block(merge_block);
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
                                        let target_ty = self.ctx.get_type(self.unit_id, *idx_target);
                                        let is_string_or_match = target_ty.map_or(false, |t| self.ctx.is_string(t))
                                            || matches!(ast.expr(*idx_target).kind, ExprKind::String(_))
                                            || (if let ExprKind::Identifier(id) = &ast.expr(*idx_target).kind {
                                                self.ctx.symbol_name(id.sym) == "m"
                                            } else { false });
                                        if is_string_or_match {
                                            self.emit(
                                                Instruction::CallRuntime {
                                                    name: "dartforge_list_get_bits".to_string(),
                                                    args: vec![(t_op, Type::Ref), (i_op, Type::I64)],
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
                if let Some(ptr) = self.local_ptrs.get(&sym) {
                    return self.emit(Instruction::Load { ptr: ptr.clone(), ty: Type::I64 }, Type::I64);
                }
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
                let id_name = self.ctx.symbol_name(sym);
                if id_name == "String" {
                    return Operand::Constant(Constant::Int(-2));
                } else if id_name == "Error" {
                    return Operand::Constant(Constant::Int(1007));
                } else if let Some(idx) = self.ctx.program.classes.iter().position(|c| self.ctx.symbol_name(c.name) == id_name) {
                    let class_id = (idx + 1) as i64;
                    return Operand::Constant(Constant::Int(class_id));
                }
                self.emit(Instruction::Const(Constant::Int(0)), Type::I64)
            }
            ExprKind::Parenthesized(sub) => self.lower_expr(ast, *sub),
            ExprKind::Binary { op, left, right } => {
                // Curto-circuito vem antes do lowering dos dois lados: && e ||
                // nao podem avaliar a direita quando a esquerda ja decide, e ??
                // so avalia a direita quando a esquerda e nula. O caminho
                // aritmetico abaixo avalia os dois de uma vez, entao esses tres
                // nao podem passar por ele.
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull) {
                    return self.lower_curto_circuito(ast, *op, *left, *right);
                }
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
                        let lop_is_ref = self.operand_type(&lop) == Type::Ref;
                        if is_string || lop_is_ref {
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
                    BinaryOp::TruncDiv => self.emit_trunc_div(lop, rop),
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
                    // BinaryOp::And, Or e IfNull foram desviados no inicio deste
                    // arm, para lower_curto_circuito.
                    _ => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                }
            }
            ExprKind::Unary { op, operand } => {
                match op {
                    UnaryOp::PrefixInc | UnaryOp::PostfixInc | UnaryOp::PrefixDec | UnaryOp::PostfixDec => {
                        if let ExprKind::Identifier(id) = &ast.expr(*operand).kind {
                            let curr = if let Some(ptr) = self.local_ptrs.get(&id.sym) {
                                self.emit(Instruction::Load { ptr: ptr.clone(), ty: Type::I64 }, Type::I64)
                            } else if let Some(op) = self.named_locals.get(&id.sym) {
                                op.clone()
                            } else {
                                Operand::Constant(Constant::Int(0))
                            };
                            let delta = Operand::Constant(Constant::Int(1));
                            let next = match op {
                                UnaryOp::PrefixInc | UnaryOp::PostfixInc => {
                                    self.emit(Instruction::Add(curr.clone(), delta), Type::I64)
                                }
                                _ => {
                                    self.emit(Instruction::Sub(curr.clone(), delta), Type::I64)
                                }
                            };
                            if let Some(ptr) = self.local_ptrs.get(&id.sym).cloned() {
                                self.emit(Instruction::Store { ptr, val: next.clone() }, Type::Void);
                            }
                            self.named_locals.insert(id.sym, next.clone());
                            match op {
                                UnaryOp::PrefixInc | UnaryOp::PrefixDec => return next,
                                _ => return curr,
                            }
                        }
                    }
                    _ => {}
                }
                if *op == UnaryOp::NullAssert {
                    let sub_op = self.lower_expr(ast, *operand);
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
                let then_reaches = !self.is_terminated();
                if then_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }

                self.set_block(else_block);
                let else_op = self.lower_expr(ast, *else_);
                let else_end = self.current_block;
                let else_reaches = !self.is_terminated();
                if else_reaches {
                    self.terminate(Terminator::Branch(merge_block));
                }

                self.set_block(merge_block);
                if then_reaches && else_reaches {
                    let ty = self.operand_type(&then_op);
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
            ExprKind::Property { target, name, .. } => {
                let prop_name = self.ctx.symbol_name(name.sym);

                if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                    if self.ctx.symbol_name(id.sym) == "StackTrace" {
                        if prop_name == "current" {
                            return self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_stack_trace_get".to_string(),
                                    args: Vec::new(),
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            );
                        } else if prop_name == "empty" {
                            return self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_stack_trace_empty".to_string(),
                                    args: Vec::new(),
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            );
                        }
                    }
                }

                let target_op = self.lower_expr(ast, *target);

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
                            let curr_val = self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_object_get".to_string(),
                                    args: vec![
                                        (target_op.clone(), Type::Ref),
                                        (Operand::Constant(Constant::Int(field_idx as i64)), Type::I64),
                                    ],
                                    ret_ty: Type::I64,
                                },
                                Type::I64,
                            );
                            if let MemberRef::Variable(var_id) = member {
                                let v_elem = &self.ctx.program.variables[var_id.0 as usize];
                                if v_elem.late {
                                    if let Some(init_id) = self.variable_initializer(*var_id) {
                                        let is_null = self.emit(
                                            Instruction::ICmp(
                                                ICmpOp::Eq,
                                                curr_val.clone(),
                                                Operand::Constant(Constant::Int(0)),
                                            ),
                                            Type::I1,
                                        );
                                        let init_b = self.new_block();
                                        let done_b = self.new_block();
                                        let curr_b = self.current_block;
                                        self.terminate(Terminator::CondBranch {
                                            cond: is_null,
                                            then_block: init_b,
                                            else_block: done_b,
                                        });
                                        self.set_block(init_b);
                                        let init_val = self.lower_expr(ast, init_id);
                                        let is_ref = if self.operand_type(&init_val) == Type::Ref { 1 } else { 0 };
                                        self.emit(
                                            Instruction::CallRuntime {
                                                name: "dartforge_object_set".to_string(),
                                                args: vec![
                                                    (target_op, Type::Ref),
                                                    (Operand::Constant(Constant::Int(field_idx as i64)), Type::I64),
                                                    (init_val.clone(), Type::I64),
                                                    (Operand::Constant(Constant::Int(is_ref)), Type::I8),
                                                ],
                                                ret_ty: Type::Void,
                                            },
                                            Type::Void,
                                        );
                                        let init_end = self.current_block;
                                        self.terminate(Terminator::Branch(done_b));
                                        self.set_block(done_b);
                                        return self.emit(
                                            Instruction::Phi {
                                                incoming: vec![(curr_b, curr_val), (init_end, init_val)],
                                                ty: Type::I64,
                                            },
                                            Type::I64,
                                        );
                                    }
                                }
                            }
                            return curr_val;
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
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_list_last".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "first" {
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_list_first".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "single" {
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_list_single".to_string(),
                            args: vec![(target_op, Type::Ref)],
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
                } else if prop_name == "isOdd" {
                    let rem = self.emit(
                        Instruction::And(target_op, Operand::Constant(Constant::Int(1))),
                        Type::I64,
                    );
                    self.emit(
                        Instruction::ICmp(ICmpOp::Ne, rem, Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    )
                } else if prop_name == "isEven" {
                    let rem = self.emit(
                        Instruction::And(target_op, Operand::Constant(Constant::Int(1))),
                        Type::I64,
                    );
                    self.emit(
                        Instruction::ICmp(ICmpOp::Eq, rem, Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    )
                } else if prop_name == "message" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_message".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "name" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_name".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "invalidValue" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_invalid_value".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "start" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_start".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "end" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_end".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "source" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_source".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "offset" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_offset".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "stackTrace" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_error_get_stack_trace".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else if prop_name == "runtimeType" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_value_class".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    )
                } else if prop_name == "keys" {
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_map_keys".to_string(),
                            args: vec![(target_op, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    )
                } else {
                    if let Some(fid) = self.metodo_de_instancia(name.sym) {
                        let f_elem = &self.ctx.program.functions[fid];
                        let sanitized_name = crate::lower::sanitize_symbol(self.ctx.symbol_name(f_elem.name));
                        let symbol = format!("df_fn_{fid}_{sanitized_name}");
                        let ret_ty = self.ctx.outline.functions.get(fid)
                            .map(|fd| self.ctx.to_hir_type(fd.return_type))
                            .unwrap_or(Type::Ref);
                        return self.emit_call_with_check(
                            Instruction::CallStatic {
                                symbol,
                                args: vec![target_op],
                                ret_ty,
                            },
                            ret_ty,
                        );
                    }
                    let mut found_field = None;
                    for (c_idx, _) in self.ctx.program.classes.iter().enumerate() {
                        let cid = dartforge_elements::model::ClassId(c_idx as u32);
                        if let Some(f_idx) = self.find_field_index(cid, name.sym) {
                            found_field = Some(f_idx);
                            break;
                        }
                    }
                    if let Some(f_idx) = found_field {
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_object_get".to_string(),
                                args: vec![
                                    (target_op, Type::Ref),
                                    (Operand::Constant(Constant::Int(f_idx as i64)), Type::I64),
                                ],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                    }
                    // Getter inexistente em dynamic: lança NoSuchMethodError
                    let err_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_no_such_method_error_new".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );
                    self.emit_throw_op(err_op);
                    self.default_return_operand()
                }
            }
            ExprKind::Index { target, index, .. } => {
                let target_op = self.lower_expr(ast, *target);
                let idx_op = self.lower_expr(ast, *index);
                if let Some(fid) = self.ctx.program.functions.iter().position(|f| {
                    f.class.is_some()
                        && !f.static_
                        && self.ctx.symbol_name(f.name) == "[]"
                        && matches!(f.node, dartforge_elements::model::FunctionRef::Function { .. })
                }) {
                    let f_elem = &self.ctx.program.functions[fid];
                    let sanitized_name = crate::lower::sanitize_symbol(self.ctx.symbol_name(f_elem.name));
                    let symbol = format!("df_fn_{fid}_{sanitized_name}");
                    let ret_ty = self.ctx.outline.functions.get(fid)
                        .map(|fd| self.ctx.to_hir_type(fd.return_type))
                        .unwrap_or(Type::Ref);
                    let target_ty = self.ctx.get_type(self.unit_id, *target);
                    let is_std_map = target_ty.map_or(false, |t| self.ctx.is_map(t));
                    let is_std_list = target_ty.map_or(false, |t| self.ctx.is_list(t));
                    if !is_std_map && !is_std_list && !matches!(ast.expr(*target).kind, ExprKind::List { .. } | ExprKind::SetOrMap { .. }) {
                        return self.emit_call_with_check(
                            Instruction::CallStatic {
                                symbol,
                                args: vec![target_op, idx_op],
                                ret_ty,
                            },
                            ret_ty,
                        );
                    }
                }
                let target_ty = self.ctx.get_type(self.unit_id, *target);
                let is_map = target_ty.map_or(false, |t| self.ctx.is_map(t))
                    || self.operand_type(&idx_op) == Type::Ref;
                let is_string_or_match = target_ty.map_or(false, |t| self.ctx.is_string(t))
                    || matches!(ast.expr(*target).kind, ExprKind::String(_))
                    || (if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                        self.ctx.symbol_name(id.sym) == "m"
                    } else { false });
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
                    self.emit_call_with_check(
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
                            // O tipo ESTATICO do argumento manda, e so depois o
                            // tipo do operando. Um local passa por alloca/store/
                            // load como i64, entao `bool x = a < b; print(x)`
                            // chegava aqui como i64 e imprimia 1 em vez de true.
                            let ty_estatico = self.ctx.get_type(self.unit_id, first_arg.value);
                            let e_bool = ty_estatico.map_or(false, |t| self.ctx.is_bool(t));
                            let e_int = ty_estatico.map_or(false, |t| self.ctx.is_int(t));
                            let e_double = ty_estatico.map_or(false, |t| self.ctx.is_double(t));
                            let op_ty = self.operand_type(&arg_op);
                            let op_ty = if e_bool {
                                if op_ty == Type::I1 { Type::I1 } else { Type::I8 }
                            } else if e_int {
                                Type::I64
                            } else if e_double {
                                Type::F64
                            } else {
                                op_ty
                            };

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
                    } else if id_str == "RegExp" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_regexp_new".to_string(),
                                args: vec![(pat_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if id_str == "identical" && arguments.args.len() == 2 {
                        let a_op = self.lower_expr(ast, arguments.args[0].value);
                        let b_op = self.lower_expr(ast, arguments.args[1].value);
                        return self.emit(
                            Instruction::ICmp(ICmpOp::Eq, a_op, b_op),
                            Type::I1,
                        );
                    } else if let Some(err_cid) = match id_str {
                        "Exception" => Some(1000),
                        "FormatException" => Some(1001),
                        "StateError" => Some(1002),
                        "ArgumentError" => Some(1003),
                        "RangeError" => Some(1004),
                        "UnsupportedError" => Some(1005),
                        "UnimplementedError" => Some(1008),
                        _ => None,
                    } {
                        let msg_op = if let Some(first_arg) = arguments.args.first() {
                            self.lower_expr(ast, first_arg.value)
                        } else {
                            self.emit(Instruction::Const(Constant::String("".to_string())), Type::Ref)
                        };
                        return self.emit(
                            Instruction::AllocObject {
                                class_id: err_cid,
                                fields: vec![msg_op],
                            },
                            Type::Ref,
                        );
                    } else if let Some((sym, ret_ty, _)) = self.local_functions.get(&id.sym).cloned() {
                        let mut args_ops = Vec::new();
                        for a in &arguments.args {
                            args_ops.push(self.lower_expr(ast, a.value));
                        }
                        return self.emit_call_with_check(
                            Instruction::CallStatic {
                                symbol: sym,
                                args: args_ops,
                                ret_ty,
                            },
                            ret_ty,
                        );
                    } else if let Some(fid) = self.funcao_de_topo(id.sym) {
                        let f_elem = &self.ctx.program.functions[fid];
                        let sanitized_name = crate::lower::sanitize_symbol(self.ctx.symbol_name(f_elem.name));
                        let symbol = format!("df_fn_{fid}_{sanitized_name}");
                        let ret_ty = self.ctx.outline.functions.get(fid)
                            .map(|fd| self.ctx.to_hir_type(fd.return_type))
                            .unwrap_or(Type::Ref);
                        let mut args_ops = Vec::new();
                        for a in &arguments.args {
                            args_ops.push(self.lower_expr(ast, a.value));
                        }
                        return self.emit_call_with_check(
                            Instruction::CallStatic {
                                symbol,
                                args: args_ops,
                                ret_ty,
                            },
                            ret_ty,
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

                    if m_name == "add" {
                        if let Some(first_arg) = arguments.args.first() {
                            let val_op = self.lower_expr(ast, first_arg.value);
                            let tag = self.operand_tag(&val_op);
                            return self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_list_push".to_string(),
                                    args: vec![
                                        (recv_op, Type::Ref),
                                        (val_op, Type::I64),
                                        (Operand::Constant(Constant::Int(tag as i64)), Type::I8),
                                    ],
                                    ret_ty: Type::Void,
                                },
                                Type::Void,
                            );
                        }
                    } else if m_name == "toUpperCase" {
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
                        let start_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            Operand::Constant(Constant::Int(-1))
                        };
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_last_index_of".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref), (start_op, Type::I64)],
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
                        let start_op = if arguments.args.len() > 1 {
                            self.lower_expr(ast, arguments.args[1].value)
                        } else {
                            Operand::Constant(Constant::Int(0))
                        };
                        let c_i8 = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_contains".to_string(),
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
                    } else if m_name == "characters" {
                        let empty_str = self.emit(Instruction::Const(Constant::String("".to_string())), Type::Ref);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_split".to_string(),
                                args: vec![(recv_op, Type::Ref), (empty_str, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                    } else if m_name == "splitMapJoin" {
                        let pat_op = self.lower_expr(ast, arguments.args[0].value);
                        let on_match_arg = arguments.args.iter().find(|a| a.name.map_or(false, |n| self.ctx.symbol_name(n.sym) == "onMatch"));
                        let on_non_match_arg = arguments.args.iter().find(|a| a.name.map_or(false, |n| self.ctx.symbol_name(n.sym) == "onNonMatch"));

                        let pieces_list = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_split_map_pieces".to_string(),
                                args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );

                        let res_buf = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_buffer_new".to_string(),
                                args: Vec::new(),
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );

                        let total_len = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_list_len".to_string(),
                                args: vec![(pieces_list.clone(), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );

                        let loop_header = self.new_block();
                        let loop_body = self.new_block();
                        let loop_update = self.new_block();
                        let exit_block = self.new_block();

                        let entry_block = self.current_block;
                        self.terminate(Terminator::Branch(loop_header));

                        self.set_block(loop_header);
                        let phi_vid = ValueId(self.next_value);
                        self.next_value += 1;
                        let phi_op = Operand::Val(phi_vid);
                        self.value_types.insert(phi_vid, Type::I64);

                        let header_idx = self.func.blocks.iter().position(|b| b.id == self.current_block).unwrap();
                        let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
                        self.func.blocks[header_idx].instructions.push((
                            phi_vid,
                            Instruction::Phi { incoming: Vec::new(), ty: Type::I64 },
                            Type::I64,
                        ));

                        let cmp = self.emit(Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), total_len), Type::I1);
                        self.terminate(Terminator::CondBranch {
                            cond: cmp,
                            then_block: loop_body,
                            else_block: exit_block,
                        });

                        self.set_block(loop_body);
                        let is_match_bits = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_list_get_bits".to_string(),
                                args: vec![(pieces_list.clone(), Type::Ref), (phi_op.clone(), Type::I64)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        );
                        let is_match_cond = self.emit(Instruction::ICmp(ICmpOp::Ne, is_match_bits, Operand::Constant(Constant::Int(0))), Type::I1);

                        let part_idx = self.emit(Instruction::Add(phi_op.clone(), Operand::Constant(Constant::Int(1))), Type::I64);
                        let part_val = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_list_get_bits".to_string(),
                                args: vec![(pieces_list.clone(), Type::Ref), (part_idx, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );

                        let match_block = self.new_block();
                        let non_match_block = self.new_block();
                        let write_block = self.new_block();

                        self.terminate(Terminator::CondBranch {
                            cond: is_match_cond,
                            then_block: match_block,
                            else_block: non_match_block,
                        });

                        // Match block
                        self.set_block(match_block);
                        let match_res = if let Some(arg) = on_match_arg {
                            if let ExprKind::FunctionExpression(func_id) = ast.expr(arg.value).kind {
                                let closure = &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                                let param_sym = closure.parameters.as_ref()
                                    .and_then(|p| p.first())
                                    .and_then(|p| p.name.as_ref())
                                    .map(|n| n.sym);
                                if let Some(sym) = param_sym {
                                    self.named_locals.insert(sym, part_val.clone());
                                }
                                match &closure.body {
                                    FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                                    _ => part_val.clone(),
                                }
                            } else {
                                part_val.clone()
                            }
                        } else {
                            part_val.clone()
                        };
                        let match_end_block = self.current_block;
                        self.terminate(Terminator::Branch(write_block));

                        // Non-match block
                        self.set_block(non_match_block);
                        let non_match_res = if let Some(arg) = on_non_match_arg {
                            if let ExprKind::FunctionExpression(func_id) = ast.expr(arg.value).kind {
                                let closure = &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                                let param_sym = closure.parameters.as_ref()
                                    .and_then(|p| p.first())
                                    .and_then(|p| p.name.as_ref())
                                    .map(|n| n.sym);
                                if let Some(sym) = param_sym {
                                    self.named_locals.insert(sym, part_val.clone());
                                }
                                match &closure.body {
                                    FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                                    _ => part_val.clone(),
                                }
                            } else {
                                part_val.clone()
                            }
                        } else {
                            part_val.clone()
                        };
                        let non_match_end_block = self.current_block;
                        self.terminate(Terminator::Branch(write_block));

                        // Write block
                        self.set_block(write_block);
                        let piece_res = self.emit(
                            Instruction::Phi {
                                incoming: vec![(match_end_block, match_res), (non_match_end_block, non_match_res)],
                                ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                        self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_buffer_write".to_string(),
                                args: vec![(res_buf.clone(), Type::Ref), (piece_res, Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        );
                        self.terminate(Terminator::Branch(loop_update));

                        // Update block
                        self.set_block(loop_update);
                        let next_i = self.emit(Instruction::Add(phi_op.clone(), Operand::Constant(Constant::Int(2))), Type::I64);
                        let update_block_id = self.current_block;
                        self.terminate(Terminator::Branch(loop_header));

                        let header_idx = self.func.blocks.iter().position(|b| b.id == loop_header).unwrap();
                        self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                            incoming: vec![
                                (entry_block, Operand::Constant(Constant::Int(0))),
                                (update_block_id, next_i),
                            ],
                            ty: Type::I64,
                        };

                        self.set_block(exit_block);
                        let final_str = self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_to_string_handle".to_string(),
                                args: vec![(res_buf, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        );
                        return final_str;
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
            ExprKind::Throw(inner) => {
                self.emit_throw(ast, *inner)
            }
            ExprKind::Rethrow => {
                self.emit_rethrow()
            }
            ExprKind::Is { value, ty, negated } => {
                let val_op = self.lower_expr(ast, *value);
                let tag = self.operand_tag(&val_op);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                let is_m = self.lower_type_match(ast_ty, val_op, Operand::Constant(Constant::Int(tag as i64)));
                if *negated {
                    self.emit(Instruction::LNot(is_m), Type::I1)
                } else {
                    is_m
                }
            }
            ExprKind::As { value, ty } => {
                let val_op = self.lower_expr(ast, *value);
                let tag = self.operand_tag(&val_op);
                let ast_ty = self.ctx.program.unit(self.unit_id).ast.ty(*ty);
                let is_m = self.lower_type_match(ast_ty, val_op.clone(), Operand::Constant(Constant::Int(tag as i64)));
                let fail_block = self.new_block();
                let pass_block = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: is_m,
                    then_block: pass_block,
                    else_block: fail_block,
                });
                self.set_block(fail_block);
                let err_op = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_type_error_new".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                self.emit_throw_op(err_op);
                self.set_block(pass_block);
                val_op
            }
            ExprKind::Assign { op, target, value } => {
                let val_op = self.lower_expr(ast, *value);
                if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
                    let final_val = match op {
                        ast::AssignOp::Assign => val_op,
                        ast::AssignOp::Compound(bin_op) => {
                            let curr = if let Some(ptr) = self.local_ptrs.get(&id.sym) {
                                self.emit(Instruction::Load { ptr: ptr.clone(), ty: Type::I64 }, Type::I64)
                            } else if let Some(op) = self.named_locals.get(&id.sym) {
                                op.clone()
                            } else {
                                Operand::Constant(Constant::Int(0))
                            };
                            self.lower_binary_op_helper(*bin_op, curr, val_op)
                        }
                    };
                    if let Some(ptr) = self.local_ptrs.get(&id.sym).cloned() {
                        self.emit(Instruction::Store { ptr, val: final_val.clone() }, Type::Void);
                    }
                    self.named_locals.insert(id.sym, final_val.clone());
                    return final_val;
                }
                val_op
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

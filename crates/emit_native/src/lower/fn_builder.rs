//! Construtor de funções HIR durante o lowering.

use crate::context::Context;
use crate::hir::*;
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
    /// Escopos léxicos dos locais, do mais externo (parâmetros) ao corrente (R6).
    pub escopos: Vec<HashMap<SymbolId, super::locais::Local>>,
    /// Quantos `alloca` já estão no começo do bloco de entrada.
    pub n_allocas: usize,
    pub value_types: HashMap<ValueId, Type>,
    pub break_targets: Vec<BlockId>,
    pub continue_targets: Vec<BlockId>,
    pub this_param: Option<Operand>,
    pub enclosing_class: Option<dartforge_elements::model::ClassId>,
    pub exception_targets: Vec<BlockId>,
    pub finally_scopes: Vec<FinallyScope>,
    pub active_catch_stack: Vec<(Operand, u8)>,
    pub terminated_blocks: std::collections::HashSet<BlockId>,
    pub local_functions: HashMap<SymbolId, (String, Type, Vec<Type>)>,
    pub extra_functions: Vec<Function>,
    pub labeled_break_targets: HashMap<SymbolId, BlockId>,
    pub labeled_continue_targets: HashMap<SymbolId, BlockId>,
    pub pending_labels: Vec<SymbolId>,
    pub current_cascade_target: Option<Operand>,
    /// Diagnósticos de construto não suportado (N1).
    pub erros: Vec<String>,
    /// Cadeia `?.` em curso: bloco de saída com null e as entradas do phi
    /// (N3). `None` fora de cadeia.
    pub cadeia_nula: Option<(BlockId, Vec<(BlockId, Operand)>)>,
    /// O próximo `lower_expr` é o alvo de um elo da cadeia corrente.
    pub continuar_cadeia: bool,
    /// Valor lido antes de uma atribuição composta (resultado de `x++`).
    pub valor_antigo: Option<Operand>,
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
        match self.operand_type(&op) {
            Type::I1 => return op,
            // Um `bool` encaixotado não é "diferente de zero": o handle da
            // caixa de `false` também é. Volta pelo `Unbox` (R3).
            Type::Ref => return self.coagir(op, Type::I1),
            _ => {}
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
            self.terminate(Terminator::CondBranch { cond, then_block: bloco_dir, else_block: bloco_fim });
        } else {
            self.terminate(Terminator::CondBranch { cond, then_block: bloco_fim, else_block: bloco_dir });
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
    fn lower_se_nulo(&mut self, ast: &ast::Ast, lop: Operand, right: ExprId, expr_id: ExprId) -> Operand {
        if self.operand_type(&lop) != Type::Ref {
            // Escalar nunca é null: a direita é código morto.
            return lop;
        }
        let ty = self.repr_da_expressao(expr_id).unwrap_or(Type::Ref);
        let e_nulo = self.emit(Instruction::ICmp(ICmpOp::Eq, lop.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
        let bloco_dir = self.new_block();
        let bloco_nn = self.new_block();
        let bloco_fim = self.new_block();
        self.terminate(Terminator::CondBranch { cond: e_nulo, then_block: bloco_dir, else_block: bloco_nn });
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
        self.emit(Instruction::Phi { incoming: entradas, ty }, ty)
    }

    /// Representação do tipo estático de uma expressão, quando ele é
    /// conhecido e não é `dynamic`.
    pub fn repr_da_expressao(&self, e: ExprId) -> Option<Type> {
        let t = self.ctx.get_type(self.unit_id, e)?;
        if t == self.ctx.core.dynamic_ || self.ctx.is_void(t) {
            return None;
        }
        Some(self.ctx.to_hir_type(t))
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
            escopos: vec![HashMap::new()],
            n_allocas: 0,
            value_types: HashMap::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            this_param: None,
            enclosing_class: None,
            exception_targets: Vec::new(),
            finally_scopes: Vec::new(),
            active_catch_stack: Vec::new(),
            terminated_blocks: std::collections::HashSet::new(),
            local_functions: HashMap::new(),
            extra_functions: Vec::new(),
            labeled_break_targets: HashMap::new(),
            labeled_continue_targets: HashMap::new(),
            pending_labels: Vec::new(),
            current_cascade_target: None,
            erros: Vec::new(),
            cadeia_nula: None,
            continuar_cadeia: false,
            valor_antigo: None,
        }
    }

    /// Declara `this` (quando `com_this`) e os parâmetros do outline de `fid`.
    pub fn declarar_parametros(&mut self, fid: usize, com_this: bool) {
        if com_this {
            let this_vid = self.add_param("this".to_string(), Type::Ref);
            self.this_param = Some(Operand::Val(this_vid));
        }
        let Some(dados) = self.ctx.outline.functions.get(fid) else { return };
        for p in dados.parameters.iter() {
            let p_name = p.name.map(|s| self.ctx.symbol_name(s).to_string()).unwrap_or_else(|| "arg".to_string());
            let p_ty = self.repr(p.ty);
            let vid = self.add_param(p_name, p_ty);
            if let Some(sym) = p.name {
                self.declarar_local_com_valor(sym, p_ty, Operand::Val(vid));
            }
        }
    }

    /// Entrega a função (e as funções locais) ao módulo, com os diagnósticos.
    pub fn finalizar(self, module: &mut Module) {
        module.erros.extend(self.erros);
        module.functions.push(self.func);
        module.functions.extend(self.extra_functions);
    }

    /// Construto que o lowering não sabe baixar: diagnóstico com posição
    /// (N1). O operando devolvido nunca chega a ser emitido — um módulo com
    /// erros não gera código.
    pub fn nao_suportado(&mut self, oque: &str, span: dartforge_diagnostics::Span) -> Operand {
        let unit = self.ctx.program.unit(self.unit_id);
        let fonte = &unit.source;
        let ini = span.start.min(fonte.len());
        let antes = &fonte[..ini];
        let linha = antes.matches('\n').count() + 1;
        let coluna = ini - antes.rfind('\n').map_or(0, |p| p + 1) + 1;
        let arquivo = unit
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| unit.uri.clone(), |n| n.to_string_lossy().into_owned());
        self.erros.push(format!("não suportado no backend nativo: {oque} ({arquivo}:{linha}:{coluna})"));
        Operand::Constant(Constant::Null)
    }

    /// Converte um operando para a representação `para` (R4).
    ///
    /// É o único lugar que muda representação: escalar numa posição `Ref`
    /// vira caixa (`Box`), `Ref` numa posição escalar volta por `Unbox` —
    /// que lança `TypeError` para null ou outro tipo, com a verificação de
    /// exceção de uma chamada. `int` para `double` é conversão numérica (o
    /// literal `1` num contexto `double`); os bits de `double` que vêm do
    /// heap usam `Bitcast` explícito, nunca esta função.
    pub fn coagir(&mut self, op: Operand, para: Type) -> Operand {
        let de = self.operand_type(&op);
        if de == para || matches!(para, Type::Void | Type::Ptr) || matches!(de, Type::Void | Type::Ptr) {
            return op;
        }
        match (de, para) {
            (Type::I64 | Type::F64 | Type::I1, Type::Ref) => self.emit(Instruction::Box { op, from: de }, Type::Ref),
            (Type::I8, Type::Ref) => {
                let b = self.emit(Instruction::Trunc { op, from: Type::I8, to: Type::I1 }, Type::I1);
                self.emit(Instruction::Box { op: b, from: Type::I1 }, Type::Ref)
            }
            (Type::Ref, Type::I64 | Type::F64 | Type::I1) => {
                self.emit_call_with_check(Instruction::Unbox { op, to: para }, para)
            }
            (Type::Ref, Type::I8) => {
                let b = self.emit_call_with_check(Instruction::Unbox { op, to: Type::I1 }, Type::I1);
                self.emit(Instruction::ZExt { op: b, from: Type::I1, to: Type::I8 }, Type::I8)
            }
            (Type::I64, Type::F64) => match op {
                Operand::Constant(Constant::Int(n)) => Operand::Constant(Constant::Double(n as f64)),
                op => self.emit(Instruction::IntToDouble(op), Type::F64),
            },
            (Type::F64, Type::I64) => self.emit(Instruction::DoubleToInt(op), Type::I64),
            (Type::I1, Type::I64 | Type::I8) => self.emit(Instruction::ZExt { op, from: Type::I1, to: para }, para),
            (Type::I8, Type::I64) => self.emit(Instruction::ZExt { op, from: Type::I8, to: Type::I64 }, Type::I64),
            (Type::I8, Type::I1) => self.emit(Instruction::Trunc { op, from: Type::I8, to: Type::I1 }, Type::I1),
            (Type::I64, Type::I1) => {
                self.emit(Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))), Type::I1)
            }
            (Type::I64, Type::I8) => self.emit(Instruction::Trunc { op, from: Type::I64, to: Type::I8 }, Type::I8),
            (Type::F64, Type::I1 | Type::I8) => {
                self.emit(Instruction::FCmp(FCmpOp::Ne, op, Operand::Constant(Constant::Double(0.0))), Type::I1)
            }
            (Type::I1 | Type::I8, Type::F64) => {
                let i = self.emit(Instruction::ZExt { op, from: de, to: Type::I64 }, Type::I64);
                self.emit(Instruction::IntToDouble(i), Type::F64)
            }
            _ => op,
        }
    }

    /// Inicializador de uma variável/campo, de qualquer unidade (quem baixa
    /// usa `lower_expr_de` com a unidade da variável).
    pub fn variable_initializer_em(
        &self,
        var_id: dartforge_elements::model::VariableId,
    ) -> Option<dartforge_frontend::ast::ExprId> {
        use dartforge_elements::model::VariableRef;
        use dartforge_frontend::ast::{DeclKind, MemberKind};
        let v_elem = &self.ctx.program.variables[var_id.0 as usize];
        match v_elem.node {
            VariableRef::Field { unit, member, index } => match &self.ctx.program.unit(unit).ast.member(member).kind {
                MemberKind::Field(list) => list.variables.get(index)?.initializer,
                _ => None,
            },
            VariableRef::TopLevel { unit, decl, index } => match &self.ctx.program.unit(unit).ast.decl(decl).kind {
                DeclKind::Variables(list) => list.variables.get(index)?.initializer,
                _ => None,
            },
            _ => None,
        }
    }

    /// `assert(cond, msg)`: lança `AssertionError` capturável (asserts
    /// ligados, como no oráculo `dart run --enable-asserts`).
    pub fn lower_assert(&mut self, ast: &ast::Ast, condition: ExprId, message: Option<ExprId>) {
        let c = self.lower_expr(ast, condition);
        let c = self.para_bool(c);
        let fail_b = self.new_block();
        let cont_b = self.new_block();
        self.terminate(Terminator::CondBranch { cond: c, then_block: cont_b, else_block: fail_b });
        self.set_block(fail_b);
        let msg = match message {
            Some(m) => self.lower_expr(ast, m),
            None => Operand::Constant(Constant::Null),
        };
        let (bits, is_ref) = self.para_bits(msg);
        let err = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_assertion_error_new".to_string(),
                args: vec![(bits, Type::I64), (Operand::Constant(Constant::Int(i64::from(is_ref))), Type::I8)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(err);
        self.set_block(cont_b);
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
        // R4: o valor devolvido na representação do retorno da função.
        let term = match term {
            Terminator::Return(Some(op)) if !matches!(self.func.return_ty, Type::Void) => {
                let r = self.func.return_ty;
                let op = self.coagir(op, r);
                if self.is_terminated() {
                    return;
                }
                Terminator::Return(Some(op))
            }
            t => t,
        };
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
            // Entrada do phi do valor de retorno do `finally`: na
            // representação dele (R4), coagida aqui, no bloco de origem.
            let ty_phi = if self.func.return_ty == Type::Void { Type::Ref } else { self.func.return_ty };
            let val = self.coagir(val, ty_phi);
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

    /// `op is T` na representação de `op`.
    ///
    /// Escalar (`I64`/`F64`/`I1`) tem o tipo decidido em compilação. `Ref`
    /// pergunta a classe ao runtime (`dartforge_value_class`, que devolve
    /// -12 para null, -9/-10/-11 para as caixas de int/double/bool, -2 para
    /// String…) — sem desreferenciar null (H6).
    pub fn testar_tipo(&mut self, ast_ty: &ast::TypeAnnotation, op: Operand) -> Operand {
        let ast::TypeKind::Named { name, .. } = &ast_ty.kind else {
            return self.nao_suportado("teste de tipo estrutural", ast_ty.span);
        };
        let Some(ultimo) = name.last() else {
            return self.nao_suportado("teste de tipo", ast_ty.span);
        };
        let nome = self.ctx.symbol_name(ultimo.sym).to_string();
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            let r = matches!(
                (nome.as_str(), repr),
                ("int", Type::I64)
                    | ("double", Type::F64)
                    | ("bool", Type::I1 | Type::I8)
                    | ("num", Type::I64 | Type::F64)
                    | ("Object" | "dynamic" | "Comparable", _)
            );
            return Operand::Constant(Constant::Bool(r));
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(op, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let igual = |b: &mut Self, v: i64| {
            b.emit(Instruction::ICmp(ICmpOp::Eq, cls.clone(), Operand::Constant(Constant::Int(v))), Type::I1)
        };
        let base = match nome.as_str() {
            "dynamic" => Operand::Constant(Constant::Bool(true)),
            "Object" => self.emit(
                Instruction::ICmp(ICmpOp::Ne, cls.clone(), Operand::Constant(Constant::Int(-12))),
                Type::I1,
            ),
            "Null" => igual(self, -12),
            "num" => {
                let i = igual(self, -9);
                let d = igual(self, -10);
                self.emit(Instruction::Or(i, d), Type::I1)
            }
            _ => {
                let lib = self.ctx.program.unit(self.unit_id).library;
                let cid = self.ctx.program.lookup(lib, ultimo.sym).and_then(|b| match b.getter {
                    Some(dartforge_elements::model::Element::Class(c)) => Some(c),
                    _ => None,
                });
                let Some(id) = cid.and_then(|c| self.id_de_classe(c)) else {
                    return self.nao_suportado(&format!("teste de tipo `{nome}`"), ast_ty.span);
                };
                if id < 0 {
                    igual(self, id)
                } else {
                    let sub = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_is_subclass".to_string(),
                            args: vec![(cls.clone(), Type::I64), (Operand::Constant(Constant::Int(id)), Type::I64)],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                    self.emit(Instruction::ICmp(ICmpOp::Ne, sub, Operand::Constant(Constant::Int(0))), Type::I1)
                }
            }
        };
        if !ast_ty.nullable {
            return base;
        }
        let nulo = igual(self, -12);
        self.emit(Instruction::Or(base, nulo), Type::I1)
    }

    /// `as T` implícito ou explícito: `TypeError` se o valor não é um `T`.
    pub fn checar_tipo_ou_lancar(&mut self, ast_ty: &ast::TypeAnnotation, op: Operand) {
        let ok = self.testar_tipo(ast_ty, op);
        let ok = self.para_bool(ok);
        let fail_b = self.new_block();
        let pass_b = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: pass_b, else_block: fail_b });
        self.set_block(fail_b);
        let err_op = self.emit(
            Instruction::CallRuntime { name: "dartforge_type_error_new".to_string(), args: Vec::new(), ret_ty: Type::Ref },
            Type::Ref,
        );
        self.emit_throw_op(err_op);
        self.set_block(pass_b);
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

    /// Operador de uma atribuição composta (`a op= b`).
    pub fn lower_binary_op_helper(&mut self, op: BinaryOp, lop: Operand, rop: Operand) -> Operand {
        // Local `int?` promovido: o valor corrente é `Ref`, o outro lado diz
        // o escalar.
        let (tl, tr) = (self.operand_type(&lop), self.operand_type(&rop));
        let lop = if tl == Type::Ref && matches!(tr, Type::I64 | Type::F64) {
            self.coagir(lop, tr)
        } else {
            lop
        };
        let rop = if tr == Type::Ref && matches!(tl, Type::I64 | Type::F64) {
            self.coagir(rop, tl)
        } else {
            rop
        };
        let texto = self.operand_type(&lop) == Type::Ref && matches!(op, BinaryOp::Add | BinaryOp::Mul);
        self.operar(op, lop, rop, texto, dartforge_diagnostics::Span { start: 0, end: 0 })
    }

    pub fn lower_stmt(&mut self, ast: &ast::Ast, stmt_id: StmtId) {
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
                self.lower_expr(ast, *expr_id);
            }
            StmtKind::Variables(var_list) => {
                let var_ty_opt = var_list.ty;
                for var in &var_list.variables {
                    let sym = var.name.sym;
                    // R6: o local guarda a representação do tipo declarado
                    // (ou inferido do inicializador), não a do valor.
                    let ty = self.repr_do_local(var.name.span.start);
                    let init_op = if let Some(init_id) = var.initializer {
                        let op = self.lower_expr(ast, init_id);
                        // Checagem na declaração só quando o estático não a
                        // garante: inicializador `dynamic` num tipo declarado.
                        // Antes ela rodava sempre, e `Foo? x = null` chamava
                        // `dartforge_value_class(0)` (H6).
                        let init_dinamico = self.ctx.get_type(self.unit_id, init_id) == Some(self.ctx.core.dynamic_);
                        if let Some(tid) = var_ty_opt.filter(|_| init_dinamico) {
                            self.checar_tipo_ou_lancar(ast.ty(tid), op.clone());
                        }
                        op
                    } else {
                        Self::valor_zero(ty)
                    };
                    self.declarar_local_com_valor(sym, ty, init_op);
                }
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
                                let init_op = if let Some(init_id) = var.initializer {
                                    self.lower_expr(ast, init_id)
                                } else {
                                    Self::valor_zero(ty)
                                };
                                self.declarar_local_com_valor(sym, ty, init_op);
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
                self.abrir_escopo();
                match target {
                    ast::ForInTarget::Declared { name, .. } => {
                        // O elemento é lido na representação da variável (R5):
                        // `for (Cell c in cells)` quer o handle, `for (int n
                        // in ns)` quer os bits.
                        let ty = self.repr_do_local(name.span.start);
                        let item_val = self.ler_elemento_lista(iterable_op.clone(), phi_op.clone(), ty);
                        self.declarar_local_com_valor(name.sym, ty, item_val);
                    }
                    ast::ForInTarget::Expression(e) => {
                        if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                            let ty = self.buscar_local(id.sym).map_or(Type::Ref, |l| l.ty);
                            let item_val = self.ler_elemento_lista(iterable_op.clone(), phi_op.clone(), ty);
                            self.gravar_local(id.sym, item_val);
                        } else {
                            self.nao_suportado("alvo de for-in", stmt.span);
                        }
                    }
                    ast::ForInTarget::Pattern { .. } => {
                        self.nao_suportado("for-in com padrão", stmt.span);
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
                                b.declarar_local_com_valor(n.sym, Type::Ref, Operand::Val(vid));
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
            StmtKind::PatternVariables { .. } => {
                self.nao_suportado("declaração por padrão", stmt.span);
            }
            StmtKind::Switch { .. } => {
                self.nao_suportado("comando switch", stmt.span);
            }
            StmtKind::Yield { .. } => {
                self.nao_suportado("yield", stmt.span);
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

            let mut current_test_block = dispatch_block;

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
                    self.declarar_local_com_valor(ex_name.sym, Type::Ref, ex_bits.clone());
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
                    self.declarar_local_com_valor(st_name.sym, Type::Ref, st_val);
                }

                self.active_catch_stack.push((ex_bits.clone(), 3));
                self.lower_stmt(ast, clause.body);
                self.fechar_escopo();
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

    /// Elo de cadeia de acesso: `a.b`, `a[i]`, `a.m()`.
    fn e_elo(ast: &ast::Ast, e: ExprId) -> bool {
        match &ast.expr(e).kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } => true,
            ExprKind::Call { target, .. } => matches!(ast.expr(*target).kind, ExprKind::Property { .. }),
            _ => false,
        }
    }

    /// A cadeia que termina em `e` tem algum `?.`/`?[`?
    fn cadeia_tem_null_aware(ast: &ast::Ast, e: ExprId) -> bool {
        let mut atual = e;
        loop {
            match &ast.expr(atual).kind {
                ExprKind::Property { target, null_aware, .. } | ExprKind::Index { target, null_aware, .. } => {
                    if *null_aware {
                        return true;
                    }
                    atual = *target;
                }
                ExprKind::Call { target, .. } if matches!(ast.expr(*target).kind, ExprKind::Property { .. }) => {
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
        let Some((saida, _)) = self.cadeia_nula.as_ref() else { return };
        let saida = *saida;
        let e_nulo = self.emit(
            Instruction::ICmp(ICmpOp::Eq, recv.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let segue = self.new_block();
        let origem = self.current_block;
        self.terminate(Terminator::CondBranch { cond: e_nulo, then_block: saida, else_block: segue });
        if let Some((_, entradas)) = self.cadeia_nula.as_mut() {
            entradas.push((origem, Operand::Constant(Constant::Null)));
        }
        self.set_block(segue);
    }

    pub fn lower_expr(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let continuar = std::mem::replace(&mut self.continuar_cadeia, false);
        let salvo = if continuar { None } else { self.cadeia_nula.take() };
        let raiz = !continuar && Self::e_elo(ast, expr_id) && Self::cadeia_tem_null_aware(ast, expr_id);
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
                self.emit(Instruction::Phi { incoming: entradas, ty }, ty)
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
                                {
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
                                        Type::Void | Type::Ptr => self.emit(
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
                        return self.ler_membro_implicito(member, span);
                    }
                    Some(Resolved::Element(el)) => {
                        return self.ler_elemento(el, span);
                    }
                    _ => {}
                }
                if let Some(op) = self.ler_local_por_nome(sym) {
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
                let lop = self.lower_expr(ast, *left);
                let rop = self.lower_expr(ast, *right);
                let l_ty = self.ctx.get_type(self.unit_id, *left);
                let r_ty = self.ctx.get_type(self.unit_id, *right);
                let desconhecido = |t: Option<dartforge_types::table::TypeId>| t.is_none_or(|t| t == self.ctx.core.dynamic_);
                // `String` pelo tipo estático; com o tipo desconhecido (corpo
                // não inferido, parâmetro de closure sem tipo) um `Ref` à
                // esquerda de `+`/`*` é texto, como era antes.
                let texto = l_ty.is_some_and(|t| self.ctx.is_string(t))
                    || (matches!(op, BinaryOp::Add) && r_ty.is_some_and(|t| self.ctx.is_string(t)))
                    || (matches!(op, BinaryOp::Add | BinaryOp::Mul)
                        && desconhecido(l_ty)
                        && self.operand_type(&lop) == Type::Ref);
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
                    UnaryOp::PrefixInc | UnaryOp::PostfixInc | UnaryOp::PrefixDec | UnaryOp::PostfixDec => {
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
                    UnaryOp::Neg if self.operand_type(&sub_op) == Type::F64 => self.emit(Instruction::FNeg(sub_op), Type::F64),
                    UnaryOp::Neg => self.emit(Instruction::Neg(sub_op), Type::I64),
                    UnaryOp::Not => {
                        let b = self.para_bool(sub_op);
                        self.emit(Instruction::LNot(b), Type::I1)
                    }
                    UnaryOp::BitNot => self.emit(Instruction::Not(sub_op), Type::I64),
                    _ => self.nao_suportado("incremento/decremento de não-local", expr.span),
                }
            }
            ExprKind::Conditional { condition, then, else_ } => {
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
                    return self.nao_suportado("condicional com ramos de representações diferentes", expr.span);
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
            ExprKind::Property { target, name, null_aware } => {
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

                let span = expr.span;
                let resolved = self.ctx.get_resolved(self.unit_id, expr_id).cloned();

                // `C.x`: membro estático (o alvo é um literal de classe).
                let alvo_e_classe = matches!(
                    self.ctx.get_resolved(self.unit_id, *target),
                    Some(Resolved::Element(dartforge_elements::model::Element::Class(_)))
                );
                if alvo_e_classe {
                    return match resolved {
                        Some(Resolved::Member { member, .. }) => self.ler_membro_estatico(member, span),
                        _ => self.nao_suportado(&format!("membro estático `{prop_name}`"), span),
                    };
                }

                let target_op = self.lower_alvo(ast, *target);
                if *null_aware {
                    self.desviar_se_nulo(&target_op);
                }

                // Membro de classe do usuário: pelo elemento resolvido (R7),
                // nunca pelo nome.
                if let Some((_, member)) = self.membro_do_usuario(expr_id, *target, name.sym, false) {
                    {
                        let vid = match member {
                            MemberRef::Variable(v) => Some(v),
                            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                        };
                        if let Some(vid) = vid {
                            if crate::lower::e_global(self.ctx, vid) {
                                return self.ler_global(vid, span);
                            }
                            return self.ler_campo_com_late(target_op, vid, span);
                        }
                        let MemberRef::Function(f) = member else { unreachable!() };
                        if self.ctx.program.functions[f.0 as usize].kind == dartforge_elements::model::FunctionKind::Getter {
                            return self.chamar_membro(target_op, f.0 as usize, &[], span);
                        }
                        return self.nao_suportado(&format!("tear-off de método `{prop_name}`"), span);
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
                    self.ler_extremo_lista(target_op, "last", expr_id)
                } else if prop_name == "first" {
                    self.ler_extremo_lista(target_op, "first", expr_id)
                } else if prop_name == "single" {
                    self.ler_extremo_lista(target_op, "single", expr_id)
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
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
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
                    self.nao_suportado(&format!("membro `{prop_name}`"), span)
                }
            }
            ExprKind::Index { target, index, null_aware } => {
                let target_op = self.lower_alvo(ast, *target);
                if *null_aware {
                    self.desviar_se_nulo(&target_op);
                }
                let idx_op = self.lower_expr(ast, *index);
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
                    } else { false });
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
                            // R5: a representação do operando É o tipo. Um
                            // `Ref` é impresso pelo `toString` dele (o do
                            // usuário, pelo despacho; o do runtime para
                            // coleções, strings, caixas e null).
                            return match self.operand_type(&arg_op) {
                                Type::F64 => self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_f64".to_string(),
                                        args: vec![(arg_op, Type::F64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                ),
                                Type::I64 => self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_i64".to_string(),
                                        args: vec![(arg_op, Type::I64)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                ),
                                Type::I1 | Type::I8 => self.emit(
                                    Instruction::CallRuntime {
                                        name: "dartforge_print_bool".to_string(),
                                        args: vec![(arg_op, Type::I8)],
                                        ret_ty: Type::Void,
                                    },
                                    Type::Void,
                                ),
                                _ => {
                                    let arg_op = self.coagir(arg_op, Type::Ref);
                                    let texto = self.emit_call_with_check(
                                        Instruction::CallStatic {
                                            symbol: "dartforge_dispatch_toString".to_string(),
                                            args: vec![arg_op],
                                            ret_ty: Type::Ref,
                                        },
                                        Type::Ref,
                                    );
                                    self.emit(
                                        Instruction::CallRuntime {
                                            name: "dartforge_print_handle".to_string(),
                                            args: vec![(texto, Type::Ref)],
                                            ret_ty: Type::Void,
                                        },
                                        Type::Void,
                                    )
                                }
                            };
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
                        return self.identicos(a_op, b_op);
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
                        // Função local: parâmetros e retorno são `Ref` (R4).
                        let mut args_ops = Vec::new();
                        for a in &arguments.args {
                            let v = self.lower_expr(ast, a.value);
                            args_ops.push(self.coagir(v, Type::Ref));
                        }
                        return self.emit_call_with_check(
                            Instruction::CallStatic {
                                symbol: sym,
                                args: args_ops,
                                ret_ty,
                            },
                            ret_ty,
                        );
                    }
                    // Função de topo, membro implícito (`m()` = `this.m()`) ou
                    // estático: pelo elemento resolvido do alvo.
                    match self.ctx.get_resolved(self.unit_id, *target).cloned() {
                        Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                            if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                                && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
                        {
                            let fid = f.0 as usize;
                            let avaliados = self.avaliar_args(ast, &arguments.args);
                            let args = self.casar_args(fid, &avaliados);
                            return self.chamar_direto(fid, None, args);
                        }
                        Some(Resolved::Member { member: MemberRef::Function(f), .. })
                            if crate::lower::funcao_do_usuario(self.ctx, f.0 as usize)
                                && self.ctx.program.functions[f.0 as usize].variable.is_none() =>
                        {
                            let fid = f.0 as usize;
                            let avaliados = self.avaliar_args(ast, &arguments.args);
                            if self.ctx.program.functions[fid].static_ {
                                let args = self.casar_args(fid, &avaliados);
                                return self.chamar_direto(fid, None, args);
                            }
                            let Some(this) = self.this_param.clone() else {
                                return self.nao_suportado("método de instância fora de membro de instância", expr.span);
                            };
                            return self.chamar_membro(this, fid, &avaliados, expr.span);
                        }
                        _ => {}
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
                                let code_op = self.coagir(code_op, Type::I64);
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

                // Instanciação sem `new`: `C(…)`, `C.nome(…)`.
                if let Some(Resolved::Constructor(fid)) = self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    return self.instanciar(ast, fid, &arguments.args, expr.span);
                }

                // Chamadas de método sobre objeto / coleção
                if let ExprKind::Property { target: inner_target, name: method_name, null_aware } = &ast.expr(*target).kind {
                    let m_name = self.ctx.symbol_name(method_name.sym);
                    let resolved_alvo = self.ctx.get_resolved(self.unit_id, *target).cloned();

                    // `C.m(…)`: método estático do usuário.
                    let alvo_e_classe = matches!(
                        self.ctx.get_resolved(self.unit_id, *inner_target),
                        Some(Resolved::Element(dartforge_elements::model::Element::Class(_)))
                    );
                    if alvo_e_classe {
                        if let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = resolved_alvo {
                            let fid = f.0 as usize;
                            if crate::lower::funcao_do_usuario(self.ctx, fid) && self.ctx.program.functions[fid].static_ {
                                let avaliados = self.avaliar_args(ast, &arguments.args);
                                let args = self.casar_args(fid, &avaliados);
                                return self.chamar_direto(fid, None, args);
                            }
                        }
                        return self.nao_suportado(&format!("chamada estática `{m_name}`"), expr.span);
                    }

                    let recv_op = self.lower_alvo(ast, *inner_target);
                    if *null_aware {
                        self.desviar_se_nulo(&recv_op);
                    }

                    // Método de classe do usuário: pelo elemento resolvido, com
                    // despacho pela classe dinâmica (R7).
                    if let Some((_, member)) = self.membro_do_usuario(*target, *inner_target, method_name.sym, false) {
                        {
                            let MemberRef::Function(f) = member else {
                                return self.nao_suportado("chamada de campo de tipo função", expr.span);
                            };
                            if self.ctx.program.functions[f.0 as usize].variable.is_some() {
                                return self.nao_suportado("chamada de campo de tipo função", expr.span);
                            }
                            let avaliados = self.avaliar_args(ast, &arguments.args);
                            return self.chamar_membro(recv_op, f.0 as usize, &avaliados, expr.span);
                        }
                    }

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
                        let str_op = self.texto_de(arg_op);
                        return self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_buffer_write".to_string(),
                                args: vec![(recv_op, Type::Ref), (str_op, Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        );
                    } else if m_name == "toString" {
                        return self.texto_de(recv_op);
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
                                // O elemento na representação do tipo de
                                // elemento da lista (R5).
                                let repr_item = self
                                    .ctx
                                    .get_type(self.unit_id, *inner_target)
                                    .and_then(|t| self.tipo_elemento(t))
                                    .map_or(Type::Ref, |t| self.repr(t));
                                let item_val = self.ler_elemento_lista(recv_op.clone(), phi_op.clone(), repr_item);

                                if let Some(sym) = param_sym {
                                    self.ligar_local(sym, item_val);
                                }

                                let mapped_val = match &closure.body {
                                    FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                                    _ => self.nao_suportado("closure com corpo de bloco", expr.span),
                                };

                                let mapped_tag = i64::from(self.operand_tag(&mapped_val));
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
                                    self.ligar_local(sym, part_val.clone());
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
                                    self.ligar_local(sym, part_val.clone());
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

                let oque = match &ast.expr(*target).kind {
                    ExprKind::Property { name, .. } | ExprKind::Identifier(name) => {
                        format!("chamada `{}`", self.ctx.symbol_name(name.sym))
                    }
                    _ => "chamada de valor de função".to_string(),
                };
                self.nao_suportado(&oque, expr.span)
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
                match self.ctx.get_resolved(self.unit_id, expr_id).cloned() {
                    Some(Resolved::Constructor(fid)) => self.instanciar(ast, fid, &arguments.args, expr.span),
                    _ => self.nao_suportado("instanciação não resolvida", expr.span),
                }
            }
            ExprKind::Throw(inner) => {
                self.emit_throw(ast, *inner)
            }
            ExprKind::Rethrow => {
                self.emit_rethrow()
            }
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
            ExprKind::Assign { op, target, value } => {
                self.lower_atribuicao(ast, *op, *target, super::atribuicao::Rhs::Expr(*value), expr.span)
            }
            ExprKind::This => match self.this_param.clone() {
                Some(t) => t,
                None => self.nao_suportado("`this` fora de membro de instância", expr.span),
            },
            outro => {
                let oque = match outro {
                    ExprKind::Super => "`super` como valor",
                    ExprKind::Symbol(_) => "literal de símbolo",
                    ExprKind::FunctionExpression(_) => "closure",
                    ExprKind::TypeArguments { .. } => "instanciação de tipo genérico",
                    ExprKind::PatternAssign { .. } => "atribuição por padrão",
                    ExprKind::Cascade { .. } | ExprKind::CascadeTarget => "cascata",
                    ExprKind::Await(_) => "await",
                    ExprKind::Switch { .. } => "expressão switch",
                    _ => "expressão",
                };
                self.nao_suportado(oque, expr.span)
            }
        }
    }
}

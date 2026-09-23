//! Construtor de funções HIR durante o lowering.

use crate::context::Context;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::LocalId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FinallyScope {
    pub entry_block: BlockId,
    pub resume_block: BlockId,
    pub reason_phi: ValueId,
    pub ret_val_phi: ValueId,
    pub incoming: Vec<(BlockId, i64, Operand)>,
    /// Quantos alvos de `break`/`continue` sem rótulo existiam quando o
    /// `try` começou: um salto para um deles atravessa este `finally`.
    pub prof_break: usize,
    pub prof_continue: usize,
    /// Rótulos que já existiam quando o `try` começou (os de fora).
    pub rotulos_break: std::collections::HashSet<SymbolId>,
    pub rotulos_continue: std::collections::HashSet<SymbolId>,
    /// Saltos (`break`/`continue`, com o rótulo) que atravessam este
    /// `finally`: o de índice `k` entra com a razão `5 + k` e, no fim do
    /// `finally`, continua o salto.
    pub saltos: Vec<(bool, Option<SymbolId>)>,
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
    // --- P1 (closures, α) ---
    /// Offsets das declarações desta função que moram numa célula (captura.rs).
    pub celulas: std::collections::HashSet<usize>,
    /// Closures anônimas já criadas nesta função (nome do corpo).
    pub n_closures: u32,
    /// Nomes de corpos de funções locais já usados nesta função.
    pub nomes_locais: std::collections::HashSet<String>,
    /// Entradas de tear-off já geradas por esta função.
    pub entradas_feitas: std::collections::HashSet<String>,
    // --- P3 (const canônico, α) ---
    /// Globais criados por esta função (as constantes canônicas).
    pub globais_extras: Vec<(u32, Type, String)>,
    /// A expressão constante que o getter canônico corrente avalia (não é
    /// canonizada de novo dentro dele).
    pub constante_em_curso: Option<ExprId>,
    /// Dentro de um contexto constante (inicializador `const`, valor padrão,
    /// argumentos de um `const C(…)`): `C(…)`, `[…]` e `{…}` são constantes.
    pub em_contexto_const: bool,
    /// A chave de valor (`constantes.rs`) de cada local `const` visível.
    pub chaves_de_const_locais: HashMap<SymbolId, (String, ExprId)>,
    /// O padrão corrente é de casamento (`case`, `if-case`): um nome solto
    /// nele é um padrão constante, não uma variável nova.
    pub padrao_refutavel: bool,
    /// O teste de tipo corrente é o de um `as` (confere só a classe).
    pub cast_so_pela_classe: bool,
    // --- P5c (SDK da fonte, δ) ---
    /// Esta função é um adaptador da tabela de métodos (`sdk_fonte.rs`): o
    /// membro que ele adapta é chamado direto, nunca pelo seletor de novo.
    pub em_adaptador: bool,
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
            extra_functions: Vec::new(),
            labeled_break_targets: HashMap::new(),
            labeled_continue_targets: HashMap::new(),
            pending_labels: Vec::new(),
            current_cascade_target: None,
            erros: Vec::new(),
            cadeia_nula: None,
            continuar_cadeia: false,
            valor_antigo: None,
            celulas: std::collections::HashSet::new(),
            n_closures: 0,
            nomes_locais: std::collections::HashSet::new(),
            entradas_feitas: std::collections::HashSet::new(),
            globais_extras: Vec::new(),
            constante_em_curso: None,
            em_contexto_const: false,
            chaves_de_const_locais: HashMap::new(),
            padrao_refutavel: false,
            cast_so_pela_classe: false,
            em_adaptador: false,
        }
    }

    /// Declara `this` (quando `com_this`) e os parâmetros do outline de `fid`.
    pub fn declarar_parametros(&mut self, fid: usize, com_this: bool) {
        if com_this {
            let this_vid = self.add_param("this".to_string(), Type::Ref);
            self.this_param = Some(Operand::Val(this_vid));
        }
        let Some(dados) = self.ctx.outline.functions.get(fid) else {
            return;
        };
        // Os offsets dos nomes na declaração: a chave das células (P1).
        let program = self.ctx.program;
        let ast_params: &[ast::Parameter] = match program.functions[fid].node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => program
                .unit(unit)
                .ast
                .function(function)
                .parameters
                .as_deref()
                .unwrap_or(&[]),
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => {
                match &program.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Constructor(c) => &c.parameters[..],
                    _ => &[],
                }
            }
            dartforge_elements::model::FunctionRef::None => &[],
        };
        for (i, p) in dados.parameters.iter().enumerate() {
            let p_name = p
                .name
                .map(|s| self.ctx.symbol_name(s).to_string())
                .unwrap_or_else(|| "arg".to_string());
            let p_ty = self.repr(p.ty);
            let vid = self.add_param(p_name, p_ty);
            if let Some(sym) = p.name {
                match ast_params.get(i).and_then(|a| a.name) {
                    Some(n) => {
                        self.declarar_variavel(sym, n.span.start as usize, p_ty, Operand::Val(vid))
                    }
                    None => self.declarar_local_com_valor(sym, p_ty, Operand::Val(vid)),
                }
            }
        }
    }

    /// Entrega a função (e as funções locais) ao módulo, com os diagnósticos.
    pub fn finalizar(self, module: &mut Module) {
        module.erros.extend(self.erros);
        module.globais.extend(self.globais_extras);
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
        self.erros.push(format!(
            "{}{oque} ({arquivo}:{linha}:{coluna})",
            crate::PREFIXO_NAO_SUPORTADO
        ));
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
        if de == para
            || matches!(para, Type::Void | Type::Ptr)
            || matches!(de, Type::Void | Type::Ptr)
        {
            return op;
        }
        match (de, para) {
            (Type::I64 | Type::F64 | Type::I1, Type::Ref) => {
                self.emit(Instruction::Box { op, from: de }, Type::Ref)
            }
            (Type::I8, Type::Ref) => {
                let b = self.emit(
                    Instruction::Trunc {
                        op,
                        from: Type::I8,
                        to: Type::I1,
                    },
                    Type::I1,
                );
                self.emit(
                    Instruction::Box {
                        op: b,
                        from: Type::I1,
                    },
                    Type::Ref,
                )
            }
            (Type::Ref, Type::I64 | Type::F64 | Type::I1) => {
                self.emit_call_with_check(Instruction::Unbox { op, to: para }, para)
            }
            (Type::Ref, Type::I8) => {
                let b =
                    self.emit_call_with_check(Instruction::Unbox { op, to: Type::I1 }, Type::I1);
                self.emit(
                    Instruction::ZExt {
                        op: b,
                        from: Type::I1,
                        to: Type::I8,
                    },
                    Type::I8,
                )
            }
            (Type::I64, Type::F64) => match op {
                Operand::Constant(Constant::Int(n)) => {
                    Operand::Constant(Constant::Double(n as f64))
                }
                op => self.emit(Instruction::IntToDouble(op), Type::F64),
            },
            (Type::F64, Type::I64) => self.emit(Instruction::DoubleToInt(op), Type::I64),
            (Type::I1, Type::I64 | Type::I8) => self.emit(
                Instruction::ZExt {
                    op,
                    from: Type::I1,
                    to: para,
                },
                para,
            ),
            (Type::I8, Type::I64) => self.emit(
                Instruction::ZExt {
                    op,
                    from: Type::I8,
                    to: Type::I64,
                },
                Type::I64,
            ),
            (Type::I8, Type::I1) => self.emit(
                Instruction::Trunc {
                    op,
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            ),
            (Type::I64, Type::I1) => self.emit(
                Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))),
                Type::I1,
            ),
            (Type::I64, Type::I8) => self.emit(
                Instruction::Trunc {
                    op,
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
            (Type::F64, Type::I1 | Type::I8) => self.emit(
                Instruction::FCmp(FCmpOp::Ne, op, Operand::Constant(Constant::Double(0.0))),
                Type::I1,
            ),
            (Type::I1 | Type::I8, Type::F64) => {
                let i = self.emit(
                    Instruction::ZExt {
                        op,
                        from: de,
                        to: Type::I64,
                    },
                    Type::I64,
                );
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
            VariableRef::Field {
                unit,
                member,
                index,
            } => match &self.ctx.program.unit(unit).ast.member(member).kind {
                MemberKind::Field(list) => list.variables.get(index)?.initializer,
                _ => None,
            },
            VariableRef::TopLevel { unit, decl, index } => {
                match &self.ctx.program.unit(unit).ast.decl(decl).kind {
                    DeclKind::Variables(list) => list.variables.get(index)?.initializer,
                    _ => None,
                }
            }
            _ => None,
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
        let idx = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == self.current_block)
            .unwrap();
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
                // `=> print(x)` numa função que devolve valor: a expressão
                // `void` vale null (não há valor SSA a devolver).
                let op = if matches!(self.operand_type(&op), Type::Void) {
                    Self::valor_zero(r)
                } else {
                    op
                };
                let op = self.coagir(op, r);
                if self.is_terminated() {
                    return;
                }
                Terminator::Return(Some(op))
            }
            t => t,
        };
        self.terminated_blocks.insert(self.current_block);
        let idx = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == self.current_block)
            .unwrap();
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
            let ty_phi = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
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
        self.saltar(false, label);
    }

    /// `break`/`continue` (com ou sem rótulo): direto ao alvo, ou pelo
    /// `finally` mais interno que o salto atravessa (a razão `5 + k` do
    /// salto `k` desse `finally`; no fim dele o salto continua — por outros
    /// `finally` de fora, se atravessar mais). Um salto para um alvo DENTRO
    /// do `try` (o laço do próprio corpo) não passa pelo `finally`.
    pub fn saltar(&mut self, e_continue: bool, label: Option<SymbolId>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let (pilha, rotulados) = if e_continue {
            (&self.continue_targets, &self.labeled_continue_targets)
        } else {
            (&self.break_targets, &self.labeled_break_targets)
        };
        let target_opt = match label {
            Some(sym) => rotulados.get(&sym).copied(),
            None => pilha.last().copied(),
        };
        let indice = pilha.len().checked_sub(1);
        if let Some(target) = target_opt {
            let atravessa = self.finally_scopes.last().is_some_and(|fin| match label {
                Some(sym) => {
                    if e_continue {
                        fin.rotulos_continue.contains(&sym)
                    } else {
                        fin.rotulos_break.contains(&sym)
                    }
                }
                None => {
                    let prof = if e_continue { fin.prof_continue } else { fin.prof_break };
                    indice.is_some_and(|i| i < prof)
                }
            });
            if !atravessa {
                self.terminate(Terminator::Branch(target));
            } else {
                let default_ret = self.default_return_operand();
                let atual = self.current_block;
                let fin = self.finally_scopes.last_mut().unwrap();
                let k = match fin.saltos.iter().position(|s| *s == (e_continue, label)) {
                    Some(k) => k,
                    None => {
                        fin.saltos.push((e_continue, label));
                        fin.saltos.len() - 1
                    }
                };
                fin.incoming.push((atual, 5 + k as i64, default_ret));
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
        self.saltar(true, label);
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
            Type::I1 => self.emit(
                Instruction::ZExt {
                    op: ex_op,
                    from: Type::I1,
                    to: Type::I8,
                },
                Type::I8,
            ),
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
        let ast::TypeKind::Named { name, args } = &ast_ty.kind else {
            return self.nao_suportado("teste de tipo estrutural", ast_ty.span);
        };
        // `x is List<int>`: os argumentos de tipo em tempo de execução (RTI)
        // ainda não existem — responder pela classe só daria a resposta
        // errada. Argumentos triviais (`dynamic`, `Object?`) não mudam nada.
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let trivial = |t: &ast::TypeAnnotation| match &t.kind {
            ast::TypeKind::Named { name, args } if args.is_empty() => name
                .last()
                .is_some_and(|n| matches!(self.ctx.symbol_name(n.sym), "dynamic") || (self.ctx.symbol_name(n.sym) == "Object" && t.nullable)),
            _ => false,
        };
        if !args.is_empty() && !args.iter().all(|a| trivial(unit_ast.ty(*a))) && !self.cast_so_pela_classe {
            return self.nao_suportado("teste de tipo genérico (RTI)", ast_ty.span);
        }
        let Some(ultimo) = name.last() else {
            return self.nao_suportado("teste de tipo", ast_ty.span);
        };
        if let Some(r) = self.testar_tipo_fonte(ast_ty, name, op.clone()) {
            return r;
        }
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
            b.emit(
                Instruction::ICmp(ICmpOp::Eq, cls.clone(), Operand::Constant(Constant::Int(v))),
                Type::I1,
            )
        };
        let base = match nome.as_str() {
            "dynamic" => Operand::Constant(Constant::Bool(true)),
            "Object" => self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    cls.clone(),
                    Operand::Constant(Constant::Int(-12)),
                ),
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
                // `p.Tipo` (import com prefixo) ou `Tipo`.
                let binding = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, ultimo.sym),
                };
                let cid = binding
                    .and_then(|b| match b.getter {
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
                            args: vec![
                                (cls.clone(), Type::I64),
                                (Operand::Constant(Constant::Int(id)), Type::I64),
                            ],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                    self.emit(
                        Instruction::ICmp(ICmpOp::Ne, sub, Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    )
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
        // `as List<int>`: até a RTI, o cast confere só a classe — num
        // programa correto ele nunca falha; o que se perde é o `TypeError`
        // de um cast errado nos argumentos de tipo.
        let salvo = std::mem::replace(&mut self.cast_so_pela_classe, true);
        let ok = self.testar_tipo(ast_ty, op);
        self.cast_so_pela_classe = salvo;
        let ok = self.para_bool(ok);
        let fail_b = self.new_block();
        let pass_b = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
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
        let msg = self.emit(
            Instruction::Const(Constant::String(
                "IntegerDivisionByZeroException".to_string(),
            )),
            Type::Ref,
        );
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
        let texto =
            self.operand_type(&lop) == Type::Ref && matches!(op, BinaryOp::Add | BinaryOp::Mul);
        self.operar(
            op,
            lop,
            rop,
            texto,
            dartforge_diagnostics::Span { start: 0, end: 0 },
        )
    }
}

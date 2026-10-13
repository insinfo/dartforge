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
    pub break_labels: std::collections::HashSet<SymbolId>,
    pub continue_labels: std::collections::HashSet<SymbolId>,
    /// Saltos (`break`/`continue`, com o rótulo) que atravessam este
    /// `finally`: o de índice `k` entra com a razão `5 + k` e, no fim do
    /// `finally`, continua o salto.
    pub jumps: Vec<(bool, Option<SymbolId>)>,
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
    pub(super) scopes: Vec<super::locals::LocalScope>,
    /// Identidade monotônica dos escopos, independente da profundidade.
    pub(super) next_scope: u32,
    /// Inicializadores `late` que o lowering já está expandindo. Uma leitura
    /// recursiva deve consultar a célula no runtime, não expandir o AST de
    /// novo durante a compilação.
    pub late_initializers_in_lowering: std::collections::HashSet<usize>,
    /// Quantos `alloca` já estão no começo do bloco de entrada.
    pub n_allocas: usize,
    pub value_types: HashMap<ValueId, Type>,
    /// Fatos dos parâmetros declarados, incluindo funções absorvidas.
    pub dart_scalar_parameters: HashMap<String, std::collections::HashSet<ValueId>>,
    /// IDs de RTI com origem nativa explícita, incluindo funções absorvidas.
    pub dart_rti_parameters: HashMap<String, std::collections::HashSet<ValueId>>,
    /// Vetores nativos com papel declarado pela ABI do produtor.
    pub dart_vector_parameters: HashMap<String, HashMap<ValueId, DartEntryVector>>,
    pub arc_uniform_returns: std::collections::HashSet<String>,
    /// Corpos Dart construídos aqui à parte (closures) cujo retorno Ref segue
    /// a convenção Owned das funções Dart (`Module::dart_ref_returns`).
    pub dart_ref_returns: std::collections::HashSet<String>,
    /// Caches de IDs nativos RTI declarados pelo lowering, com flag implícita `$ok`.
    /// Todas as escritas e representações exigem conferência antes de produzir contratos.
    pub arc_rti_caches: std::collections::HashSet<String>,
    /// Definições compartilháveis declaradas nominalmente pelo produtor ARC.
    /// Não certifica equivalência das cópias nem dependências da emissão.
    pub arc_shared_functions: std::collections::HashSet<String>,
    pub arc_direct_local_functions: std::collections::HashSet<String>,
    pub break_targets: Vec<BlockId>,
    pub continue_targets: Vec<BlockId>,
    pub this_param: Option<Operand>,
    /// Obrigação do receptor estático. None exige prova; só há obrigação
    /// de captura quando this_param representa uma captura efetiva.
    pub finalizable_this: Option<bool>,
    pub enclosing_class: Option<dartforge_elements::model::ClassId>,
    pub exception_targets: Vec<BlockId>,
    pub finally_scopes: Vec<FinallyScope>,
    pub active_catch_stack: Vec<(Operand, u8)>,
    pub terminated_blocks: std::collections::HashSet<BlockId>,
    pub extra_functions: Vec<Function>,
    pub labeled_break_targets: HashMap<SymbolId, BlockId>,
    pub labeled_continue_targets: HashMap<SymbolId, BlockId>,
    pub pending_labels: Vec<SymbolId>,
    /// O tipo estático do valor função que a chamada em curso vai chamar
    /// (`lower_call`, lido por `call_function_value`).
    pub callee_type: Option<dartforge_types::TypeId>,
    /// A expressão da chamada em curso (`lower_call`), de onde
    /// `call_function_value` tira os argumentos de tipo que passa a uma
    /// closure genérica (escritos, ou os inferidos pelo tipo estático).
    pub current_call: Option<ExprId>,
    pub current_cascade_target: Option<Operand>,
    /// Um receptor já avaliado: quando `lower_expr` chega à expressão, usa o
    /// valor em vez de avaliá-la de novo (a atribuição `a?.b = v` avalia `a`
    /// antes, para testar o null).
    pub receiver_ready: Option<(ExprId, Operand)>,
    /// A atribuição `?.`/`?[` cujo teste de null já foi feito (o alvo).
    pub handled_null_aware: Option<ExprId>,
    /// Diagnósticos de construto não suportado (N1).
    pub errors: Vec<String>,
    /// Cadeia `?.` em curso: bloco de saída com null e as entradas do phi
    /// (N3). `None` fora de cadeia.
    pub null_chain: Option<(BlockId, Vec<(BlockId, Operand)>)>,
    /// O próximo `lower_expr` é o alvo de um elo da cadeia corrente.
    pub continue_chain: bool,
    /// Valor lido antes de uma atribuição composta (resultado de `x++`).
    pub old_value: Option<Operand>,
    // --- P1 (closures, α) ---
    /// Offsets das declarações desta função que moram numa célula (captura.rs).
    pub cells: std::collections::HashSet<usize>,
    /// Closures anônimas já criadas nesta função (nome do corpo).
    pub n_closures: u32,
    /// Nomes de corpos de funções locais já usados nesta função.
    pub local_names: std::collections::HashSet<String>,
    /// Entradas de tear-off já geradas por esta função.
    pub done_entries: std::collections::HashSet<String>,
    // --- P3 (const canônico, α) ---
    /// Globais criados por esta função (as constantes canônicas).
    pub extra_globals: Vec<(u32, Type, String)>,
    /// A expressão constante que o getter canônico corrente avalia (não é
    /// canonizada de novo dentro dele).
    pub constant_in_progress: Option<ExprId>,
    /// Dentro de um contexto constante (inicializador `const`, valor padrão,
    /// argumentos de um `const C(…)`): `C(…)`, `[…]` e `{…}` são constantes.
    pub in_const_context: bool,
    /// A chave de valor (`constants.rs`) de cada local `const` visível.
    pub local_const_keys: HashMap<SymbolId, (String, ExprId)>,
    /// O padrão corrente é de casamento (`case`, `if-case`): um nome solto
    /// nele é um padrão constante, não uma variável nova.
    pub refutable_pattern: bool,
    /// O teste de tipo corrente é o de um `as` (confere só a classe).
    pub cast_by_class_only: bool,
    // --- P5c (SDK da fonte, δ) ---
    /// Esta função é um adaptador da tabela de métodos (`sdk_source.rs`): o
    /// membro que ele adapta é chamado direto, nunca pelo seletor de novo.
    pub in_adapter: bool,
    /// Entrada tipada (`$tc`/`$ts`, `typed_entry.rs`): a aridade e os
    /// nomes já foram garantidos pelo chamador estático, e o
    /// `unpack` não os confere de novo.
    pub guaranteed_arity: bool,
    /// Os tipos dos parâmetros da invocação cujos argumentos são avaliados
    /// a seguir (a lista de argumentos pelo endereço): o `evaluate_args`
    /// confere cada argumento `dynamic` logo depois de avaliá-lo, como o
    /// cast implícito que o CFE põe em volta dele (`typed_entry.rs`).
    pub arg_types: Option<(usize, super::typed_entry::InvocationTypes)>,
    /// Valores de argumento cujo tipo o chamador estático garante (tipo
    /// estático conhecido e não `dynamic`, ou conferido pelo cast
    /// implícito): só com todos assim a chamada usa a entrada tipada; o
    /// resto fica com a que confere tudo (`c:`).
    pub checked_args: std::collections::HashSet<ValueId>,
    // --- funções locais diretas (`direct_functions.rs`) ---
    /// As funções locais que `capture.rs` decidiu diretas nesta função.
    pub allowed_directs: std::collections::HashSet<dartforge_frontend::ast::FunctionId>,
    /// Offsets das declarações desta função gravadas em algum ponto.
    pub assigned: std::collections::HashSet<usize>,
    /// Offsets das variáveis de fora que chegam a esta função direta pelo
    /// endereço (repassadas pelo endereço a outra direta).
    pub pointers: std::collections::HashSet<usize>,
    /// As funções diretas visíveis, pelo offset do nome.
    pub direct_functions: HashMap<usize, super::direct_functions::Direct>,
    /// Sem funções diretas (corpo `async`/gerador: os `alloca` viram
    /// posições do quadro, e o endereço deles não serve).
    pub without_directs: bool,
    /// O endereço dos elementos da lista tipada cujo comprimento
    /// `fast_length` acabou de ler (a lista e o endereço): o acesso
    /// que ele guarda usa o mesmo, sem ler de novo (`typed_items.rs`).
    pub typed_data: Option<(Operand, Operand)>,
    /// O cache do cabeçalho de lista tipada lida de campo, por tipo de
    /// elemento: os dois locais (handle, cabeçalho) que todos os acessos da
    /// função compartilham (`typed_items.rs`, `cabecalho_tipado`).
    pub header_caches: std::collections::HashMap<i64, (Operand, Operand)>,
    /// Os valores lidos de uma variável global pelo getter dela
    /// (`members.rs`, `read_global`): relidos a cada acesso, como um campo,
    /// então também passam pelo cache de cabeçalho.
    pub read_from_global: std::collections::HashSet<ValueId>,
    // --- P6 (async, `async_sm.rs`) ---
    /// Corpo de uma função `async` em curso: o quadro, as retomadas.
    pub async_state: Option<Box<super::async_sm::AsyncState>>,
    // --- RTI (`rti.rs`) ---
    /// Os parâmetros de tipo da função corrente (`M<i>`), pelo nome.
    pub function_type_params: Vec<SymbolId>,
    /// Os parâmetros de tipo das closures genéricas em volta (a função local
    /// `T f<T>()`, a expressão `<T>(x) => …`) e a própria: nome, parâmetro e
    /// posição na tupla (`M<i>`), depois dos de quem as criou
    /// (`closures.rs`, `lower_closure`). O mais interno vem por último.
    pub local_params: Vec<(SymbolId, dartforge_types::table::TypeParamId, usize)>,
    /// Os parâmetros de tipo da classe vêm na tupla (fábrica de classe
    /// genérica: não há `this`).
    pub class_by_tuple: bool,
    /// A tupla de argumentos de tipo da função corrente (`I64`), se há.
    pub type_tuple: Option<Operand>,
    /// O tipo estático da criação que `instantiate` vai baixar (`C<T…>`).
    /// A classe concreta da próxima criação, quando o construtor escolhido é o
    /// da superclasse que uma aplicação de mixin encaminha
    /// (`members::constructor_of`).
    pub concrete_class: Option<dartforge_elements::model::ClassId>,
    pub creation_type: Option<dartforge_types::table::TypeId>,
    /// A tupla de argumentos de tipo da chamada genérica corrente, que
    /// `call_direct` acrescenta quando o alvo tem parâmetros de tipo.
    pub armed_tuple: Option<Operand>,
    /// O tipo estático de `this` num membro de extensão (o `on`): o
    /// receptor implícito de outra chamada de extensão.
    pub this_extension: Option<(dartforge_elements::model::ExtensionId, dartforge_types::table::TypeId)>,
    /// Num membro de instância de tipo de extensão: o tipo e o tipo estático
    /// de `this` (`E<T…>`, não apagado) — o receptor implícito dos membros
    /// dele (`extension_types.rs`).
    pub this_ext_type: Option<(dartforge_elements::model::ClassId, dartforge_types::table::TypeId)>,
    /// O valor de um `return;` (e do fim do corpo): a representação, no
    /// construtor generativo de tipo de extensão.
    pub constructor_return: Option<Operand>,
    /// A tupla dos argumentos de tipo escritos no padrão de objeto de tipo
    /// de extensão genérico em curso (`patterns.rs`).
    pub pattern_tuple_te: Option<Operand>,
    /// N13: as listas do núcleo cujo comprimento e dados são fixos nas
    /// voltas em emissão (`statements::Counted::Lista`), pelo endereço do
    /// local.
    pub fixed_lists: Vec<FixedList>,
    /// A lista fixa do acesso indexado em emissão (`typed_items.rs`).
    pub access_fixed: Option<FixedList>,
    /// As listas tipadas das voltas rápidas em emissão
    /// (`statements::Counted::TypedSet`): o contador do laço está provado
    /// dentro dos limites delas, e os dados e o comprimento foram lidos
    /// antes das voltas.
    pub proven_typed: Vec<ProvenTyped>,
    /// A lista tipada provada do acesso indexado em emissão (`typed_items.rs`).
    pub access_proven: Option<ProvenTyped>,
    /// A classe que declara a função corrente (também num membro
    /// estático): os estáticos dela estão no escopo léxico.
    pub member_class: Option<dartforge_elements::model::ClassId>,
    /// Quantos `Pointer.fromFunction` a função já baixou: com o símbolo, o
    /// sítio de cada um (o trampolim é um por sítio, como na VM).
    pub callback_sites: u32,
    /// J05: `(line, column)` do comando que está sendo baixado, com a
    /// depuração ligada; cada instrução emitida a registra.
    pub position: Option<(u32, u32)>,
    /// O que o rastro simbólico precisa da função além das posições (§13.14).
    pub stack_trace: FunctionStackTrace,
}

/// O que o rastro no formato da VM precisa de uma função além das posições
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14), passado à
/// [`crate::hir::FunctionDebugInfo`] quando a função é entregue
/// ([`FnBuilder::close_stack_trace`]).
#[derive(Debug, Clone, Default)]
pub struct FunctionStackTrace {
    /// A posição do token da função.
    pub token: Option<(u32, u32)>,
    /// [`crate::hir::stack_trace_marks`].
    pub marks: u8,
    pub async_body: Option<crate::hir::TraceAsyncBody>,
    pub closure_entry: Option<String>,
    pub chain_link: Option<crate::hir::WaitLink>,
    /// A pilha do rastro (`dartforge_rastro_entrar(ty, value)` na entrada e
    /// `dartforge_rastro_sair` em cada saída): o quadro de um corpo `async`
    /// (tipo 0) e o ouvinte de `_FutureListener.handleValue` (tipo 1), que o
    /// runtime casa com os quadros da pilha de máquina.
    pub stack: Option<(i64, Operand)>,
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
    pub fn to_bool(&mut self, op: Operand) -> Operand {
        match self.operand_type(&op) {
            Type::I1 => return op,
            // Um `bool` encaixotado não é "diferente de zero": o handle da
            // caixa de `false` também é. Volta pelo `Unbox` (R3).
            Type::Ref => return self.coerce(op, Type::I1),
            _ => {}
        }
        self.emit(
            Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))),
            Type::I1,
        )
    }

    /// Representação do tipo estático de uma expressão, quando ele é
    /// conhecido e não é `dynamic`.
    pub fn expression_repr(&self, e: ExprId) -> Option<Type> {
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
            debug_info: None,
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
            scopes: vec![super::locals::LocalScope::new(0)],
            next_scope: 1,
            late_initializers_in_lowering: std::collections::HashSet::new(),
            n_allocas: 0,
            value_types: HashMap::new(),
            dart_scalar_parameters: HashMap::new(),
            dart_rti_parameters: HashMap::new(),
            dart_vector_parameters: HashMap::new(),
            arc_uniform_returns: Default::default(),
            dart_ref_returns: Default::default(),
            arc_rti_caches: Default::default(),
            arc_shared_functions: Default::default(),
            arc_direct_local_functions: Default::default(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            this_param: None,
            finalizable_this: None,
            enclosing_class: None,
            exception_targets: Vec::new(),
            finally_scopes: Vec::new(),
            active_catch_stack: Vec::new(),
            terminated_blocks: std::collections::HashSet::new(),
            extra_functions: Vec::new(),
            labeled_break_targets: HashMap::new(),
            labeled_continue_targets: HashMap::new(),
            pending_labels: Vec::new(),
            callee_type: None,
            current_call: None,
            current_cascade_target: None,
            receiver_ready: None,
            handled_null_aware: None,
            errors: Vec::new(),
            null_chain: None,
            continue_chain: false,
            old_value: None,
            cells: std::collections::HashSet::new(),
            n_closures: 0,
            local_names: std::collections::HashSet::new(),
            done_entries: std::collections::HashSet::new(),
            extra_globals: Vec::new(),
            constant_in_progress: None,
            in_const_context: false,
            local_const_keys: HashMap::new(),
            refutable_pattern: false,
            cast_by_class_only: false,
            in_adapter: false,
            guaranteed_arity: false,
            arg_types: None,
            checked_args: std::collections::HashSet::new(),
            allowed_directs: std::collections::HashSet::new(),
            assigned: std::collections::HashSet::new(),
            pointers: std::collections::HashSet::new(),
            direct_functions: HashMap::new(),
            without_directs: false,
            typed_data: None,
            header_caches: std::collections::HashMap::new(),
            read_from_global: std::collections::HashSet::new(),
            async_state: None,
            function_type_params: Vec::new(),
            local_params: Vec::new(),
            class_by_tuple: false,
            type_tuple: None,
            concrete_class: None,
            creation_type: None,
            armed_tuple: None,
            this_extension: None,
            this_ext_type: None,
            constructor_return: None,
            pattern_tuple_te: None,
            fixed_lists: Vec::new(),
            access_fixed: None,
            proven_typed: Vec::new(),
            access_proven: None,
            member_class: None,
            callback_sites: 0,
            position: None,
            stack_trace: FunctionStackTrace::default(),
        }
    }

    /// Passa à depuração da função o que o rastro precisa dela e insere a
    /// pilha do rastro, quando pedida. Sem o rastro simbólico, nada.
    pub fn close_stack_trace(&mut self) {
        if !self.ctx.stack_trace {
            return;
        }
        let r = std::mem::take(&mut self.stack_trace);
        let needs = r.marks != 0 || r.async_body.is_some() || r.closure_entry.is_some() || r.chain_link.is_some() || r.stack.is_some();
        if self.func.debug_info.is_none() {
            if !needs {
                return;
            }
            let unit = self.ctx.program.unit(self.unit_id);
            self.func.debug_info = Some(Box::new(crate::hir::FunctionDebugInfo {
                file: unit.path.as_ref().map_or_else(|| unit.uri.clone(), |p| p.display().to_string()),
                url: self.ctx.stack_trace_url(self.unit_id),
                line: r.token.map_or(0, |t| t.0),
                ..Default::default()
            }));
        }
        if let Some(d) = self.func.debug_info.as_mut() {
            d.token = r.token.unwrap_or((d.line, 0));
            d.marks |= r.marks;
            d.async_body = r.async_body;
            d.closure_entry = r.closure_entry;
            d.chain_link = r.chain_link;
        }
        if let Some((ty, value)) = r.stack {
            self.instrument_stack_trace_stack(ty, value);
        }
    }

    /// `dartforge_rastro_entrar(ty, value)` no começo do bloco de entrada
    /// (depois da definição de `value`, quando é dele) e
    /// `dartforge_rastro_sair(depth)` antes de cada `Return` e `Throw`:
    /// a profundidade devolvida pela entrada corta também o que funções
    /// desenroladas por exceção deixaram em cima.
    fn instrument_stack_trace_stack(&mut self, ty: i64, value: Operand) {
        if self.func.blocks.is_empty() {
            return;
        }
        let new = |n: &mut u32| {
            let v = ValueId(*n);
            *n += 1;
            v
        };
        let prof = new(&mut self.next_value);
        let call = Instruction::CallRuntime {
            name: "dartforge_rastro_entrar".to_string(),
            args: vec![(Operand::Constant(Constant::Int(ty)), Type::I64), (value.clone(), Type::Ref)],
            ret_ty: Type::I64,
        };
        self.value_types.insert(prof, Type::I64);
        let block = &mut self.func.blocks[0];
        let after = match &value {
            Operand::Val(v) => block.instructions.iter().position(|(x, _, _)| x == v).map_or(0, |i| i + 1),
            _ => 0,
        };
        // Um `phi` não há no bloco de entrada; os `alloca` podem ficar antes.
        block.instructions.insert(after, (prof, call, Type::I64));
        let mut exits = Vec::new();
        for (bi, b) in self.func.blocks.iter().enumerate() {
            if matches!(b.terminator, Terminator::Return(_) | Terminator::Throw(_)) {
                exits.push(bi);
            }
        }
        for bi in exits {
            let v = new(&mut self.next_value);
            self.value_types.insert(v, Type::Void);
            self.func.blocks[bi].instructions.push((
                v,
                Instruction::CallRuntime {
                    name: "dartforge_rastro_sair".to_string(),
                    args: vec![(Operand::Val(prof), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ));
        }
    }

    /// J05: a posição corrente passa a ser a do byte `offset` de `ast`, se é
    /// a árvore da unidade desta função (a posição é dela) e a depuração
    /// está ligada.
    pub fn mark_position(&mut self, ast: &ast::Ast, offset: usize) {
        if self.ctx.debug_info.is_none() || !std::ptr::eq(ast, &self.ctx.program.unit(self.unit_id).ast) {
            return;
        }
        self.position = self.ctx.line_and_column(self.unit_id, offset);
    }

    /// Declara `this` (quando `with_this`) e os parâmetros do outline de `fid`.
    pub fn declare_parameters(&mut self, fid: usize, with_this: bool) {
        if with_this {
            let this_vid = self.add_param("this".to_string(), Type::Ref);
            self.this_param = Some(Operand::Val(this_vid));
            self.finalizable_this = self.ctx.classify_this(fid);
        }
        let Some(data) = self.ctx.outline.functions.get(fid) else {
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
        for (i, p) in data.parameters.iter().enumerate() {
            let p_name = p
                .name
                .map(|s| self.ctx.symbol_name(s).to_string())
                .unwrap_or_else(|| "arg".to_string());
            let p_ty = self.repr(p.ty);
            let vid = self.add_param(p_name, p_ty);
            if self.ctx.arc_memory && matches!(p_ty, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
                self.dart_scalar_parameters.entry(self.func.symbol.clone()).or_default().insert(vid);
            }
            if let Some(sym) = p.name {
                match ast_params.get(i).and_then(|a| a.name) {
                    Some(n) => {
                        self.declare_variable(sym, n.span.start as usize, p_ty, Operand::Val(vid))
                    }
                    None => self.declare_local_with_value(sym, p_ty, Operand::Val(vid)),
                }
                // O outline guarda o tipo Dart mesmo sem offset no corpo.
                self.assign_semantic_type(sym, Some(p.ty));
            }
        }
    }

    /// Entrega a função (e as funções locais) ao módulo, com os diagnósticos.
    pub fn finalizar(mut self, module: &mut Module) {
        self.close_stack_trace();
        // P6: o diagnóstico de uma função da fonte só vale se a poda a
        // mantiver (`source::prune`).
        let lib = self.ctx.program.unit(self.unit_id).library;
        if self.ctx.from_source.contains(&lib) && crate::source::sdk_symbol(&self.func.symbol) {
            if !self.errors.is_empty() {
                module.source_errors.push((self.func.symbol.clone(), self.errors));
            }
        } else {
            module.errors.extend(self.errors);
        }
        module.globals.extend(self.extra_globals);
        module.dart_scalar_parameters.extend(self.dart_scalar_parameters);
        module.dart_rti_parameters.extend(self.dart_rti_parameters);
        module.dart_vector_parameters.extend(self.dart_vector_parameters);
        module.arc_uniform_returns.extend(self.arc_uniform_returns);
        module.dart_ref_returns.extend(self.dart_ref_returns);
        module.arc_rti_caches.extend(self.arc_rti_caches);
        module.arc_shared_functions.extend(self.arc_shared_functions);
        module.arc_direct_local_functions.extend(self.arc_direct_local_functions);
        module.functions.push(self.func);
        module.functions.extend(self.extra_functions);
    }

    /// Construto que o lowering não sabe baixar: diagnóstico com posição
    /// (N1). O operando devolvido nunca chega a ser emitido — um módulo com
    /// erros não gera código.
    /// O corpo de uma função (ou getter de global) que o mundo fechado do
    /// programa não alcança (C7, `native_world.rs`): lança
    /// `UnsupportedError` com o símbolo. Só roda se a análise errou — o erro
    /// é alto e diz o que faltou.
    pub fn pruned_body(&mut self) {
        let msg = format!("dartforge: código podado como inalcançável foi chamado: {}", self.func.symbol);
        let m = self.emit(Instruction::Const(Constant::String(msg)), Type::Ref);
        let e = self.new_runtime_error("dartforge_unsupported_error_new", vec![(m, Type::Ref)]);
        self.emit_throw_op(e);
    }

    pub fn unsupported(&mut self, what: &str, span: dartforge_diagnostics::Span) -> Operand {
        let unit = self.ctx.program.unit(self.unit_id);
        // P6: no código do SDK compilado da fonte (mundo aberto: a poda é
        // conservadora — o `toString` de toda classe, todo alvo de um
        // despacho), o construto que falta vira `UnsupportedError` em tempo
        // de execução, com o mesmo texto: o programa que não passa por ali
        // compila, e o que passa falha alto, nunca em silêncio.
        if self.ctx.from_source.contains(&unit.library) {
            // `DARTFORGE_FONTE_NAO_SUPORTADO=1`: lista, na compilação, cada
            // construto que virou `UnsupportedError` no código da fonte.
            if std::env::var_os("DARTFORGE_FONTE_NAO_SUPORTADO").is_some() {
                eprintln!("fonte não suportado em {}: {what}", self.func.symbol);
            }
            if !self.is_terminated() {
                let msg = format!("{}{what}", crate::UNSUPPORTED_PREFIX);
                let m = self.emit(Instruction::Const(Constant::String(msg)), Type::Ref);
                let e = self.new_runtime_error("dartforge_unsupported_error_new", vec![(m, Type::Ref)]);
                self.emit_throw_op(e);
            }
            return Operand::Constant(Constant::Null);
        }
        let position = self.position(span);
        self.errors.push(format!("{}{what} ({position})", crate::UNSUPPORTED_PREFIX));
        Operand::Constant(Constant::Null)
    }

    /// `file:line:column` de `span` na unidade corrente.
    fn position(&self, span: dartforge_diagnostics::Span) -> String {
        let unit = self.ctx.program.unit(self.unit_id);
        let source = &unit.source;
        let ini = span.start.min(source.len());
        let before = &source[..ini];
        let line = before.matches('\n').count() + 1;
        let column = ini - before.rfind('\n').map_or(0, |p| p + 1) + 1;
        let file = unit
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| unit.uri.clone(), |n| n.to_string_lossy().into_owned());
        format!("{file}:{line}:{column}")
    }

    /// Um erro de compilação da linguagem (o programa é inválido, não um
    /// construto que falta), com o texto do front-end da VM. No código do
    /// SDK compilado da fonte não acontece: o SDK é válido.
    pub fn language_error(&mut self, message: &str, span: dartforge_diagnostics::Span) -> Operand {
        let position = self.position(span);
        self.errors.push(format!("{message} ({position})"));
        Operand::Constant(Constant::Null)
    }

    /// Converte um operando para a representação `to` (R4).
    ///
    /// É o único lugar que muda representação: escalar numa posição `Ref`
    /// vira caixa (`Box`), `Ref` numa posição escalar volta por `Unbox` —
    /// que lança `TypeError` para null ou outro tipo, com a verificação de
    /// exceção de uma chamada. `int` para `double` é conversão numérica (o
    /// literal `1` num contexto `double`); os bits de `double` que vêm do
    /// heap usam `Bitcast` explícito, nunca esta função.
    pub fn coerce(&mut self, op: Operand, to: Type) -> Operand {
        let de = self.operand_type(&op);
        if de == to
            || matches!(to, Type::Void | Type::Ptr)
            || matches!(de, Type::Void | Type::Ptr)
        {
            return op;
        }
        match (de, to) {
            // O vetor SIMD e a caixa do tipo estático (`lower/simd.rs`): o
            // tipo estático não anulável garante a caixa certa.
            (k, Type::Ref) if k.is_vector() => self.emit(Instruction::Box { op, from: k }, Type::Ref),
            (Type::Ref, k) if k.is_vector() => self.emit(Instruction::Unbox { op, to: k }, k),
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
                self.emit_call_with_check(Instruction::Unbox { op, to: to }, to)
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
                    to: to,
                },
                to,
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
    /// usa `lower_expr_of` com a unidade da variável).
    pub fn variable_initializer_in(
        &self,
        var_id: dartforge_elements::model::VariableId,
    ) -> Option<dartforge_frontend::ast::ExprId> {
        super::layouts_arc::initializer(self.ctx, var_id)
    }

    pub fn add_param(&mut self, name: String, ty: Type) -> ValueId {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty);
        self.func.params.push((vid, name, ty));
        vid
    }

    /// Declara um vetor da entrada uniforme, sem inferir papel pelo tipo Ptr.
    pub(super) fn add_param_vector(&mut self, role: DartEntryVector) -> ValueId {
        let name_str = match role { DartEntryVector::Argumentos => "args", DartEntryVector::Descriptor => "desc" };
        let id = self.add_param(name_str.to_string(), Type::Ptr);
        if self.ctx.arc_memory {
            self.dart_vector_parameters.entry(self.func.symbol.clone()).or_default().insert(id, role);
        }
        id
    }

    /// Declara a convenção alvo de uma entrada uniforme gerada nesta imagem.
    /// Só a preparação/verificação completa pode publicar seu corpo Owned;
    /// callbacks e dispatch indireto ainda exigem contratos dos chamadores.
    pub(super) fn declare_arc_uniform_return(&mut self) {
        if !self.ctx.arc_memory || self.func.return_ty != Type::Ref { return; }
        assert_eq!(self.func.params.iter().map(|p| p.2).collect::<Vec<_>>(),
            [Type::Ref, Type::Ptr, Type::Ptr], "declaração de entrada uniforme incompatível");
        let roles = &self.dart_vector_parameters[&self.func.symbol];
        assert_eq!(roles.get(&self.func.params[1].0), Some(&DartEntryVector::Argumentos));
        assert_eq!(roles.get(&self.func.params[2].0), Some(&DartEntryVector::Descriptor));
        self.arc_uniform_returns.insert(self.func.symbol.clone());
    }

    /// Um parâmetro na representação do tipo Dart (de closure, função local
    /// ou captura por valor): sob ARC, a representação escalar é fato
    /// escalar, como em `declare_parameters`.
    pub(super) fn add_param_dart(&mut self, name: String, ty: Type) -> ValueId {
        let v = self.add_param(name, ty);
        if self.ctx.arc_memory && matches!(ty, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
            self.dart_scalar_parameters.entry(self.func.symbol.clone()).or_default().insert(v);
        }
        v
    }

    /// Declara um ID do universo RTI, sem inferir referência pela largura.
    /// O chamador deve passar somente IDs canônicos nativos de tipos/tuplas.
    pub fn add_param_rti(&mut self, name: String) -> ValueId {
        let v = self.add_param(name, Type::I64);
        if self.ctx.arc_memory {
            self.dart_rti_parameters.entry(self.func.symbol.clone()).or_default().insert(v);
        }
        v
    }

    pub fn operand_type(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.value_types.get(v).cloned().unwrap_or(Type::Ref),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::Function(_)) => Type::I64,
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
        if let Some((pos, d)) = self.function_debug_info() {
            d.positions.insert(vid, pos);
        }
        Operand::Val(vid)
    }

    /// J05: a posição corrente e as posições da função (criadas na primeira
    /// instrução com posição), com a depuração ligada.
    fn function_debug_info(&mut self) -> Option<((u32, u32), &mut crate::hir::FunctionDebugInfo)> {
        let pos = self.position?;
        let unit = self.ctx.program.unit(self.unit_id);
        let d = self.func.debug_info.get_or_insert_with(|| {
            Box::new(crate::hir::FunctionDebugInfo {
                file: unit.path.as_ref().map_or_else(|| unit.uri.clone(), |p| p.display().to_string()),
                url: self.ctx.stack_trace_url(self.unit_id),
                line: pos.0,
                ..Default::default()
            })
        });
        Some((pos, d))
    }

    pub fn terminate(&mut self, term: Terminator) {
        if self.is_terminated() {
            return;
        }
        // P6: o `return` de um corpo `async` completa o `Future`
        // (`_asyncReturn`); a suspensão e o tratador do topo são crus.
        if let Terminator::Return(r) = &term
            && self.async_state.as_ref().is_some_and(|e| !e.raw_return)
        {
            let r = r.clone();
            self.async_return(r);
            return;
        }
        // R4: o valor devolvido na representação do retorno da função.
        let term = match term {
            Terminator::Return(Some(op)) if !matches!(self.func.return_ty, Type::Void) => {
                let r = self.func.return_ty;
                // `=> print(x)` numa função que devolve valor: a expressão
                // `void` vale null (não há valor SSA a devolver).
                let op = if matches!(self.operand_type(&op), Type::Void) {
                    Self::zero_value(r)
                } else {
                    op
                };
                let op = self.coerce(op, r);
                if self.is_terminated() {
                    return;
                }
                Terminator::Return(Some(op))
            }
            // `set x(v) => expr;` numa função void: a expressão já foi
            // avaliada e o valor dela é descartado (o LLVM emite `ret void`).
            Terminator::Return(Some(_)) => Terminator::Return(None),
            t => t,
        };
        self.terminated_blocks.insert(self.current_block);
        let block = self.current_block;
        if let Some((pos, d)) = self.function_debug_info() {
            d.exits.insert(block, pos);
        }
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
        let ret_val = ret_val.or_else(|| self.constructor_return.clone());
        // O valor na representação do retorno da função (o corpo de uma
        // closure tipada devolve `int` sem caixa, por exemplo).
        let ret_val = match ret_val {
            Some(r) if !matches!(self.func.return_ty, Type::Void) && self.finally_scopes.is_empty() => {
                Some(self.coerce(r, self.func.return_ty))
            }
            other => other,
        };
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
            let val = self.coerce(val, ty_phi);
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
        self.jump(false, label);
    }

    /// `break`/`continue` (com ou sem rótulo): direto ao alvo, ou pelo
    /// `finally` mais interno que o salto atravessa (a razão `5 + k` do
    /// salto `k` desse `finally`; no fim dele o salto continua — por outros
    /// `finally` de fora, se atravessar mais). Um salto para um alvo DENTRO
    /// do `try` (o laço do próprio corpo) não passa pelo `finally`.
    pub fn jump(&mut self, is_continue: bool, label: Option<SymbolId>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let (stack, labeled_set) = if is_continue {
            (&self.continue_targets, &self.labeled_continue_targets)
        } else {
            (&self.break_targets, &self.labeled_break_targets)
        };
        let target_opt = match label {
            Some(sym) => labeled_set.get(&sym).copied(),
            None => stack.last().copied(),
        };
        let index = stack.len().checked_sub(1);
        if let Some(target) = target_opt {
            let crosses = self.finally_scopes.last().is_some_and(|fin| match label {
                Some(sym) => {
                    if is_continue {
                        fin.continue_labels.contains(&sym)
                    } else {
                        fin.break_labels.contains(&sym)
                    }
                }
                None => {
                    let prof = if is_continue { fin.prof_continue } else { fin.prof_break };
                    index.is_some_and(|i| i < prof)
                }
            });
            if !crosses {
                self.terminate(Terminator::Branch(target));
            } else {
                let default_ret = self.default_return_operand();
                let current = self.current_block;
                let fin = self.finally_scopes.last_mut().unwrap();
                let k = match fin.jumps.iter().position(|s| *s == (is_continue, label)) {
                    Some(k) => k,
                    None => {
                        fin.jumps.push((is_continue, label));
                        fin.jumps.len() - 1
                    }
                };
                fin.incoming.push((current, 5 + k as i64, default_ret));
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
        self.jump(true, label);
    }

    /// O ponto seguro de uma volta de laço (J01), no começo do corpo: lê o
    /// pedido de interrupção do isolado (um byte, pelo contexto da thread) e,
    /// com pedido, chama o runtime, que atende a porta de controle (`ping`,
    /// pausa, `kill`). Um `kill` imediato deixa a exceção pendente não
    /// capturável, e o caminho de exceção comum desenrola o isolado. É o papel
    /// da verificação de pilha da VM nas voltas de laço.
    pub fn emit_safepoint(&mut self) {
        if self.is_terminated() {
            return;
        }
        let request = self.emit(
            Instruction::CallRuntime { name: "dartforge_interrupcao_pendente".to_string(), args: Vec::new(), ret_ty: Type::I8 },
            Type::I8,
        );
        let ha = self.emit(Instruction::ICmp(ICmpOp::Ne, request, Operand::Constant(Constant::Int(0))), Type::I1);
        let slow = self.new_block();
        let follows = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ha, then_block: slow, else_block: follows });
        self.set_block(slow);
        self.emit_call_with_check(
            Instruction::CallRuntime { name: "dartforge_ponto_seguro".to_string(), args: Vec::new(), ret_ty: Type::Void },
            Type::Void,
        );
        self.terminate(Terminator::Branch(follows));
        self.set_block(follows);
    }

    pub fn emit_call_with_check(&mut self, inst: Instruction, ret_ty: Type) -> Operand {
        // A extern que a tabela de efeitos marca sem lançar (e sem rodar
        // Dart) não deixa exceção pendente: não há o que conferir depois
        // dela (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.2, E1.1). A
        // própria leitura da pendência é a conferência avulsa (o fim de um
        // comando dentro de `try`) e continua.
        // `DARTFORGE_SEM_EFEITOS_NA_CONFERENCIA=1` volta a conferir tudo (medida).
        if let Instruction::CallRuntime { name, .. } = &inst
            && name != "dartforge_exception_pending"
            && !Self::check_every_extern()
        {
            let e = crate::llvm::externs::effects_of(name);
            if !e.throws && !e.calls_dart {
                return self.emit(inst, ret_ty);
            }
        }
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

    /// A conferência depois de toda extern, também das que não lançam (o
    /// comportamento anterior a E1.1, para medir).
    fn check_every_extern() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var_os("DARTFORGE_SEM_EFEITOS_NA_CONFERENCIA").is_some_and(|v| !v.is_empty() && v != "0")
        })
    }

    pub fn emit_throw(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let ex_op = self.lower_expr(ast, expr_id);
        self.emit_throw_op(ex_op)
    }

    pub fn emit_throw_op(&mut self, ex_op: Operand) -> Operand {
        self.emit_throw_value(ex_op, None);
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
        if let Some((ex_op, _)) = self.active_catch_stack.last().cloned() {
            self.emit_throw_value(ex_op, None);
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
    pub fn test_type(&mut self, ast_ty: &ast::TypeAnnotation, op: Operand) -> Operand {
        if let Some(r) = self.test_source_type(ast_ty, op.clone()) {
            return r;
        }
        // `x is List<int>`, `x is T`, `x is FutureOr<T>`, tipo de função ou
        // de record: pelo RTI (`rti.rs`). O teste pela classe abaixo só vale
        // para a classe sem argumentos de tipo (ou com argumentos triviais).
        if self.annotation_needs_rti(ast_ty) {
            return match self.annotation_recipe(ast_ty) {
                Some(r) => {
                    let t = self.recipe_rti(&r);
                    self.test_rti(op, t)
                }
                None => self.unsupported("teste de tipo com nome não resolvido", ast_ty.span),
            };
        }
        let ast::TypeKind::Named { name, .. } = &ast_ty.kind else {
            return self.unsupported("teste de tipo estrutural", ast_ty.span);
        };
        let Some(last_one) = name.last() else {
            return self.unsupported("teste de tipo", ast_ty.span);
        };
        let name_str = self.ctx.symbol_name(last_one.sym).to_string();
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            let r = matches!(
                (name_str.as_str(), repr),
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
        let equal = |b: &mut Self, v: i64| {
            b.emit(
                Instruction::ICmp(ICmpOp::Eq, cls.clone(), Operand::Constant(Constant::Int(v))),
                Type::I1,
            )
        };
        let base = match name_str.as_str() {
            "dynamic" => Operand::Constant(Constant::Bool(true)),
            "Object" => self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    cls.clone(),
                    Operand::Constant(Constant::Int(-12)),
                ),
                Type::I1,
            ),
            "Null" => equal(self, -12),
            "num" => {
                let i = equal(self, -9);
                let d = equal(self, -10);
                self.emit(Instruction::Or(i, d), Type::I1)
            }
            _ => {
                let lib = self.ctx.program.unit(self.unit_id).library;
                // `p.Tipo` (import com prefixo) ou `Tipo`.
                let binding = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, last_one.sym),
                };
                let cid = binding
                    .and_then(|b| match b.getter {
                        Some(dartforge_elements::model::Element::Class(c)) => Some(c),
                        _ => None,
                    });
                let Some(id) = cid.and_then(|c| self.class_id_of(c)) else {
                    return self.unsupported(&format!("teste de tipo `{name_str}`"), ast_ty.span);
                };
                if id < 0 {
                    equal(self, id)
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
        let null = equal(self, -12);
        self.emit(Instruction::Or(base, null), Type::I1)
    }

    /// `as T` implícito ou explícito: `TypeError` se o valor não é um `T`,
    /// com a mensagem da VM do `context` ("… in type cast" no `as`, sem
    /// sufixo na atribuição implícita de um `dynamic`).
    pub fn check_type_or_throw(&mut self, ast_ty: &ast::TypeAnnotation, op: Operand, context: super::rti::CastContext) {
        // `as List<int>`, `as T`…: o cast inteiro pelo RTI.
        if self.annotation_needs_rti(ast_ty) {
            match self.annotation_recipe(ast_ty) {
                Some(r) => {
                    let t = self.recipe_rti(&r);
                    self.cast_rti_in(op, t, context);
                }
                None => {
                    self.unsupported("cast com nome não resolvido", ast_ty.span);
                }
            }
            return;
        }
        let ok = self.test_type(ast_ty, op.clone());
        let ok = self.to_bool(ok);
        let fail_b = self.new_block();
        let pass_b = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: pass_b,
            else_block: fail_b,
        });
        self.set_block(fail_b);
        // A falha (caminho frio): o RTI decide e lança com a mensagem da VM
        // — o tipo dinâmico do valor e o da anotação.
        match self.annotation_recipe(ast_ty) {
            Some(r) => {
                let t = self.recipe_rti(&r);
                self.cast_rti_in(op, t, context);
                self.terminate(Terminator::Branch(pass_b));
            }
            None => {
                let err_op = self.new_runtime_error("dartforge_type_error_new", Vec::new());
                self.emit_throw_op(err_op);
            }
        }
        self.set_block(pass_b);
    }

    pub fn emit_trunc_div(&mut self, lop: Operand, rop: Operand) -> Operand {
        match rop {
            // Divisor constante: nem zero a conferir, nem o -1.
            Operand::Constant(Constant::Int(-1)) => return self.emit(Instruction::Neg(lop), Type::I64),
            Operand::Constant(Constant::Int(c)) if c != 0 => return self.emit(Instruction::SDiv(lop, rop), Type::I64),
            _ => {}
        }
        self.require_divisor(&rop);
        // `x ~/ -1` é `-x` com o estouro de 64 bits (`-2^63 ~/ -1 == -2^63`);
        // o `sdiv` do LLVM não define esse caso (o `idiv` do x86 dá SIGFPE).
        let minus_one = self.emit(Instruction::ICmp(ICmpOp::Eq, rop.clone(), Operand::Constant(Constant::Int(-1))), Type::I1);
        let negate = self.new_block();
        let split = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: minus_one, then_block: negate, else_block: split });
        self.set_block(negate);
        let n = self.emit(Instruction::Neg(lop.clone()), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(split);
        let q = self.emit(Instruction::SDiv(lop, rop), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(negate, n), (split, q)], ty: Type::I64 }, Type::I64)
    }

    /// `a % b` de `int` com divisor variável, em linha depois do guarda do
    /// zero: o resto truncado mais `|b|` quando ele é negativo, o que dá o
    /// valor em `[0, |b|)` do Dart (`Integer::ArithmeticOp`, kMOD). `b == -1`
    /// dá sempre 0 e evita o `srem` de `i64::MIN` por `-1`, indefinido no
    /// LLVM; `|i64::MIN|` dá `i64::MIN`, que somado dá o mesmo que subtraído.
    pub fn emit_module(&mut self, lop: Operand, rop: Operand) -> Operand {
        self.require_divisor(&rop);
        let int = |c| Operand::Constant(Constant::Int(c));
        let minus_one = self.emit(Instruction::ICmp(ICmpOp::Eq, rop.clone(), int(-1)), Type::I1);
        let zero = self.new_block();
        let rest = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: minus_one, then_block: zero, else_block: rest });
        self.set_block(zero);
        self.terminate(Terminator::Branch(join));
        self.set_block(rest);
        let r = self.emit(Instruction::SRem(lop, rop.clone()), Type::I64);
        let sign_b = self.emit(Instruction::AShr(rop.clone(), int(63)), Type::I64);
        let inverted = self.emit(Instruction::Xor(rop, sign_b.clone()), Type::I64);
        let module_b = self.emit(Instruction::Sub(inverted, sign_b), Type::I64);
        let sign_r = self.emit(Instruction::AShr(r.clone(), int(63)), Type::I64);
        let adjustment = self.emit(Instruction::And(sign_r, module_b), Type::I64);
        let value = self.emit(Instruction::Add(r, adjustment), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(zero, int(0)), (rest, value)], ty: Type::I64 }, Type::I64)
    }

    /// Divisor inteiro zero (`~/` e `%` de `int`) lança
    /// `IntegerDivisionByZeroException`; o bloco corrente segue no caminho do
    /// divisor válido.
    pub fn require_divisor(&mut self, rop: &Operand) {
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
        // SDK da fonte: o `IntegerDivisionByZeroException` do `dart:core`, o
        // que a VM lança (e o que `on IntegerDivisionByZeroException` pega).
        if let Some(class) = self.ctx.sdk_class("core", "IntegerDivisionByZeroException")
            && let Some(empty) = self.ctx.interner.lookup("")
            && let Some(&ctor) = self.ctx.program.classes[class.0 as usize].constructors.get(&empty)
        {
            let error = self.instantiate_evaluated(ctor, &[], dartforge_diagnostics::Span { start: 0, end: 0 });
            if !self.is_terminated() {
                self.emit_throw_op(error);
            }
            self.set_block(normal_div_block);
            return;
        }
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
        self.emit_throw_value(err_obj, None);
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
    }

    /// Operador de uma atribuição composta (`a op= b`).
    pub fn lower_binary_op_helper(&mut self, op: BinaryOp, lop: Operand, rop: Operand, admits_smi: bool) -> Operand {
        // O valor corrente em `Ref` pode ser `String`, um número anulável ou
        // outro objeto. O tipo do operando direito não prova que ele seja
        // numérico (`s *= 2`); o despacho dinâmico trata esses casos.
        let (tl, tr) = (self.operand_type(&lop), self.operand_type(&rop));
        let rop = if tr == Type::Ref && tl == Type::F64 {
            // `d op= n` com `d` `double` e `n` `num`: o `int` vira `double`
            // (o `other.toDouble()` dos operadores de `_Double`). O caminho
            // que desencaixa pode deixar `TypeError` pendente.
            self.emit_call_with_check(
                Instruction::CallRuntime { name: "df.num_para_double".to_string(), args: vec![(rop, Type::Ref)], ret_ty: Type::F64 },
                Type::F64,
            )
        } else if tr == Type::Ref && tl == Type::I64 {
            self.coerce(rop, tl)
        } else {
            rop
        };
        self.operate_with_smi(
            op,
            lop,
            rop,
            false,
            admits_smi,
            dartforge_diagnostics::Span { start: 0, end: 0 },
        )
    }
}

/// Uma lista do runtime com comprimento e endereço dos elementos lidos uma
/// vez antes das voltas de um laço que não pode mudá-los (N13).
#[derive(Debug, Clone)]
pub struct FixedList {
    /// O endereço do local da lista (o `alloca` dele): a identidade do
    /// local, também de um parâmetro.
    pub key: Operand,
    pub length: Operand,
    pub data: Operand,
    /// A forma dos elementos (N14, `heap::FormaDeLista`: 0 geral, 1 `int`,
    /// 2 `double`, 3 `bool`), conferida antes das voltas: nelas, o
    /// tamanho e a interpretação de cada elemento são conhecidos na emissão.
    pub shape: i64,
    /// Numa forma compacta, o comprimento para as gravações diretas
    /// (`dartforge_lista_len_gravavel`, chamado antes das voltas: o
    /// comprimento se a lista é modificável, senão 0 — e toda gravação vai
    /// ao `[]=` do SDK).
    pub writable_length: Option<Operand>,
}

/// Uma lista tipada (`dart:typed_data`) das voltas rápidas de um laço
/// versionado (`statements::Counted::TypedSet`): antes delas, provou-se que
/// todo valor do contador fica em `0..length`, e o comprimento e o
/// endereço dos elementos foram lidos uma vez — fixos enquanto a lista
/// vive (lista tipada não muda de tamanho, e os bytes moram atrás de um
/// cabeçalho de endereço fixo, `heap::CabecalhoTipado`).
#[derive(Debug, Clone)]
pub struct ProvenTyped {
    /// O endereço do local da lista (o `alloca` dele).
    pub key: Operand,
    /// O endereço do local do contador do laço.
    pub counter: Operand,
    pub data: Operand,
    pub length: Operand,
    /// Provada também para a gravação: o comprimento de uma visão veio de
    /// `dartforge_typed_len(.., 1)`, que dá 0 para a não modificável.
    pub write: bool,
}

//! Variáveis locais: escopo léxico e `alloca` na representação do tipo (R6),
//! e as variáveis capturadas por closures (P1).
//!
//! Antes, um local era um `alloca i64` criado onde a declaração aparecia (no
//! meio de um laço, o `alloca` crescia a pilha a cada volta) e um mapa único
//! por nome: um bloco que sombreava `current` sobrescrevia o `current` de
//! fora para sempre, e a variável de um `for (var i = 0; …; i++)` nem tinha
//! `alloca` — o `i++` trocava o valor SSA no mapa, e a condição do laço,
//! baixada antes, continuava lendo o `0` da entrada.
//!
//! Uma variável capturada por uma closure e **atribuída** em algum ponto
//! (`capture.rs` decide, antes de baixar a função) mora numa `Cell` do heap: o
//! `alloca` guarda o handle da célula, e quem lê ou grava passa por ela — a
//! closure vê a gravação feita depois da captura, e a gravação dela é vista
//! de fora. Capturada e nunca atribuída, ela é copiada para o ambiente da
//! closure na criação. Dentro da closure, a variável de fora é uma posição do
//! ambiente (`Environment`), com o handle da célula ou a cópia.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::ExprId;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Identidade semântica preservada ao retirar/restaurar casos de switch.
/// Zero é a invocação; os demais IDs não são profundidades reutilizáveis.
#[derive(Debug, Clone)]
pub(super) struct LocalScope {
    pub id: u32,
    locals: HashMap<SymbolId, Local>,
}

impl LocalScope {
    pub(super) fn new(id: u32) -> Self {
        Self {
            id,
            locals: HashMap::new(),
        }
    }
}

impl std::ops::Deref for LocalScope {
    type Target = HashMap<SymbolId, Local>;
    fn deref(&self) -> &Self::Target {
        &self.locals
    }
}

impl std::ops::DerefMut for LocalScope {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.locals
    }
}

/// Onde um local visível mora.
#[derive(Debug, Clone)]
pub enum Mode {
    /// `alloca` na representação do local.
    Memory(Operand),
    /// Valor imutável já calculado (o parâmetro de uma closure baixada em
    /// linha pelo código congelado de `sdk_por_nome.rs`).
    Value(Operand),
    /// `alloca` `Ref` com o handle da célula que guarda o valor.
    Cell(Operand),
    /// Posição `index` do ambiente da closure corrente (`env`); com
    /// `cell`, o que está lá é o handle da célula.
    Environment {
        env: Operand,
        index: usize,
        cell: bool,
    },
}

/// Um local visível.
#[derive(Debug, Clone)]
pub struct Local {
    pub mode: Mode,
    pub ty: Type,
    /// Tipo Dart antes de apagar a representação, para obrigações léxicas
    /// como Finalizable. None indica informação ainda não fornecida; não
    /// prova ausência da obrigação. Capturas preservam o TypeId original.
    pub static_type: Option<dartforge_types::TypeId>,
    /// Classificação da obrigação léxica. None ainda exige prova; não emite
    /// nem substitui ArcKeepAlive ou uma ocorrência proprietária.
    pub finalizable: Option<bool>,
    /// Escopo desta ligação, atribuído ao inseri-la no corpo corrente.
    /// Capturas preservam o tipo original, mas não o escopo de outra função.
    /// Não é uma prova de cleanup nem uma ocorrência proprietária.
    pub scope: u32,
    /// Offset do nome na declaração (a chave de `capture.rs`); `None` para
    /// os ligados por valor.
    pub offset: Option<usize>,
    /// Marca separada do valor, pois zero/null são atribuições legítimas.
    pub late: Option<LateLocal>,
}

#[derive(Debug, Clone)]
pub struct LateLocal {
    pub initialized: Operand,
    /// `true`: `initialized` é uma `Cell` do heap e usa índice -1 na
    /// tabela lateral; `false`: é um `alloca i8` desta função.
    pub cell: bool,
    pub final_: bool,
    pub name: String,
    pub initializer: Option<(UnitId, ExprId)>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn open_scope(&mut self) {
        let id = self.next_scope;
        self.next_scope = id.checked_add(1).expect("limite de escopos léxicos");
        debug_assert!(self.scopes.iter().all(|e| e.id != id));
        self.scopes.push(LocalScope::new(id));
    }

    pub fn close_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// `alloca` no começo do bloco de entrada: executa uma vez por
    /// ativação e domina todos os usos (o Clang recusava `alloca` criado
    /// num ramo e usado depois da junção).
    pub fn entry_alloca(&mut self, ty: Type) -> Operand {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, Type::Ptr);
        let b0 = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == BlockId(0))
            .expect("bloco de entrada");
        self.func.blocks[b0]
            .instructions
            .insert(self.n_allocas, (vid, Instruction::Alloca(ty), Type::Ptr));
        self.n_allocas += 1;
        Operand::Val(vid)
    }

    fn local_repr(ty: Type) -> Type {
        if matches!(ty, Type::Void | Type::I8 | Type::Ptr) {
            Type::Ref
        } else {
            ty
        }
    }

    fn insert_local(&mut self, sym: SymbolId, mut local: Local) {
        let scope = self.scopes.last_mut().expect("escopo da função");
        local.scope = scope.id;
        scope.insert(sym, local);
    }

    /// Declara um local (sem offset: nunca é célula) no escopo corrente,
    /// guardado na representação `ty`.
    pub fn declare_local(&mut self, sym: SymbolId, ty: Type) -> Operand {
        let ty = Self::local_repr(ty);
        let ptr = self.entry_alloca(ty);
        self.insert_local(
            sym,
            Local {
                mode: Mode::Memory(ptr.clone()),
                ty,
                static_type: None,
                finalizable: None,
                scope: 0,
                offset: None,
                late: None,
            },
        );
        ptr
    }

    /// Declara e inicializa: o valor é coagido para a representação do local.
    pub fn declare_local_with_value(&mut self, sym: SymbolId, ty: Type, value: Operand) {
        let ptr = self.declare_local(sym, ty);
        let ty = self.lookup_local(sym).map_or(ty, |l| l.ty);
        let v = self.coerce(value, ty);
        self.emit(Instruction::Store { ptr, val: v }, Type::Void);
    }

    /// Declara a variável do usuário cujo nome começa em `offset` e a
    /// inicializa com `value`. Se `capture.rs` a marcou (capturada e
    /// atribuída), ela mora numa célula nova — cada execução da declaração
    /// cria uma variável nova (a do corpo de um laço, uma por volta).
    pub fn declare_variable(&mut self, sym: SymbolId, offset: usize, ty: Type, value: Operand) {
        let static_type = self.ctx.semantic_local_type(self.unit_id, offset);
        let finalizable = static_type.and_then(|t| self.ctx.classify_finalizable(t));
        // Outra declaração do mesmo nome no mesmo escopo: o programa é
        // inválido (a de um escopo de fora pode ser sombreada; a mesma
        // declaração baixada de novo, num `finally`, não conta).
        // Com curingas (Dart 3.7), `_` pode repetir: não liga nome.
        let library = self.ctx.program.unit(self.unit_id).library;
        let wildcard = self.ctx.symbol_name(sym) == "_"
            && self.ctx.program.library(library).features.tem(dartforge_frontend::Feature::WildcardVariables);
        let repeated = !wildcard
            && self.scopes.last().and_then(|e| e.get(&sym)).is_some_and(|l| l.offset.is_some_and(|o| o != offset));
        if repeated {
            let name = self.ctx.symbol_name(sym).to_string();
            let end = offset + name.len();
            self.language_error(&format!("'{name}' is already declared in this scope."), dartforge_diagnostics::Span { start: offset, end: end });
        }
        let ty = Self::local_repr(ty);
        // Capturada só por funções diretas e atribuída: vai pelo endereço do
        // `alloca` (`direct_functions.rs`); o de um `Ref` mora no slot do
        // quadro de raízes (`llvm/mod.rs`, `frame_allocas`), onde o
        // coletor vê o que a função direta grava.
        let cell = self.cells.contains(&offset);
        if !cell {
            let ptr = self.entry_alloca(ty);
            let v = self.coerce(value, ty);
            self.emit(
                Instruction::Store {
                    ptr: ptr.clone(),
                    val: v,
                },
                Type::Void,
            );
            self.insert_local(
                sym,
                Local {
                    mode: Mode::Memory(ptr),
                    ty,
                    static_type,
                    finalizable,
                    scope: 0,
                    offset: Some(offset),
                    late: None,
                },
            );
            return;
        }
        // A célula guarda uma palavra, na representação do local (a mesma
        // com que `CellGet`/`CellSet` a leem e gravam); um vetor SIMD vai na
        // caixa.
        let ty = if ty.is_vector() { Type::Ref } else { ty };
        let ptr = self.entry_alloca(Type::Ref);
        let v = self.coerce(value, ty);
        let cell = self.emit(Instruction::AllocCell { value: v }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: cell,
            },
            Type::Void,
        );
        self.insert_local(
            sym,
            Local {
                mode: Mode::Cell(ptr),
                ty,
                static_type,
                finalizable,
                scope: 0,
                offset: Some(offset),
                late: None,
            },
        );
    }

    /// Ativa a semântica de `late`. O estado de um local capturado segue o
    /// handle da sua `Cell`, compartilhado entre função e closures.
    pub fn configure_late_local(&mut self, sym: SymbolId, final_: bool, init: Option<ExprId>) -> bool {
        let Some(local) = self.lookup_local(sym) else { return false };
        let (state, cell) = match &local.mode {
            Mode::Memory(_) => {
                let state = self.entry_alloca(Type::I8);
                self.emit(
                    Instruction::Store { ptr: state.clone(), val: Operand::Constant(Constant::Int(0)) },
                    Type::Void,
                );
                (state, false)
            }
            Mode::Cell(_) if init.is_none() => (self.local_cell(&local).expect("célula late"), true),
            _ => return false,
        };
        let name = self.ctx.symbol_name(sym).to_string();
        if let Some(local) = self.scopes.last_mut().and_then(|e| e.get_mut(&sym)) {
            local.late = Some(LateLocal {
                initialized: state,
                cell,
                final_,
                name,
                initializer: init.map(|e| (self.unit_id, e)),
            });
        }
        true
    }

    fn read_late_state(&mut self, late: &LateLocal) -> Operand {
        if late.cell {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![
                        (late.initialized.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(-1)), Type::I64),
                    ],
                    ret_ty: Type::I8,
                },
                Type::I8,
            )
        } else {
            self.emit(Instruction::Load { ptr: late.initialized.clone(), ty: Type::I8 }, Type::I8)
        }
    }

    fn define_late_state(&mut self, late: &LateLocal, state: i64) {
        if late.cell {
            assert_eq!(state, 1, "célula late capturada não tem inicializador preguiçoso");
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_mark_initialized".to_string(),
                    args: vec![
                        (late.initialized.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(-1)), Type::I64),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        } else {
            self.emit(
                Instruction::Store { ptr: late.initialized.clone(), val: Operand::Constant(Constant::Int(state)) },
                Type::Void,
            );
        }
    }

    /// Liga um nome a um posição do ambiente da closure corrente.
    pub fn bind_env(
        &mut self,
        sym: SymbolId,
        env: Operand,
        index: usize,
        cell: bool,
        ty: Type,
        origin: &Local,
    ) {
        let late = origin.late.as_ref().filter(|l| l.cell && cell).map(|l| {
            let state = self.emit(Instruction::EnvGet { env: env.clone(), index: index }, Type::Ref);
            LateLocal {
                initialized: state,
                cell: true,
                final_: l.final_,
                name: l.name.clone(),
                initializer: None,
            }
        });
        self.insert_local(
            sym,
            Local {
                mode: Mode::Environment {
                    env,
                    index,
                    cell,
                },
                ty,
                static_type: origin.static_type,
                finalizable: origin.finalizable,
                scope: 0,
                offset: None,
                late,
            },
        );
    }

    /// Liga um nome a um local já montado (as capturas de uma função
    /// direta, `direct_functions.rs`).
    pub(super) fn bind_local_as(&mut self, sym: SymbolId, local: Local) {
        self.insert_local(sym, local);
    }

    /// Liga um nome a um valor já calculado (parâmetro de closure em linha).
    pub fn bind_local(&mut self, sym: SymbolId, value: Operand) {
        let ty = self.operand_type(&value);
        self.insert_local(
            sym,
            Local {
                mode: Mode::Value(value),
                ty,
                static_type: None,
                finalizable: None,
                scope: 0,
                offset: None,
                late: None,
            },
        );
    }

    /// Completa o tipo de um local já ligado usando a origem semântica.
    /// None mantém a obrigação indeterminada; não infere pelo tipo HIR.
    ///
    /// # Panics
    /// Se o chamador não ligou o local no escopo visível antes desta atualização.
    pub(super) fn assign_semantic_type(&mut self, sym: SymbolId, ty: Option<dartforge_types::TypeId>) {
        let finalizable = ty.and_then(|t| self.ctx.classify_finalizable(t));
        let local = self.scopes.iter_mut().rev().find_map(|e| e.get_mut(&sym))
            .expect("local semântico deve estar ligado");
        local.static_type = ty;
        local.finalizable = finalizable;
    }

    /// O local visível com esse nome, do escopo mais interno para fora.
    pub fn lookup_local(&self, sym: SymbolId) -> Option<Local> {
        self.scopes.iter().rev().find_map(|e| e.get(&sym).cloned())
    }

    /// O handle da célula de um local que mora numa (declarado aqui ou
    /// vindo do ambiente); `None` se o local não é célula.
    pub fn local_cell(&mut self, local: &Local) -> Option<Operand> {
        match &local.mode {
            Mode::Cell(ptr) => Some(self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: Type::Ref,
                },
                Type::Ref,
            )),
            Mode::Environment {
                env,
                index,
                cell: true,
            } => Some(self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *index,
                },
                Type::Ref,
            )),
            _ => None,
        }
    }

    /// Lê um local já encontrado, na representação dele.
    pub fn read_local(&mut self, local: &Local) -> Operand {
        if let Some(late) = &local.late {
            if let Some((unit, init)) = late.initializer
                && !local.offset.is_some_and(|off| self.late_initializers_in_lowering.contains(&off))
            {
                let state = self.read_late_state(late);
                let empty = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, state, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_init = self.new_block();
                let b_verify = self.new_block();
                self.terminate(Terminator::CondBranch { cond: empty, then_block: b_init, else_block: b_verify });
                self.set_block(b_init);
                self.define_late_state(late, 2);
                let b_fail = self.new_block();
                self.exception_targets.push(b_fail);
                if let Some(off) = local.offset {
                    self.late_initializers_in_lowering.insert(off);
                }
                let value = self.lower_expr_of(unit, init);
                if let Some(off) = local.offset {
                    self.late_initializers_in_lowering.remove(&off);
                }
                self.exception_targets.pop();
                let value = self.coerce(value, local.ty);
                let Mode::Memory(ptr) = &local.mode else { unreachable!("late capturado recusado") };
                self.emit(Instruction::Store { ptr: ptr.clone(), val: value }, Type::Void);
                self.define_late_state(late, 1);
                self.terminate(Terminator::Branch(b_verify));
                self.set_block(b_fail);
                self.define_late_state(late, 0);
                // Encaminha a exceção ao `catch`/`finally` externo após
                // restaurar a célula para permitir nova tentativa de leitura.
                self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_exception_pending".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                );
                self.terminate(Terminator::Unreachable);
                self.set_block(b_verify);
            }
            let state = self.read_late_state(late);
            let ready = self.emit(
                Instruction::ICmp(ICmpOp::Eq, state, Operand::Constant(Constant::Int(1))),
                Type::I1,
            );
            let b_error = self.new_block();
            let b_read = self.new_block();
            self.terminate(Terminator::CondBranch { cond: ready, then_block: b_read, else_block: b_error });
            self.set_block(b_error);
            self.throw_late_error(&late.name, 2);
            self.set_block(b_read);
        }
        if let Some(c) = self.local_cell(local) {
            return self.emit(Instruction::CellGet { cell: c }, local.ty);
        }
        match &local.mode {
            Mode::Memory(ptr) => self.emit(
                Instruction::Load {
                    ptr: ptr.clone(),
                    ty: local.ty,
                },
                local.ty,
            ),
            Mode::Value(v) => v.clone(),
            Mode::Environment { env, index, .. } => self.emit(
                Instruction::EnvGet {
                    env: env.clone(),
                    index: *index,
                },
                local.ty,
            ),
            Mode::Cell(_) => unreachable!("célula tratada acima"),
        }
    }

    /// Lê um local (ou parâmetro) pelo nome, na representação dele.
    pub fn read_local_by_name(&mut self, sym: SymbolId) -> Option<Operand> {
        let local = self.lookup_local(sym)?;
        Some(self.read_local(&local))
    }

    /// Grava num local visível; devolve o valor já na representação dele.
    /// Um local copiado para o ambiente nunca é gravado (`capture.rs` põe
    /// numa célula toda variável capturada que é atribuída).
    pub fn store_local(&mut self, sym: SymbolId, value: Operand) -> Option<Operand> {
        let local = self.lookup_local(sym)?;
        let v = self.coerce(value, local.ty);
        if let Some(late) = &local.late {
            let state = self.read_late_state(late);
            let starting = self.emit(
                Instruction::ICmp(ICmpOp::Eq, state.clone(), Operand::Constant(Constant::Int(2))),
                Type::I1,
            );
            let b_add = self.new_block();
            let b_next = self.new_block();
            self.terminate(Terminator::CondBranch { cond: starting, then_block: b_add, else_block: b_next });
            self.set_block(b_add);
            self.throw_late_error(&late.name, 5);
            self.set_block(b_next);
            if late.final_ {
                let already_initialized = self.emit(
                    Instruction::ICmp(ICmpOp::Ne, state, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_error = self.new_block();
                let b_store = self.new_block();
                self.terminate(Terminator::CondBranch { cond: already_initialized, then_block: b_error, else_block: b_store });
                self.set_block(b_error);
                self.throw_late_error(&late.name, 3);
                self.set_block(b_store);
            }
            self.define_late_state(late, 1);
        }
        if let Some(c) = self.local_cell(&local) {
            self.emit(
                Instruction::CellSet {
                    cell: c,
                    value: v.clone(),
                },
                Type::Void,
            );
            return Some(v);
        }
        match local.mode {
            Mode::Memory(ptr) => {
                self.emit(
                    Instruction::Store {
                        ptr,
                        val: v.clone(),
                    },
                    Type::Void,
                );
            }
            Mode::Value(_) => {
                // Valor imutável religado no escopo onde ele mora.
                for e in self.scopes.iter_mut().rev() {
                    if let Some(l) = e.get_mut(&sym) {
                        l.mode = Mode::Value(v.clone());
                        break;
                    }
                }
            }
            Mode::Environment { .. } | Mode::Cell(_) => return None,
        }
        Some(v)
    }

    /// Uma volta nova de um `for` clássico (a especificação, "for loop":
    /// cada iteração tem a sua variável, iniciada com o valor da anterior).
    /// Só importa para a variável que mora numa célula — uma closure da
    /// volta anterior continua com a célula antiga.
    pub fn renew_cell(&mut self, sym: SymbolId) {
        let Some(local) = self.lookup_local(sym) else {
            return;
        };
        let Mode::Cell(ptr) = &local.mode else {
            return;
        };
        let old = self.emit(
            Instruction::Load {
                ptr: ptr.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        let value = self.emit(Instruction::CellGet { cell: old }, local.ty);
        let new_one = self.emit(Instruction::AllocCell { value: value }, Type::Ref);
        self.emit(
            Instruction::Store {
                ptr: ptr.clone(),
                val: new_one,
            },
            Type::Void,
        );
    }

    /// Tira um nome de todos os escopos (os `this.x` no corpo do construtor).
    pub fn remove_local(&mut self, sym: SymbolId) {
        for e in &mut self.scopes {
            e.remove(&sym);
        }
    }

    /// Representação guardada de um local declarado em `offset` (R6): o
    /// tipo declarado, ou o inferido do inicializador. Sem informação, `Ref`
    /// — a caixa é sempre correta, só mais lenta.
    pub fn local_repr_of(&self, offset: usize) -> Type {
        let ty = self.ctx.local_type(self.unit_id, offset);
        // SIMD sem caixa (`simd.rs`), salvo o local capturado (a célula
        // guarda 64 bits: a caixa).
        if let Some(k) = self.simd_type(ty)
            && !self.cells.contains(&offset)
        {
            return k;
        }
        ty.map_or(Type::Ref, |t| self.repr(t))
    }
}

#[cfg(test)]
mod semantic_finalizable_tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn parameters_and_this_preserve_sdk_semantic_identity() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let dir = tempfile::tempdir().unwrap();
                let entry = dir.path().join("main.dart");
                std::fs::write(
                    &entry,
                    r#"
import 'dart:ffi' as ffi;
final class Recurso implements ffi.Finalizable {
  void usar(Recurso? recurso) {}
}
class Finalizable {}
extension type Envelope(Recurso recurso) implements Recurso {}
void testar(Envelope envelope, Recurso? recurso, Finalizable homonimo,
    dynamic outro, Never nunca) {}
int escalares<T>(int numero, double fracao, bool ligado, T valor) => numero;
void main() {}
"#,
                )
                .unwrap();
                let sdk = crate::sdk_modulo::load_native_sdk(crate::sdk_tests()).unwrap();
                let mut interner = Interner::new();
                let (program, diags) =
                    dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &program, &interner);
                let (mut outline, diags_outline) =
                    dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
                let (bodies, diags_body) = dartforge_types::infer_program_bodies(
                    &program,
                    &interner,
                    &mut table,
                    &core,
                    &mut outline,
                );
                for d in diags_outline.iter().chain(diags_body.iter()) {
                    assert!(
                        !dartforge_types::codes::e_erro_de_compilacao(&d.message),
                        "{d:?}"
                    );
                }
                let te = crate::erasure::compute(&program, &outline, &mut table);
                let mut ctx = crate::context::Context::new(
                    &program, &interner, &table, &core, &outline, &bodies,
                );
                ctx.te = te;
                let lib = program.entry.unwrap();
                let unit = program.library(lib).units[0];
                for name in ["testar", "usar"] {
                    let fid = program
                        .functions
                        .iter()
                        .position(|f| f.library == lib && interner.resolve(f.name) == name)
                        .unwrap();
                    let mut b = FnBuilder::new(&ctx, unit, name.into(), name.into(), Type::Void);
                    b.declare_parameters(fid, name == "usar");
                    if name == "usar" {
                        assert_eq!(b.finalizable_this, Some(true));
                    }
                    for p in &outline.functions[fid].parameters {
                        let sym = p.name.unwrap();
                        let name = interner.resolve(sym);
                        let local = b.lookup_local(sym).unwrap();
                        assert_eq!(local.scope, 0, "parâmetro {name}");
                        assert_eq!(local.static_type, Some(p.ty), "{name}");
                        assert_eq!(
                            local.finalizable,
                            Some(matches!(name, "envelope" | "recurso")),
                            "{name}"
                        );
                        if name == "envelope" {
                            assert_ne!(
                                ctx.erase(p.ty),
                                p.ty,
                                "tipo de extensão precisa ser preservado antes do apagamento"
                            );
                        }
                    }
                    // Mesma profundidade não identifica o mesmo escopo.
                    // Casos de switch restauram o registro original completo.
                    let parameter = outline.functions[fid].parameters[0].name.unwrap();
                    assert_eq!(b.scopes[0].id, 0);
                    let external = b.lookup_local(parameter).unwrap();
                    b.open_scope();
                    let first_one = b.scopes.last().unwrap().id;
                    b.bind_local(parameter, Operand::Constant(Constant::Null));
                    assert_eq!(b.lookup_local(parameter).unwrap().scope, first_one);
                    let saved = b.scopes.pop().unwrap();
                    assert_eq!(
                        b.lookup_local(parameter).unwrap().static_type,
                        external.static_type
                    );
                    b.open_scope();
                    let second = b.scopes.last().unwrap().id;
                    assert_ne!(first_one, second);
                    b.close_scope();
                    b.scopes.push(saved.clone());
                    assert_eq!(b.scopes.last().unwrap().id, first_one);
                    assert_eq!(b.lookup_local(parameter).unwrap().scope, first_one);
                    assert_eq!(
                        b.lookup_local(parameter).unwrap().static_type,
                        None
                    );
                    b.close_scope();
                    b.open_scope();
                    let third = b.scopes.last().unwrap().id;
                    assert!(third > second);
                    // Copiar a descrição de uma captura não transporta a
                    // identidade léxica do corpo que a declarou.
                    let mut capture = external.clone();
                    capture.scope = first_one;
                    b.bind_local_as(parameter, capture);
                    assert_eq!(b.lookup_local(parameter).unwrap().scope, third);
                    assert_eq!(
                        b.lookup_local(parameter).unwrap().static_type,
                        external.static_type
                    );
                    b.bind_env(
                        parameter,
                        Operand::Constant(Constant::Null),
                        0,
                        false,
                        external.ty,
                        &external,
                    );
                    let env = b.lookup_local(parameter).unwrap();
                    assert_eq!(env.scope, third);
                    assert_eq!(env.static_type, external.static_type);
                    assert_eq!(env.finalizable, external.finalizable);
                    b.close_scope();
                    assert_eq!(b.lookup_local(parameter).unwrap().scope, 0);
                    b.close_scope();
                    assert_eq!(b.scopes.len(), 1);
                }
                let fid = program.functions.iter().position(|f| ctx.symbol_name(f.name) == "escalares").unwrap();
                let mut tracing = FnBuilder::new(&ctx, unit, "tracing".into(), "tracing".into(), Type::I64);
                tracing.declare_parameters(fid, false);
                assert!(tracing.dart_scalar_parameters.is_empty());
                tracing.add_param_rti("$tipos".into());
                assert!(tracing.dart_rti_parameters.is_empty());
                drop(tracing);
                ctx.arc_memory = true;
                let mut b = FnBuilder::new(&ctx, unit, "escalares".into(), "escalares".into(), Type::I64);
                b.declare_parameters(fid, false);
                let native = b.add_param_rti("$tipos".into());
                assert!(b.dart_rti_parameters["escalares"].contains(&native));
                let scalars = &b.dart_scalar_parameters["escalares"];
                for (v, _, ty) in &b.func.params {
                    assert_eq!(scalars.contains(v), *v != native && *ty != Type::Ref);
                }
                assert_eq!(scalars.len(), 3);
                b.terminate(Terminator::Return(Some(Operand::Val(b.func.params[0].0))));
                let mut module = Module::new();
                b.finish(&mut module);
                assert_eq!(module.dart_scalar_parameters["escalares"].len(), 3);
                assert!(!module.dart_scalar_parameters["escalares"].contains(&native));
                assert!(module.dart_rti_parameters["escalares"].contains(&native));
                crate::optimize::optimize(&mut module);
                assert_eq!(module.dart_scalar_parameters["escalares"].len(), 3);
                for id in &module.dart_scalar_parameters["escalares"] {
                    assert!(module.functions[0].params.iter().any(|(v, _, _)| v == id));
                }
                module.arc_memory = true;
                let mut plans = std::collections::HashMap::from([("escalares".into(), crate::optimize::arc::DartFunctionPlan::default())]);
                assert_eq!(crate::optimize::arc::produce_lowering_scalar_parameters(&module, &mut plans).unwrap(), 3);
                assert!(!plans["escalares"].classes.contains_key(&native));
                assert_eq!(crate::optimize::arc::produce_lowering_scalar_parameters(&module, &mut plans).unwrap(), 0);
                assert!(crate::optimize::arc::insert_arc_dart_functions(&mut module.functions, &mut plans).is_err());
                assert_eq!(crate::optimize::arc::produce_lowering_rti_parameters(&module, &mut plans).unwrap(), 1);
                assert_eq!(crate::optimize::arc::produce_lowering_rti_parameters(&module, &mut plans).unwrap(), 0);
                assert_eq!(crate::optimize::arc::insert_arc_dart_functions(&mut module.functions, &mut plans).unwrap(), (0, 0));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

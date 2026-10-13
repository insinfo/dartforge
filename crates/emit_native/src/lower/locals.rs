//! Local variables: lexical scope and `alloca` in the representation of the type (R6),
//! and the variables captured by closures (P1).
//!
//! Before, a local was an `alloca i64` created where the declaration appeared (in
//! the middle of a loop, the `alloca` grew the stack on every iteration) and a single map
//! by name: a block that shadowed `current` overwrote the outer `current`
//! forever, and the variable of a `for (var i = 0; …; i++)` did not even have an
//! `alloca` — the `i++` replaced the SSA value in the map, and the loop condition,
//! lowered before, kept reading the `0` of the entry.
//!
//! A variable captured by a closure and **assigned** at some point
//! (`capture.rs` decides, before lowering the function) lives in a heap `Cell`: the
//! `alloca` keeps the handle of the cell, and whoever reads or writes goes through it — the
//! closure sees the write made after the capture, and its write is seen
//! from outside. Captured and never assigned, it is copied into the environment of the
//! closure at creation. Inside the closure, the outer variable is a position of the
//! environment (`Environment`), with the handle of the cell or the copy.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::ExprId;
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Semantic identity preserved when removing/restoring switch cases.
/// Zero is the invocation; the other IDs are not reusable depths.
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

/// Where a visible local lives.
#[derive(Debug, Clone)]
pub enum Mode {
    /// An `alloca` in the representation of the local.
    Memory(Operand),
    /// An already computed immutable value (the parameter of a closure lowered
    /// inline by the frozen code of `sdk_por_nome.rs`).
    Value(Operand),
    /// A `Ref` `alloca` with the handle of the cell that keeps the value.
    Cell(Operand),
    /// Position `index` of the environment of the current closure (`env`); with
    /// `cell`, what is there is the handle of the cell.
    Environment {
        env: Operand,
        index: usize,
        cell: bool,
    },
}

/// A visible local.
#[derive(Debug, Clone)]
pub struct Local {
    pub mode: Mode,
    pub ty: Type,
    /// Dart type before erasing the representation, for lexical obligations
    /// like Finalizable. None indicates information not supplied yet; it does not
    /// prove the absence of the obligation. Captures preserve the original TypeId.
    pub static_type: Option<dartforge_types::TypeId>,
    /// Classification of the lexical obligation. None still requires proof; it does not emit
    /// nor replace ArcKeepAlive or an owner occurrence.
    pub finalizable: Option<bool>,
    /// Scope of this binding, assigned when inserting it into the current body.
    /// Captures preserve the original type, but not the scope of another function.
    /// It is neither a proof of cleanup nor an owner occurrence.
    pub scope: u32,
    /// Offset of the name in the declaration (the key of `capture.rs`); `None` for
    /// those bound by value.
    pub offset: Option<usize>,
    /// A mark separate from the value, since zero/null are legitimate assignments.
    pub late: Option<LateLocal>,
}

#[derive(Debug, Clone)]
pub struct LateLocal {
    pub initialized: Operand,
    /// `true`: `initialized` is a heap `Cell` and uses index -1 in the
    /// side table; `false`: it is an `alloca i8` of this function.
    pub cell: bool,
    pub final_: bool,
    pub name: String,
    pub initializer: Option<(UnitId, ExprId)>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    pub fn open_scope(&mut self) {
        let id = self.next_scope;
        self.next_scope = id.checked_add(1).expect("limit of lexical scopes");
        debug_assert!(self.scopes.iter().all(|e| e.id != id));
        self.scopes.push(LocalScope::new(id));
    }

    pub fn close_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// An `alloca` at the start of the entry block: runs once per
    /// activation and dominates every use (Clang rejected an `alloca` created
    /// in a branch and used after the join).
    pub fn entry_alloca(&mut self, ty: Type) -> Operand {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, Type::Ptr);
        let b0 = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == BlockId(0))
            .expect("entry block");
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
        let scope = self.scopes.last_mut().expect("function scope");
        local.scope = scope.id;
        scope.insert(sym, local);
    }

    /// Declares a local (without an offset: never a cell) in the current scope,
    /// kept in representation `ty`.
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

    /// Declares and initializes: the value is coerced to the representation of the local.
    pub fn declare_local_with_value(&mut self, sym: SymbolId, ty: Type, value: Operand) {
        let ptr = self.declare_local(sym, ty);
        let ty = self.lookup_local(sym).map_or(ty, |l| l.ty);
        let v = self.coerce(value, ty);
        self.emit(Instruction::Store { ptr, val: v }, Type::Void);
    }

    /// Declares the user variable whose name starts at `offset` and
    /// initializes it with `value`. If `capture.rs` marked it (captured and
    /// assigned), it lives in a new cell — each execution of the declaration
    /// creates a new variable (that of a loop body, one per iteration).
    pub fn declare_variable(&mut self, sym: SymbolId, offset: usize, ty: Type, value: Operand) {
        let static_type = self.ctx.semantic_local_type(self.unit_id, offset);
        let finalizable = static_type.and_then(|t| self.ctx.classify_finalizable(t));
        // Another declaration of the same name in the same scope: the program is
        // invalid (one of an outer scope may be shadowed; the same
        // declaration lowered again, in a `finally`, does not count).
        // With wildcards (Dart 3.7), `_` may repeat: it binds no name.
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
        // Captured only by direct functions and assigned: goes by the address of the
        // `alloca` (`direct_functions.rs`); that of a `Ref` lives in the slot of the
        // roots frame (`llvm/mod.rs`, `frame_allocas`), where the
        // collector sees what the direct function writes.
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
        // The cell keeps a word, in the representation of the local (the same
        // in which `CellGet`/`CellSet` read and write it); a SIMD vector goes in the
        // box.
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

    /// Enables the `late` semantics. The state of a captured local follows the
    /// handle of its `Cell`, shared between the function and closures.
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
            Mode::Cell(_) if init.is_none() => (self.local_cell(&local).expect("late cell"), true),
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
            assert_eq!(state, 1, "captured late cell has no lazy initializer");
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

    /// Binds a name to a position of the environment of the current closure.
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

    /// Binds a name to an already built local (the captures of a direct
    /// function, `direct_functions.rs`).
    pub(super) fn bind_local_as(&mut self, sym: SymbolId, local: Local) {
        self.insert_local(sym, local);
    }

    /// Binds a name to an already computed value (an inline closure parameter).
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

    /// Completes the type of an already bound local using the semantic origin.
    /// None keeps the obligation undetermined; does not infer from the HIR type.
    ///
    /// # Panics
    /// If the caller did not bind the local in the visible scope before this update.
    pub(super) fn assign_semantic_type(&mut self, sym: SymbolId, ty: Option<dartforge_types::TypeId>) {
        let finalizable = ty.and_then(|t| self.ctx.classify_finalizable(t));
        let local = self.scopes.iter_mut().rev().find_map(|e| e.get_mut(&sym))
            .expect("semantic local must be bound");
        local.static_type = ty;
        local.finalizable = finalizable;
    }

    /// The visible local with this name, from the innermost scope outwards.
    pub fn lookup_local(&self, sym: SymbolId) -> Option<Local> {
        self.scopes.iter().rev().find_map(|e| e.get(&sym).cloned())
    }

    /// The handle of the cell of a local that lives in one (declared here or
    /// coming from the environment); `None` if the local is not a cell.
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

    /// Reads an already found local, in its representation.
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
                let Mode::Memory(ptr) = &local.mode else { unreachable!("captured late rejected") };
                self.emit(Instruction::Store { ptr: ptr.clone(), val: value }, Type::Void);
                self.define_late_state(late, 1);
                self.terminate(Terminator::Branch(b_verify));
                self.set_block(b_fail);
                self.define_late_state(late, 0);
                // Forwards the exception to the outer `catch`/`finally` after
                // restoring the cell to allow a new read attempt.
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
            Mode::Cell(_) => unreachable!("cell handled above"),
        }
    }

    /// Reads a local (or parameter) by name, in its representation.
    pub fn read_local_by_name(&mut self, sym: SymbolId) -> Option<Operand> {
        let local = self.lookup_local(sym)?;
        Some(self.read_local(&local))
    }

    /// Writes into a visible local; returns the value already in its representation.
    /// A local copied into the environment is never written (`capture.rs` puts
    /// every captured variable that is assigned into a cell).
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
                // An immutable value rebound in the scope where it lives.
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

    /// A new iteration of a classic `for` (the specification, "for loop":
    /// each iteration has its own variable, initialized with the value of the previous one).
    /// It only matters for a variable that lives in a cell — a closure of the
    /// previous iteration keeps the old cell.
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

    /// Removes a name from every scope (the `this.x` in the body of the constructor).
    pub fn remove_local(&mut self, sym: SymbolId) {
        for e in &mut self.scopes {
            e.remove(&sym);
        }
    }

    /// Stored representation of a local declared at `offset` (R6): the
    /// declared type, or the one inferred from the initializer. Without information, `Ref`
    /// — the box is always correct, only slower.
    pub fn local_repr_of(&self, offset: usize) -> Type {
        let ty = self.ctx.local_type(self.unit_id, offset);
        // Unboxed SIMD (`simd.rs`), except for a captured local (the cell
        // keeps 64 bits: the box).
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
                        assert_eq!(local.scope, 0, "parameter {name}");
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
                                "extension type must be preserved before erasure"
                            );
                        }
                    }
                    // The same depth does not identify the same scope.
                    // Switch cases restore the full original record.
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
                    // Copying the description of a capture does not carry the
                    // lexical identity of the body that declared it.
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

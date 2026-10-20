//! HIR verifier, before emission (E3 of the contract,
//! `docs/NATIVO-PLANO.md` §6.3).
//!
//! Rejects what, once emitted, becomes a `panic` in the runtime or IR silently
//! accepted by Clang: a heap write with a tag inconsistent with the representation of the value
//! (a scalar marked as a reference becomes an "invalid handle" in the collector;
//! a reference marked as a scalar is collected while alive); an integer constant
//! in a `Ref` position (the only `Ref` constant is `Null`); an instruction the
//! emitter cannot lower (the old `; inst pending`) or that left with the
//! unified space (`AllocMap`, `AllocSet`); a positional record with a field that
//! is not a `Ref` (the `_Record` is `REFS`); and a capture read in a representation
//! different from the one written (docs/NATIVO-ESPACO-UNIFICADO.md §6, risk 12: the
//! `_Contexto` and the `_Celula` keep the raw word, and only the map bit tells whether
//! it is a reference — reading an `int` written raw as a `Ref` is a fake handle).
//!
//! A problem here is a compiler bug, not a program bug: the message names the
//! function and the instruction.

use crate::hir::*;
use std::collections::HashMap;

/// Pairs (index of the argument with the bits, index of the argument with the tag)
/// of the runtime externs that receive a tagged value (the flat `(bits,
/// tag)` ABI: 1 `int`, 2 `bool`, 3 `Ref`, 4 `double`).
fn pairs_with_tag(name: &str) -> &'static [(usize, usize)] {
    match name {
        "dartforge_exception_throw" | "dartforge_tagged_to_string" => &[(0, 1)],
        _ => &[],
    }
}

/// Pairs (bits, `is_ref`) of the externs that receive the reference mark.
fn pairs_with_is_ref(name: &str) -> &'static [(usize, usize)] {
    match name {
        "dartforge_object_set" => &[(2, 3)],
        "dartforge_assertion_error_new"
        | "dartforge_exception_new"
        | "dartforge_argument_error_value" => &[(0, 1)],
        _ => &[],
    }
}

/// A position of a `_Contexto` (`AllocEnv`): the written representation and,
/// if the value is a `_Celula` of the same function, the representations written
/// into it.
#[derive(Clone, PartialEq)]
struct Capture {
    ty: Type,
    cell: Option<Vec<Type>>,
}

/// Is the representation of `a` and `b` the same word in the context? (`I1` and `I8`
/// are the same 0/1 `bool`.)
fn same_representation(a: Type, b: Type) -> bool {
    a == b || matches!((a, b), (Type::I1 | Type::I8, Type::I1 | Type::I8))
}

struct Context<'m> {
    types: HashMap<ValueId, Type>,
    allocates: HashMap<ValueId, Type>,
    params: &'m HashMap<&'m str, Vec<Type>>,
    function: &'m Function,
    errors: Vec<String>,
    /// The representations written into each `_Celula` allocated in this function
    /// (`AllocCell` and `CellSet` over the same value).
    cells: HashMap<ValueId, Vec<Type>>,
    /// The values read from the closure environment (`EnvGet` over the `env`
    /// parameter), with the position read.
    read_from_env: HashMap<ValueId, usize>,
    /// The environment of this function (that of every closure that uses it as its body),
    /// when it is known and unique in the module.
    env: Option<&'m [Capture]>,
}

impl Context<'_> {
    fn ty(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.types.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
                Type::Ref
            }
            Operand::Constant(Constant::Function(_)) => Type::I64,
        }
    }

    fn error(&mut self, msg: String) {
        self.errors
            .push(format!("HIR verifier ({}): {msg}", self.function.name));
    }

    /// E1: the written tag must be that of the value's representation.
    fn check_tag(&mut self, location: &str, bits: &Operand, tag: &Operand) {
        let Operand::Constant(Constant::Int(t)) = tag else {
            return;
        };
        let ty = self.ty(bits);
        let coherent = match t {
            3 => ty == Type::Ref,
            1 | 2 | 4 => ty != Type::Ref,
            _ => true,
        };
        if !coherent {
            self.error(format!("{location}: tag {t} for a {ty:?} value"));
        }
    }

    fn check_is_ref(&mut self, location: &str, bits: &Operand, is_ref: &Operand) {
        let Operand::Constant(Constant::Int(r)) = is_ref else {
            return;
        };
        let ty = self.ty(bits);
        if (*r != 0) != (ty == Type::Ref) {
            self.error(format!("{location}: is_ref {r} for a {ty:?} value"));
        }
    }

    /// E3: an integer constant in a `Ref` position.
    fn check_ref(&mut self, location: &str, op: &Operand, position: Type) {
        if position == Type::Ref && matches!(op, Operand::Constant(Constant::Int(_))) {
            self.error(format!("{location}: integer constant in a Ref position"));
        }
    }

    /// The `env` parameter of this function (the environment of a closure body).
    fn is_env_parameter(&self, op: &Operand) -> bool {
        let Operand::Val(v) = op else { return false };
        self.function
            .params
            .iter()
            .any(|(p, name, _)| p == v && name == "env")
    }

    /// The representations written into cell `cell`, if known: a
    /// `_Celula` of this function, or one read from the closure environment.
    fn stored_in_cell(&self, cell: &Operand) -> Option<Vec<Type>> {
        let Operand::Val(v) = cell else { return None };
        if let Some(t) = self.cells.get(v) {
            return Some(t.clone());
        }
        let i = *self.read_from_env.get(v)?;
        self.env?.get(i)?.cell.clone()
    }

    /// Risk 12: the value read from a capture must be in the representation
    /// it was written with.
    fn check_read(&mut self, location: &str, read_value: Type, written_set: &[Type]) {
        if let Some(g) = written_set
            .iter()
            .find(|&&g| !same_representation(g, read_value))
        {
            self.error(format!(
                "{location}: read as {read_value:?}, written as {g:?}"
            ));
        }
    }
}

/// The captures of each `AllocEnv` of `f` (by its value), with the cells
/// allocated in `f` that it holds.
fn function_envs(f: &Function) -> HashMap<ValueId, Vec<Capture>> {
    let mut types: HashMap<ValueId, Type> = HashMap::new();
    for (vid, _, ty) in &f.params {
        types.insert(*vid, *ty);
    }
    let mut cells: HashMap<ValueId, Vec<Type>> = HashMap::new();
    for b in &f.blocks {
        for (vid, inst, ty) in &b.instructions {
            types.insert(*vid, *ty);
            if let Instruction::AllocCell { value } = inst {
                cells.insert(*vid, vec![operand_type(&types, value)]);
            }
        }
    }
    for b in &f.blocks {
        for (_, inst, _) in &b.instructions {
            if let Instruction::CellSet {
                cell: Operand::Val(c),
                value,
            } = inst
                && let Some(t) = cells.get_mut(c)
            {
                t.push(operand_type(&types, value));
            }
        }
    }
    let mut exit = HashMap::new();
    for b in &f.blocks {
        for (vid, inst, _) in &b.instructions {
            if let Instruction::AllocEnv { values } = inst {
                let captures = values
                    .iter()
                    .map(|v| Capture {
                        ty: operand_type(&types, v),
                        cell: match v {
                            Operand::Val(c) => cells.get(c).cloned(),
                            _ => None,
                        },
                    })
                    .collect();
                exit.insert(*vid, captures);
            }
        }
    }
    exit
}

/// The representation of `op` from the recorded types.
fn operand_type(types: &HashMap<ValueId, Type>, op: &Operand) -> Type {
    match op {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            Type::Ref
        }
    }
}

/// The environment of each closure body of the module (the uniform entry and the typed
/// body): the `AllocEnv` passed to `AllocClosure`/`AllocClosureTyped`. A
/// body with environments of different shapes is left out (`None`).
fn body_envs(module: &Module) -> HashMap<String, Option<Vec<Capture>>> {
    let mut exit: HashMap<String, Option<Vec<Capture>>> = HashMap::new();
    for f in &module.functions {
        let envs = function_envs(f);
        let mut register = |symbol: &str, env: &Operand| {
            let Operand::Val(e) = env else { return };
            let Some(captures) = envs.get(e) else { return };
            match exit.get(symbol) {
                Some(Some(already)) if already != captures => {
                    exit.insert(symbol.to_string(), None);
                }
                Some(_) => {}
                None => {
                    exit.insert(symbol.to_string(), Some(captures.clone()));
                }
            }
        };
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::AllocClosure { code_symbol, env } => register(code_symbol, env),
                    Instruction::AllocClosureTyped {
                        code_symbol,
                        env,
                        typed,
                        direct: false,
                        ..
                    } => {
                        register(code_symbol, env);
                        register(typed, env);
                    }
                    _ => {}
                }
            }
        }
    }
    exit
}

/// Verifies the whole module; returns the problems found.
pub fn verify(module: &Module) -> Vec<String> {
    let params: HashMap<&str, Vec<Type>> = module
        .functions
        .iter()
        .map(|f| (f.symbol.as_str(), f.params.iter().map(|p| p.2).collect()))
        .collect();
    let bodies = body_envs(module);
    let mut errors = Vec::new();
    for f in &module.functions {
        let mut c = Context {
            types: HashMap::new(),
            allocates: HashMap::new(),
            params: &params,
            function: f,
            errors: Vec::new(),
            cells: HashMap::new(),
            read_from_env: HashMap::new(),
            env: bodies.get(&f.symbol).and_then(|a| a.as_deref()),
        };
        for (vid, _, ty) in &f.params {
            c.types.insert(*vid, *ty);
        }
        for b in &f.blocks {
            for (vid, inst, ty) in &b.instructions {
                c.types.insert(*vid, *ty);
                if let Instruction::Alloca(t) = inst {
                    c.allocates.insert(*vid, *t);
                }
            }
        }
        // The cells of this function and what was written into them, and what was
        // read from the environment (the position), before checking the reads.
        for b in &f.blocks {
            for (vid, inst, _) in &b.instructions {
                match inst {
                    Instruction::AllocCell { value } => {
                        let t = c.ty(value);
                        c.cells.insert(*vid, vec![t]);
                    }
                    Instruction::EnvGet { env, index } if c.is_env_parameter(env) => {
                        c.read_from_env.insert(*vid, *index);
                    }
                    _ => {}
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                if let Instruction::CellSet {
                    cell: Operand::Val(cel),
                    value,
                } = inst
                    && c.cells.contains_key(cel)
                {
                    let t = c.ty(value);
                    c.cells.get_mut(cel).expect("checked cell").push(t);
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, ty) in &b.instructions {
                verify_instruction(&mut c, inst, *ty);
                if let Instruction::ArcLoadStrong {
                    slot: StrongSlot::Global { symbol },
                }
                | Instruction::ArcStoreStrong {
                    slot: StrongSlot::Global { symbol },
                    ..
                } = inst
                {
                    let declarations: Vec<_> = module
                        .globals
                        .iter()
                        .filter(|(_, _, s)| s == symbol)
                        .collect();
                    if declarations.len() != 1 || declarations[0].1 != Type::Ref {
                        c.error(format!(
                            "global ARC slot @{symbol} requires a single Ref declaration"
                        ));
                    }
                }
            }
            if let Terminator::Return(Some(op)) = &b.terminator {
                c.check_ref("return", op, f.return_ty);
            }
        }
        errors.extend(c.errors);
    }
    errors
}

fn verify_instruction(c: &mut Context, inst: &Instruction, ty: Type) {
    match inst {
        Instruction::ArcLoadStrong { slot } | Instruction::ArcStoreStrong { slot, .. } => {
            let result = if matches!(inst, Instruction::ArcLoadStrong { .. }) {
                Type::Ref
            } else {
                Type::Void
            };
            if ty != result {
                c.error(format!("ARC slot: result {ty:?}, expected {result:?}"));
            }
            if let StrongSlot::Frame { frame, .. } = slot
                && (c.ty(frame) != Type::I64 || !matches!(frame, Operand::Val(_)))
            {
                c.error(format!("ARC slot requires an SSA I64 frame ID: {frame:?}"));
            }
            if let Instruction::ArcStoreStrong { value, .. } = inst
                && (c.ty(value) != Type::Ref
                    || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null)))
            {
                c.error(format!(
                    "ARC store requires an already evaluated reference: {value:?}"
                ));
            }
        }
        Instruction::ArcCopy { value }
        | Instruction::ArcDrop { value }
        | Instruction::ArcMove { value } => {
            let result = if matches!(inst, Instruction::ArcDrop { .. }) {
                Type::Void
            } else {
                Type::Ref
            };
            if ty != result {
                c.error(format!("ARC operation: result {ty:?}, expected {result:?}"));
            }
            if c.ty(value) != Type::Ref
                || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null))
            {
                c.error(format!(
                    "ARC operation requires an already evaluated reference: {value:?}"
                ));
            }
        }
        Instruction::CallRuntime { name, args, .. }
        | Instruction::DeclaredFieldCall { name, args, .. } => {
            for &(b, t) in pairs_with_tag(name) {
                if let (Some((bits, _)), Some((tag, _))) = (args.get(b), args.get(t)) {
                    c.check_tag(name, bits, tag);
                }
            }
            for &(b, r) in pairs_with_is_ref(name) {
                if let (Some((bits, _)), Some((is_ref, _))) = (args.get(b), args.get(r)) {
                    c.check_is_ref(name, bits, is_ref);
                }
            }
            for (op, position) in args {
                c.check_ref(name, op, *position);
            }
        }
        Instruction::CallStatic { symbol, args, .. } => {
            if let Some(ps) = c.params.get(symbol.as_str()).cloned() {
                for (op, p) in args.iter().zip(ps) {
                    c.check_ref(symbol, op, p);
                }
            }
        }
        Instruction::AllocList { elements } => {
            for (op, tag) in elements {
                c.check_tag(
                    "collection literal",
                    op,
                    &Operand::Constant(Constant::Int(i64::from(*tag))),
                );
            }
        }
        // The positional record is `REFS` (§2.5): every field is a `Ref`.
        Instruction::AllocRecord { elements } => {
            for (op, _) in elements {
                let t = c.ty(op);
                if t != Type::Ref {
                    c.error(format!("positional record: field {t:?} (must be Ref)"));
                }
                c.check_ref("positional record", op, Type::Ref);
            }
        }
        // Risk 12: the capture read in the written representation.
        Instruction::EnvGet { env, index } if c.is_env_parameter(env) => {
            if let Some(g) = c.env.and_then(|a| a.get(*index)).map(|x| x.ty) {
                c.check_read(&format!("environment capture {index}"), ty, &[g]);
            }
        }
        Instruction::CellGet { cell } => {
            if let Some(g) = c.stored_in_cell(cell) {
                c.check_read("cell", ty, &g);
            }
        }
        Instruction::CellSet { cell, value } => {
            // Writing into a cell that came from the environment: in its representation.
            if let Operand::Val(v) = cell
                && !c.cells.contains_key(v)
                && let Some(g) = c.stored_in_cell(cell)
            {
                let t = c.ty(value);
                c.check_read("write to the cell", t, &g);
            }
        }
        Instruction::Phi { incoming, ty } => {
            for (_, op) in incoming {
                c.check_ref("phi", op, *ty);
            }
        }
        Instruction::Store {
            ptr: Operand::Val(p),
            val,
        } => {
            if let Some(&t) = c.allocates.get(p) {
                c.check_ref("store", val, t);
            }
        }
        Instruction::Const(Constant::Int(_)) if ty == Type::Ref => {
            c.error("integer constant recorded as Ref".to_string());
        }
        Instruction::CallClosure {
            closure,
            args,
            tuple_types,
            ..
        } => {
            c.check_ref("closure call", closure, Type::Ref);
            if c.ty(tuple_types) != Type::I64 {
                c.error(format!("closure type tuple is not I64: {tuple_types:?}"));
            }
            for a in args {
                c.check_ref("closure call", a, Type::Ref);
                if c.ty(a) != Type::Ref {
                    c.error(format!("closure argument is not Ref: {a:?}"));
                }
            }
        }
        Instruction::CallSelectorForward { recv, .. } => {
            c.check_ref("selector forwarding", recv, Type::Ref);
        }
        Instruction::CallSelector {
            recv,
            args,
            tuple_types,
            ..
        } => {
            c.check_ref("selector call", recv, Type::Ref);
            if c.ty(tuple_types) != Type::I64 {
                c.error(format!("selector type tuple is not I64: {tuple_types:?}"));
            }
            for a in args {
                if c.ty(a) != Type::Ref {
                    c.error(format!("selector argument is not Ref: {a:?}"));
                }
            }
        }
        Instruction::GetListElement { .. }
        | Instruction::SetListElement { .. }
        | Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CheckNotNull(_)
        | Instruction::IsClass { .. } => {
            c.error(format!("instruction without emission: {inst:?}"));
        }
        _ => {}
    }
}

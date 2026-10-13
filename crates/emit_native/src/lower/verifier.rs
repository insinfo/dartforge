//! Verificador da HIR, antes da emissão (E3 do contrato,
//! `docs/NATIVO-PLANO.md` §6.3).
//!
//! Recusa o que, emitido, vira `panic` no runtime ou IR aceito calado pelo
//! Clang: gravação no heap com tag incoerente com a representação do valor
//! (um escalar marcado como referência vira "handle inválido" no coletor;
//! uma referência marcada como escalar é coletada viva); constante inteira
//! numa posição `Ref` (a única constante `Ref` é `Null`); instrução que o
//! emissor não sabe baixar (o antigo `; inst pending`) ou que saiu com o
//! espaço unificado (`AllocMap`, `AllocSet`); record posicional com campo que
//! não é `Ref` (o `_Record` é `REFS`); e captura lida numa representação
//! diferente da gravada (docs/NATIVO-ESPACO-UNIFICADO.md §6, risco 12: o
//! `_Contexto` e a `_Celula` guardam a palavra crua, e só o bit do mapa diz se
//! é referência — ler um `int` gravado cru como `Ref` é um handle falso).
//!
//! Um problema aqui é bug do compilador, não do programa: a mensagem diz a
//! função e a instrução.

use crate::hir::*;
use std::collections::HashMap;

/// Pares (índice do argumento com os bits, índice do argumento com a tag)
/// das externs do runtime que recebem um valor com tag (a ABI plana `(bits,
/// tag)`: 1 `int`, 2 `bool`, 3 `Ref`, 4 `double`).
fn pairs_with_tag(name: &str) -> &'static [(usize, usize)] {
    match name {
        "dartforge_exception_throw" | "dartforge_tagged_to_string" => &[(0, 1)],
        _ => &[],
    }
}

/// Pares (bits, `is_ref`) das externs que recebem a marca de referência.
fn pairs_with_is_ref(name: &str) -> &'static [(usize, usize)] {
    match name {
        "dartforge_object_set" => &[(2, 3)],
        "dartforge_assertion_error_new"
        | "dartforge_exception_new"
        | "dartforge_argument_error_value" => &[(0, 1)],
        _ => &[],
    }
}

/// Uma posição de um `_Contexto` (`AllocEnv`): a representação gravada e,
/// se o valor é uma `_Celula` da mesma função, as representações gravadas
/// nela.
#[derive(Clone, PartialEq)]
struct Capture {
    ty: Type,
    cell: Option<Vec<Type>>,
}

/// A representação de `a` e `b` é a mesma palavra no contexto? (`I1` e `I8`
/// são o mesmo `bool` de 0/1.)
fn same_representation(a: Type, b: Type) -> bool {
    a == b || matches!((a, b), (Type::I1 | Type::I8, Type::I1 | Type::I8))
}

struct Context<'m> {
    types: HashMap<ValueId, Type>,
    allocates: HashMap<ValueId, Type>,
    params: &'m HashMap<&'m str, Vec<Type>>,
    function: &'m Function,
    errors: Vec<String>,
    /// As representações gravadas em cada `_Celula` alocada nesta função
    /// (`AllocCell` e `CellSet` sobre o mesmo valor).
    cells: HashMap<ValueId, Vec<Type>>,
    /// Os valores lidos do ambiente da closure (`EnvGet` sobre o parâmetro
    /// `env`), com a posição lida.
    read_from_env: HashMap<ValueId, usize>,
    /// O ambiente desta função (o de toda closure que a usa como corpo),
    /// quando ele é conhecido e único no módulo.
    env: Option<&'m [Capture]>,
}

impl Context<'_> {
    fn ty(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.types.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Function(_)) => Type::I64,
        }
    }

    fn error(&mut self, msg: String) {
        self.errors
            .push(format!("verificador da HIR ({}): {msg}", self.function.name));
    }

    /// E1: a tag gravada tem de ser a da representação do valor.
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
            self.error(format!("{location}: tag {t} para um valor {ty:?}"));
        }
    }

    fn check_is_ref(&mut self, location: &str, bits: &Operand, is_ref: &Operand) {
        let Operand::Constant(Constant::Int(r)) = is_ref else {
            return;
        };
        let ty = self.ty(bits);
        if (*r != 0) != (ty == Type::Ref) {
            self.error(format!("{location}: is_ref {r} para um valor {ty:?}"));
        }
    }

    /// E3: constante inteira numa posição `Ref`.
    fn check_ref(&mut self, location: &str, op: &Operand, position: Type) {
        if position == Type::Ref && matches!(op, Operand::Constant(Constant::Int(_))) {
            self.error(format!("{location}: constante inteira numa posição Ref"));
        }
    }

    /// O parâmetro `env` desta função (o ambiente do corpo de uma closure).
    fn is_env_parameter(&self, op: &Operand) -> bool {
        let Operand::Val(v) = op else { return false };
        self.function.params.iter().any(|(p, name, _)| p == v && name == "env")
    }

    /// As representações gravadas na célula `cell`, se conhecidas: uma
    /// `_Celula` desta função, ou uma lida do ambiente da closure.
    fn stored_in_cell(&self, cell: &Operand) -> Option<Vec<Type>> {
        let Operand::Val(v) = cell else { return None };
        if let Some(t) = self.cells.get(v) {
            return Some(t.clone());
        }
        let i = *self.read_from_env.get(v)?;
        self.env?.get(i)?.cell.clone()
    }

    /// Risco 12: o valor lido de uma captura tem de estar na representação
    /// com que ela foi gravada.
    fn check_read(&mut self, location: &str, read_value: Type, written_set: &[Type]) {
        if let Some(g) = written_set.iter().find(|&&g| !same_representation(g, read_value)) {
            self.error(format!("{location}: lida como {read_value:?}, gravada como {g:?}"));
        }
    }
}

/// As capturas de cada `AllocEnv` de `f` (pelo valor dele), com as células
/// alocadas em `f` que ele guarda.
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
            if let Instruction::CellSet { cell: Operand::Val(c), value } = inst
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

/// A representação de `op` pelos tipos registrados.
fn operand_type(types: &HashMap<ValueId, Type>, op: &Operand) -> Type {
    match op {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
    }
}

/// O ambiente de cada corpo de closure do módulo (a entrada uniforme e o corpo
/// tipado): o `AllocEnv` passado a `AllocClosure`/`AllocClosureTyped`. Um
/// corpo com ambientes de formas diferentes fica de fora (`None`).
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
                    Instruction::AllocClosureTyped { code_symbol, env, typed, direct: false, .. } => {
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

/// Verifica o módulo inteiro; devolve os problemas encontrados.
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
        // As células desta função e o que foi gravado nelas, e o que foi
        // lido do ambiente (a posição), antes de conferir as leituras.
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
                if let Instruction::CellSet { cell: Operand::Val(cel), value } = inst
                    && c.cells.contains_key(cel)
                {
                    let t = c.ty(value);
                    c.cells.get_mut(cel).expect("célula conferida").push(t);
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, ty) in &b.instructions {
                verify_instruction(&mut c, inst, *ty);
                if let Instruction::ArcLoadStrong { slot: StrongSlot::Global { symbol } }
                    | Instruction::ArcStoreStrong { slot: StrongSlot::Global { symbol }, .. } = inst {
                    let declarations: Vec<_> = module.globals.iter().filter(|(_, _, s)| s == symbol).collect();
                    if declarations.len() != 1 || declarations[0].1 != Type::Ref {
                        c.error(format!("slot ARC global @{symbol} exige uma declaração Ref única"));
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
            let result = if matches!(inst, Instruction::ArcLoadStrong { .. }) { Type::Ref } else { Type::Void };
            if ty != result {
                c.error(format!("slot ARC: resultado {ty:?}, esperado {result:?}"));
            }
            if let StrongSlot::Quadro { frame, .. } = slot {
                if c.ty(frame) != Type::I64 || !matches!(frame, Operand::Val(_)) {
                    c.error(format!("slot ARC exige ID de quadro SSA I64: {frame:?}"));
                }
            }
            if let Instruction::ArcStoreStrong { value, .. } = inst {
                if c.ty(value) != Type::Ref || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null)) {
                    c.error(format!("store ARC exige referência já avaliada: {value:?}"));
                }
            }
        }
        Instruction::ArcCopy { value } | Instruction::ArcDrop { value } | Instruction::ArcMove { value } => {
            let result = if matches!(inst, Instruction::ArcDrop { .. }) { Type::Void } else { Type::Ref };
            if ty != result {
                c.error(format!("operação ARC: resultado {ty:?}, esperado {result:?}"));
            }
            if c.ty(value) != Type::Ref
                || !matches!(value, Operand::Val(_) | Operand::Constant(Constant::Null))
            {
                c.error(format!("operação ARC exige referência já avaliada: {value:?}"));
            }
        }
        Instruction::CallRuntime { name, args, .. } => {
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
                    "literal de coleção",
                    op,
                    &Operand::Constant(Constant::Int(i64::from(*tag))),
                );
            }
        }
        // O record posicional é `REFS` (§2.5): todo campo é `Ref`.
        Instruction::AllocRecord { elements } => {
            for (op, _) in elements {
                let t = c.ty(op);
                if t != Type::Ref {
                    c.error(format!("record posicional: campo {t:?} (tem de ser Ref)"));
                }
                c.check_ref("record posicional", op, Type::Ref);
            }
        }
        // Risco 12: a captura lida na representação gravada.
        Instruction::EnvGet { env, index } if c.is_env_parameter(env) => {
            if let Some(g) = c.env.and_then(|a| a.get(*index)).map(|x| x.ty) {
                c.check_read(&format!("captura {index} do ambiente"), ty, &[g]);
            }
        }
        Instruction::CellGet { cell } => {
            if let Some(g) = c.stored_in_cell(cell) {
                c.check_read("célula", ty, &g);
            }
        }
        Instruction::CellSet { cell, value } => {
            // Gravar numa célula que veio do ambiente: na representação dela.
            if let Operand::Val(v) = cell
                && !c.cells.contains_key(v)
                && let Some(g) = c.stored_in_cell(cell)
            {
                let t = c.ty(value);
                c.check_read("gravação na célula", t, &g);
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
            c.error("constante inteira registrada como Ref".to_string());
        }
        Instruction::CallClosure { closure, args, tuple_types, .. } => {
            c.check_ref("chamada de closure", closure, Type::Ref);
            if c.ty(tuple_types) != Type::I64 {
                c.error(format!("tupla de tipos da closure não é I64: {tuple_types:?}"));
            }
            for a in args {
                c.check_ref("chamada de closure", a, Type::Ref);
                if c.ty(a) != Type::Ref {
                    c.error(format!("argumento de closure não é Ref: {a:?}"));
                }
            }
        }
        Instruction::CallSelectorForward { recv, .. } => {
            c.check_ref("repasse por seletor", recv, Type::Ref);
        }
        Instruction::CallSelector { recv, args, tuple_types, .. } => {
            c.check_ref("chamada por seletor", recv, Type::Ref);
            if c.ty(tuple_types) != Type::I64 {
                c.error(format!("tupla de tipos do seletor não é I64: {tuple_types:?}"));
            }
            for a in args {
                if c.ty(a) != Type::Ref {
                    c.error(format!("argumento de seletor não é Ref: {a:?}"));
                }
            }
        }
        Instruction::GetListElement { .. }
        | Instruction::SetListElement { .. }
        | Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CheckNotNull(_)
        | Instruction::IsClass { .. } => {
            c.error(format!("instrução sem emissão: {inst:?}"));
        }
        _ => {}
    }
}

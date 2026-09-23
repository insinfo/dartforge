//! Verificador da HIR, antes da emissão (E3 do contrato,
//! `docs/NATIVO-PLANO.md` §6.3).
//!
//! Recusa o que, emitido, vira `panic` no runtime ou IR aceito calado pelo
//! Clang: gravação no heap com tag incoerente com a representação do valor
//! (um escalar marcado como referência vira "handle além da tabela" no
//! `collect`; uma referência marcada como escalar é coletada viva); constante
//! inteira numa posição `Ref` (a única constante `Ref` é `Null`); e
//! instrução que o emissor não sabe baixar (o antigo `; inst pendente`).
//!
//! Um problema aqui é bug do compilador, não do programa: a mensagem diz a
//! função e a instrução.

use crate::hir::*;
use std::collections::HashMap;

/// Pares (índice do argumento com os bits, índice do argumento com a tag)
/// das externs do runtime que recebem um valor com tag.
fn pares_com_tag(nome: &str) -> &'static [(usize, usize)] {
    match nome {
        "dartforge_list_push"
        | "dartforge_set_add"
        | "dartforge_set_contains"
        | "dartforge_map_contains"
        | "dartforge_map_get_bits"
        | "dartforge_map_get_tag"
        | "dartforge_map_get_ref"
        | "dartforge_map_remove"
        | "dartforge_cell_set"
        | "dartforge_list_filled"
        | "dartforge_map_get_to_string" => &[(1, 2)],
        "dartforge_list_set" => &[(2, 3)],
        "dartforge_map_set" => &[(1, 2), (3, 4)],
        "dartforge_cell_new" | "dartforge_exception_throw" | "dartforge_tagged_to_string" => {
            &[(0, 1)]
        }
        _ => &[],
    }
}

/// Pares (bits, `is_ref`) das externs que recebem a marca de referência.
fn pares_com_is_ref(nome: &str) -> &'static [(usize, usize)] {
    match nome {
        "dartforge_object_set" => &[(2, 3)],
        "dartforge_assertion_error_new"
        | "dartforge_exception_new"
        | "dartforge_argument_error_value" => &[(0, 1)],
        _ => &[],
    }
}

struct Contexto<'m> {
    tipos: HashMap<ValueId, Type>,
    alocas: HashMap<ValueId, Type>,
    params: &'m HashMap<&'m str, Vec<Type>>,
    funcao: &'m Function,
    erros: Vec<String>,
}

impl Contexto<'_> {
    fn tipo(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.tipos.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null | Constant::String(_)) => Type::Ref,
        }
    }

    fn erro(&mut self, msg: String) {
        self.erros
            .push(format!("verificador da HIR ({}): {msg}", self.funcao.name));
    }

    /// E1: a tag gravada tem de ser a da representação do valor.
    fn checar_tag(&mut self, onde: &str, bits: &Operand, tag: &Operand) {
        let Operand::Constant(Constant::Int(t)) = tag else {
            return;
        };
        let ty = self.tipo(bits);
        let coerente = match t {
            3 => ty == Type::Ref,
            1 | 2 | 4 => ty != Type::Ref,
            _ => true,
        };
        if !coerente {
            self.erro(format!("{onde}: tag {t} para um valor {ty:?}"));
        }
    }

    fn checar_is_ref(&mut self, onde: &str, bits: &Operand, is_ref: &Operand) {
        let Operand::Constant(Constant::Int(r)) = is_ref else {
            return;
        };
        let ty = self.tipo(bits);
        if (*r != 0) != (ty == Type::Ref) {
            self.erro(format!("{onde}: is_ref {r} para um valor {ty:?}"));
        }
    }

    /// E3: constante inteira numa posição `Ref`.
    fn checar_ref(&mut self, onde: &str, op: &Operand, posicao: Type) {
        if posicao == Type::Ref && matches!(op, Operand::Constant(Constant::Int(_))) {
            self.erro(format!("{onde}: constante inteira numa posição Ref"));
        }
    }
}

/// Verifica o módulo inteiro; devolve os problemas encontrados.
pub fn verificar(module: &Module) -> Vec<String> {
    let params: HashMap<&str, Vec<Type>> = module
        .functions
        .iter()
        .map(|f| (f.symbol.as_str(), f.params.iter().map(|p| p.2).collect()))
        .collect();
    let mut erros = Vec::new();
    for f in &module.functions {
        let mut c = Contexto {
            tipos: HashMap::new(),
            alocas: HashMap::new(),
            params: &params,
            funcao: f,
            erros: Vec::new(),
        };
        for (vid, _, ty) in &f.params {
            c.tipos.insert(*vid, *ty);
        }
        for b in &f.blocks {
            for (vid, inst, ty) in &b.instructions {
                c.tipos.insert(*vid, *ty);
                if let Instruction::Alloca(t) = inst {
                    c.alocas.insert(*vid, *t);
                }
            }
        }
        for b in &f.blocks {
            for (_, inst, ty) in &b.instructions {
                verificar_instrucao(&mut c, inst, *ty);
            }
            if let Terminator::Return(Some(op)) = &b.terminator {
                c.checar_ref("return", op, f.return_ty);
            }
        }
        erros.extend(c.erros);
    }
    erros
}

fn verificar_instrucao(c: &mut Contexto, inst: &Instruction, ty: Type) {
    match inst {
        Instruction::CallRuntime { name, args, .. } => {
            for &(b, t) in pares_com_tag(name) {
                if let (Some((bits, _)), Some((tag, _))) = (args.get(b), args.get(t)) {
                    c.checar_tag(name, bits, tag);
                }
            }
            for &(b, r) in pares_com_is_ref(name) {
                if let (Some((bits, _)), Some((is_ref, _))) = (args.get(b), args.get(r)) {
                    c.checar_is_ref(name, bits, is_ref);
                }
            }
            for (op, posicao) in args {
                c.checar_ref(name, op, *posicao);
            }
        }
        Instruction::CallStatic { symbol, args, .. } => {
            if let Some(ps) = c.params.get(symbol.as_str()).cloned() {
                for (op, p) in args.iter().zip(ps) {
                    c.checar_ref(symbol, op, p);
                }
            }
        }
        Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
            for (op, tag) in elements {
                c.checar_tag(
                    "literal de coleção",
                    op,
                    &Operand::Constant(Constant::Int(i64::from(*tag))),
                );
            }
        }
        Instruction::AllocMap { entries } => {
            for ((k, kt), (v, vt)) in entries {
                c.checar_tag(
                    "literal de mapa",
                    k,
                    &Operand::Constant(Constant::Int(i64::from(*kt))),
                );
                c.checar_tag(
                    "literal de mapa",
                    v,
                    &Operand::Constant(Constant::Int(i64::from(*vt))),
                );
            }
        }
        Instruction::Phi { incoming, ty } => {
            for (_, op) in incoming {
                c.checar_ref("phi", op, *ty);
            }
        }
        Instruction::Store {
            ptr: Operand::Val(p),
            val,
        } => {
            if let Some(&t) = c.alocas.get(p) {
                c.checar_ref("store", val, t);
            }
        }
        Instruction::Const(Constant::Int(_)) if ty == Type::Ref => {
            c.erro("constante inteira registrada como Ref".to_string());
        }
        Instruction::AllocSet { .. }
        | Instruction::AllocCell { .. }
        | Instruction::AllocEnv { .. }
        | Instruction::AllocClosure { .. }
        | Instruction::GetListElement { .. }
        | Instruction::SetListElement { .. }
        | Instruction::CellGet { .. }
        | Instruction::CellSet { .. }
        | Instruction::EnvGet { .. }
        | Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CheckNotNull(_)
        | Instruction::IsClass { .. } => {
            c.erro(format!("instrução sem emissão: {inst:?}"));
        }
        _ => {}
    }
}

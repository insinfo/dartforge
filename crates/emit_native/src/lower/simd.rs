//! `Float32x4`, `Int32x4` e `Float64x2` sem caixa (`docs/SIMD-NATIVO.md`).
//!
//! Os locais SIMD de uma função síncrona que não são capturados guardam o
//! vetor (`Type::V4F32`…), e as expressões SIMD que o compilador reconhece
//! (operadores, métodos, getters e construtores do SDK, `a[i]` de uma lista
//! SIMD) viram instruções vetoriais (`Instruction::Simd`, emitidas em
//! `llvm/simd.rs`). A caixa (`Ref`) continua sendo o valor em toda
//! fronteira — parâmetros, retornos, campos, coleções, `dynamic`: o
//! `lower_expr` geral coage ao tipo estático, que é `Ref`, e só
//! [`FnBuilder::lower_simd`] entrega o vetor.
//!
//! O que não é reconhecido (máscara de `shuffle` que não é constante, um
//! membro sem forma vetorial) segue o caminho de sempre, pela caixa e pelo
//! `simd.rs` do runtime, com a mesma semântica.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_types::resolved::Resolved;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

/// Como um argumento entra na operação.
#[derive(Clone, Copy)]
enum Arg {
    /// Vetor do tipo dado (por [`FnBuilder::lower_simd`]).
    Vetor(Type),
    /// Escalar na representação dada (`double`, `int`, `bool`).
    Escalar(Type),
}

/// Uma expressão SIMD com forma sem caixa: a operação, o receptor (se há),
/// os argumentos e o tipo do resultado.
struct Receita {
    op: OpSimd,
    receptor: Option<(ExprId, Type)>,
    args: Vec<(ExprId, Arg)>,
    resultado: Type,
}

/// A pista de `x`/`y`/`z`/`w`.
fn pista(c: char) -> Option<u8> {
    match c {
        'x' => Some(0),
        'y' => Some(1),
        'z' => Some(2),
        'w' => Some(3),
        _ => None,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O vetor sem caixa do tipo estático `ty`, se ele é `Float32x4`,
    /// `Int32x4` ou `Float64x2` não anulável e a função é síncrona (um
    /// quadro assíncrono guarda 64 bits por posição).
    pub(super) fn tipo_simd(&self, ty: Option<TypeId>) -> Option<Type> {
        if !self.ctx.sdk_da_fonte || self.async_estado.is_some() {
            return None;
        }
        let T::Interface { class, nullable: false, .. } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = &self.ctx.program.classes[class.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:typed_data" {
            return None;
        }
        match self.ctx.symbol_name(c.name) {
            "Float32x4" | "_Float32x4" => Some(Type::V4F32),
            "Int32x4" | "_Int32x4" => Some(Type::V4I32),
            "Float64x2" | "_Float64x2" => Some(Type::V2F64),
            _ => None,
        }
    }

    /// O vetor sem caixa do tipo estático da expressão `e`.
    pub(super) fn simd_da_expr(&self, e: ExprId) -> Option<Type> {
        self.tipo_simd(self.ctx.get_type(self.unit_id, e))
    }

    /// A máscara constante de um `shuffle`: literal `0..=255`, ou uma das
    /// constantes `xyzw` das classes SIMD (o nome dá as pistas).
    fn mascara_constante(&self, ast: &ast::Ast, e: ExprId) -> Option<u8> {
        match &ast.expr(e).kind {
            ExprKind::Parenthesized(x) => self.mascara_constante(ast, *x),
            ExprKind::Int(span) => {
                let texto = self.source()[span.start..span.end].replace('_', "");
                let v = match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).ok()?,
                    None => texto.parse::<i64>().ok()?,
                };
                u8::try_from(v).ok()
            }
            ExprKind::Property { target, name, .. } => {
                let ExprKind::Identifier(c) = &ast.expr(*target).kind else { return None };
                if !matches!(self.ctx.symbol_name(c.sym), "Float32x4" | "Int32x4") {
                    return None;
                }
                let nome = self.ctx.symbol_name(name.sym);
                let pistas: Vec<u8> = nome.chars().map(pista).collect::<Option<_>>()?;
                if pistas.len() != 4 {
                    return None;
                }
                Some(pistas[0] | (pistas[1] << 2) | (pistas[2] << 4) | (pistas[3] << 6))
            }
            _ => None,
        }
    }

    /// A forma sem caixa da expressão `e`, se o compilador a reconhece.
    fn receita_simd(&self, ast: &ast::Ast, e: ExprId) -> Option<Receita> {
        use Type::{V2F64, V4F32, V4I32};
        if let Some(r) = self.receita_api_314(ast, e) {
            return Some(r);
        }
        let expr = ast.expr(e);
        match &expr.kind {
            ExprKind::Binary { op, left, right } => {
                let k = self.simd_da_expr(*left)?;
                let op = match (op, k) {
                    (BinaryOp::Add, _) => OpSimd::Add,
                    (BinaryOp::Sub, _) => OpSimd::Sub,
                    (BinaryOp::Mul, V4F32 | V2F64) => OpSimd::Mul,
                    (BinaryOp::Div, V4F32 | V2F64) => OpSimd::Div,
                    (BinaryOp::BitAnd, V4I32) => OpSimd::And,
                    (BinaryOp::BitOr, V4I32) => OpSimd::Or,
                    (BinaryOp::BitXor, V4I32) => OpSimd::Xor,
                    _ => return None,
                };
                (self.simd_da_expr(*right)? == k).then_some(())?;
                Some(Receita { op, receptor: Some((*left, k)), args: vec![(*right, Arg::Vetor(k))], resultado: k })
            }
            ExprKind::Unary { op: UnaryOp::Neg, operand } => {
                let k = self.simd_da_expr(*operand)?;
                (k != V4I32).then_some(())?;
                Some(Receita { op: OpSimd::Neg, receptor: Some((*operand, k)), args: Vec::new(), resultado: k })
            }
            ExprKind::Property { target, name, null_aware: false } => {
                let k = self.simd_da_expr(*target)?;
                let nome = self.ctx.symbol_name(name.sym);
                let escalar = if k == V4I32 { Type::I64 } else { Type::F64 };
                let (op, resultado) = match nome {
                    "signMask" => (OpSimd::SinalMask, Type::I64),
                    "x" | "y" | "z" | "w" => {
                        let i = pista(nome.chars().next()?)?;
                        (k != V2F64 || i < 2).then_some(())?;
                        (OpSimd::Pista(i), escalar)
                    }
                    "flagX" | "flagY" | "flagZ" | "flagW" if k == V4I32 => {
                        (OpSimd::Flag(pista(nome.chars().nth(4)?.to_ascii_lowercase())?), Type::I1)
                    }
                    _ => return None,
                };
                Some(Receita { op, receptor: Some((*target, k)), args: Vec::new(), resultado })
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() || arguments.args.iter().any(|a| a.name.is_some()) {
                    return None;
                }
                let args: Vec<ExprId> = arguments.args.iter().map(|a| a.value).collect();
                if let Some(Resolved::Constructor(fid)) = self.ctx.get_resolved(self.unit_id, e) {
                    return self.receita_de_construtor(fid.0 as usize, args);
                }
                let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
                    return None;
                };
                let k = self.simd_da_expr(*recv)?;
                let nome = self.ctx.symbol_name(name.sym);
                let receptor = Some((*recv, k));
                let um = |op: OpSimd, a: Arg, resultado: Type| {
                    (args.len() == 1).then(|| Receita { op, receptor, args: vec![(args[0], a)], resultado })
                };
                let nenhum = |op: OpSimd| args.is_empty().then(|| Receita { op, receptor, args: Vec::new(), resultado: k });
                match (nome, k) {
                    ("scale", V4F32 | V2F64) => um(OpSimd::Escala, Arg::Escalar(Type::F64), k),
                    ("abs", V4F32 | V2F64) => nenhum(OpSimd::Abs),
                    ("sqrt", V4F32 | V2F64) => nenhum(OpSimd::Sqrt),
                    ("reciprocal", V4F32) => nenhum(OpSimd::Recip),
                    ("reciprocalSqrt", V4F32) => nenhum(OpSimd::RecipSqrt),
                    ("min", V4F32 | V2F64) => um(OpSimd::Min, Arg::Vetor(k), k),
                    ("max", V4F32 | V2F64) => um(OpSimd::Max, Arg::Vetor(k), k),
                    ("clamp", V4F32 | V2F64) if args.len() == 2 => Some(Receita {
                        op: OpSimd::Clamp,
                        receptor,
                        args: vec![(args[0], Arg::Vetor(k)), (args[1], Arg::Vetor(k))],
                        resultado: k,
                    }),
                    ("equal", V4F32) => um(OpSimd::Cmp(FCmpOp::Eq), Arg::Vetor(k), V4I32),
                    ("notEqual", V4F32) => um(OpSimd::Cmp(FCmpOp::Ne), Arg::Vetor(k), V4I32),
                    ("lessThan", V4F32) => um(OpSimd::Cmp(FCmpOp::Lt), Arg::Vetor(k), V4I32),
                    ("lessThanOrEqual", V4F32) => um(OpSimd::Cmp(FCmpOp::Le), Arg::Vetor(k), V4I32),
                    ("greaterThan", V4F32) => um(OpSimd::Cmp(FCmpOp::Gt), Arg::Vetor(k), V4I32),
                    ("greaterThanOrEqual", V4F32) => um(OpSimd::Cmp(FCmpOp::Ge), Arg::Vetor(k), V4I32),
                    ("shuffle", V4F32 | V4I32) if args.len() == 1 => {
                        let m = self.mascara_constante(ast, args[0])?;
                        Some(Receita { op: OpSimd::Shuffle(m), receptor, args: Vec::new(), resultado: k })
                    }
                    ("shuffleMix", V4F32 | V4I32) if args.len() == 2 => {
                        let m = self.mascara_constante(ast, args[1])?;
                        Some(Receita { op: OpSimd::ShuffleMix(m), receptor, args: vec![(args[0], Arg::Vetor(k))], resultado: k })
                    }
                    ("withX" | "withY" | "withZ" | "withW", _) => {
                        let i = pista(nome.chars().nth(4)?.to_ascii_lowercase())?;
                        (k != V2F64 || i < 2).then_some(())?;
                        let escalar = if k == V4I32 { Type::I64 } else { Type::F64 };
                        um(OpSimd::ComPista(i), Arg::Escalar(escalar), k)
                    }
                    ("withFlagX" | "withFlagY" | "withFlagZ" | "withFlagW", V4I32) => {
                        let i = pista(nome.chars().nth(8)?.to_ascii_lowercase())?;
                        um(OpSimd::ComFlag(i), Arg::Escalar(Type::I1), k)
                    }
                    ("select", V4I32) if args.len() == 2 => Some(Receita {
                        op: OpSimd::Select,
                        receptor,
                        args: vec![(args[0], Arg::Vetor(V4F32)), (args[1], Arg::Vetor(V4F32))],
                        resultado: V4F32,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `e` chama um membro de uma extensão sobre `Int32x4` marcada com
    /// `@pragma('dartforge:simd-api', '3.14')` — a API de `Int32x4` do Dart
    /// 3.14 escrita em Dart 3.6 (`docs/SIMD-NATIVO.md` §6). O corpo dos
    /// membros é o do `typed_data_patch.dart` da VM (roda na VM 3.6.2, que é
    /// o oráculo); aqui a chamada vira a instrução vetorial.
    fn api_314(&self, e: ExprId) -> bool {
        let Some(Resolved::ExtensionMember { extension, .. }) = self.ctx.get_resolved(self.unit_id, e) else {
            return false;
        };
        let decl = self.ctx.program.extension(*extension).decl;
        let unit = self.ctx.program.unit(decl.unit);
        let Some(d) = unit.ast.decls.get(decl.decl.0 as usize) else { return false };
        d.metadata.iter().any(|an| {
            if an.name.len() != 1 || self.ctx.symbol_name(an.name[0].sym) != "pragma" {
                return false;
            }
            let Some(args) = &an.arguments else { return false };
            let textos: Vec<Option<String>> = args
                .args
                .iter()
                .map(|a| match &unit.ast.exprs[a.value.0 as usize].kind {
                    ExprKind::String(lit) => lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned()),
                    _ => None,
                })
                .collect();
            matches!(textos.as_slice(), [Some(a), Some(b)] if a == "dartforge:simd-api" && b == "3.14")
        })
    }

    /// `e` tem tipo estático `int` não anulável.
    fn e_int(&self, e: ExprId) -> bool {
        matches!(
            self.ctx.get_type(self.unit_id, e).map(|t| self.ctx.table.get(t)),
            Some(T::Interface { class, nullable: false, .. }) if Some(*class) == self.ctx.core.int_class
        )
    }

    /// A forma sem caixa de um membro da API de `Int32x4` do Dart 3.14
    /// ([`Self::api_314`]): operadores `~`, `-` unário, `*`, `<<`, `>>`;
    /// `andNot`, `abs`, as comparações com sinal, `min`/`max`; `anyTrue` e
    /// `allTrue`.
    fn receita_api_314(&self, ast: &ast::Ast, e: ExprId) -> Option<Receita> {
        use Type::V4I32;
        let int4 = |x: ExprId| self.simd_da_expr(x) == Some(V4I32);
        match &ast.expr(e).kind {
            ExprKind::Binary { op, left, right } if self.api_314(e) && int4(*left) => {
                let receptor = Some((*left, V4I32));
                match op {
                    BinaryOp::Mul if int4(*right) => {
                        Some(Receita { op: OpSimd::Mul, receptor, args: vec![(*right, Arg::Vetor(V4I32))], resultado: V4I32 })
                    }
                    BinaryOp::Shl | BinaryOp::Shr if self.e_int(*right) => Some(Receita {
                        op: OpSimd::Desloca(*op == BinaryOp::Shr),
                        receptor,
                        args: vec![(*right, Arg::Escalar(Type::I64))],
                        resultado: V4I32,
                    }),
                    _ => None,
                }
            }
            ExprKind::Unary { op: op @ (UnaryOp::Neg | UnaryOp::BitNot), operand } if self.api_314(e) && int4(*operand) => {
                let op = if *op == UnaryOp::Neg { OpSimd::Neg } else { OpSimd::Not };
                Some(Receita { op, receptor: Some((*operand, V4I32)), args: Vec::new(), resultado: V4I32 })
            }
            ExprKind::Property { target, name, null_aware: false } if self.api_314(e) && int4(*target) => {
                let op = match self.ctx.symbol_name(name.sym) {
                    "anyTrue" => OpSimd::Algum,
                    "allTrue" => OpSimd::Todos,
                    _ => return None,
                };
                Some(Receita { op, receptor: Some((*target, V4I32)), args: Vec::new(), resultado: Type::I1 })
            }
            ExprKind::Call { target, arguments } => {
                if !arguments.type_args.is_empty() || arguments.args.iter().any(|a| a.name.is_some()) {
                    return None;
                }
                let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
                    return None;
                };
                if !(self.api_314(e) || self.api_314(*target)) || !int4(*recv) {
                    return None;
                }
                let args: Vec<ExprId> = arguments.args.iter().map(|a| a.value).collect();
                let receptor = Some((*recv, V4I32));
                let op = match (self.ctx.symbol_name(name.sym), args.as_slice()) {
                    ("abs", []) => {
                        return Some(Receita { op: OpSimd::Abs, receptor, args: Vec::new(), resultado: V4I32 });
                    }
                    (_, [a]) if !int4(*a) => return None,
                    ("andNot", [_]) => OpSimd::AndNot,
                    ("equal", [_]) => OpSimd::CmpInt(ICmpOp::Eq),
                    ("notEqual", [_]) => OpSimd::CmpInt(ICmpOp::Ne),
                    ("lessThan", [_]) => OpSimd::CmpInt(ICmpOp::Slt),
                    ("lessThanOrEqual", [_]) => OpSimd::CmpInt(ICmpOp::Sle),
                    ("greaterThan", [_]) => OpSimd::CmpInt(ICmpOp::Sgt),
                    ("greaterThanOrEqual", [_]) => OpSimd::CmpInt(ICmpOp::Sge),
                    ("min", [_]) => OpSimd::Min,
                    ("max", [_]) => OpSimd::Max,
                    _ => return None,
                };
                Some(Receita { op, receptor, args: vec![(args[0], Arg::Vetor(V4I32))], resultado: V4I32 })
            }
            _ => None,
        }
    }

    /// A forma sem caixa de um construtor das classes SIMD.
    fn receita_de_construtor(&self, fid: usize, args: Vec<ExprId>) -> Option<Receita> {
        use Type::{V2F64, V4F32, V4I32};
        let f = &self.ctx.program.functions[fid];
        let c = &self.ctx.program.classes[f.class?.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:typed_data" || self.async_estado.is_some() {
            return None;
        }
        let k = match self.ctx.symbol_name(c.name) {
            "Float32x4" => V4F32,
            "Int32x4" => V4I32,
            "Float64x2" => V2F64,
            _ => return None,
        };
        let nome = self.ctx.symbol_name(f.name);
        let pistas = if k == V2F64 { 2 } else { 4 };
        let receita = |op: OpSimd, a: Vec<Arg>| {
            (a.len() == args.len()).then(|| Receita {
                op,
                receptor: None,
                args: args.iter().copied().zip(a).collect(),
                resultado: k,
            })
        };
        match (nome, k) {
            ("", V4I32) => receita(OpSimd::Monta, vec![Arg::Escalar(Type::I64); 4]),
            ("", _) => receita(OpSimd::Monta, vec![Arg::Escalar(Type::F64); pistas]),
            ("bool", V4I32) => receita(OpSimd::MontaFlags, vec![Arg::Escalar(Type::I1); 4]),
            ("splat", V4F32 | V2F64) => receita(OpSimd::Splat, vec![Arg::Escalar(Type::F64)]),
            ("zero", V4F32 | V2F64) => receita(OpSimd::Zero, Vec::new()),
            ("fromInt32x4Bits", V4F32) => receita(OpSimd::Bits, vec![Arg::Vetor(V4I32)]),
            ("fromFloat32x4Bits", V4I32) => receita(OpSimd::Bits, vec![Arg::Vetor(V4F32)]),
            ("fromFloat64x2", V4F32) => receita(OpSimd::Converte, vec![Arg::Vetor(V2F64)]),
            ("fromFloat32x4", V2F64) => receita(OpSimd::Converte, vec![Arg::Vetor(V4F32)]),
            _ => None,
        }
    }

    /// Emite a receita: o receptor e os argumentos (vetores sem caixa pelo
    /// próprio `lower_simd`, escalares na representação da operação), na
    /// ordem de avaliação do Dart.
    fn emitir_receita(&mut self, ast: &ast::Ast, r: Receita) -> Operand {
        let mut ops = Vec::with_capacity(r.args.len() + 1);
        if let Some((recv, k)) = r.receptor {
            ops.push(self.lower_simd(ast, recv, k));
        }
        for (e, a) in r.args {
            let op = match a {
                Arg::Vetor(k) => self.lower_simd(ast, e, k),
                Arg::Escalar(t) => {
                    let v = self.lower_expr(ast, e);
                    self.coagir(v, t)
                }
            };
            ops.push(op);
        }
        self.emit(Instruction::Simd { op: r.op, args: ops }, r.resultado)
    }

    /// Uma expressão SIMD reconhecida, no `lower_expr` geral: o resultado
    /// escalar direto, ou o vetor (que o `lower_expr` põe na caixa do tipo
    /// estático). `None`: a expressão segue o caminho de sempre.
    pub(super) fn expressao_simd(&mut self, ast: &ast::Ast, e: ExprId) -> Option<Operand> {
        // O membro de um tipo de extensão sobre um tipo SIMD é o dele.
        let membro = match &ast.expr(e).kind {
            ExprKind::Call { target, .. } => *target,
            _ => e,
        };
        if self.membro_te(membro).is_some() {
            return None;
        }
        let r = self.receita_simd(ast, e)?;
        Some(self.emitir_receita(ast, r))
    }

    /// A expressão `e`, de tipo estático SIMD `k`, como vetor sem caixa:
    /// pela forma reconhecida, pelo local que guarda o vetor, pelo acesso a
    /// uma lista SIMD, ou desencaixotando o valor do caminho geral.
    pub(super) fn lower_simd(&mut self, ast: &ast::Ast, e: ExprId, k: Type) -> Operand {
        match &ast.expr(e).kind {
            ExprKind::Parenthesized(x) => return self.lower_simd(ast, *x, k),
            ExprKind::Identifier(_) => {
                // O local sem caixa sai direto (o `lower_expr` o encaixotaria).
                let v = self.lower_expr_cru(ast, e);
                return self.coagir(v, k);
            }
            ExprKind::Index { target, index, null_aware: false } => {
                if let Some(ix) = self.indexavel(self.ctx.get_type(self.unit_id, *target))
                    && let super::tipados::Indexavel::Simd { .. } = ix
                {
                    let lista = self.lower_expr(ast, *target);
                    let i = self.lower_expr(ast, *index);
                    // Coagido uma vez: o caminho lento reusa o mesmo valor.
                    let i = if self.operand_type(&i) == Type::Ref { self.coagir(i, Type::I64) } else { i };
                    if let Some(v) = self.ler_indexado(lista, i, ix, k) {
                        return v;
                    }
                }
            }
            _ => {}
        }
        if let Some(r) = self.receita_simd(ast, e).filter(|r| r.resultado == k) {
            return self.emitir_receita(ast, r);
        }
        let v = self.lower_expr(ast, e);
        self.coagir(v, k)
    }
}

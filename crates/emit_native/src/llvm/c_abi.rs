//! A passagem de structs e unions por valor na ABI C do alvo — o que o
//! Clang faz na lowering de chamadas (`clang/lib/CodeGen/Targets/*.cpp`) e
//! o que o `dart:ffi` da VM reproduz (`runtime/vm/compiler/ffi/`):
//!
//! * **System V x86-64** (Linux, macOS x64): até 16 bytes, o composto é
//!   dividido em palavras de 8 bytes; uma palavra com algum inteiro é
//!   INTEGER (`iN`), só com ponto flutuante é SSE (`double`, `float` ou
//!   `<2 x float>`). Maior que 16 bytes, com campo desalinhado (`@Packed`)
//!   ou sem registradores livres para todas as palavras: MEMORY (`byval` no
//!   argumento, `sret` no retorno).
//! * **AArch64** (Linux, macOS, Windows): HFA (1 a 4 membros do mesmo tipo
//!   de ponto flutuante) vai em registradores de vetor como `[N x T]`; os
//!   outros até 16 bytes como `i64` ou `[2 x i64]`; maiores, por ponteiro
//!   para uma cópia do chamador e `sret` no retorno.
//! * **Windows x64**: tamanho 1, 2, 4 ou 8 bytes vai como inteiro desse
//!   tamanho; qualquer outro, por ponteiro para uma cópia e `sret`.
//!
//! As peças de uma passagem direta são lidas de uma cópia do composto
//! alinhada e zerada (nunca além da memória do chamador), e o retorno direto
//! é gravado numa temporária antes de ir para o destino.

use crate::hir::{LayoutC, CType};

/// A convenção C do alvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    SysV,
    Win64,
    AArch64 { apple: bool },
}

impl Convention {
    /// A convenção do alvo desta compilação.
    pub fn of_target() -> Convention {
        let arm = cfg!(target_arch = "aarch64");
        match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows if !arm => Convention::Win64,
            crate::alvo::Sistema::MacOs if arm => Convention::AArch64 { apple: true },
            _ if arm => Convention::AArch64 { apple: false },
            _ => Convention::SysV,
        }
    }
}

/// Uma peça de uma passagem direta: o deslocamento no composto e o tipo LLVM
/// lido dali.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub offset: usize,
    pub ty: String,
    /// Atributos do parâmetro (`alignstack(8) ` numa HFA do AArch64 Linux).
    pub attributes: &'static str,
}

/// Como um argumento composto é passado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgPassing {
    /// Em registradores: cada peça é um argumento LLVM.
    Direct(Vec<Piece>),
    /// `ptr byval([N x i8]) align A` para uma cópia (System V MEMORY).
    Byval { alignment: usize },
    /// `ptr` para uma cópia do chamador.
    Indirect,
}

/// Como um retorno composto volta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetPassing {
    /// O tipo de retorno LLVM é a peça, ou a struct literal das peças.
    Direct(Vec<Piece>),
    /// `ptr sret([N x i8]) align A` como primeiro argumento.
    Sret { alignment: usize },
}

/// Registradores de argumento ainda livres (System V: 6 inteiros, 8 SSE).
#[derive(Debug, Clone, Copy)]
pub struct Registers {
    pub integers: usize,
    pub sse: usize,
}

impl Registers {
    pub fn new_set() -> Registers {
        Registers { integers: 6, sse: 8 }
    }

    /// Consome o registrador de um argumento primitivo.
    pub fn consume_primitive(&mut self, t: CType) {
        match t {
            CType::F32 | CType::F64 => self.sse = self.sse.saturating_sub(1),
            CType::Void => {}
            _ => self.integers = self.integers.saturating_sub(1),
        }
    }
}

fn integer_of(bytes: usize) -> String {
    format!("i{}", bytes * 8)
}

/// A HFA de um composto no AArch64: o tipo do membro e quantos.
fn hfa(l: &LayoutC) -> Option<(CType, usize)> {
    let first_one = l.leaves.first()?.1;
    if !matches!(first_one, CType::F32 | CType::F64) || l.leaves.iter().any(|(_, t)| *t != first_one) {
        return None;
    }
    let n = l.size / first_one.c_size();
    // Uma union de floats sobrepostos também é HFA pelo número de membros
    // que cabem no tamanho; exige que tudo seja membro, sem enchimento.
    ((1..=4).contains(&n) && n * first_one.c_size() == l.size).then_some((first_one, n))
}

fn float_name(t: CType) -> &'static str {
    if t == CType::F32 { "float" } else { "double" }
}

/// As palavras de 8 bytes de um composto de até 16 bytes no System V
/// (`None` = MEMORY).
fn sysv_words(l: &LayoutC) -> Option<Vec<Piece>> {
    if l.size > 16 || l.misaligned() {
        return None;
    }
    let n = l.size.div_ceil(8);
    let mut pieces = Vec::with_capacity(n);
    for p in 0..n {
        let ini = p * 8;
        let bytes = (l.size - ini).min(8);
        let leaves: Vec<&(usize, CType)> = l.leaves.iter().filter(|(o, _)| *o >= ini && *o < ini + 8).collect();
        let sse = !leaves.is_empty() && leaves.iter().all(|(_, t)| matches!(t, CType::F32 | CType::F64));
        // Uma palavra cheia com uma única folha no início e o resto
        // enchimento: o Clang lê só a folha (`{int32, double}` → `i32,
        // double`; `{int8, int8, double}` → `i64, double`).
        let ty = if !sse {
            match leaves.as_slice() {
                [(o, t)] if bytes == 8 && *o == ini && t.c_size() < 8 => integer_of(t.c_size()),
                _ => integer_of(bytes),
            }
        } else if leaves.iter().any(|(_, t)| *t == CType::F64) {
            "double".to_string()
        } else if leaves.iter().any(|(o, _)| *o >= ini + 4) {
            "<2 x float>".to_string()
        } else {
            "float".to_string()
        };
        pieces.push(Piece { offset: ini, ty, attributes: "" });
    }
    Some(pieces)
}

/// Como passar um argumento composto (consome os registradores dele).
pub fn argument(conv: Convention, l: &LayoutC, regs: &mut Registers) -> ArgPassing {
    match conv {
        Convention::SysV => {
            let Some(pieces) = sysv_words(l) else {
                return ArgPassing::Byval { alignment: l.alignment.max(8) };
            };
            let sse = pieces.iter().filter(|p| !p.ty.starts_with('i')).count();
            let integers = pieces.len() - sse;
            // Todas as palavras em registradores, ou o composto inteiro vai
            // para a pilha (nunca dividido).
            if integers > regs.integers || sse > regs.sse {
                return ArgPassing::Byval { alignment: l.alignment.max(8) };
            }
            regs.integers -= integers;
            regs.sse -= sse;
            ArgPassing::Direct(pieces)
        }
        Convention::Win64 => match l.size {
            1 | 2 | 4 | 8 => ArgPassing::Direct(vec![Piece { offset: 0, ty: integer_of(l.size), attributes: "" }]),
            _ => ArgPassing::Indirect,
        },
        Convention::AArch64 { apple } => {
            if let Some((t, n)) = hfa(l) {
                let attributes = if apple { "" } else { "alignstack(8) " };
                return ArgPassing::Direct(vec![Piece { offset: 0, ty: format!("[{n} x {}]", float_name(t)), attributes }]);
            }
            match l.size {
                0..=8 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "i64".to_string(), attributes: "" }]),
                9..=16 if l.alignment >= 16 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "i128".to_string(), attributes: "" }]),
                9..=16 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "[2 x i64]".to_string(), attributes: "" }]),
                _ => ArgPassing::Indirect,
            }
        }
    }
}

/// Como vai um composto na parte variádica de uma chamada: como um fixo,
/// exceto no arm64 da Apple, onde tudo o que é variádico vai na pilha e a
/// HFA não tem tratamento próprio (o clang a passa como os inteiros do
/// tamanho dela; `AArch64ABIInfo::classifyArgumentType` com `IsVariadic`).
pub fn variadic_argument(conv: Convention, l: &LayoutC, regs: &mut Registers) -> ArgPassing {
    match conv {
        Convention::AArch64 { apple: true } => match l.size {
            0..=8 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "i64".to_string(), attributes: "" }]),
            9..=16 if l.alignment >= 16 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "i128".to_string(), attributes: "" }]),
            9..=16 => ArgPassing::Direct(vec![Piece { offset: 0, ty: "[2 x i64]".to_string(), attributes: "" }]),
            _ => ArgPassing::Indirect,
        },
        _ => argument(conv, l, regs),
    }
}

/// Como volta um retorno composto (um `sret` consome um registrador
/// inteiro no System V).
pub fn ret(conv: Convention, l: &LayoutC, regs: &mut Registers) -> RetPassing {
    let sret = |regs: &mut Registers| {
        regs.integers = regs.integers.saturating_sub(1);
        RetPassing::Sret { alignment: l.alignment }
    };
    match conv {
        Convention::SysV => match sysv_words(l) {
            Some(pieces) => RetPassing::Direct(pieces),
            None => sret(regs),
        },
        Convention::Win64 => match l.size {
            1 | 2 | 4 | 8 => RetPassing::Direct(vec![Piece { offset: 0, ty: integer_of(l.size), attributes: "" }]),
            _ => sret(regs),
        },
        Convention::AArch64 { .. } => {
            if let Some((t, n)) = hfa(l) {
                return RetPassing::Direct(vec![Piece { offset: 0, ty: format!("[{n} x {}]", float_name(t)), attributes: "" }]);
            }
            match l.size {
                0..=8 => RetPassing::Direct(vec![Piece { offset: 0, ty: integer_of(l.size), attributes: "" }]),
                9..=16 if l.alignment >= 16 => RetPassing::Direct(vec![Piece { offset: 0, ty: "i128".to_string(), attributes: "" }]),
                9..=16 => RetPassing::Direct(vec![Piece { offset: 0, ty: "[2 x i64]".to_string(), attributes: "" }]),
                _ => sret(regs),
            }
        }
    }
}

/// O tipo LLVM do retorno direto (a peça, ou a struct literal das peças).
pub fn return_ty(pieces: &[Piece]) -> String {
    match pieces {
        [p] => p.ty.clone(),
        _ => format!("{{ {} }}", pieces.iter().map(|p| p.ty.as_str()).collect::<Vec<_>>().join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(size: usize, alignment: usize, leaves: &[(usize, CType)]) -> LayoutC {
        LayoutC { size, alignment, leaves: leaves.to_vec() }
    }

    fn types(p: &ArgPassing) -> Vec<String> {
        match p {
            ArgPassing::Direct(v) => v.iter().map(|x| x.ty.clone()).collect(),
            ArgPassing::Byval { .. } => vec!["byval".to_string()],
            ArgPassing::Indirect => vec!["ptr".to_string()],
        }
    }

    fn ret_text(p: &RetPassing) -> String {
        match p {
            RetPassing::Direct(v) => return_ty(v),
            RetPassing::Sret { .. } => "sret".to_string(),
        }
    }

    // Os casos de `clang -S -emit-llvm` para cada alvo (a referência).
    fn cases() -> Vec<(&'static str, LayoutC)> {
        use CType::*;
        vec![
            ("I1", l(1, 1, &[(0, I8)])),
            ("I3", l(3, 1, &[(0, I8), (1, I8), (2, I8)])),
            ("I12", l(12, 4, &[(0, I32), (4, I32), (8, I32)])),
            ("F2", l(8, 4, &[(0, F32), (4, F32)])),
            ("F3", l(12, 4, &[(0, F32), (4, F32), (8, F32)])),
            ("D2", l(16, 8, &[(0, F64), (8, F64)])),
            ("D4", l(32, 8, &[(0, F64), (8, F64), (16, F64), (24, F64)])),
            ("M", l(16, 8, &[(0, I32), (8, F64)])),
            ("P16", l(16, 8, &[(0, I64), (8, I64)])),
            ("G", l(24, 8, &[(0, I64), (8, I64), (16, I64)])),
            ("MF", l(12, 4, &[(0, F32), (4, I32), (8, F32)])),
            ("PK", l(5, 1, &[(0, I8), (1, I32)])),
        ]
    }

    #[test]
    fn sysv_like_clang() {
        let expected = [
            ("I1", "i8", "i8"),
            ("I3", "i24", "i24"),
            ("I12", "i64,i32", "{ i64, i32 }"),
            ("F2", "<2 x float>", "<2 x float>"),
            ("F3", "<2 x float>,float", "{ <2 x float>, float }"),
            ("D2", "double,double", "{ double, double }"),
            ("D4", "byval", "sret"),
            ("M", "i32,double", "{ i32, double }"),
            ("P16", "i64,i64", "{ i64, i64 }"),
            ("G", "byval", "sret"),
            ("MF", "i64,float", "{ i64, float }"),
            ("PK", "byval", "sret"),
        ];
        for ((name, layout), (n2, a, r)) in cases().iter().zip(expected) {
            assert_eq!(*name, n2);
            let mut regs = Registers::new_set();
            assert_eq!(types(&argument(Convention::SysV, layout, &mut regs)).join(","), a, "{name}");
            assert_eq!(ret_text(&ret(Convention::SysV, layout, &mut regs)), r, "{name}");
        }
    }

    #[test]
    fn aarch64_like_clang() {
        let expected = [
            ("I1", "i64", "i8"),
            ("I3", "i64", "i24"),
            ("I12", "[2 x i64]", "[2 x i64]"),
            ("F2", "[2 x float]", "[2 x float]"),
            ("F3", "[3 x float]", "[3 x float]"),
            ("D2", "[2 x double]", "[2 x double]"),
            ("D4", "[4 x double]", "[4 x double]"),
            ("M", "[2 x i64]", "[2 x i64]"),
            ("P16", "[2 x i64]", "[2 x i64]"),
            ("G", "ptr", "sret"),
            ("MF", "[2 x i64]", "[2 x i64]"),
            ("PK", "i64", "i40"),
        ];
        for apple in [false, true] {
            for ((name, layout), (n2, a, r)) in cases().iter().zip(expected) {
                assert_eq!(*name, n2);
                let mut regs = Registers::new_set();
                let conv = Convention::AArch64 { apple };
                assert_eq!(types(&argument(conv, layout, &mut regs)).join(","), a, "{name}");
                assert_eq!(ret_text(&ret(conv, layout, &mut regs)), r, "{name}");
            }
        }
    }

    #[test]
    fn win64_like_clang() {
        let expected = [
            ("I1", "i8", "i8"),
            ("I3", "ptr", "sret"),
            ("I12", "ptr", "sret"),
            ("F2", "i64", "i64"),
            ("F3", "ptr", "sret"),
            ("D2", "ptr", "sret"),
            ("D4", "ptr", "sret"),
            ("M", "ptr", "sret"),
            ("P16", "ptr", "sret"),
            ("G", "ptr", "sret"),
            ("MF", "ptr", "sret"),
            ("PK", "ptr", "sret"),
        ];
        for ((name, layout), (n2, a, r)) in cases().iter().zip(expected) {
            assert_eq!(*name, n2);
            let mut regs = Registers::new_set();
            assert_eq!(types(&argument(Convention::Win64, layout, &mut regs)).join(","), a, "{name}");
            assert_eq!(ret_text(&ret(Convention::Win64, layout, &mut regs)), r, "{name}");
        }
    }

    #[test]
    fn sysv_padded_word() {
        use CType::*;
        let cases = [
            (l(16, 8, &[(0, I8), (8, F64)]), "i8,double"),
            (l(16, 8, &[(0, I16), (8, F64)]), "i16,double"),
            (l(16, 8, &[(0, I8), (1, I8), (8, F64)]), "i64,double"),
            (l(16, 8, &[(0, I32), (4, I8), (8, F64)]), "i64,double"),
        ];
        for (layout, expected) in cases {
            let mut regs = Registers::new_set();
            assert_eq!(types(&argument(Convention::SysV, &layout, &mut regs)).join(","), expected);
        }
    }

    #[test]
    fn sysv_without_registers_goes_to_stack() {
        let d2 = l(16, 8, &[(0, CType::F64), (8, CType::F64)]);
        let mut regs = Registers { integers: 6, sse: 1 };
        assert_eq!(argument(Convention::SysV, &d2, &mut regs), ArgPassing::Byval { alignment: 8 });
        assert_eq!(regs.sse, 1, "o composto inteiro vai para a pilha, sem consumir registradores");
    }
}

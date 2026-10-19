//! Passing structs and unions by value in the target's C ABI — what
//! Clang does when lowering calls (`clang/lib/CodeGen/Targets/*.cpp`) and
//! what the VM's `dart:ffi` reproduces (`runtime/vm/compiler/ffi/`):
//!
//! * **System V x86-64** (Linux, macOS x64): up to 16 bytes, the composite is
//!   split into 8-byte words; a word with some integer is
//!   INTEGER (`iN`), one with only floating point is SSE (`double`, `float` or
//!   `<2 x float>`). Larger than 16 bytes, with a misaligned field (`@Packed`)
//!   or without free registers for all the words: MEMORY (`byval` for the
//!   argument, `sret` for the return).
//! * **AArch64** (Linux, macOS, Windows): an HFA (1 to 4 members of the same
//!   floating-point type) goes in vector registers as `[N x T]`; the
//!   others up to 16 bytes as `i64` or `[2 x i64]`; larger ones, through a pointer
//!   to a caller copy and `sret` for the return.
//! * **Windows x64**: a size of 1, 2, 4 or 8 bytes goes as an integer of that
//!   size; any other, through a pointer to a copy and `sret`.
//!
//! The pieces of a direct passing are read from an aligned, zeroed copy of the
//! composite (never beyond the caller's memory), and the direct return
//! is written into a temporary before going to the destination.

use crate::hir::{LayoutC, CType};

/// The C convention of the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    SysV,
    Win64,
    AArch64 { apple: bool },
}

impl Convention {
    /// The target convention of this compilation.
    pub fn of_target() -> Convention {
        let arm = crate::target::is_aarch64();
        match crate::target::target_os() {
            crate::target::Os::Windows if !arm => Convention::Win64,
            crate::target::Os::MacOs if arm => Convention::AArch64 { apple: true },
            _ if arm => Convention::AArch64 { apple: false },
            _ => Convention::SysV,
        }
    }
}

/// A piece of a direct passing: the offset into the composite and the LLVM type
/// read from there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub offset: usize,
    pub ty: String,
    /// Parameter attributes (`alignstack(8) ` for an HFA on AArch64 Linux).
    pub attributes: &'static str,
}

/// How a composite argument is passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgPassing {
    /// In registers: each piece is an LLVM argument.
    Direct(Vec<Piece>),
    /// `ptr byval([N x i8]) align A` to a copy (System V MEMORY).
    Byval { alignment: usize },
    /// `ptr` to a caller copy.
    Indirect,
}

/// How a composite return comes back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetPassing {
    /// The LLVM return type is the piece, or the literal struct of the pieces.
    Direct(Vec<Piece>),
    /// `ptr sret([N x i8]) align A` as the first argument.
    Sret { alignment: usize },
}

/// Argument registers still free (System V: 6 integer, 8 SSE).
#[derive(Debug, Clone, Copy)]
pub struct Registers {
    pub integers: usize,
    pub sse: usize,
}

impl Registers {
    pub fn new_set() -> Registers {
        Registers { integers: 6, sse: 8 }
    }

    /// Consumes the register of a primitive argument.
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

/// The HFA of a composite on AArch64: the member type and how many.
fn hfa(l: &LayoutC) -> Option<(CType, usize)> {
    let first_one = l.leaves.first()?.1;
    if !matches!(first_one, CType::F32 | CType::F64) || l.leaves.iter().any(|(_, t)| *t != first_one) {
        return None;
    }
    let n = l.size / first_one.c_size();
    // A union of overlapping floats is also an HFA by the number of members
    // that fit in its size; requires everything to be a member, without padding.
    ((1..=4).contains(&n) && n * first_one.c_size() == l.size).then_some((first_one, n))
}

fn float_name(t: CType) -> &'static str {
    if t == CType::F32 { "float" } else { "double" }
}

/// The 8-byte words of a composite of up to 16 bytes on System V
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
        // A full word with a single leaf at the start and the rest
        // padding: Clang reads only the leaf (`{int32, double}` → `i32,
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

/// How to pass a composite argument (consumes its registers).
pub fn argument(conv: Convention, l: &LayoutC, regs: &mut Registers) -> ArgPassing {
    match conv {
        Convention::SysV => {
            let Some(pieces) = sysv_words(l) else {
                return ArgPassing::Byval { alignment: l.alignment.max(8) };
            };
            let sse = pieces.iter().filter(|p| !p.ty.starts_with('i')).count();
            let integers = pieces.len() - sse;
            // All the words in registers, or the whole composite goes
            // to the stack (never split).
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

/// How a composite goes in the variadic part of a call: like a fixed one,
/// except on Apple's arm64, where everything variadic goes on the stack and an
/// HFA has no special treatment (clang passes it as the integers of its
/// size; `AArch64ABIInfo::classifyArgumentType` with `IsVariadic`).
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

/// How a composite return comes back (an `sret` consumes an integer
/// register on System V).
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

/// The LLVM type of the direct return (the piece, or the literal struct of the pieces).
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

    // The `clang -S -emit-llvm` cases for each target (the reference).
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
        assert_eq!(regs.sse, 1, "the whole compound goes to the stack, without consuming registers");
    }
}

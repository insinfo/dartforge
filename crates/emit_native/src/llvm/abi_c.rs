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

use crate::hir::{LayoutC, TipoC};

/// A convenção C do alvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convencao {
    SysV,
    Win64,
    AArch64 { apple: bool },
}

impl Convencao {
    /// A convenção do alvo desta compilação.
    pub fn do_alvo() -> Convencao {
        let arm = cfg!(target_arch = "aarch64");
        match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows if !arm => Convencao::Win64,
            crate::alvo::Sistema::MacOs if arm => Convencao::AArch64 { apple: true },
            _ if arm => Convencao::AArch64 { apple: false },
            _ => Convencao::SysV,
        }
    }
}

/// Uma peça de uma passagem direta: o deslocamento no composto e o tipo LLVM
/// lido dali.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peca {
    pub deslocamento: usize,
    pub tipo: String,
    /// Atributos do parâmetro (`alignstack(8) ` numa HFA do AArch64 Linux).
    pub atributos: &'static str,
}

/// Como um argumento composto é passado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassagemArg {
    /// Em registradores: cada peça é um argumento LLVM.
    Direta(Vec<Peca>),
    /// `ptr byval([N x i8]) align A` para uma cópia (System V MEMORY).
    Byval { alinhamento: usize },
    /// `ptr` para uma cópia do chamador.
    Indireta,
}

/// Como um retorno composto volta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassagemRet {
    /// O tipo de retorno LLVM é a peça, ou a struct literal das peças.
    Direta(Vec<Peca>),
    /// `ptr sret([N x i8]) align A` como primeiro argumento.
    Sret { alinhamento: usize },
}

/// Registradores de argumento ainda livres (System V: 6 inteiros, 8 SSE).
#[derive(Debug, Clone, Copy)]
pub struct Registradores {
    pub inteiros: usize,
    pub sse: usize,
}

impl Registradores {
    pub fn novos() -> Registradores {
        Registradores { inteiros: 6, sse: 8 }
    }

    /// Consome o registrador de um argumento primitivo.
    pub fn consumir_primitivo(&mut self, t: TipoC) {
        match t {
            TipoC::F32 | TipoC::F64 => self.sse = self.sse.saturating_sub(1),
            TipoC::Void => {}
            _ => self.inteiros = self.inteiros.saturating_sub(1),
        }
    }
}

fn inteiro_de(bytes: usize) -> String {
    format!("i{}", bytes * 8)
}

/// A HFA de um composto no AArch64: o tipo do membro e quantos.
fn hfa(l: &LayoutC) -> Option<(TipoC, usize)> {
    let primeiro = l.folhas.first()?.1;
    if !matches!(primeiro, TipoC::F32 | TipoC::F64) || l.folhas.iter().any(|(_, t)| *t != primeiro) {
        return None;
    }
    let n = l.tamanho / primeiro.tamanho_c();
    // Uma union de floats sobrepostos também é HFA pelo número de membros
    // que cabem no tamanho; exige que tudo seja membro, sem enchimento.
    ((1..=4).contains(&n) && n * primeiro.tamanho_c() == l.tamanho).then_some((primeiro, n))
}

fn nome_float(t: TipoC) -> &'static str {
    if t == TipoC::F32 { "float" } else { "double" }
}

/// As palavras de 8 bytes de um composto de até 16 bytes no System V
/// (`None` = MEMORY).
fn palavras_sysv(l: &LayoutC) -> Option<Vec<Peca>> {
    if l.tamanho > 16 || l.desalinhado() {
        return None;
    }
    let n = l.tamanho.div_ceil(8);
    let mut pecas = Vec::with_capacity(n);
    for p in 0..n {
        let ini = p * 8;
        let bytes = (l.tamanho - ini).min(8);
        let folhas: Vec<&(usize, TipoC)> = l.folhas.iter().filter(|(o, _)| *o >= ini && *o < ini + 8).collect();
        let sse = !folhas.is_empty() && folhas.iter().all(|(_, t)| matches!(t, TipoC::F32 | TipoC::F64));
        // Uma palavra cheia com uma única folha no início e o resto
        // enchimento: o Clang lê só a folha (`{int32, double}` → `i32,
        // double`; `{int8, int8, double}` → `i64, double`).
        let tipo = if !sse {
            match folhas.as_slice() {
                [(o, t)] if bytes == 8 && *o == ini && t.tamanho_c() < 8 => inteiro_de(t.tamanho_c()),
                _ => inteiro_de(bytes),
            }
        } else if folhas.iter().any(|(_, t)| *t == TipoC::F64) {
            "double".to_string()
        } else if folhas.iter().any(|(o, _)| *o >= ini + 4) {
            "<2 x float>".to_string()
        } else {
            "float".to_string()
        };
        pecas.push(Peca { deslocamento: ini, tipo, atributos: "" });
    }
    Some(pecas)
}

/// Como passar um argumento composto (consome os registradores dele).
pub fn argumento(conv: Convencao, l: &LayoutC, regs: &mut Registradores) -> PassagemArg {
    match conv {
        Convencao::SysV => {
            let Some(pecas) = palavras_sysv(l) else {
                return PassagemArg::Byval { alinhamento: l.alinhamento.max(8) };
            };
            let sse = pecas.iter().filter(|p| !p.tipo.starts_with('i')).count();
            let inteiros = pecas.len() - sse;
            // Todas as palavras em registradores, ou o composto inteiro vai
            // para a pilha (nunca dividido).
            if inteiros > regs.inteiros || sse > regs.sse {
                return PassagemArg::Byval { alinhamento: l.alinhamento.max(8) };
            }
            regs.inteiros -= inteiros;
            regs.sse -= sse;
            PassagemArg::Direta(pecas)
        }
        Convencao::Win64 => match l.tamanho {
            1 | 2 | 4 | 8 => PassagemArg::Direta(vec![Peca { deslocamento: 0, tipo: inteiro_de(l.tamanho), atributos: "" }]),
            _ => PassagemArg::Indireta,
        },
        Convencao::AArch64 { apple } => {
            if let Some((t, n)) = hfa(l) {
                let atributos = if apple { "" } else { "alignstack(8) " };
                return PassagemArg::Direta(vec![Peca { deslocamento: 0, tipo: format!("[{n} x {}]", nome_float(t)), atributos }]);
            }
            match l.tamanho {
                0..=8 => PassagemArg::Direta(vec![Peca { deslocamento: 0, tipo: "i64".to_string(), atributos: "" }]),
                9..=16 if l.alinhamento >= 16 => PassagemArg::Direta(vec![Peca { deslocamento: 0, tipo: "i128".to_string(), atributos: "" }]),
                9..=16 => PassagemArg::Direta(vec![Peca { deslocamento: 0, tipo: "[2 x i64]".to_string(), atributos: "" }]),
                _ => PassagemArg::Indireta,
            }
        }
    }
}

/// Como volta um retorno composto (um `sret` consome um registrador
/// inteiro no System V).
pub fn retorno(conv: Convencao, l: &LayoutC, regs: &mut Registradores) -> PassagemRet {
    let sret = |regs: &mut Registradores| {
        regs.inteiros = regs.inteiros.saturating_sub(1);
        PassagemRet::Sret { alinhamento: l.alinhamento }
    };
    match conv {
        Convencao::SysV => match palavras_sysv(l) {
            Some(pecas) => PassagemRet::Direta(pecas),
            None => sret(regs),
        },
        Convencao::Win64 => match l.tamanho {
            1 | 2 | 4 | 8 => PassagemRet::Direta(vec![Peca { deslocamento: 0, tipo: inteiro_de(l.tamanho), atributos: "" }]),
            _ => sret(regs),
        },
        Convencao::AArch64 { .. } => {
            if let Some((t, n)) = hfa(l) {
                return PassagemRet::Direta(vec![Peca { deslocamento: 0, tipo: format!("[{n} x {}]", nome_float(t)), atributos: "" }]);
            }
            match l.tamanho {
                0..=8 => PassagemRet::Direta(vec![Peca { deslocamento: 0, tipo: inteiro_de(l.tamanho), atributos: "" }]),
                9..=16 if l.alinhamento >= 16 => PassagemRet::Direta(vec![Peca { deslocamento: 0, tipo: "i128".to_string(), atributos: "" }]),
                9..=16 => PassagemRet::Direta(vec![Peca { deslocamento: 0, tipo: "[2 x i64]".to_string(), atributos: "" }]),
                _ => sret(regs),
            }
        }
    }
}

/// O tipo LLVM do retorno direto (a peça, ou a struct literal das peças).
pub fn tipo_do_retorno(pecas: &[Peca]) -> String {
    match pecas {
        [p] => p.tipo.clone(),
        _ => format!("{{ {} }}", pecas.iter().map(|p| p.tipo.as_str()).collect::<Vec<_>>().join(", ")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn l(tamanho: usize, alinhamento: usize, folhas: &[(usize, TipoC)]) -> LayoutC {
        LayoutC { tamanho, alinhamento, folhas: folhas.to_vec() }
    }

    fn tipos(p: &PassagemArg) -> Vec<String> {
        match p {
            PassagemArg::Direta(v) => v.iter().map(|x| x.tipo.clone()).collect(),
            PassagemArg::Byval { .. } => vec!["byval".to_string()],
            PassagemArg::Indireta => vec!["ptr".to_string()],
        }
    }

    fn ret(p: &PassagemRet) -> String {
        match p {
            PassagemRet::Direta(v) => tipo_do_retorno(v),
            PassagemRet::Sret { .. } => "sret".to_string(),
        }
    }

    // Os casos de `clang -S -emit-llvm` para cada alvo (a referência).
    fn casos() -> Vec<(&'static str, LayoutC)> {
        use TipoC::*;
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
    fn sysv_como_o_clang() {
        let esperado = [
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
        for ((nome, layout), (n2, a, r)) in casos().iter().zip(esperado) {
            assert_eq!(*nome, n2);
            let mut regs = Registradores::novos();
            assert_eq!(tipos(&argumento(Convencao::SysV, layout, &mut regs)).join(","), a, "{nome}");
            assert_eq!(ret(&retorno(Convencao::SysV, layout, &mut regs)), r, "{nome}");
        }
    }

    #[test]
    fn aarch64_como_o_clang() {
        let esperado = [
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
            for ((nome, layout), (n2, a, r)) in casos().iter().zip(esperado) {
                assert_eq!(*nome, n2);
                let mut regs = Registradores::novos();
                let conv = Convencao::AArch64 { apple };
                assert_eq!(tipos(&argumento(conv, layout, &mut regs)).join(","), a, "{nome}");
                assert_eq!(ret(&retorno(conv, layout, &mut regs)), r, "{nome}");
            }
        }
    }

    #[test]
    fn win64_como_o_clang() {
        let esperado = [
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
        for ((nome, layout), (n2, a, r)) in casos().iter().zip(esperado) {
            assert_eq!(*nome, n2);
            let mut regs = Registradores::novos();
            assert_eq!(tipos(&argumento(Convencao::Win64, layout, &mut regs)).join(","), a, "{nome}");
            assert_eq!(ret(&retorno(Convencao::Win64, layout, &mut regs)), r, "{nome}");
        }
    }

    #[test]
    fn sysv_palavra_com_enchimento() {
        use TipoC::*;
        let casos = [
            (l(16, 8, &[(0, I8), (8, F64)]), "i8,double"),
            (l(16, 8, &[(0, I16), (8, F64)]), "i16,double"),
            (l(16, 8, &[(0, I8), (1, I8), (8, F64)]), "i64,double"),
            (l(16, 8, &[(0, I32), (4, I8), (8, F64)]), "i64,double"),
        ];
        for (layout, esperado) in casos {
            let mut regs = Registradores::novos();
            assert_eq!(tipos(&argumento(Convencao::SysV, &layout, &mut regs)).join(","), esperado);
        }
    }

    #[test]
    fn sysv_sem_registradores_vai_para_a_pilha() {
        let d2 = l(16, 8, &[(0, TipoC::F64), (8, TipoC::F64)]);
        let mut regs = Registradores { inteiros: 6, sse: 1 };
        assert_eq!(argumento(Convencao::SysV, &d2, &mut regs), PassagemArg::Byval { alinhamento: 8 });
        assert_eq!(regs.sse, 1, "o composto inteiro vai para a pilha, sem consumir registradores");
    }
}

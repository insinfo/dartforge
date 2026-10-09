//! Medição isolada dos metadados do módulo ARC real, sem heap nem coleta.
//!
//! Oito páginas com 1.024 blocos cada; dez milhões de pares retain/release
//! e consultas por rodada, em ordem permutada. O RC permanece em um: a fila
//! de zeros não cresce e cada objeto vira candidato só uma vez.
//! Compilar com `rustc --edition=2024 -O bench/arc/metadados.rs -o <exe>`.
//! Fixar a afinidade do processo medidor e alternar versões antes/depois;
//! desprezar a primeira rodada. Não representa o custo total do ARC no Dart.
#![allow(dead_code)]

#[path = "../../crates/runtime/src/arc.rs"]
mod arc;
#[path = "../../crates/runtime/src/hash.rs"]
mod hash;
#[path = "../../crates/runtime/src/layout.rs"]
mod layout;

use arc::{EstadoDoArc, Geometria};
use std::{hint::black_box, time::Instant};

#[inline(never)]
fn medir(arc: &mut EstadoDoArc, handles: &[i64]) -> u64 {
    let mut soma = 0;
    for i in 0..10_000_000usize {
        let h = black_box(handles[i.wrapping_mul(4099) & (handles.len() - 1)]);
        arc.reter(h).unwrap();
        arc.soltar(h).unwrap();
        soma += black_box(arc.meta(h).unwrap().rc);
    }
    soma
}

fn main() {
    let mut arc = EstadoDoArc::novo();
    let mut handles = Vec::new();
    for p in 0..8 {
        let g = Geometria {
            primeiro: (1u64 << 32) + p * 65536 + 1024,
            tamanho: 48,
            blocos: 1024,
        };
        for i in 0..g.blocos {
            let h = (g.primeiro + u64::from(i) * g.tamanho) as i64 + layout::DESLOCAMENTO_DO_HANDLE;
            arc.registrar_vivo_em(h, Some(g));
            arc.reter(h).unwrap();
            handles.push(h);
        }
    }
    for _ in 0..6 {
        let t = Instant::now();
        let soma = medir(&mut arc, black_box(&handles));
        assert_eq!(soma, 10_000_000);
        println!("{} {soma}", t.elapsed().as_micros());
    }
}

//! Caixas, células, contextos, closures e records do espaço unificado (P2,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5, §2.10 e §3.3).
//!
//! * `_Mint` (cid 3) e `_Double` (cid 4): blocos `BRUTO` de uma palavra, o valor
//!   em `b+16`. A forma é canônica: um `int` que cabe no `Smi` nunca vira `_Mint`.
//! * `bool` (cid 5): só as duas estáticas do runtime (`dartforge_falso`,
//!   `dartforge_verdadeiro`, estado `PERMANENTE`); o handle é o endereço + 2.
//! * `_Celula` (14): `INSTANCIA` de um campo, o bit do mapa pela gravação.
//! * `_Contexto` (13): `INSTANCIA` com as capturas, cada bit pela gravação.
//! * `_Closure` (11): `INSTANCIA` de quatro campos — código (bruto), contexto
//!   (`Ref`: o `_Contexto`, ou o valor capturado no "ambiente direto"), corpo
//!   tipado (bruto) e ABI (bruta); mapa `0b10`.
//! * `_Record` posicional (12): `REFS`, a palavra 0 é o número de campos (bruto)
//!   e as seguintes os campos (`Ref`).
//!
//! As funções que alocam enraízam elas mesmas os `Ref` que recebem (um quadro
//! de raízes do runtime em volta da alocação): quem chama só precisa enraizar o
//! resultado antes da próxima alocação.

use crate::heap::{Campo, Heap, Valor};
use crate::layout::{DESLOCAMENTO_DO_HANDLE, Ref, cid, e_objeto, flags, smi};

/// A vista de uma `_Closure`: o código, o contexto (ou o valor capturado no
/// "ambiente direto"), o corpo tipado e a ABI.
#[derive(Clone, Copy, Debug)]
pub struct ClosureRef {
    pub codigo: i64,
    pub contexto: Campo,
    pub tipado: i64,
    pub abi: i64,
}

/// Campos de uma `_Closure` (a ordem do layout, §2.5).
const CAMPOS_DA_CLOSURE: usize = 4;

impl Heap {
    /// Executa `f` com os `refs` enraizados num quadro do runtime (os que não
    /// são handle são ignorados).
    fn com_refs_enraizados<R>(&mut self, refs: impl IntoIterator<Item = Ref>, f: impl FnOnce(&mut Self) -> R) -> R {
        let quadro = self.push_frame();
        for r in refs {
            self.root(quadro, r);
        }
        let saida = f(self);
        self.pop_frame(quadro);
        saida
    }

    /// A classe de `r` (1 para null, 2 para `Smi`; pânico N4 para handle morto).
    fn cid_de(&self, r: Ref) -> i32 {
        self.classe(r)
    }

    /// O `int` `v` em posição `Ref`: `Smi` se cabe, senão um `_Mint` novo.
    pub fn caixa_int(&mut self, v: i64) -> Ref {
        match smi::de(v) {
            Some(r) => r,
            None => {
                let h = self.alocar(cid::MINT, 1, flags::BRUTO);
                self.palavras_mut(h)[0] = v;
                h
            }
        }
    }
    /// Um `_Double` novo com `v`.
    pub fn caixa_double(&mut self, v: f64) -> Ref {
        let h = self.alocar(cid::DOUBLE, 1, flags::BRUTO);
        self.palavras_mut(h)[0] = v.to_bits() as i64;
        h
    }
    /// Uma das duas caixas estáticas de `bool`.
    pub fn caixa_bool(v: bool) -> Ref {
        let bloco = if v {
            std::ptr::addr_of!(crate::heap::dartforge_verdadeiro)
        } else {
            std::ptr::addr_of!(crate::heap::dartforge_falso)
        };
        bloco as i64 + DESLOCAMENTO_DO_HANDLE
    }
    /// O valor em posição `Ref` (encaixota o que precisa).
    pub fn como_ref(&mut self, v: Valor) -> Ref {
        match v {
            Valor::Ref(r) => r,
            Valor::Int(i) => self.caixa_int(i),
            Valor::Double(d) => self.caixa_double(d),
            Valor::Bool(b) => Heap::caixa_bool(b),
        }
    }
    /// O valor de um `Ref`, desencaixotado (`Smi`, `_Mint`, `_Double`, `bool`); o
    /// resto como `Valor::Ref`.
    pub fn valor(&self, r: Ref) -> Valor {
        if smi::e_smi(r) {
            return Valor::Int(smi::valor(r));
        }
        if !e_objeto(r) {
            return Valor::Ref(r);
        }
        match self.cid_de(r) {
            cid::MINT => Valor::Int(self.palavras(r)[0]),
            cid::DOUBLE => Valor::Double(f64::from_bits(self.palavras(r)[0] as u64)),
            cid::BOOL => Valor::Bool(self.palavras(r)[0] != 0),
            _ => Valor::Ref(r),
        }
    }
    /// O `int` de um `Smi` ou `_Mint`.
    pub fn int_de(&self, r: Ref) -> Option<i64> {
        if smi::e_smi(r) {
            return Some(smi::valor(r));
        }
        (e_objeto(r) && self.cid_de(r) == cid::MINT).then(|| self.palavras(r)[0])
    }
    /// O `double` de um `_Double`.
    pub fn double_de(&self, r: Ref) -> Option<f64> {
        (e_objeto(r) && self.cid_de(r) == cid::DOUBLE).then(|| f64::from_bits(self.palavras(r)[0] as u64))
    }
    /// O `bool` de uma das caixas estáticas.
    pub fn bool_de(&self, r: Ref) -> Option<bool> {
        if r == Heap::caixa_bool(true) {
            Some(true)
        } else if r == Heap::caixa_bool(false) {
            Some(false)
        } else if e_objeto(r) && self.cid_de(r) == cid::BOOL {
            // A estática de outra cópia do runtime (a DLL do SDK e o
            // executável, §4.10 9a): o valor está no corpo.
            Some(self.palavras(r)[0] != 0)
        } else {
            None
        }
    }
    /// `identical(a, b)` (§2.10): o mesmo handle, ou `_Mint`/`_Double` de mesmo valor.
    pub fn identico(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return true;
        }
        if !e_objeto(a) || !e_objeto(b) {
            // `Smi` e null só são idênticos pelo handle (forma canônica).
            return false;
        }
        let ca = self.cid_de(a);
        if ca != self.cid_de(b) {
            // As duas estáticas de `bool` de cópias diferentes do runtime.
            return ca == cid::BOOL && self.bool_de(a) == self.bool_de(b);
        }
        match ca {
            // Os bits: `identical(NaN, NaN)` é verdadeiro e `0.0`/`-0.0` não.
            cid::MINT | cid::DOUBLE => self.palavras(a)[0] == self.palavras(b)[0],
            cid::BOOL => self.bool_de(a) == self.bool_de(b),
            _ => false,
        }
    }
    /// Uma `_Celula` nova com `valor`.
    pub fn nova_celula(&mut self, valor: Campo) -> Ref {
        let raiz = valor.1.then_some(valor.0);
        self.com_refs_enraizados(raiz, |heap| {
            let h = heap.alocar_instancia(cid::CELULA, 1);
            heap.definir_campo(h, 0, valor.0, valor.1);
            h
        })
    }
    /// O valor da `_Celula` `h`.
    pub fn celula(&self, h: Ref) -> Campo {
        let o = self.objeto(h).expect("célula esperada");
        debug_assert_eq!(o.class_id, i64::from(cid::CELULA), "célula esperada");
        o.campo(0)
    }
    /// Grava na `_Celula` `h` (com barreira).
    pub fn gravar_celula(&mut self, h: Ref, valor: Campo) {
        self.definir_campo(h, 0, valor.0, valor.1);
    }
    /// Um `_Contexto` novo com as capturas.
    pub fn novo_contexto(&mut self, capturas: &[Campo]) -> Ref {
        let raizes = capturas.iter().filter(|c| c.1).map(|c| c.0).collect::<Vec<_>>();
        self.com_refs_enraizados(raizes, |heap| {
            let h = heap.alocar_instancia(cid::CONTEXTO, capturas.len());
            for (i, &(bits, e_ref)) in capturas.iter().enumerate() {
                heap.definir_campo(h, i, bits, e_ref);
            }
            h
        })
    }
    /// A captura `i` do `_Contexto` `h`.
    pub fn captura(&self, h: Ref, i: usize) -> Campo {
        let o = self.objeto(h).expect("contexto esperado");
        debug_assert_eq!(o.class_id, i64::from(cid::CONTEXTO), "contexto esperado");
        o.campo(i)
    }
    /// Uma `_Closure` nova.
    pub fn nova_closure(&mut self, codigo: i64, contexto: Campo, tipado: i64, abi: i64) -> Ref {
        let raiz = contexto.1.then_some(contexto.0);
        self.com_refs_enraizados(raiz, |heap| {
            let h = heap.alocar_instancia(cid::CLOSURE, CAMPOS_DA_CLOSURE);
            heap.definir_campo(h, 0, codigo, false);
            heap.definir_campo(h, 1, contexto.0, contexto.1);
            heap.definir_campo(h, 2, tipado, false);
            heap.definir_campo(h, 3, abi, false);
            h
        })
    }
    /// A vista da `_Closure` `h`; `None` se não é closure.
    pub fn closure(&self, h: Ref) -> Option<ClosureRef> {
        if !e_objeto(h) || self.cid_de(h) != cid::CLOSURE {
            return None;
        }
        let o = self.objeto(h)?;
        Some(ClosureRef { codigo: o.campo(0).0, contexto: o.campo(1), tipado: o.campo(2).0, abi: o.campo(3).0 })
    }
    /// O tear-off canônico da função de topo `codigo` (tabela `tearoffs`): uma
    /// closure sem contexto e sem corpo tipado, a mesma a cada chamada
    /// (`identical(f, f)`), raiz permanente.
    pub fn tearoff(&mut self, codigo: i64) -> Ref {
        assert!(codigo >= 0, "ID de código inválido");
        if let Some(h) = self.tearoff_registrado(codigo) {
            return h;
        }
        let h = self.nova_closure(codigo, (0, false), 0, 0);
        self.registrar_tearoff(codigo, h);
        h
    }
    /// Um record posicional (`_Record`, `REFS`) com os campos.
    pub fn novo_record(&mut self, campos: &[Ref]) -> Ref {
        self.com_refs_enraizados(campos.iter().copied(), |heap| {
            let h = heap.alocar(cid::RECORD, 1 + campos.len(), flags::REFS);
            // Recém-alocado (jovem, sem publicar): gravação sem barreira.
            let corpo = heap.palavras_mut(h);
            corpo[0] = campos.len() as i64;
            corpo[1..=campos.len()].copy_from_slice(campos);
            h
        })
    }
    /// Os campos do record posicional `h`; `None` se não é `_Record`.
    pub fn record(&self, h: Ref) -> Option<&[i64]> {
        if !e_objeto(h) || self.cid_de(h) != cid::RECORD {
            return None;
        }
        let corpo = self.palavras(h);
        let n = usize::try_from(corpo[0]).expect("forma de record");
        Some(&corpo[1..=n])
    }
}

#[cfg(test)]
mod testes_caixas {
    use super::*;

    #[test]
    fn caixas_de_bool_sao_as_estaticas() {
        let v = Heap::caixa_bool(true);
        let f = Heap::caixa_bool(false);
        assert_ne!(v, f);
        assert!(e_objeto(v) && e_objeto(f));
        assert_eq!(v, Heap::caixa_bool(true));
    }
}

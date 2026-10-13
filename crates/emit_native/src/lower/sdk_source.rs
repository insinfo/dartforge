//! O SDK compilado da fonte no lowering (P5c/P5d, δ; docs/NATIVO-PLANO.md §7).
//!
//! As sete bibliotecas de
//! `sdk_modulo::SOURCE_LIBRARIES` são código Dart compilado como o do
//! programa, cada uma no seu módulo (objeto em cache), e o programa as chama
//! pelos símbolos estáveis. Três coisas mudam em relação ao mundo fechado:
//!
//! 1. **Chamada por seletor.** Um membro público de instância de uma classe
//!    do SDK pode ser sobrescrito por uma classe do programa, que o SDK não
//!    conhece quando é compilado. A chamada vai pelo seletor (`c:m`, `g:x`,
//!    `s:x`; o nome privado leva `@<library>`), que a tabela de métodos da
//!    classe dinâmica resolve (`llvm/selectors.rs`, `runtime/selectors.rs`).
//!    Continua direta (ou pelo `switch` do mundo fechado de `members.rs`) a
//!    chamada que só a biblioteca da classe pode sobrescrever: membro
//!    privado, ou classe privada cujos subtipos na biblioteca são todos
//!    privados.
//! 2. **Adaptadores.** Cada membro de instância ganha as entradas uniformes
//!    que a tabela de métodos aponta: `<símbolo>$c` (chamar), `$g` (ler: o
//!    getter, o campo, ou o tear-off de um método) e `$s` (gravar), com a
//!    convenção das closures (`i64 (i64 receiver, ptr args, ptr desc)`).
//! 3. **`external`.** O membro de patch (`patched_by`) é o corpo; um native
//!    (`vm:external-name`) é a função do runtime da tabela `natives.rs`; o
//!    que não tem implementação (native pendente, intrínseco da VM) é
//!    diagnóstico — o membro que o chama é **recusado** no módulo do SDK
//!    (`sdk_modulo`), com o motivo, e nunca emitido errado.

use super::fn_builder::FnBuilder;
use super::members::{Evaluated, linearization, subclass_of};
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::ParameterKind;

/// O que o seletor faz com o membro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    /// `o.m(…)`.
    Call,
    /// `o.x`.
    Read,
    /// `o.x = v`.
    Write,
}

/// O texto do seletor: `c:m`, `g:x`, `s:x`; um nome privado leva
/// `@<library>` (dois `_m` de bibliotecas diferentes são membros
/// diferentes). O `_=` da chave de um setter nas tabelas do elemento sai
/// (`==` e `[]=` são operadores, não setters).
pub fn selector_string(ctx: &Context, ty: Tipo, name_str: &str, lib: LibraryId) -> String {
    let p = match ty {
        Tipo::Call => "c",
        Tipo::Read => "g",
        Tipo::Write => "s",
    };
    let name_str = name_str.strip_suffix("_=").unwrap_or(name_str);
    if name_str.starts_with('_') {
        format!("{p}:{name_str}@{}", ctx.library_name(lib))
    } else {
        format!("{p}:{name_str}")
    }
}

/// Nenhuma classe fora da biblioteca de `cid` pode ser subtipo dela: ela e
/// todos os seus subtipos na biblioteca são privados.
pub fn closed_class(ctx: &Context, cid: ClassId) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !ctx.symbol_name(class.name).starts_with('_') {
        return false;
    }
    ctx.subtypes(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library != class.library || ctx.symbol_name(c.name).starts_with('_')
    })
}

/// O membro `name` de `cid` só pode ser sobrescrito dentro da biblioteca
/// dela (o mundo fechado de `members.rs` vale para ele).
pub fn closed_member(ctx: &Context, cid: ClassId, name_str: &str) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !ctx.program.library(class.library).is_sdk {
        // Classe do programa: o programa conhece todos os subtipos dela.
        return true;
    }
    name_str.starts_with('_') || closed_class(ctx, cid) || class_closed_by_modifier(ctx, cid)
}

/// Os modificadores de classe do Dart 3 fecham `cid` na biblioteca dela:
/// ela é `final` ou `sealed`, e todo subtipo dela no programa é da mesma
/// biblioteca e `final`, `sealed` ou privado — nenhum código de fora pode
/// estendê-la, implementá-la nem sobrescrever um membro dela. É o caso de
/// `String`, `int`, `double`, `num` e `bool`: sem isto, `s.codeUnitAt(i)`
/// com `s` estático `String` ia pelo seletor e pela entrada uniforme, que
/// confere os argumentos no RTI (a escrita dos cabeçalhos do `dart:_http`
/// fazia centenas dessas chamadas por requisição).
pub fn class_closed_by_modifier(ctx: &Context, cid: ClassId) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !(class.modifiers.final_ || class.modifiers.sealed) {
        return false;
    }
    ctx.subtypes(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library == class.library
            && (c.modifiers.final_ || c.modifiers.sealed || ctx.symbol_name(c.name).starts_with('_'))
    })
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.<selector>(evaluated)` pela convenção uniforme; o resultado é
    /// `Ref`.
    pub fn call_by_selector(&mut self, recv: Operand, selector: String, evaluated: &[Evaluated]) -> Operand {
        self.call_by_selector_with_tuple(recv, selector, evaluated, Operand::Constant(Constant::Int(0)))
    }

    pub(super) fn call_by_selector_with_tuple(
        &mut self,
        recv: Operand,
        selector: String,
        evaluated: &[Evaluated],
        tuple_types: Operand,
    ) -> Operand {
        let recv = self.coerce(recv, Type::Ref);
        let mut args = Vec::with_capacity(evaluated.len());
        for (n, v) in evaluated {
            if n.is_none() {
                let v = self.coerce(v.clone(), Type::Ref);
                args.push(v);
            }
        }
        let mut named: Vec<(String, Operand)> = evaluated
            .iter()
            .filter_map(|(n, v)| n.map(|n| (self.ctx.symbol_name(n).to_string(), v.clone())))
            .collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut names = Vec::with_capacity(named.len());
        for (n, v) in named {
            let v = self.coerce(v, Type::Ref);
            args.push(v);
            names.push(n);
        }
        self.emit_call_with_check(Instruction::CallSelector { selector, recv, args, names, tuple_types }, Type::Ref)
    }

    /// `recv.name(evaluated)` pelo seletor, com o nome privado da biblioteca
    /// da unidade corrente.
    pub fn call_by_name(&mut self, recv: Operand, ty: Tipo, name_str: &str, evaluated: &[Evaluated]) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = selector_string(self.ctx, ty, name_str, lib);
        self.call_by_selector(recv, s, evaluated)
    }

    /// [`Self::call_by_name`] com o receptor `alvo` de tipo estático
    /// conhecido: se os argumentos têm todos o tipo garantido, pelo seletor
    /// tipado (`typed_entry.rs`), que só confere os covariantes.
    pub fn call_by_name_typed(
        &mut self,
        recv: Operand,
        alvo: dartforge_frontend::ast::ExprId,
        ty: Tipo,
        name_str: &str,
        evaluated: &[Evaluated],
    ) -> Operand {
        let typed = self.ctx.get_type(self.unit_id, alvo).is_some_and(|t| t != self.ctx.core.dynamic_);
        self.call_by_name_with_typed_receiver(recv, typed, ty, name_str, evaluated)
    }

    /// [`Self::call_by_name_typed`] com a decisão sobre o receptor já
    /// tomada (`typed_receiver`: o tipo estático dele é conhecido).
    pub fn call_by_name_with_typed_receiver(
        &mut self,
        recv: Operand,
        typed_receiver: bool,
        ty: Tipo,
        name_str: &str,
        evaluated: &[Evaluated],
    ) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = selector_string(self.ctx, ty, name_str, lib);
        let s = if typed_receiver && matches!(ty, Tipo::Call | Tipo::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        self.call_by_selector(recv, s, evaluated)
    }

    /// Hook de `call_member` (SDK da fonte): o membro de instância de uma
    /// classe do SDK vai pelo seletor, a não ser que só a biblioteca dela o
    /// possa sobrescrever e ele tenha **uma** implementação, que é um método
    /// (chamada direta). Um getter de interface do programa também usa o
    /// seletor quando alguma implementação concreta é um campo: o despacho
    /// por funções de `members.rs` não pode representar esse getter implícito.
    pub fn call_source_member(&mut self, recv: Operand, decl_fid: usize, evaluated: &[Evaluated]) -> Option<Operand> {
        if self.in_adapter {
            return None;
        }
        let f = &self.ctx.program.functions[decl_fid];
        let cid = f.class?;
        if f.static_ {
            return None;
        }
        let name_str = self.ctx.symbol_name(f.name).to_string();
        let key = if f.kind == FunctionKind::Setter { format!("{name_str}_=") } else { name_str.clone() };
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !is_sdk && (!matches!(f.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor)
            || !implementations(self.ctx, cid, &key).iter().any(|i| matches!(i, Implementation::Field(_))))
        {
            return None;
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && let [Implementation::Function(alvo)] = implementations(self.ctx, cid, &key)[..]
        {
            // O `this` de um membro do SDK é sempre `Ref`: um `int` escalar
            // vira `Smi` ou `_Mint` aqui (o `int` fechado por modificador
            // chega com o receptor em `i64`; sem a caixa, um valor par ou
            // fora do `Smi` seria lido como handle).
            let recv = self.coerce(recv, Type::Ref);
            let args = self.match_args(alvo, evaluated);
            let r = self.call_direct(alvo, Some(recv), args);
            let ret = self.return_repr(decl_fid);
            return Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) });
        }
        let ty = match f.kind {
            FunctionKind::Getter => Tipo::Read,
            FunctionKind::Setter => Tipo::Write,
            FunctionKind::ImplicitAccessor if evaluated.len() == 1 => Tipo::Write,
            FunctionKind::ImplicitAccessor => Tipo::Read,
            _ => Tipo::Call,
        };
        if is_sdk && closed_member(self.ctx, cid, &name_str) {
            let ret = match self.return_repr(decl_fid) {
                Type::Void => Type::Ref,
                t => t,
            };
            if let Some(r) = self.class_dispatch(&recv, cid, &key, ty, f.library, evaluated, ret) {
                let void = matches!(self.return_repr(decl_fid), Type::Void);
                return Some(if void { Operand::Constant(Constant::Null) } else { r });
            }
        }
        let s = selector_string(self.ctx, ty, &name_str, f.library);
        // Receptor tipado e argumentos garantidos: a entrada que só confere
        // os covariantes (`typed_entry.rs`).
        let s = if matches!(ty, Tipo::Call | Tipo::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        let tuple = if self.generic_function(decl_fid) {
            self.armed_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0)))
        } else {
            Operand::Constant(Constant::Int(0))
        };
        let r = self.call_by_selector_with_tuple(recv, s, evaluated, tuple);
        let ret = self.return_repr(decl_fid);
        Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) })
    }

    /// Despacho de poucos alvos (P2) de um membro fechado (só a biblioteca
    /// dele o implementa, e o programa inteiro dela está compilado) com 2 a
    /// [`FEW_TARGETS`] implementações: um `switch` pela classe do receptor
    /// escolhe a chamada direta ou o acesso ao campo de cada uma — o mesmo
    /// que a chamada direta do caso de uma implementação só, repetido por
    /// classe —, e o seletor fica no `default` (uma classe que o módulo não
    /// conhece). Sem isto, cada acesso ia pelo seletor e pela entrada
    /// uniforme, que confere os argumentos (os `_index`, `_data`,
    /// `_usedData` dos mapas e conjuntos: duas hierarquias no
    /// `compact_hash.dart`). `None`: sem despacho por classe (o chamador
    /// segue pelo seletor).
    #[allow(clippy::too_many_arguments)]
    fn class_dispatch(
        &mut self,
        recv: &Operand,
        cid: ClassId,
        key: &str,
        ty: Tipo,
        library: dartforge_elements::model::LibraryId,
        evaluated: &[Evaluated],
        ret: Type,
    ) -> Option<Operand> {
        let by_class = implementations_by_class(self.ctx, cid, key)?;
        let mut distinct: Vec<Implementation> = Vec::new();
        for (_, i) in &by_class {
            if !distinct.contains(i) {
                distinct.push(*i);
            }
        }
        if distinct.len() < 2 || distinct.len() > FEW_TARGETS {
            return None;
        }
        // Chamar um campo (uma closure guardada) fica com o seletor.
        if ty == Tipo::Call && distinct.iter().any(|i| matches!(i, Implementation::Field(_))) {
            return None;
        }
        let recv = self.coerce(recv.clone(), Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let blocks: Vec<BlockId> = distinct.iter().map(|_| self.new_block()).collect();
        let pattern = self.new_block();
        let join = self.new_block();
        let cases = by_class
            .iter()
            .map(|(id, i)| (*id, blocks[distinct.iter().position(|d| d == i).expect("implementação listada")]))
            .collect();
        self.terminate(Terminator::Switch { val: cls, default: pattern, cases });
        let span = dartforge_diagnostics::Span { start: 0, end: 0 };
        let mut entries = Vec::new();
        for (i, &b) in distinct.iter().zip(&blocks) {
            self.set_block(b);
            let r = match (*i, ty) {
                (Implementation::Field(vid), Tipo::Write) => {
                    let v = evaluated.first().map(|(_, v)| v.clone()).unwrap_or(Operand::Constant(Constant::Null));
                    let v = self.coerce(v, self.field_repr(vid));
                    self.store_field(recv.clone(), vid, v, span);
                    Operand::Constant(Constant::Null)
                }
                (Implementation::Field(vid), _) => self.read_field_with_late_direct(recv.clone(), vid, span),
                (Implementation::Function(alvo), _) => {
                    let args = self.match_args(alvo, evaluated);
                    let r = self.call_direct(alvo, Some(recv.clone()), args);
                    if matches!(self.return_repr(alvo), Type::Void) { Operand::Constant(Constant::Null) } else { r }
                }
            };
            if self.is_terminated() {
                continue;
            }
            let r = self.coerce(r, ret);
            entries.push((self.current_block, r));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(pattern);
        let name_str = key.strip_suffix("_=").unwrap_or(key);
        let s = selector_string(self.ctx, ty, name_str, library);
        let s = if matches!(ty, Tipo::Call | Tipo::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        let r = self.call_by_selector(recv, s, evaluated);
        if !self.is_terminated() {
            let r = self.coerce(r, ret);
            entries.push((self.current_block, r));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if entries.is_empty() {
            // Todo caminho lança: a junção é inalcançável.
            return Some(Self::zero_value(ret));
        }
        Some(self.emit(Instruction::Phi { incoming: entries, ty: ret }, ret))
    }

    /// Hook da leitura de campo: um getter de subclasse pode sobrescrever
    /// inclusive o getter implícito de um campo da classe base.
    pub fn read_source_field(&mut self, obj: Operand, vid: VariableId) -> Option<Operand> {
        if self.in_adapter {
            return None;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let cid = v.class?;
        let name_str = self.ctx.symbol_name(v.name).to_string();
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !is_sdk {
            // A leitura direta do campo só vale se toda classe concreta do
            // tipo tem ESTE campo: um getter que o sobrescreve, ou outro
            // campo de mesmo nome numa classe que só implementa a interface
            // (`class ScalarToken implements Token { final FileSpan span; }`
            // do `package:yaml`, noutra posição do objeto), vão pelo seletor.
            let only_this_field =
                implementations(self.ctx, cid, &name_str).iter().all(|i| *i == Implementation::Field(vid));
            if only_this_field {
                return None;
            }
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && implementations(self.ctx, cid, &name_str).iter().all(|i| *i == Implementation::Field(vid))
        {
            return None;
        }
        let repr = self.field_repr(vid);
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && let Some(r) = self.class_dispatch(&obj, cid, &name_str, Tipo::Read, v.library, &[], repr)
        {
            return Some(r);
        }
        let s = selector_string(self.ctx, Tipo::Read, &name_str, v.library);
        let r = self.call_by_selector(obj, s, &[]);
        let repr = self.field_repr(vid);
        Some(self.coerce(r, repr))
    }

    /// Hook da gravação de campo por atribuição (SDK da fonte): o campo de
    /// uma classe do SDK que um setter pode sobrescrever vai pelo seletor
    /// `s:x`. `false`: a gravação direta de sempre vale.
    pub fn store_source_field(&mut self, obj: Operand, vid: VariableId, value: Operand) -> bool {
        if self.in_adapter {
            return false;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let Some(cid) = v.class else { return false };
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        let name_str = self.ctx.symbol_name(v.name).to_string();
        // Como na leitura: direto só se toda classe concreta grava ESTE
        // campo (no SDK, também se o membro é fechado).
        let only_this_field =
            implementations(self.ctx, cid, &format!("{name_str}_=")).iter().all(|i| *i == Implementation::Field(vid));
        if only_this_field && (!is_sdk || closed_member(self.ctx, cid, &name_str)) {
            return false;
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && self
                .class_dispatch(&obj, cid, &format!("{name_str}_="), Tipo::Write, v.library, &[(None, value.clone())], Type::Ref)
                .is_some()
        {
            return true;
        }
        let s = selector_string(self.ctx, Tipo::Write, &name_str, v.library);
        self.call_by_selector(obj, s, &[(None, value)]);
        true
    }

    /// `a == b` com o SDK da fonte (§17.26 "Equality"): com um lado null,
    /// `identical(a, b)`; senão `a.==(b)` pela classe dinâmica de `a`.
    pub fn source_equality(&mut self, a: Operand, b: Operand) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let na = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), zero.clone()), Type::I1);
        let nb = self.emit(Instruction::ICmp(ICmpOp::Eq, b.clone(), zero), Type::I1);
        let any = self.emit(Instruction::Or(na, nb), Type::I1);
        let any = self.emit(Instruction::ICmp(ICmpOp::Ne, any, Operand::Constant(Constant::Int(0))), Type::I1);
        let b_id = self.new_block();
        let b_din = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: any, then_block: b_id, else_block: b_din });
        self.set_block(b_id);
        let r1 = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b.clone()), Type::I1);
        let end1 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(b_din);
        let r = self.call_by_selector(a, "c:==".to_string(), &[(None, b)]);
        let r2 = self.coerce(r, Type::I1);
        let end2 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(end1, r1), (end2, r2)], ty: Type::I1 }, Type::I1)
    }

    /// `a == b` (ou `a != b`, com `negate`) quando o tipo estático de `a` é
    /// uma classe cujo `==` é fechado ([`closed_member`]: `String`, `int`,
    /// `double`, `bool`…): a regra do null de [`Self::source_equality`] e,
    /// com os dois lados não nulos, o `==` pelo despacho de
    /// [`Self::call_source_member`] — a chamada direta, ou o `switch` pela
    /// classe com o seletor no `default` —, sem a entrada uniforme e a
    /// conferência dos argumentos do seletor `c:==`. É o `char == " "` dos
    /// analisadores do `dart:_http`. `None`: o caminho geral.
    pub fn closed_class_equality(
        &mut self,
        type_of_a: Option<dartforge_types::table::TypeId>,
        a: &Operand,
        b: &Operand,
        negate: bool,
    ) -> Option<Operand> {
        if self.in_adapter || self.operand_type(a) != Type::Ref {
            return None;
        }
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(type_of_a?) else {
            return None;
        };
        let fid = self.member_in_class(*class, "==")?;
        let owner = self.ctx.program.functions[fid].class?;
        if !self.ctx.program.library(self.ctx.program.classes[owner.0 as usize].library).is_sdk
            || !closed_member(self.ctx, owner, "==")
        {
            return None;
        }
        let a = a.clone();
        let b = self.coerce(b.clone(), Type::Ref);
        // `String`: o `==` das duas classes concretas é o mesmo native
        // (`_StringBase._igualA`, que já trata `null` e o outro lado que
        // não é `String`): uma chamada, sem a regra do null nem o `switch`
        // pela classe — o intrínseco `String_equality` da VM.
        if self.ctx.core.string_class == Some(*class) {
            let r = self.emit(
                Instruction::CallRuntime {
                    name: crate::natives::symbol("DartForge_string_igual_a"),
                    args: vec![(a, Type::Ref), (b, Type::Ref)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let e = self.coerce(r, Type::I1);
            return Some(if negate { self.emit(Instruction::LNot(e), Type::I1) } else { e });
        }
        let zero = Operand::Constant(Constant::Int(0));
        let na = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), zero.clone()), Type::I1);
        let nb = self.emit(Instruction::ICmp(ICmpOp::Eq, b.clone(), zero), Type::I1);
        let any = self.emit(Instruction::Or(na, nb), Type::I1);
        let any = self.emit(Instruction::ICmp(ICmpOp::Ne, any, Operand::Constant(Constant::Int(0))), Type::I1);
        let b_id = self.new_block();
        let b_din = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: any, then_block: b_id, else_block: b_din });
        self.set_block(b_id);
        let r1 = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b.clone()), Type::I1);
        let end1 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(b_din);
        // As condições de `call_source_member` já valem (membro de
        // instância de classe do SDK, fora de adaptador); o seletor fica só
        // por garantia, sem deixar blocos pela metade.
        let r = match self.call_source_member(a.clone(), fid, &[(None, b.clone())]) {
            Some(r) => r,
            None => self.call_by_selector(a, "c:==".to_string(), &[(None, b)]),
        };
        let r2 = self.coerce(r, Type::I1);
        let mut entries = vec![(end1, r1)];
        if !self.is_terminated() {
            entries.push((self.current_block, r2));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        let e = self.emit(Instruction::Phi { incoming: entries, ty: Type::I1 }, Type::I1);
        Some(if negate { self.emit(Instruction::LNot(e), Type::I1) } else { e })
    }

    /// `op is T` com o SDK da fonte (hook de `test_type`): toda classe
    /// (`int`, `num`, `String`, `Object`…) é uma classe compilada com id, e
    /// a resposta é a do grafo de subtipos que as bibliotecas registram; null
    /// só é um `T` quando `T` é `Null` ou anulável (§20.3). Parâmetros de tipo
    /// são resolvidos pelo RTI da instância ou pela tupla do método.
    pub fn test_source_type(
        &mut self,
        ast_ty: &dartforge_frontend::ast::TypeAnnotation,
        op: Operand,
    ) -> Option<Operand> {
        // No código do SDK, os testes com argumentos de tipo (`is List<E>`)
        // escolhem um caminho rápido equivalente para um programa correto
        // (`ListBase.setRange`, `List.from`, `ListQueue.addAll`…): até a RTI,
        // eles conferem a classe. No programa, só o cast (`as`) confere a
        // classe; o `is` com argumentos continua diagnóstico.
        let in_sdk = self.ctx.program.library(self.ctx.program.unit(self.unit_id).library).is_sdk;
        let only_class = self.cast_by_class_only || in_sdk;
        // Tipos sem classe que não têm receita resolvida continuam recusados.
        let without_class = |b: &mut Self| -> Option<Operand> {
            if b.cast_by_class_only {
                Some(Operand::Constant(Constant::Bool(true)))
            } else if in_sdk {
                Some(b.unsupported("teste de tipo sem classe no código do SDK (RTI pelo seletor)", ast_ty.span))
            } else {
                None
            }
        };
        let dartforge_frontend::ast::TypeKind::Named { name, args } = &ast_ty.kind else {
            // `Iterable.generate<E>` testa `_id is E Function(int)` antes de
            // aceitar o gerador implícito. A RTI representa a assinatura e
            // substitui `E` pela tupla da factory do SDK.
            if let Some(recipe) = self.annotation_recipe(ast_ty) {
                let ty = self.recipe_rti(&recipe);
                return Some(self.test_rti(op, ty));
            }
            return without_class(self);
        };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let trivial = |t: &dartforge_frontend::ast::TypeAnnotation| match &t.kind {
            dartforge_frontend::ast::TypeKind::Named { name, args } if args.is_empty() => name.last().is_some_and(|n| {
                let s = self.ctx.symbol_name(n.sym);
                s == "dynamic" || (s == "Object" && t.nullable)
            }),
            _ => false,
        };
        let arguments = !args.is_empty() && !args.iter().all(|a| trivial(unit_ast.ty(*a)));
        if arguments && !only_class {
            return None;
        }
        // No SDK, `is C<…>` com argumentos confere a RTI inteira (o
        // `value is Future<T>` do `_Future._asyncComplete` decide entre
        // encadear e completar com o próprio `Future`); o `as` segue pela
        // classe.
        if arguments
            && in_sdk
            && !self.cast_by_class_only
            && let Some(recipe) = self.annotation_recipe(ast_ty)
        {
            let ty = self.recipe_rti(&recipe);
            return Some(self.test_rti(op, ty));
        }
        let last_one = name.last()?;
        let name_str = self.ctx.symbol_name(last_one.sym).to_string();
        // Parâmetros da classe vêm do RTI de `this` (`P<i>`). Os de método
        // vêm da tupla oculta repassada pelo adaptador do seletor (`M<i>`).
        if in_sdk
            && !self.cast_by_class_only
            && args.is_empty()
            && (self.function_type_params.contains(&last_one.sym)
                || self.local_type_index(last_one.sym).is_some()
                || self.class_params().contains(&last_one.sym))
        {
            let recipe = self.annotation_recipe(ast_ty)?;
            let ty = self.recipe_rti(&recipe);
            return Some(self.test_rti(op, ty));
        }
        let mut op = op;
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            // Escalar: verdadeiro já em compilação para os supertipos óbvios;
            // o resto (`_Smi`, `_IntegerImplementation`, `Pattern`…) pela
            // classe da caixa — um `int` é `_Smi` ou `_Mint` conforme o valor.
            let correct = matches!(
                (name_str.as_str(), repr),
                ("int", Type::I64)
                    | ("double", Type::F64)
                    | ("bool", Type::I1 | Type::I8)
                    | ("num", Type::I64 | Type::F64)
                    | ("Object" | "dynamic" | "Comparable", _)
            );
            if correct {
                return Some(Operand::Constant(Constant::Bool(true)));
            }
            op = self.coerce(op, Type::Ref);
        }
        if name_str == "dynamic" {
            return Some(Operand::Constant(Constant::Bool(true)));
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let binding = match &name[..] {
            [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
            _ => self.ctx.program.lookup(lib, last_one.sym),
        };
        let cid = binding.and_then(|b| match b.getter {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            _ => None,
        });
        let Some(id) = cid.and_then(|c| self.ctx.class_id_of(c)) else {
            return without_class(self);
        };
        let _ = only_class;
        let zero = Operand::Constant(Constant::Int(0));
        let null = self.emit(Instruction::ICmp(ICmpOp::Eq, op.clone(), zero.clone()), Type::I1);
        if name_str == "Null"
            && cid.is_some_and(|c| self.ctx.program.library(self.ctx.program.classes[c.0 as usize].library).uri == "dart:core")
        {
            return Some(null);
        }
        let cls = self.emit(
            Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(op, Type::Ref)], ret_ty: Type::I64 },
            Type::I64,
        );
        let sub = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_is_subclass".to_string(),
                args: vec![(cls, Type::I64), (Operand::Constant(Constant::Int(i64::from(id))), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let sub = self.emit(Instruction::ICmp(ICmpOp::Ne, sub, zero), Type::I1);
        // null não é um `T` não anulável, mesmo que `Null` seja subclasse de
        // `Object` no grafo das classes.
        let non_null = self.emit(Instruction::LNot(null.clone()), Type::I1);
        let base = self.emit(Instruction::And(sub, non_null), Type::I64);
        let base = self.emit(Instruction::ICmp(ICmpOp::Ne, base, Operand::Constant(Constant::Int(0))), Type::I1);
        if !ast_ty.nullable {
            return Some(base);
        }
        let r = self.emit(Instruction::Or(base, null), Type::I64);
        Some(self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1))
    }

    /// `toString()` de um valor pelo seletor (interpolação com o SDK da
    /// fonte): o texto de null é `"null"` (o `toString` de `Null`).
    pub fn string_by_selector(&mut self, op: Operand) -> Operand {
        let r = self.call_by_selector(op, "c:toString".to_string(), &[]);
        r
    }

    /// Hook de `call_direct`: a chamada a um `external` do SDK da fonte.
    /// `None` quando a função tem corpo (a chamada direta de sempre).
    pub fn call_external(&mut self, fid: usize, this: Option<Operand>, args: &[Operand]) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        if !f.external || f.variable.is_some() {
            return None;
        }
        if let Some(p) = f.patched_by
            && p.0 as usize != fid
        {
            return Some(self.call_direct(p.0 as usize, this, args.to_vec()));
        }
        let (native, recognized) = crate::natives::pragmas(self.ctx.program, self.ctx.interner, f);
        let span = Span { start: 0, end: 0 };
        let owner = f
            .class
            .map(|c| format!("{}.", self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name)))
            .unwrap_or_default();
        let member = format!("{owner}{}", self.ctx.symbol_name(f.name));
        if let Some(r) = self.inline_string(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        // Os ganchos de cada pacote do espaço unificado
        // (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, passo 4): listas do núcleo
        // (P3), listas tipadas e SIMD (P4), caixas, closures e records (P2).
        if let Some(r) = self.inline_list(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.typed_inline(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.inline_box(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        let name_str = match native {
            Some(n) => {
                match crate::natives::native(&n).map(|x| x.state) {
                    Some(crate::natives::State::Runtime) => {}
                    Some(crate::natives::State::Builtin) => return Some(self.embedded_native(&n, fid, args)),
                    Some(crate::natives::State::Pendente) => {
                        return Some(self.unsupported(&format!("native pendente `{n}` ({member})"), span));
                    }
                    None => return Some(self.unsupported(&format!("native desconhecido `{n}` ({member})"), span)),
                }
                n
            }
            None if recognized && let Some(op) = self.typed_factory(&member, args) => return Some(op),
            None if recognized => match crate::natives::intrinsic(&member) {
                Some(n) => n.to_string(),
                None => return Some(self.unsupported(&format!("intrínseco da VM `{member}`"), span)),
            },
            None => {
                if let Some(r) = self.call_annotated_native(fid, args, span) {
                    return Some(r);
                }
                return Some(self.unsupported(&format!("external sem implementação `{member}`"), span));
            }
        };
        // A assinatura do native é a representação dos tipos declarados;
        // `bool` cruza a fronteira como `i8` (nativos.rs).
        let mut a = Vec::with_capacity(args.len() + 1);
        if let Some(t) = this {
            // O receptor de um native de `int`/`double`/`bool` é o valor (as
            // funções do runtime recebem `i64`/`double`, como a VM, que
            // desencaixota o `Smi`); o de qualquer outra classe, o `Ref`.
            let r = f.class.map_or(Type::Ref, |c| self.receiver_repr(c));
            let t = self.coerce(t, r);
            if r == Type::I1 {
                let b = self.emit(Instruction::ZExt { op: t, from: Type::I1, to: Type::I8 }, Type::I8);
                a.push((b, Type::I8));
            } else {
                a.push((t, r));
            }
        }
        // Os parâmetros e o retorno na representação de valor dos tipos
        // declarados: `_Smi`/`_Double` (as implementações de `int`/`double`)
        // também cruzam como `i64`/`double`.
        let types: Vec<Type> = self
            .ctx
            .outline
            .functions
            .get(fid)
            .map(|d| d.parameters.iter().map(|p| self.native_repr(p.ty)).collect())
            .unwrap_or_default();
        for (i, v) in args.iter().enumerate() {
            let t = types.get(i).copied().unwrap_or_else(|| self.operand_type(v));
            let t = if t == Type::Void { Type::Ref } else { t };
            let v = self.coerce(v.clone(), t);
            if t == Type::I1 {
                let b = self.emit(Instruction::ZExt { op: v, from: Type::I1, to: Type::I8 }, Type::I8);
                a.push((b, Type::I8));
            } else {
                a.push((v, t));
            }
        }
        let ret = self.return_repr(fid);
        let ret_value = if super::generative_constructor(self.ctx, fid) {
            Type::Void
        } else {
            self.ctx.outline.functions.get(fid).map_or(ret, |d| self.native_repr(d.return_type))
        };
        // `RETURN_REF`: o runtime devolve a caixa, e o valor declarado sai
        // da coerção (nativos.rs).
        let ret_value = if crate::natives::RETURN_REF.contains(&name_str.as_str()) && ret_value != Type::Void {
            Type::Ref
        } else {
            ret_value
        };
        let native_ret = if ret_value == Type::I1 { Type::I8 } else { ret_value };
        let r = self.emit_call_with_check(
            Instruction::CallRuntime { name: crate::natives::symbol(&name_str), args: a, ret_ty: native_ret },
            native_ret,
        );
        let r = match ret_value {
            Type::Void => return Some(Operand::Constant(Constant::Null)),
            Type::I1 => self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1),
            _ => r,
        };
        Some(if ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) })
    }

    /// As fábricas `vm:recognized` do `typed_data_patch.dart` da VM: a
    /// lista pública (`Uint8List(n)`) aloca a lista interna da classe
    /// correspondente (`_Uint8List`), e `_XArrayView._(base, shift, n)` a
    /// visão (`typed_data.rs` do runtime). A alocação registra a tabela de
    /// métodos da classe, como a de um objeto comum (emissor LLVM).
    fn typed_factory(&mut self, member: &str, args: &[Operand]) -> Option<Operand> {
        const LISTS: &[(&str, &str, i64)] = &[
            ("Int8List.", "_Int8List", 0),
            ("Uint8List.", "_Uint8List", 1),
            ("Uint8ClampedList.", "_Uint8ClampedList", 2),
            ("Int16List.", "_Int16List", 3),
            ("Uint16List.", "_Uint16List", 4),
            ("Int32List.", "_Int32List", 5),
            ("Uint32List.", "_Uint32List", 6),
            ("Int64List.", "_Int64List", 7),
            ("Uint64List.", "_Uint64List", 8),
            ("Float32List.", "_Float32List", 9),
            ("Float64List.", "_Float64List", 10),
            ("Float32x4List.", "_Float32x4List", 11),
            ("Int32x4List.", "_Int32x4List", 12),
            ("Float64x2List.", "_Float64x2List", 13),
        ];
        const VIEWS: &[(&str, i64)] = &[
            ("_Int8ArrayView", 0),
            ("_Uint8ArrayView", 1),
            ("_Uint8ClampedArrayView", 2),
            ("_Int16ArrayView", 3),
            ("_Uint16ArrayView", 4),
            ("_Int32ArrayView", 5),
            ("_Uint32ArrayView", 6),
            ("_Int64ArrayView", 7),
            ("_Uint64ArrayView", 8),
            ("_Float32ArrayView", 9),
            ("_Float64ArrayView", 10),
            ("_Float32x4ArrayView", 11),
            ("_Int32x4ArrayView", 12),
            ("_Float64x2ArrayView", 13),
            ("_ByteDataView", 14),
        ];
        let id_of = |b: &Self, name: &str| -> Option<i64> {
            b.ctx.sdk_class("typed_data", name).and_then(|c| b.ctx.class_id_of(c)).map(i64::from)
        };
        if let Some(&(_, internal, ty)) = LISTS.iter().find(|(m, _, _)| *m == member) {
            let cid = id_of(self, internal)?;
            let n = self.coerce(args.first()?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_novo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(ty)), Type::I64),
                        (n, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        // `Pointer<X>.asTypedList(n)` (`_asExternalTypedDataX` do
        // `ffi_patch.dart`): a lista interna da classe, sobre a memória
        // nativa do ponteiro (o `ExternalTypedData` da VM).
        const EXTERNALS: &[(&str, &str, i64)] = &[
            ("_asExternalTypedDataInt8", "_Int8List", 0),
            ("_asExternalTypedDataUint8", "_Uint8List", 1),
            ("_asExternalTypedDataInt16", "_Int16List", 3),
            ("_asExternalTypedDataUint16", "_Uint16List", 4),
            ("_asExternalTypedDataInt32", "_Int32List", 5),
            ("_asExternalTypedDataUint32", "_Uint32List", 6),
            ("_asExternalTypedDataInt64", "_Int64List", 7),
            ("_asExternalTypedDataUint64", "_Uint64List", 8),
            ("_asExternalTypedDataFloat", "_Float32List", 9),
            ("_asExternalTypedDataDouble", "_Float64List", 10),
        ];
        if let Some(&(_, internal, ty)) = EXTERNALS.iter().find(|(m, _, _)| *m == member) {
            let cid = id_of(self, internal)?;
            let pointer = self.coerce(args.first()?.clone(), Type::Ref);
            let n = self.coerce(args.get(1)?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_externo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(ty)), Type::I64),
                        (pointer, Type::Ref),
                        (n, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        let (class, factory) = member.split_once('.')?;
        if factory != "_" {
            return None;
        }
        // `_UnmodifiableXArrayView` estende `_XArrayView`: o mesmo tipo.
        let modifiable = class.strip_prefix("_Unmodifiable").map(|r| format!("_{r}"));
        let searched = modifiable.as_deref().unwrap_or(class);
        let &(_, ty) = VIEWS.iter().find(|(c, _)| *c == searched)?;
        // O bit `VISAO_IMUTAVEL` do runtime: a visão não modificável recusa
        // o caminho rápido de `[]=` (o `[]=` dela lança, como na VM).
        let ty = if modifiable.is_some() { ty | 0x100 } else { ty };
        let cid = id_of(self, class)?;
        let base = self.coerce(args.first()?.clone(), Type::Ref);
        let shift = self.coerce(args.get(1)?.clone(), Type::I64);
        let n = self.coerce(args.get(2)?.clone(), Type::I64);
        Some(self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_view_nova".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(cid)), Type::I64),
                    (Operand::Constant(Constant::Int(ty)), Type::I64),
                    (base, Type::Ref),
                    (shift, Type::I64),
                    (n, Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// A representação de um tipo na fronteira com um native: a de valor
    /// (`i64`, `double`, `i1`) para as classes de `int`, `double` e `bool`
    /// não anuláveis (inclusive `_Smi`, `_Mint`, `_Double`), senão a da HIR.
    fn native_repr(&self, ty: dartforge_types::table::TypeId) -> Type {
        if self.ctx.is_void(ty) {
            return Type::Void;
        }
        if let dartforge_types::table::Type::Interface { class, nullable: false, .. } = self.ctx.table.get(ty) {
            let r = self.receiver_repr(*class);
            if r != Type::Ref {
                return r;
            }
        }
        self.repr(ty)
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// A representação do receptor de um membro de `cid`: o valor para as
    /// classes de `int`, `double` e `bool` (e as implementações delas,
    /// `_Smi`, `_Mint`, `_Double`), senão `Ref`.
    fn receiver_repr(&self, cid: ClassId) -> Type {
        let core = self.ctx.core;
        let de = |c: Option<ClassId>| c.is_some_and(|c| subclass_of(self.ctx, cid, c));
        if de(core.int_class) {
            Type::I64
        } else if de(core.double_class) {
            Type::F64
        } else if de(core.bool_class) {
            Type::I1
        } else {
            Type::Ref
        }
    }

    /// Um native que o lowering gera no lugar da chamada
    /// (`natives::State::Builtin`).
    fn embedded_native(&mut self, name_str: &str, fid: usize, args: &[Operand]) -> Operand {
        let ret = self.return_repr(fid);
        match name_str {
            // `unsafeCast<T>(v)`: o próprio valor, sem checagem.
            "Internal_unsafeCast" => {
                let v = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                if ret == Type::Void { v } else { self.coerce(v, ret) }
            }
            // `ClassID.getID(o)`: a classe do valor em linha (`@df.class`,
            // docs/NATIVO-ESPACO-UNIFICADO.md §3.8), sem chamar o runtime.
            "ClassID_getID" => {
                let o = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                let o = self.coerce(o, Type::Ref);
                let c = self.emit(
                    Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(o, Type::Ref)], ret_ty: Type::I64 },
                    Type::I64,
                );
                if ret == Type::Void { c } else { self.coerce(c, ret) }
            }
            _ => self.unsupported(&format!("native embutido `{name_str}`"), Span { start: 0, end: 0 }),
        }
    }
}

/// Uma implementação concreta de um membro de instância.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Implementation {
    /// Método, getter, setter ou operador (com corpo, patch ou native).
    Function(usize),
    /// Campo (o getter/setter implícito).
    Field(VariableId),
}

/// As implementações distintas de `key` (o nome; `x_=` para o setter)
/// nas classes concretas compiladas que são subtipos de `cid`, cada uma
/// achada pela linearização da classe (a primeira que implementa).
pub fn implementations(ctx: &Context, cid: ClassId, key: &str) -> Vec<Implementation> {
    let Some(sym) = ctx.interner.lookup(key) else { return Vec::new() };
    if let Some(r) = ctx.implementations_memo.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().clone();
    }
    let r = implementations_without_memo(ctx, cid, sym);
    ctx.implementations_memo.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), std::sync::Arc::new(r.clone()));
    r
}

fn implementations_without_memo(ctx: &Context, cid: ClassId, sym: dartforge_intern::SymbolId) -> Vec<Implementation> {
    let mut exit = Vec::new();
    for &kid in ctx.subtypes(cid).iter() {
        let class = &ctx.program.classes[kid.0 as usize];
        if !ctx.compiled_library(class.library)
            || class.modifiers.abstract_
            || super::members::is_mixin(ctx, kid)
        {
            continue;
        }
        for c in linearization(ctx, kid) {
            let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&sym) else { continue };
            let f = f.0 as usize;
            if !implemented(ctx, f) {
                continue;
            }
            let i = match ctx.program.functions[f].variable {
                Some(v) => Implementation::Field(v),
                None => Implementation::Function(f),
            };
            if !exit.contains(&i) {
                exit.push(i);
            }
            break;
        }
    }
    exit
}

/// Cada classe concreta compilada, subtipo de `cid`, com a implementação
/// de `key` que a linearização dela acha (como em [`implementations`]), pelo
/// id de classe do runtime; `None` se alguma não tem id.
pub fn implementations_by_class(ctx: &Context, cid: ClassId, key: &str) -> Option<Vec<(i64, Implementation)>> {
    let sym = ctx.interner.lookup(key)?;
    if let Some(r) = ctx.implementations_by_class_memo.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().map(|v| v.as_ref().clone());
    }
    let r = implementations_by_class_without_memo(ctx, cid, sym);
    ctx.implementations_by_class_memo.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), r.clone().map(std::sync::Arc::new));
    r
}

fn implementations_by_class_without_memo(
    ctx: &Context,
    cid: ClassId,
    sym: dartforge_intern::SymbolId,
) -> Option<Vec<(i64, Implementation)>> {
    let mut exit = Vec::new();
    for &kid in ctx.subtypes(cid).iter() {
        let class = &ctx.program.classes[kid.0 as usize];
        if !ctx.compiled_library(class.library)
            || class.modifiers.abstract_
            || super::members::is_mixin(ctx, kid)
        {
            continue;
        }
        for c in linearization(ctx, kid) {
            let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&sym) else { continue };
            let f = f.0 as usize;
            if !implemented(ctx, f) {
                continue;
            }
            let i = match ctx.program.functions[f].variable {
                Some(v) => Implementation::Field(v),
                None => Implementation::Function(f),
            };
            exit.push((i64::from(ctx.class_id_of(kid)?), i));
            break;
        }
    }
    Some(exit)
}

/// Quantas implementações distintas o despacho por classe
/// (`class_dispatch`) aceita; acima disso, o seletor.
const FEW_TARGETS: usize = 4;

/// O que uma entrada da tabela de métodos faz.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Adapter {
    /// Chama o método; lê e chama o valor de um getter.
    Call,
    /// Lê o getter; tira o tear-off de um método.
    Read,
    /// Chama o setter.
    Write,
}

impl Adapter {
    fn suffix(self) -> &'static str {
        match self {
            Adapter::Call => "$c",
            Adapter::Read => "$g",
            Adapter::Write => "$s",
        }
    }
}

/// O símbolo base de um campo de instância: `df.<lib>.<Classe>.<field>`.
fn field_symbol(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    let owner = v.class.map(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string()).unwrap_or_default();
    format!(
        "df.{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(v.library)),
        crate::context::escape_str(&owner),
        crate::context::escape_str(ctx.symbol_name(v.name))
    )
}

/// O membro de instância implementado (tem corpo, é campo, ou é `external`
/// com patch ou native).
fn implemented(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    if f.variable.is_some() {
        return !f.abstract_;
    }
    if f.external {
        return true;
    }
    super::members::has_body(ctx, fid)
}

/// A tabela de métodos de uma classe concreta: para cada seletor que ela
/// responde (os membros dela e os herdados, pela linearização, o primeiro
/// que implementa), a entrada uniforme.
pub fn method_table(ctx: &Context, cid: ClassId) -> Vec<(String, String)> {
    let mut seen_set = std::collections::HashSet::new();
    let mut exit = Vec::new();
    let mut chain = linearization(ctx, cid);
    // A hierarquia de certos elementos sintéticos (notadamente enums) não
    // inclui `Object` em `supertype_class`. Seus membros continuam herdando
    // `Object.==`, `hashCode` e os outros acessores na semântica Dart.
    if let Some(object) = ctx.core.object_class && !chain.contains(&object) {
        chain.push(object);
    }
    for c in chain {
        let class = &ctx.program.classes[c.0 as usize];
        let mut members: Vec<(&str, usize)> = class
            .instance_members
            .iter()
            .map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize))
            .collect();
        members.sort();
        for (key, fid) in members {
            if !implemented(ctx, fid) {
                continue;
            }
            let f = &ctx.program.functions[fid];
            let is_setter = key.ends_with("_=");
            let name_str = key.strip_suffix("_=").unwrap_or(key);
            // A entrada tipada do método (`typed_entry.rs`), quando ele
            // a tem, no seletor `t…` — só se o `c:`/`s:` dele venceu aqui.
            let typed = f.variable.is_none()
                && !matches!(f.kind, FunctionKind::Getter)
                && super::typed_entry::needs_typed_entry(ctx, fid);
            // Fora do mundo fechado do programa (C7, `native_world.rs`): o
            // seletor fica tomado (uma superclasse não preenche o lugar com
            // outro membro), mas sem entrada nem adaptador — a chamada
            // dinâmica a ele cai no `noSuchMethod`, como a que o mundo diz
            // que não acontece.
            let live = !ctx.pruned_member(fid);
            let mut by = |ty: Tipo, a: Adapter, base: &str| {
                let s = selector_string(ctx, ty, name_str, f.library);
                if seen_set.insert(s.clone()) && live {
                    if typed && a != Adapter::Read {
                        exit.push((super::typed_entry::typed_selector(&s), format!("{base}$t{}", &a.suffix()[1..])));
                    }
                    exit.push((s, format!("{base}{}", a.suffix())));
                }
            };
            if let Some(v) = f.variable {
                let base = field_symbol(ctx, v);
                if is_setter {
                    by(Tipo::Write, Adapter::Write, &base);
                } else {
                    by(Tipo::Read, Adapter::Read, &base);
                    by(Tipo::Call, Adapter::Call, &base);
                }
                continue;
            }
            let base = super::symbol_of(ctx, fid);
            match f.kind {
                FunctionKind::Setter => by(Tipo::Write, Adapter::Write, &base),
                FunctionKind::Getter => {
                    by(Tipo::Read, Adapter::Read, &base);
                    by(Tipo::Call, Adapter::Call, &base);
                }
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {}
                _ => {
                    by(Tipo::Call, Adapter::Call, &base);
                    by(Tipo::Read, Adapter::Read, &base);
                }
            }
        }
    }
    exit
}

/// A classe (ou uma superclasse do programa) declara `noSuchMethod`: o de
/// `Object` não conta.
fn has_own_no_such_method(ctx: &Context, cid: ClassId) -> bool {
    let Some(nsm) = ctx.interner.lookup("noSuchMethod") else { return false };
    for c in linearization(ctx, cid) {
        if Some(c) == ctx.core.object_class {
            return false;
        }
        if let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&nsm)
            && implemented(ctx, f.0 as usize)
        {
            return true;
        }
    }
    false
}

/// Os membros de instância das superinterfaces de `cid` (superclasses,
/// mixins, `implements`, `on`, transitivamente) cujo seletor a tabela não
/// tem: (seletor, uso, membro declarado).
fn members_to_forward(ctx: &Context, cid: ClassId, table: &[(String, String)]) -> Vec<(String, Tipo, usize)> {
    let mut seen_set: std::collections::HashSet<String> = table.iter().map(|(s, _)| s.clone()).collect();
    let mut exit = Vec::new();
    let mut stack = vec![cid];
    let mut classes = std::collections::HashSet::new();
    while let Some(c) = stack.pop() {
        if !classes.insert(c) || Some(c) == ctx.core.object_class {
            continue;
        }
        let class = &ctx.program.classes[c.0 as usize];
        stack.extend(class.supertype_class);
        stack.extend(class.mixin_classes.iter().copied());
        stack.extend(class.interface_classes.iter().copied());
        stack.extend(class.on_classes.iter().copied());
        let mut members: Vec<(&str, usize)> =
            class.instance_members.iter().map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize)).collect();
        members.sort();
        for (key, fid) in members {
            let f = &ctx.program.functions[fid];
            if f.static_ || matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
                continue;
            }
            let name_str = key.strip_suffix("_=").unwrap_or(key);
            let uses: &[Tipo] = match f.kind {
                FunctionKind::Setter => &[Tipo::Write],
                FunctionKind::Getter => &[Tipo::Read],
                FunctionKind::ImplicitAccessor if key.ends_with("_=") => &[Tipo::Write],
                FunctionKind::ImplicitAccessor => &[Tipo::Read],
                _ => &[Tipo::Call],
            };
            for &use_site in uses {
                let s = selector_string(ctx, use_site, name_str, f.library);
                if seen_set.insert(s.clone()) {
                    exit.push((s, use_site, fid));
                }
            }
        }
    }
    exit
}

/// O encaminhador de `noSuchMethod` do membro `fid` (entrada uniforme):
/// desempacota os argumentos como o membro declara (os padrões dos
/// opcionais omitidos), e o runtime monta o `Invocation` na ordem da
/// declaração e chama o `noSuchMethod` do receptor.
fn generate_nsm_forwarder(ctx: &Context, module: &mut Module, unit: UnitId, symbol: &str, fid: usize, ty: Tipo) {
    let f = &ctx.program.functions[fid];
    let name_str = ctx.symbol_name(f.name).to_string();
    let mut b = FnBuilder::new(ctx, unit, symbol.to_string(), name_str.clone(), Type::Ref);
    let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
    let args = Operand::Val(b.add_param_vector(DartEntryVector::Argumentos));
    let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
    b.declare_arc_uniform_return();
    let (code, values, names, tuple) = match ty {
        Tipo::Call => {
            let infos = b.function_params(fid);
            let Some(vals) = b.unpack(&infos, args.clone(), desc.clone()) else {
                b.finalizar(module);
                return;
            };
            let names: Vec<String> = infos
                .iter()
                .filter(|p| p.kind == ParameterKind::Named)
                .map(|p| p.name.clone().unwrap_or_default())
                .collect();
            // A tupla do método genérico vem depois dos argumentos.
            let tuple = if b.generic_function(fid) {
                let npos = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
                let nnom = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
                let i = b.emit(Instruction::Add(npos, nnom), Type::I64);
                b.emit(Instruction::LoadIndexed { base: args.clone(), index: i }, Type::I64)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            (0, vals, names, tuple)
        }
        Tipo::Read => (1, Vec::new(), Vec::new(), Operand::Constant(Constant::Int(0))),
        Tipo::Write => {
            let v = b.emit(Instruction::LoadIndexed { base: args.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
            (2, vec![v], Vec::new(), Operand::Constant(Constant::Int(0)))
        }
    };
    let npos = values.len() - names.len();
    let nnom = names.len();
    // `dartforge_encaminhar_nsm` lê todas as capturas como `Ref`: os valores
    // vão encaixotados (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 48).
    let mut fields: Vec<Operand> = values.into_iter().map(|v| b.coerce(v, Type::Ref)).collect();
    for n in names {
        fields.push(b.emit(Instruction::Const(Constant::String(n)), Type::Ref));
    }
    let env = b.emit(Instruction::AllocEnv { values: fields }, Type::Ref);
    let text = b.emit(Instruction::Const(Constant::String(name_str)), Type::Ref);
    let r = b.emit_call_with_check(
        Instruction::CallRuntime {
            name: "dartforge_encaminhar_nsm".to_string(),
            args: vec![
                (this, Type::Ref),
                (Operand::Constant(Constant::Int(code)), Type::I64),
                (text, Type::Ref),
                (env, Type::Ref),
                (Operand::Constant(Constant::Int(npos as i64)), Type::I64),
                (Operand::Constant(Constant::Int(nnom as i64)), Type::I64),
                (tuple, Type::I64),
            ],
            ret_ty: Type::Ref,
        },
        Type::Ref,
    );
    b.terminate(Terminator::Return(Some(r)));
    b.finalizar(module);
}

/// As entradas uniformes de um membro de instância (ou campo) do módulo.
pub fn lower_function_adapters(ctx: &Context, module: &mut Module, fid: usize) {
    let f = &ctx.program.functions[fid];
    if f.static_ || f.class.is_none() || !implemented(ctx, fid) {
        return;
    }
    if matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) || f.factory {
        return;
    }
    if f.variable.is_some() {
        return;
    }
    let base = super::symbol_of(ctx, fid);
    let adapters: &[Adapter] = match f.kind {
        FunctionKind::Setter => &[Adapter::Write],
        FunctionKind::Getter => &[Adapter::Read, Adapter::Call],
        _ => &[Adapter::Call, Adapter::Read],
    };
    // A entrada tipada (`$tc`, `$ts`): só confere os covariantes
    // (`typed_entry.rs`), e só existe quando dispensa alguma conferência.
    let mut list: Vec<(Adapter, bool)> = adapters.iter().map(|&a| (a, false)).collect();
    if !matches!(f.kind, FunctionKind::Getter) && super::typed_entry::needs_typed_entry(ctx, fid) {
        list.extend(adapters.iter().filter(|a| **a != Adapter::Read).map(|&a| (a, true)));
    }
    for (a, typed) in list {
        let symbol = if typed { format!("{base}$t{}", &a.suffix()[1..]) } else { format!("{base}{}", a.suffix()) };
        let unit = unit_of(ctx, f.class.expect("membro de classe"));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, symbol, ctx.symbol_name(f.name).to_string(), Type::Ref);
        b.in_adapter = true;
        b.guaranteed_arity = typed;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
        b.declare_arc_uniform_return();
        let span = Span { start: 0, end: 0 };
        let r = match (a, f.kind) {
            (Adapter::Read, FunctionKind::Getter) => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.call_direct(fid, Some(recv), Vec::new())
            }
            (Adapter::Call, FunctionKind::Getter) => {
                let v = b.call_direct(fid, Some(recv), Vec::new());
                let v = b.coerce(v, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureForward { arc_owned: false, closure: v, args, desc }, Type::Ref)
            }
            (Adapter::Read, _) => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.method_tearoff(recv, fid, span)
            }
            _ => {
                let infos = b.function_params(fid);
                let Some(vals) = b.unpack(&infos, args.clone(), desc.clone()) else {
                    b.finalizar(module);
                    continue;
                };
                // A VM confere os argumentos na entrada do método, antes do
                // corpo (e do native, no SDK: `String.+(String)` não pode
                // receber um Smi como handle de String via `dynamic`): os
                // covariantes de classe e os de tipo nominal, com a mensagem
                // " of 'nome'" — antes de converter cada um à representação.
                b.this_param = Some(recv.clone());
                b.finalizable_this = ctx.classify_this(fid);
                b.enclosing_class = f.class;
                b.check_entry_arguments_with(fid, &vals, typed);
                let reprs: Vec<Type> = ctx.outline.functions[fid].parameters.iter().map(|p| b.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| b.coerce(v, r)).collect();
                if b.generic_function(fid) {
                    let npos = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) },
                        Type::I64,
                    );
                    let nnom = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) },
                        Type::I64,
                    );
                    let index = b.emit(Instruction::Add(npos, nnom), Type::I64);
                    b.armed_tuple = Some(b.emit(Instruction::LoadIndexed { base: args.clone(), index: index }, Type::I64));
                }
                b.call_direct(fid, Some(recv), vals)
            }
        };
        let r = if matches!(b.operand_type(&r), Type::Void) { Operand::Constant(Constant::Null) } else { b.coerce(r, Type::Ref) };
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
    }
}

/// As entradas uniformes de um campo de instância: `$g`, `$c` e, se ele tem
/// setter, `$s`.
pub fn lower_field_adapters(ctx: &Context, module: &mut Module, vid: VariableId) {
    let v = &ctx.program.variables[vid.0 as usize];
    let Some(cid) = v.class else { return };
    if v.static_ {
        return;
    }
    let Some(unit) = unit_of(ctx, cid) else { return };
    let base = field_symbol(ctx, vid);
    let span = Span { start: 0, end: 0 };
    let mut types = vec![Adapter::Read, Adapter::Call];
    if v.setter.is_some() {
        types.push(Adapter::Write);
    }
    for a in types {
        let mut b = FnBuilder::new(ctx, unit, format!("{base}{}", a.suffix()), ctx.symbol_name(v.name).to_string(), Type::Ref);
        b.in_adapter = true;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
        b.declare_arc_uniform_return();
        let r = match a {
            Adapter::Read => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.read_field_with_late(recv, vid, span)
            }
            Adapter::Call => {
                let x = b.read_field_with_late(recv, vid, span);
                let x = b.coerce(x, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureForward { arc_owned: false, closure: x, args, desc }, Type::Ref)
            }
            Adapter::Write => {
                let one = [super::closures::EntryParam {
                    name: None,
                    kind: ParameterKind::Required,
                    required: true,
                    pattern: super::closures::Padrao::Nenhum,
                }];
                let Some(vals) = b.unpack(&one, args, desc) else {
                    b.finalizar(module);
                    continue;
                };
                b.store_field(recv, vid, vals[0].clone(), span);
                Operand::Constant(Constant::Null)
            }
        };
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
    }
}

/// A unidade que declara a classe (onde os adaptadores dela são baixados).
fn unit_of(ctx: &Context, cid: ClassId) -> Option<dartforge_elements::model::UnitId> {
    let c = &ctx.program.classes[cid.0 as usize];
    c.decl.map(|d| d.unit).or_else(|| ctx.program.library(c.library).units.first().copied())
}

/// A função no lugar de um membro do SDK recusado: a mesma assinatura, e o
/// corpo avisa em tempo de execução qual membro e por quê
/// (`dartforge_membro_recusado`) — nunca uma saída errada.
pub fn rejected_function(ctx: &Context, fid: usize, reason: &str) -> Function {
    let f = &ctx.program.functions[fid];
    let unit = match f.node {
        dartforge_elements::model::FunctionRef::Function { unit, .. }
        | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => Some(unit),
        dartforge_elements::model::FunctionRef::None => f.class.and_then(|c| unit_of(ctx, c)),
    }
    .or_else(|| ctx.program.library(f.library).units.first().copied())
    .expect("biblioteca com unidade");
    let generative = super::generative_constructor(ctx, fid);
    let ret = if generative {
        Type::Void
    } else {
        ctx.hir_return(fid)
    };
    let symbol = super::symbol_of(ctx, fid);
    let mut b = FnBuilder::new(ctx, unit, symbol.clone(), ctx.symbol_name(f.name).to_string(), ret);
    let with_this = (f.class.is_some() && !f.static_ && !f.factory) || generative;
    if with_this {
        b.add_param("this".to_string(), Type::Ref);
    }
    if let Some(d) = ctx.outline.functions.get(fid) {
        for p in &d.parameters {
            let t = b.repr(p.ty);
            b.add_param("p".to_string(), t);
        }
    }
    let text = format!("{symbol} ({reason})");
    let t = b.emit(Instruction::Const(Constant::String(text)), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// Os fatos ARC de um corpo de recusa (que nunca volta, mas mantém a
/// assinatura do membro): o retorno Ref segue a convenção Owned e os
/// parâmetros não-Ref, na representação do tipo Dart, são escalares.
fn register_rejection_facts(ctx: &Context, module: &mut Module, f: &Function) {
    if !ctx.arc_memory {
        return;
    }
    if f.return_ty == Type::Ref {
        module.dart_ref_returns.insert(f.symbol.clone());
    }
    let scalars: std::collections::HashSet<ValueId> = f
        .params
        .iter()
        .filter(|(_, _, t)| matches!(t, Type::I64 | Type::F64 | Type::I1 | Type::I8))
        .map(|(v, _, _)| *v)
        .collect();
    if !scalars.is_empty() {
        module.dart_scalar_parameters.entry(f.symbol.clone()).or_default().extend(scalars);
    }
}

/// Baixa uma função do SDK da fonte num módulo à parte; se ela não baixa
/// (diagnóstico de construto, native pendente, intrínseco, pânico do
/// lowering, problema do verificador), o módulo recebe no lugar dela a
/// função recusada (`rejected_function`), e o motivo vai para
/// `Module::rejected`.
pub fn lower_function_or_reject(ctx: &Context, module: &mut Module, fid: usize) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        super::lower_function(ctx, &mut m, fid);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    let reason = match &r {
        Ok(m) if m.errors.is_empty() => None,
        Ok(m) => Some(
            m.errors[0]
                .strip_prefix(crate::UNSUPPORTED_PREFIX)
                .unwrap_or(&m.errors[0])
                .to_string(),
        ),
        Err(p) => Some(format!(
            "pânico do lowering: {}",
            p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        )),
    };
    match (r, reason) {
        (Ok(mut m), None) => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
        }
        (_, Some(reason)) => {
            if !super::members::has_body(ctx, fid) {
                return;
            }
            let short = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
            let symbol = super::symbol_of(ctx, fid);
            let f = rejected_function(ctx, fid, &short);
            register_rejection_facts(ctx, module, &f);
            module.functions.push(f);
            // No resumo vai o diagnóstico inteiro, com a posição.
            module.rejected.push((symbol, reason));
        }
        (Err(_), None) => unreachable!(),
    }
}

/// As funções que devolvem os valores padrão não literais dos parâmetros
/// das funções do módulo (`members::pattern_symbol`): quem chama de fora
/// do módulo (o programa) não tem as resoluções das unidades do SDK e chama
/// esta função no lugar de baixar a expressão.
pub fn lower_sdk_patterns(ctx: &Context, module: &mut Module) {
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        if !ctx.library_in_module(f.library) {
            continue;
        }
        let unit = match f.node {
            dartforge_elements::model::FunctionRef::Function { unit, .. }
            | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => unit,
            dartforge_elements::model::FunctionRef::None => continue,
        };
        let base_symbol = super::symbol_of(ctx, fid);
        let b = FnBuilder::new(ctx, unit, String::new(), String::new(), Type::Ref);
        let Some((unit_ref, params)) = b.parameters_of(fid) else { continue };
        let exported: Vec<usize> = params
            .iter()
            .enumerate()
            .filter(|(_, p)| p.default_value.is_some_and(|e| !super::members::literal_pattern(&ctx.program.unit(unit_ref).ast, e)))
            .map(|(i, _)| i)
            .collect();
        drop(b);
        for i in exported {
            let symbol = format!("{base_symbol}.padrao.{i}");
            let mut b = FnBuilder::new(ctx, unit, symbol.clone(), symbol.clone(), Type::Ref);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let v = b.default_value(fid, i).unwrap_or(Operand::Constant(Constant::Null));
                let v = b.coerce(v, Type::Ref);
                b.terminate(Terminator::Return(Some(v)));
                b
            }));
            match r {
                Ok(mut b) if b.errors.is_empty() => {
                    // Corpo Dart chamado pelo módulo do parâmetro: o retorno
                    // Ref é Owned.
                    if ctx.arc_memory {
                        b.dart_ref_returns.insert(symbol.clone());
                    }
                    b.finalizar(module);
                }
                Ok(b) => {
                    let reason = b.errors[0].clone();
                    module.functions.push(rejected_pattern(ctx, unit, &symbol, &reason));
                    module.rejected.push((symbol, reason));
                }
                Err(_) => {
                    let reason = "pânico ao baixar o valor padrão".to_string();
                    module.functions.push(rejected_pattern(ctx, unit, &symbol, &reason));
                    module.rejected.push((symbol, reason));
                }
            }
        }
    }
}

/// A função de padrão que não baixa: avisa em tempo de execução, como as
/// funções recusadas.
fn rejected_pattern(ctx: &Context, unit: dartforge_elements::model::UnitId, symbol: &str, reason: &str) -> Function {
    let mut b = FnBuilder::new(ctx, unit, symbol.to_string(), symbol.to_string(), Type::Ref);
    let short = reason.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(reason);
    let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({short})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// Os adaptadores dos membros de instância das classes do módulo e as
/// tabelas de métodos das classes concretas dele (SDK da fonte).
pub fn lower_adapters_and_tables(ctx: &Context, module: &mut Module) {
    lower_sdk_patterns(ctx, module);
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        let name_str = ctx.symbol_name(f.name);
        if f.class.is_none() && name_str.starts_with("_dartforge") && ctx.library_in_module(f.library) {
            module.helpers.push((name_str.to_string(), super::symbol_of(ctx, fid)));
        }
    }
    for (k, class) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.library_in_module(class.library) {
            continue;
        }
        let mut fids: Vec<usize> = class.instance_members.values().map(|f| f.0 as usize).collect();
        fids.sort_unstable();
        fids.dedup();
        for fid in fids {
            // Fora do mundo fechado (C7): sem entrada na tabela, sem adaptador.
            if ctx.program.functions[fid].class != Some(cid) || ctx.pruned_member(fid) {
                continue;
            }
            adapters_or_reject(ctx, module, |m| lower_function_adapters(ctx, m, fid));
        }
        for &vid in &class.fields {
            if ctx.program.variables[vid.0 as usize].static_ || ctx.pruned_field(vid) {
                continue;
            }
            adapters_or_reject(ctx, module, |m| lower_field_adapters(ctx, m, vid));
        }
        let concrete = !class.modifiers.abstract_ && !super::members::is_mixin(ctx, cid);
        if concrete && let Some(id) = ctx.class_id_of(cid) {
            let mut table = method_table(ctx, cid);
            // A VM implementa `_StackTrace.toString` em C++, sem declaração
            // Dart na classe. Nosso objeto guarda a string no campo zero, ou
            // os endereços do `throw` que o runtime simboliza no primeiro
            // pedido (o rastro simbólico, §13.14).
            if ctx.symbol_name(class.name) == "_StackTrace"
                && ctx.program.library(class.library).uri == "dart:core"
                && let Some(u) = unit_of(ctx, cid)
            {
                let symbol = "df.$stackTrace.toString$c".to_string();
                let mut b = FnBuilder::new(ctx, u, symbol.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Argumentos);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let text = b.emit(Instruction::CallRuntime {
                    name: "dartforge_rastro_texto".to_string(),
                    args: vec![(this, Type::Ref)],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(text)));
                b.finalizar(module);
                table.retain(|(s, _)| s != "c:toString");
                table.push(("c:toString".to_string(), symbol));
            }
            // O layout gerado para enums guarda `_Enum._name` no campo 1.
            // Extensões do SDK (byName/asNameMap) leem esse campo pelo
            // seletor privado de dart:core, inclusive em enums do usuário.
            if super::enums::is_enum(ctx, cid)
                && let Some(decl) = class.decl
            {
                let symbol = format!(
                    "df.{}.{}.$enumName$g",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "_name".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Argumentos);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let name_str = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(1)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(name_str)));
                b.finalizar(module);
                table.retain(|(s, _)| s != "g:_name@dart:core");
                table.push(("g:_name@dart:core".to_string(), symbol));
                // `index` (campo 0) pelo seletor: `x.index` com `x` de tipo
                // `Enum` ou `T extends Enum` (o `EnumSet.updated` do
                // analyzer) chega pela tabela, não pelo tipo estático.
                let symbol = format!(
                    "df.{}.{}.$enumIndex$g",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "index".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Argumentos);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let index = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::I64,
                }, Type::I64);
                let index = b.coerce(index, Type::Ref);
                b.terminate(Terminator::Return(Some(index)));
                b.finalizar(module);
                table.retain(|(s, _)| s != "g:index");
                table.push(("g:index".to_string(), symbol));
            }
            // Um enum sem override recebe `Enum.name` do emissor. A tabela
            // por seletor deve usar esse mesmo corpo, inclusive quando o
            // valor é impresso por `List.toString` da fonte.
            if super::enums::is_enum(ctx, cid)
                && module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_none())
                && let Some(decl) = class.decl
            {
                let base = format!(
                    "df.{}.{}.toString",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let symbol = format!("{base}$c");
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Argumentos);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let text = b.emit_call_with_check(
                    Instruction::CallStatic { symbol: base, args: vec![this], ret_ty: Type::Ref },
                    Type::Ref,
                );
                b.terminate(Terminator::Return(Some(text)));
                b.finalizar(module);
                table.retain(|(s, _)| s != "c:toString");
                table.push(("c:toString".to_string(), symbol));
            }
            // Encaminhadores de `noSuchMethod`: a classe com `noSuchMethod`
            // próprio responde a todo membro das suas interfaces que não
            // implementa (a VM gera o encaminhador no kernel).
            if let Some(decl) = class.decl
                && has_own_no_such_method(ctx, cid)
            {
                for (selector, ty, fid) in members_to_forward(ctx, cid, &table) {
                    let symbol = format!(
                        "df.{}.{}.$nsm.{}",
                        crate::context::escape_str(&ctx.library_name(class.library)),
                        crate::context::escape_str(ctx.symbol_name(class.name)),
                        crate::context::escape_str(&selector)
                    );
                    generate_nsm_forwarder(ctx, module, decl.unit, &symbol, fid, ty);
                    table.push((selector, symbol));
                }
            }
            module.method_tables.push((id, table_symbol(ctx, cid), table));
        }
    }
    // A função da tabela de toda classe concreta compilada (a alocação, em
    // qualquer módulo, registra a tabela da classe).
    for (k, class) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.compiled_library(class.library) || class.modifiers.abstract_ || super::members::is_mixin(ctx, cid) {
            continue;
        }
        if let Some(id) = ctx.class_id_of(cid) {
            module.table_functions.insert(id, table_symbol(ctx, cid));
        }
    }
}

/// A função que devolve a tabela de métodos de uma classe:
/// `df.mt.<library>.<Classe>`.
pub fn table_symbol(ctx: &Context, cid: ClassId) -> String {
    let c = &ctx.program.classes[cid.0 as usize];
    format!(
        "df.mt.{}.{}",
        crate::context::escape_str(&ctx.library_name(c.library)),
        crate::context::escape_str(ctx.symbol_name(c.name))
    )
}

/// Os adaptadores gerados por `gerar`; os que não baixam viram entradas que
/// avisam em tempo de execução.
fn adapters_or_reject(ctx: &Context, module: &mut Module, gerar: impl FnOnce(&mut Module)) {
    let mut m = Module::new();
    m.sdk_mode = true;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gerar(&mut m)));
    if r.is_ok() && m.errors.is_empty() {
        module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
        module.globals.extend(m.globals);
        return;
    }
    let reason = m
        .errors
        .first()
        .map(|e| e.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(e).to_string())
        .unwrap_or_else(|| "pânico do lowering".to_string());
    let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
    // Cada entrada uniforme construída e a tipada correspondente
    // (`$tc`/`$ts`, `typed_entry.rs`), que a tabela pode citar mesmo sem
    // ter sido construída antes do erro.
    let mut entries: Vec<(String, String)> = Vec::new();
    for f in &m.functions {
        let s = &f.symbol;
        if !(s.ends_with("$c") || s.ends_with("$g") || s.ends_with("$s") || s.ends_with("$tc") || s.ends_with("$ts")) {
            continue;
        }
        let typed = (s.ends_with("$c") || s.ends_with("$s")) && !s.ends_with("$tc") && !s.ends_with("$ts");
        for symbol in [Some(s.clone()), typed.then(|| format!("{}$t{}", &s[..s.len() - 2], &s[s.len() - 1..]))].into_iter().flatten() {
            if !entries.iter().any(|(x, _)| *x == symbol) {
                entries.push((symbol, f.name.clone()));
            }
        }
    }
    for (symbol, name_str) in entries {
        let unit = ctx.program.units.iter().position(|_| true).map(|u| dartforge_elements::model::UnitId(u as u32));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, symbol.clone(), name_str, Type::Ref);
        b.add_param("this".to_string(), Type::Ref);
        b.add_param_vector(DartEntryVector::Argumentos);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({reason})"))), Type::Ref);
        b.emit(
            Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
            Type::Void,
        );
        b.terminate(Terminator::Unreachable);
        module.rejected.push((symbol, reason.clone()));
        // Com os fatos da ABI uniforme (vetores e retorno) declarados acima.
        b.finalizar(module);
    }
}
/// O getter de um campo `late` com inicializador do SDK da fonte, ou a
/// recusa dele: `build` baixa o getter num módulo à parte; se ele tem
/// diagnóstico (ou o lowering entra em pânico), fica no lugar a função que
/// avisa em tempo de execução.
pub fn lower_late_getter_or_reject(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    build: impl Fn(&mut Module),
) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        build(&mut m);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    let symbol = super::late_field_getter_symbol(ctx, vid);
    let reason = match r {
        Ok(mut m) if m.errors.is_empty() => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
            return;
        }
        Ok(m) => m.errors[0].strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(&m.errors[0]).to_string(),
        Err(_) => "pânico do lowering".to_string(),
    };
    let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let mut b = FnBuilder::new(ctx, unit, symbol.clone(), name_str, Type::Ref);
    b.add_param("this".to_string(), Type::Ref);
    let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({reason})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    register_rejection_facts(ctx, module, &b.func);
    module.functions.push(b.func);
    module.rejected.push((symbol, reason));
}

/// O getter preguiçoso de um global do SDK da fonte (ou a recusa dele) e o
/// setter `<getter>$set`, que outro módulo chama para gravar o global.
/// O global do programa fora do mundo fechado (C7, `native_world.rs`): o
/// getter lança (`FnBuilder::pruned_body`); o setter é o de sempre.
pub fn lower_pruned_global(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::global_symbol(ctx, vid);
    let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
    b.pruned_body();
    b.finalizar(module);
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), name_str, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    // O valor chega na representação do tipo Dart da global: escalar se não é Ref.
    if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
        s.dart_scalar_parameters.entry(s.func.symbol.clone()).or_default().insert(v);
    }
    s.store_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finalizar(module);
}

pub fn lower_global_or_reject(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::global_symbol(ctx, vid);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
        b.lower_global_getter(vid, repr);
        b.finalizar(&mut m);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    match r {
        Ok(mut m) if m.errors.is_empty() => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
        }
        r => {
            let reason = match r {
                Ok(m) => m.errors[0].strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(&m.errors[0]).to_string(),
                Err(_) => "pânico do lowering".to_string(),
            };
            let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
            let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
            let t = b.emit(Instruction::Const(Constant::String(format!("{getter} ({reason})"))), Type::Ref);
            b.emit(
                Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
                Type::Void,
            );
            b.terminate(Terminator::Unreachable);
            register_rejection_facts(ctx, module, &b.func);
            module.functions.push(b.func);
            module.rejected.push((getter.clone(), reason));
        }
    }
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), name_str, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    // O valor chega na representação do tipo Dart da global: escalar se não é Ref.
    if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
        s.dart_scalar_parameters.entry(s.func.symbol.clone()).or_default().insert(v);
    }
    s.store_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finalizar(module);
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `{}` com o SDK da fonte: um `_Map` (ou `_Set`) novo da fonte, o
    /// `LinkedHashMap`/`LinkedHashSet` padrão que o literal constrói.
    pub fn source_empty_collection(&mut self, map: bool) -> Operand {
        let name_str = if map { "_Map" } else { "_Set" };
        let span = Span { start: 0, end: 0 };
        let Some(cid) = self.ctx.sdk_class("_compact_hash", name_str) else {
            return self.unsupported(&format!("literal de coleção sem `{name_str}`"), span);
        };
        let empty = self.ctx.interner.lookup("");
        let Some(ctor) = empty.and_then(|v| self.ctx.program.classes[cid.0 as usize].constructors.get(&v).copied()) else {
            return self.unsupported(&format!("`{name_str}()` ausente"), span);
        };
        self.instantiate_evaluated(ctor, &[], span)
    }

    /// `for (x in source) f(x)` pelo protocolo do `Iterator` (§17.7.3):
    /// `iterator`, `moveNext()` e `current`, pelos seletores.
    pub fn iterate_source(&mut self, source: Operand, f: &mut dyn FnMut(&mut Self, Operand)) {
        let it = self.call_by_selector(source, "g:iterator".to_string(), &[]);
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let ok = self.call_by_selector(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coerce(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
        self.set_block(fn_body);
        self.emit_safepoint();
        let x = self.call_by_selector(it, "g:current".to_string(), &[]);
        f(self, x);
        self.terminate(Terminator::Branch(head));
        self.set_block(end);
    }

    /// Liga o elemento corrente de um `for-in` ao alvo dele (variável
    /// declarada, local existente ou padrão), na representação do alvo.
    pub fn bind_for_in_target(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        x: Operand,
        iterable: dartforge_frontend::ast::ExprId,
        span: Span,
    ) {
        use dartforge_frontend::ast::{ExprKind, ForInTarget};
        // Elemento `dynamic` num alvo tipado: o cast implícito de cada
        // elemento (o CFE o põe na atribuição à variável do laço), com a
        // mensagem do cast, antes de converter à representação.
        let dynamic_element = match self.ctx.get_type(self.unit_id, iterable) {
            Some(t) if t == self.ctx.core.dynamic_ => true,
            Some(t) => self.element_type(t) == Some(self.ctx.core.dynamic_),
            None => false,
        };
        match target {
            ForInTarget::Declared { name, ty: annotated, .. } => {
                if dynamic_element && let Some(tid) = annotated {
                    self.check_type_or_throw(ast.ty(*tid), x.clone(), super::rti::CastContext::Implicit);
                    if self.is_terminated() {
                        return;
                    }
                }
                let ty = self.local_repr_of(name.span.start as usize);
                let x = self.coerce(x, ty);
                self.declare_variable(name.sym, name.span.start as usize, ty, x);
            }
            ForInTarget::Expression(e) => {
                if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                    if dynamic_element
                        && let Some(t) = self.ctx.get_type(self.unit_id, *e)
                        && t != self.ctx.core.dynamic_
                        && t != self.ctx.core.object_nullable
                    {
                        let rti = self.type_rti(t);
                        self.cast_rti_in(x.clone(), rti, super::rti::CastContext::Implicit);
                        if self.is_terminated() {
                            return;
                        }
                    }
                    let ty = self.lookup_local(id.sym).map_or(Type::Ref, |l| l.ty);
                    let x = self.coerce(x, ty);
                    self.store_local(id.sym, x);
                } else {
                    self.unsupported("alvo de for-in", span);
                }
            }
            ForInTarget::Pattern { pattern, .. } => {
                self.match_irrefutable(ast, *pattern, x, super::patterns::Linkage::Declare, iterable);
            }
        }
    }

    /// `for (alvo in iterável) body` com o SDK da fonte: o protocolo do
    /// `Iterator`, com `break`/`continue` (e os rótulos) como o `for-in` de
    /// sempre.
    pub fn lower_source_for_in(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
    ) {
        let source = self.lower_expr(ast, iterable);
        // Os rótulos deste laço (os dois caminhos abaixo os usam; um laço
        // aninhado no corpo não pode vê-los como seus).
        let labels = std::mem::take(&mut self.pending_labels);
        // `List<E>`: a lista do runtime percorrida pelo índice, com a
        // conferência de comprimento do `ListIterator`; outra classe que
        // implementa `List` segue pelo `Iterator` dela (abaixo).
        let mut direct_end = None;
        if let Some(super::typed_items::Indexable::Core { write_op }) = self.indexable(self.ctx.get_type(self.unit_id, iterable)) {
            let source = self.coerce(source.clone(), Type::Ref);
            if let Some(end) = self.list_for_in(ast, target, iterable, body, span, source, write_op.unwrap_or(Type::Ref), &labels) {
                direct_end = Some(end);
            }
        }
        let it = self.call_by_selector(source, "g:iterator".to_string(), &[]);
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        for &r in &labels {
            self.labeled_break_targets.insert(r, end);
            self.labeled_continue_targets.insert(r, head);
        }
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let ok = self.call_by_selector(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coerce(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
        self.set_block(fn_body);
        self.emit_safepoint();
        self.break_targets.push(end);
        self.continue_targets.push(head);
        self.open_scope();
        let x = self.call_by_selector(it, "g:current".to_string(), &[]);
        self.bind_for_in_target(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.close_scope();
        self.terminate(Terminator::Branch(head));
        self.break_targets.pop();
        self.continue_targets.pop();
        for &r in &labels {
            self.labeled_break_targets.remove(&r);
            self.labeled_continue_targets.remove(&r);
        }
        self.set_block(end);
        // A saída do laço pelo `Iterator` encontra a do laço direto.
        if let Some(after) = direct_end {
            self.terminate(Terminator::Branch(after));
            self.set_block(after);
        }
    }

    /// O `for-in` sobre uma lista do runtime pelo índice. Deixa o bloco
    /// corrente no caminho do `Iterator` (lista de outra classe) e devolve o
    /// bloco depois do laço, aonde os dois caminhos chegam.
    ///
    /// É o que o `ListIterator` (e o `_FixedSizeArrayIterator`, que nunca vê
    /// o comprimento mudar) faz: a cada volta, comprimento diferente do
    /// inicial lança `ConcurrentModificationError(list)`; índice no fim
    /// termina; senão o elemento `list[i]`.
    #[allow(clippy::too_many_arguments)]
    fn list_for_in(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
        list: Operand,
        repr: Type,
        labels: &[dartforge_intern::SymbolId],
    ) -> Option<BlockId> {
        let class = self.ctx.sdk_class("core", "ConcurrentModificationError")?;
        let empty = self.ctx.interner.lookup("")?;
        let ctor = *self.ctx.program.classes[class.0 as usize].constructors.get(&empty)?;
        // Lista do núcleo pela classe (`@df.class`, cids 8–10 fixos,
        // docs/NATIVO-ESPACO-UNIFICADO.md §2.4): `_List`, `_ImmutableList` ou
        // `_GrowableList`; o comprimento em linha (`@df.lista_len`).
        let cid = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(list.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let since = self.emit(
            Instruction::Sub(cid, Operand::Constant(Constant::Int(i64::from(dartforge_runtime::layout::cid::LIST)))),
            Type::I64,
        );
        let is_list = self.emit(Instruction::ICmp(ICmpOp::Ult, since, Operand::Constant(Constant::Int(3))), Type::I1);
        let direct = self.new_block();
        let iterator = self.new_block();
        let after = self.new_block();
        self.terminate(Terminator::CondBranch { cond: is_list, then_block: direct, else_block: iterator });

        self.set_block(direct);
        let n0 = self.list_len_inline(list.clone());
        let index = self.emit(Instruction::Alloca(Type::I64), Type::Ptr);
        self.emit(Instruction::Store { ptr: index.clone(), val: Operand::Constant(Constant::Int(0)) }, Type::Void);
        let head = self.new_block();
        let changed = self.new_block();
        let checks = self.new_block();
        let fn_body = self.new_block();
        self.terminate(Terminator::Branch(head));

        self.set_block(head);
        // Dentro do laço, o comprimento lido do bloco a cada volta
        // (`@df.lista_len`, o mesmo deslocamento nas três classes).
        let n = self.list_len_inline(list.clone());
        let equal = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), n0), Type::I1);
        self.terminate(Terminator::CondBranch { cond: equal, then_block: checks, else_block: changed });

        self.set_block(changed);
        let error = self.instantiate_evaluated(ctor, &[(None, list.clone())], span);
        if !self.is_terminated() {
            self.emit_throw_op(error);
        }
        if !self.is_terminated() {
            self.terminate(Terminator::Unreachable);
        }

        self.set_block(checks);
        let i = self.emit(Instruction::Load { ptr: index.clone(), ty: Type::I64 }, Type::I64);
        let inside = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch { cond: inside, then_block: fn_body, else_block: after });

        self.set_block(fn_body);
        self.emit_safepoint();
        let x = self.read_list_element(&list, &i, repr);
        let next = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: index, val: next }, Type::Void);
        for &r in labels {
            self.labeled_break_targets.insert(r, after);
            self.labeled_continue_targets.insert(r, head);
        }
        self.break_targets.push(after);
        self.continue_targets.push(head);
        self.open_scope();
        self.bind_for_in_target(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.close_scope();
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(head));
        }
        self.break_targets.pop();
        self.continue_targets.pop();
        for r in labels {
            self.labeled_break_targets.remove(r);
            self.labeled_continue_targets.remove(r);
        }
        self.set_block(iterator);
        Some(after)
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `v is <name>` para uma classe de `dart:core` (`List`, `Map`,
    /// `Record`), pelo grafo de classes do SDK da fonte; null não é.
    pub fn is_core_instance(&mut self, v: Operand, name_str: &str) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let Some(id) = self.ctx.sdk_class("core", name_str).and_then(|c| self.ctx.class_id_of(c)) else {
            return Operand::Constant(Constant::Bool(false));
        };
        let cls = self.emit(
            Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(v.clone(), Type::Ref)], ret_ty: Type::I64 },
            Type::I64,
        );
        let sub = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_is_subclass".to_string(),
                args: vec![(cls, Type::I64), (Operand::Constant(Constant::Int(i64::from(id))), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let sub = self.emit(Instruction::ICmp(ICmpOp::Ne, sub, zero.clone()), Type::I1);
        let non_null = self.emit(Instruction::ICmp(ICmpOp::Ne, v, zero.clone()), Type::I1);
        let r = self.emit(Instruction::And(sub, non_null), Type::I64);
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, zero), Type::I1)
    }

    /// Segue se `cond`, senão desvia para `failure`.
    fn require_source(&mut self, cond: Operand, failure: BlockId) {
        let cond = self.to_bool(cond);
        let ok = self.new_block();
        self.terminate(Terminator::CondBranch { cond, then_block: ok, else_block: failure });
        self.set_block(ok);
    }

    /// Padrão de lista com o SDK da fonte (§19.4.6 "List pattern"): o valor é
    /// um `List`, e `length`, `[]` e `sublist` vão pelo seletor.
    #[allow(clippy::too_many_arguments)]
    pub fn match_source_list(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        elements: &[dartforge_frontend::ast::ListPatternElement],
        value: Operand,
        failure: BlockId,
        linkage: super::patterns::Linkage,
        linked: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origin: dartforge_frontend::ast::ExprId,
    ) {
        use dartforge_frontend::ast::ListPatternElement;
        let v = self.coerce(value, Type::Ref);
        let e = self.is_core_instance(v.clone(), "List");
        self.require_source(e, failure);
        let len = self.call_by_selector(v.clone(), "g:length".to_string(), &[]);
        let len = self.coerce(len, Type::I64);
        let rest = elements.iter().position(|e| matches!(e, ListPatternElement::Rest(_)));
        let n_fixed = elements.len() - usize::from(rest.is_some());
        let ok = self.emit(
            Instruction::ICmp(
                if rest.is_some() { ICmpOp::Sge } else { ICmpOp::Eq },
                len.clone(),
                Operand::Constant(Constant::Int(n_fixed as i64)),
            ),
            Type::I1,
        );
        self.require_source(ok, failure);
        for (i, el) in elements.iter().enumerate() {
            match (el, rest) {
                (ListPatternElement::Pattern(sp), r) => {
                    let idx = match r {
                        Some(r) if i > r => {
                            let after = (elements.len() - i) as i64;
                            self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(after))), Type::I64)
                        }
                        _ => Operand::Constant(Constant::Int(i as i64)),
                    };
                    let x = self.call_by_selector(v.clone(), "c:[]".to_string(), &[(None, idx)]);
                    self.match_pattern(ast, *sp, x, failure, linkage, linked, origin);
                }
                (ListPatternElement::Rest(Some(sp)), _) => {
                    let after = (elements.len() - i - 1) as i64;
                    let end = self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(after))), Type::I64);
                    let sub = self.call_by_selector(
                        v.clone(),
                        "c:sublist".to_string(),
                        &[(None, Operand::Constant(Constant::Int(i as i64))), (None, end)],
                    );
                    self.match_pattern(ast, *sp, sub, failure, linkage, linked, origin);
                }
                (ListPatternElement::Rest(None), _) => {}
            }
        }
    }

    /// Padrão de mapa com o SDK da fonte (§19.4.7 "Map pattern"): o valor é
    /// um `Map`, e cada chave existe (`containsKey`) e casa (`[]`).
    #[allow(clippy::too_many_arguments)]
    pub fn match_source_map(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        entries: &[dartforge_frontend::ast::MapPatternEntry],
        value: Operand,
        failure: BlockId,
        linkage: super::patterns::Linkage,
        linked: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origin: dartforge_frontend::ast::ExprId,
    ) {
        let v = self.coerce(value, Type::Ref);
        let e = self.is_core_instance(v.clone(), "Map");
        self.require_source(e, failure);
        for en in entries {
            let k = self.lower_expr(ast, en.key);
            let has = self.call_by_selector(v.clone(), "c:containsKey".to_string(), &[(None, k.clone())]);
            let has = self.coerce(has, Type::I1);
            self.require_source(has, failure);
            let x = self.call_by_selector(v.clone(), "c:[]".to_string(), &[(None, k)]);
            self.match_pattern(ast, en.value, x, failure, linkage, linked, origin);
        }
    }
}
/// As formas de record com campo nomeado do programa (`records.rs`) com o
/// SDK da fonte: cada uma ganha a tabela de métodos — `toString` e `==`
/// (os gerados), e os membros de `Object` — com os adaptadores.
pub fn record_shape_tables(ctx: &Context, module: &mut Module) {
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else { return };
    let object = ctx.core.object_class.map(|c| method_table(ctx, c)).unwrap_or_default();
    for k in 0..ctx.record_shapes.len() {
        let id = crate::context::SHAPE_BASE_ID + k as u32;
        let base = format!("df.$registro.{k}");
        // toString$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.toString$c"), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param_vector(DartEntryVector::Argumentos);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.toString"), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        // ==$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.$3d$3d$c"), "==".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Argumentos));
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let other = b.emit(Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
        let r = b.emit_call_with_check(
            Instruction::CallStatic {
                symbol: super::records::EQUAL_SYMBOL.to_string(),
                args: vec![recv, other],
                ret_ty: Type::I1,
            },
            Type::I1,
        );
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        // hashCode$g: o hash acompanha a igualdade estrutural do record.
        let mut b = FnBuilder::new(ctx, u, format!("{base}.hashCode$g"), "hashCode".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param_vector(DartEntryVector::Argumentos);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.hashCode"), args: vec![recv], ret_ty: Type::I64 },
            Type::I64,
        );
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        let mut table: Vec<(String, String)> = vec![
            ("c:toString".to_string(), format!("{base}.toString$c")),
            ("c:==".to_string(), format!("{base}.$3d$3d$c")),
            ("g:hashCode".to_string(), format!("{base}.hashCode$g")),
            ("c:hashCode".to_string(), format!("{base}.hashCode$g")),
        ];
        for (s, f) in &object {
            if !table.iter().any(|(x, _)| x == s) {
                table.push((s.clone(), f.clone()));
            }
        }
        module.method_tables.push((id, format!("df.mt.$registro.{k}"), table));
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn separately_lowered_function_keeps_arc_facts() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "class C { final String s; C(this.s); }\nString de(C c, int n) => n > 0 ? c.s : 'x';\nvoid main() { print(de(C('a'), 1)); }\n").unwrap();
            let sdk = crate::sdk_modulo::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, diags_corpos) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&diags_corpos) {
                assert!(!dartforge_types::codes::e_erro_de_compilacao(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            let fid = program.functions.iter().position(|f| interner.resolve(f.name) == "de").unwrap();
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let mut m = Module::new();
                lower_function_or_reject(&ctx, &mut m, fid);
                assert!(m.rejected.is_empty(), "{:?}", m.rejected);
                let symbol = super::super::symbol_of(&ctx, fid);
                assert_eq!(m.dart_ref_returns.contains(&symbol), arc);
                assert_eq!(m.dart_scalar_parameters.get(&symbol).is_some_and(|s| s.len() == 1), arc);
            }
        }).unwrap().join().unwrap();
    }
}

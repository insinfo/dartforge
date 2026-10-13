//! O caminho rápido de `[]`, `[]=` e `length` quando o tipo estático do
//! receptor é uma lista tipada numérica (`Int8List` … `Float64List` de
//! `dart:typed_data`) ou uma `List<E>` do núcleo, e os membros externos de
//! lista tipada feitos em linha (`typed_inline`).
//!
//! Essas classes de `dart:typed_data` são `final` no SDK: o valor é sempre a
//! lista interna (`_Int32List`, também sobre memória de fora), ou uma visão
//! (`_Int32ArrayView`, e a não modificável) do `typed_data_patch.dart`. No
//! espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.5) as três formas têm
//! o comprimento em `h+14` e o endereço dos bytes em `h+22`, invariantes
//! enquanto a lista vive: `@df.tipada_len`/`@df.tipada_dados`
//! (`llvm/ir_typed.rs`) os leem em linha com `!invariant.load`, e o LLVM tira
//! as cargas dos laços (e vetoriza). Na escrita, a visão não modificável
//! (conferida pelo cid, também invariante) dá comprimento 0. Um índice fora dos
//! limites cai no despacho de sempre, com os mesmos erros da VM.
//!
//! `List` não é `final`: uma classe do usuário pode implementá-la. A classe do
//! receptor (`@df.class`) diz se é lista do runtime (`_List`,
//! `_ImmutableList`, `_GrowableList`, cids 8–10); a outra recebe comprimento 0
//! e fica com o despacho. O comprimento está em `h+14` nas três; os elementos,
//! no armazenamento (a própria lista, ou a `_List` em `h+22` da expansível),
//! uma palavra cada a partir de `a+22`, com a forma no `flags` dele (§2.16):
//! `REFS` (cada palavra um `Ref`) ou compacta de `int`, `double` ou `bool`
//! (os bits sem caixa). A leitura confere a forma (salvo nas voltas de N13,
//! que a conferem antes); a gravação direta só com `E` igual a `int`,
//! `double` ou `bool` (que nenhuma classe estende; com outro `E`, a lista pode
//! ser de um subtipo e o `[]=` do SDK confere o valor), numa `_List` ou
//! `_GrowableList` (a `_ImmutableList` não aceita) de forma compacta do `E`
//! (`@df.nucleo_len_gravavel`): um escalar sem barreira.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_types::table::Type as T;
use dartforge_types::TypeId;

impl FnBuilder<'_, '_> {
    /// A lista fixa das voltas em emissão que `alvo` (um local) nomeia (N13).
    pub(super) fn fixed_list_of(&self, ast: &dartforge_frontend::ast::Ast, alvo: dartforge_frontend::ast::ExprId) -> Option<super::fn_builder::FixedList> {
        if self.fixed_lists.is_empty() {
            return None;
        }
        let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(alvo).kind else { return None };
        let super::locals::Mode::Memory(key) = self.lookup_local(n.sym)?.mode else { return None };
        self.fixed_lists.iter().rev().find(|f| f.key == key).cloned()
    }

    /// A lista tipada provada das voltas rápidas em emissão
    /// (`statements::Counted::TypedSet`) que `alvo` (um local) nomeia; com
    /// `index`, só se ele é o contador do laço (`alvo[i]`). O local é
    /// identificado pelo `alloca`, então um homônimo declarado no corpo não
    /// casa.
    pub(super) fn proven_of(
        &self,
        ast: &dartforge_frontend::ast::Ast,
        alvo: dartforge_frontend::ast::ExprId,
        index: Option<dartforge_frontend::ast::ExprId>,
    ) -> Option<super::fn_builder::ProvenTyped> {
        if self.proven_typed.is_empty() {
            return None;
        }
        let address = |e: dartforge_frontend::ast::ExprId| {
            let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(e).kind else { return None };
            match self.lookup_local(n.sym)?.mode {
                super::locals::Mode::Memory(p) => Some(p),
                _ => None,
            }
        };
        let key = address(alvo)?;
        let counter = match index {
            Some(i) => Some(address(i)?),
            None => None,
        };
        self.proven_typed
            .iter()
            .rev()
            .find(|p| p.key == key && counter.as_ref().is_none_or(|c| *c == p.counter))
            .cloned()
    }

    /// O endereço dos elementos e o comprimento de uma lista tipada ou SIMD
    /// (`list`, do tipo estático `ix`), lidos antes das voltas de um laço
    /// versionado; `write`: o comprimento de uma visão não modificável é 0.
    pub(super) fn proven_data(&mut self, list: &Operand, ix: Indexable, write: bool) -> Option<(Operand, Operand)> {
        let list = self.coerce(list.clone(), Type::Ref);
        match ix {
            Indexable::Typed(TypedList { ty: typ, .. }) | Indexable::Simd { ty: typ, .. } => {
                Some(self.typed_data_and_length(&list, typ, write))
            }
            Indexable::Core { .. } => None,
        }
    }
}

/// Um receptor com caminho rápido de índice.
#[derive(Debug, Clone, Copy)]
pub enum Indexable {
    Typed(TypedList),
    /// `List<E>`: a representação do elemento que se grava direto (`E`
    /// igual a `int` ou `double`), ou `None` (só leitura direta).
    Core { write_op: Option<Type> },
    /// `Float32x4List`, `Int32x4List`, `Float64x2List`: o tipo no runtime
    /// e o vetor sem caixa do elemento (16 bytes, lidos e gravados como um
    /// vetor LLVM).
    Simd { ty: i64, k: Type },
}

/// Uma lista tipada numérica: o tipo do elemento no runtime (`TIPO_*` de
/// `runtime/src/typed_data.rs`) e o tipo C do elemento.
#[derive(Debug, Clone, Copy)]
pub struct TypedList {
    pub ty: i64,
    pub element: CType,
}

impl TypedList {
    /// A representação Dart do elemento.
    fn repr(self) -> Type {
        if matches!(self.element, CType::F32 | CType::F64) { Type::F64 } else { Type::I64 }
    }

    /// `[]=` pode gravar direto? A `Uint8ClampedList` satura o valor; ela
    /// fica com o `[]=` do SDK.
    fn direct_write(self) -> bool {
        self.ty != 2
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O caminho rápido de índice do tipo estático `ty` (não anulável), se
    /// houver.
    pub(super) fn indexable(&self, ty: Option<TypeId>) -> Option<Indexable> {
        let T::Interface { class, args, nullable: false } = self.ctx.table.get(ty?) else {
            return None;
        };
        let c = self.ctx.program.classes.get(class.0 as usize)?;
        let library = self.ctx.program.library(c.library).uri.as_str();
        let name_str = self.ctx.symbol_name(c.name);
        if library == "dart:core" && name_str == "List" {
            let write_op = match args.first().map(|&e| self.ctx.table.get(e)) {
                Some(T::Interface { class: e, nullable: false, .. }) => {
                    let e = self.ctx.program.classes.get(e.0 as usize)?;
                    let core = self.ctx.program.library(e.library).uri == "dart:core";
                    match self.ctx.symbol_name(e.name) {
                        "int" if core => Some(Type::I64),
                        "double" if core => Some(Type::F64),
                        "bool" if core => Some(Type::I1),
                        _ => None,
                    }
                }
                _ => None,
            };
            return Some(Indexable::Core { write_op });
        }
        // Sob ARC, cada acesso pelo endereço dos bytes ganha
        // `@df.keep_alive(list)` na preparação (`optimize/arc/derived_set.rs`).
        if library != "dart:typed_data" {
            return None;
        }
        let (typ, element) = match name_str {
            "Int8List" => (0, CType::I8),
            "Uint8List" => (1, CType::U8),
            "Uint8ClampedList" => (2, CType::U8),
            "Int16List" => (3, CType::I16),
            "Uint16List" => (4, CType::U16),
            "Int32List" => (5, CType::I32),
            "Uint32List" => (6, CType::U32),
            "Int64List" => (7, CType::I64),
            "Uint64List" => (8, CType::U64),
            "Float32List" => (9, CType::F32),
            "Float64List" => (10, CType::F64),
            "Float32x4List" => return self.simd_list(11, Type::V4F32),
            "Int32x4List" => return self.simd_list(12, Type::V4I32),
            "Float64x2List" => return self.simd_list(13, Type::V2F64),
            _ => return None,
        };
        Some(Indexable::Typed(TypedList { ty: typ, element }))
    }

    /// `list.fillRange(start, end, value)` de uma lista tipada numérica
    /// (tipo estático `Int8List` … `Float64List`, menos a
    /// `Uint8ClampedList`), com os três argumentos `int`/`int`/elemento não
    /// anuláveis: o runtime preenche a faixa de uma vez
    /// (`dartforge_typed_fill_int`/`_double`); faixa inválida, lista vazia
    /// ou visão não modificável voltam ao `fillRange` do SDK com os mesmos
    /// valores, que lança o erro da VM. As classes de `dart:typed_data` são
    /// `final`: o membro é sempre o do SDK.
    pub(super) fn fill_typed(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        e: dartforge_frontend::ast::ExprId,
    ) -> Option<Operand> {
        use dartforge_frontend::ast::{ExprId, ExprKind};
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else { return None };
        if arguments.args.len() != 3 || arguments.args.iter().any(|a| a.name.is_some()) || !arguments.type_args.is_empty() {
            return None;
        }
        let ExprKind::Property { target: recv, name, null_aware: false } = &ast.expr(*target).kind else {
            return None;
        };
        if self.ctx.symbol_name(name.sym) != "fillRange" {
            return None;
        }
        let Some(Indexable::Typed(l)) = self.indexable(self.ctx.get_type(self.unit_id, *recv)) else {
            return None;
        };
        if !l.direct_write() {
            return None;
        }
        let repr = l.repr();
        let class_of = |s: &Self, x: ExprId| match s.ctx.get_type(s.unit_id, x).map(|t| s.ctx.table.get(t)) {
            Some(T::Interface { class, nullable: false, .. }) => Some(*class),
            _ => None,
        };
        let int = self.ctx.core.int_class;
        let element = if repr == Type::F64 { self.ctx.core.double_class } else { int };
        let [a, b, v] = [arguments.args[0].value, arguments.args[1].value, arguments.args[2].value];
        if int.is_none() || class_of(self, a) != int || class_of(self, b) != int || class_of(self, v) != element {
            return None;
        }
        // Ordem de avaliação do Dart: receptor, depois os argumentos.
        let mut ops = Vec::with_capacity(4);
        for (x, t) in [(*recv, Type::Ref), (a, Type::I64), (b, Type::I64), (v, repr)] {
            let op = self.lower_expr(ast, x);
            if self.is_terminated() {
                return Some(Operand::Constant(Constant::Null));
            }
            ops.push(self.coerce(op, t));
        }
        let name_str = if repr == Type::F64 { "dartforge_typed_fill_double" } else { "dartforge_typed_fill_int" };
        let done = self.emit(
            Instruction::CallRuntime {
                name: name_str.to_string(),
                args: vec![
                    (ops[0].clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(l.ty)), Type::I64),
                    (ops[1].clone(), Type::I64),
                    (ops[2].clone(), Type::I64),
                    (ops[3].clone(), repr),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ne, done, Operand::Constant(Constant::Int(0))), Type::I1);
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: join, else_block: slow });
        self.set_block(slow);
        let args: Vec<super::members::Evaluated> = ops[1..].iter().map(|o| (None, o.clone())).collect();
        self.call_by_name(ops[0].clone(), super::sdk_source::AccessKind::Call, "fillRange", &args);
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(Operand::Constant(Constant::Null))
    }

    /// Uma lista SIMD: o vetor sem caixa não atravessa quadros assíncronos
    /// (lá o elemento fica com o despacho).
    fn simd_list(&self, typ: i64, k: Type) -> Option<Indexable> {
        self.async_state.is_none().then_some(Indexable::Simd { ty: typ, k })
    }


    /// Um ajudante do prelúdio (`llvm/ir_typed.rs`): leitura em linha do
    /// bloco, sem efeitos (`HELPER_EFFECTS`).
    fn typed_helper(&mut self, name_str: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(Instruction::CallRuntime { name: name_str.to_string(), args, ret_ty: ret }, ret)
    }

    /// O comprimento para o caminho rápido, ou 0 se ele não serve: o da lista
    /// tipada (0 na visão não modificável, na escrita), ou o da lista do
    /// runtime (0 para quem não é; na escrita direta, 0 também para a
    /// imutável ou de outra forma).
    fn fast_length(&mut self, list: &Operand, ix: Indexable, write: bool) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let write_op = match ix {
            Indexable::Typed(TypedList { ty: typ, .. }) | Indexable::Simd { ty: typ, .. } => {
                let (data, n) = self.typed_data_and_length(list, typ, write);
                self.typed_data = Some((list.clone(), data));
                return n;
            }
            Indexable::Core { write_op } => write_op,
        };
        let Some(t) = write_op.filter(|_| write) else {
            // O comprimento lógico, em linha; numa lista fixa das voltas
            // (N13), o lido antes delas.
            return match &self.access_fixed {
                Some(f) => f.length.clone(),
                None => self.typed_helper("df.nucleo_len", vec![(list.clone(), Type::Ref)], Type::I64),
            };
        };
        // N14: nas voltas de N13, a gravação direta foi conferida antes delas.
        if let Some(n) = self.access_fixed.as_ref().and_then(|f| f.writable_length.clone()) {
            return n;
        }
        // Gravação direta: `_List` ou `_GrowableList` (a classe diz que é
        // modificável) com o armazenamento na forma compacta do `E` (que só
        // existe com o `E` reificado exatamente `int`, `double` ou `bool`:
        // a covariância está satisfeita).
        self.typed_helper(
            "df.nucleo_len_gravavel",
            vec![(list.clone(), Type::Ref), (int(shape_code(t)), Type::I64)],
            Type::I64,
        )
    }

    /// `list.length`.
    pub(super) fn indexed_length(&mut self, list: Operand, ix: Indexable) -> Operand {
        // Lista tipada provada das voltas rápidas: o comprimento lido antes.
        if let Some(p) = self.access_proven.take()
            && let Indexable::Typed(_) | Indexable::Simd { .. } = ix
        {
            return p.length;
        }
        let list = self.coerce(list, Type::Ref);
        // N13: a lista do runtime das voltas (comprimento não nulo, fixo).
        if let (Indexable::Core { .. }, Some(f)) = (ix, &self.access_fixed) {
            return f.length.clone();
        }
        let n = self.fast_length(&list, ix, false);
        self.typed_data = None;
        if let Indexable::Typed(_) | Indexable::Simd { .. } = ix {
            // O tipo estático garante a lista tipada do tipo: o comprimento
            // é esse.
            return n;
        }
        // 0: vazia, ou não é lista do runtime — o getter responde.
        let empty = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
        let fast_block = self.new_block();
        let slow_block = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: empty, then_block: slow_block, else_block: fast_block });
        self.set_block(fast_block);
        self.terminate(Terminator::Branch(join));
        self.set_block(slow_block);
        let s = self.call_by_name(list, super::sdk_source::AccessKind::Read, "length", &[]);
        let s = self.coerce(s, Type::I64);
        let slow_end = self.current_block;
        if self.is_terminated() {
            self.set_block(join);
            return n;
        }
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(fast_block, n), (slow_end, s)], ty: Type::I64 }, Type::I64)
    }

    /// Desvia para o caminho rápido quando `index` está nos limites da
    /// lista apta, senão para o `slow`; junta os dois resultados na
    /// representação `repr`.
    #[allow(clippy::too_many_arguments)]
    fn branch_indexed(
        &mut self,
        list: &Operand,
        index: &Operand,
        ix: Indexable,
        write: bool,
        repr: Type,
        fast: &mut dyn FnMut(&mut Self) -> Operand,
        slow: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        self.typed_data = None;
        let n = self.fast_length(list, ix, write);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, index.clone(), n), Type::I1);
        let fast_block = self.new_block();
        let slow_block = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fast_block, else_block: slow_block });

        self.set_block(fast_block);
        let r = fast(self);
        self.typed_data = None;
        let r = self.coerce(r, repr);
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));

        self.set_block(slow_block);
        let s = slow(self);
        let s = self.coerce(s, repr);
        let slow_end = self.current_block;
        let slow_arrives = !self.is_terminated();
        if slow_arrives {
            self.terminate(Terminator::Branch(join));
        }

        self.set_block(join);
        if slow_arrives {
            self.emit(Instruction::Phi { incoming: vec![(fast_end, r), (slow_end, s)], ty: repr }, repr)
        } else {
            r
        }
    }

    /// O endereço dos elementos de uma lista tipada apta: o que
    /// `fast_length` leu para o teste de limites que guarda este
    /// acesso, ou `@df.tipada_dados`.
    fn typed_addresses(&mut self, list: &Operand) -> Operand {
        if let Some((l, d)) = &self.typed_data
            && l == list
        {
            return d.clone();
        }
        self.typed_helper("df.tipada_dados", vec![(list.clone(), Type::Ref)], Type::I64)
    }

    /// O endereço dos elementos e o comprimento de uma lista tipada do tipo
    /// `ty` (interna, externa ou visão: os mesmos deslocamentos, §2.5), em
    /// linha e invariantes (`@df.tipada_dados`/`@df.tipada_len`); `write`: 0
    /// na visão não modificável do tipo (`@df.tipada_len_gravavel`, pelo cid).
    /// Antes, um cabeçalho de endereço fixo por chamada pura ao runtime e um
    /// cache por ponto de acesso.
    fn typed_data_and_length(&mut self, list: &Operand, typ: i64, write: bool) -> (Operand, Operand) {
        let data = self.typed_helper("df.tipada_dados", vec![(list.clone(), Type::Ref)], Type::I64);
        let n = if write {
            let immutable = i64::from(dartforge_runtime::layout::cid::visao(typ as u8, true));
            self.typed_helper(
                "df.tipada_len_gravavel",
                vec![(list.clone(), Type::Ref), (Operand::Constant(Constant::Int(immutable)), Type::I64)],
                Type::I64,
            )
        } else {
            self.typed_helper("df.tipada_len", vec![(list.clone(), Type::Ref)], Type::I64)
        };
        (data, n)
    }

    /// `list[index]`, no resultado `repr`. `None` quando o índice não é
    /// um `int` sem caixa (fica o despacho).
    pub(super) fn read_indexed(&mut self, list: Operand, index: Operand, ix: Indexable, repr: Type) -> Option<Operand> {
        if self.operand_type(&index) != Type::I64 {
            return None;
        }
        // `list[i]` nas voltas rápidas de um laço versionado: `i` está
        // provado em `0..length` — a carga, sem teste de limites nem
        // caminho lento (que chamaria o runtime e impediria o LLVM de
        // vetorizar o laço).
        if let Some(p) = self.access_proven.take() {
            match ix {
                Indexable::Typed(l) => {
                    let v = self.emit(Instruction::NativeLoad { address: p.data, index, ty: l.element }, l.repr());
                    return Some(self.coerce(v, repr));
                }
                Indexable::Simd { k, .. } => {
                    let v = self.emit(Instruction::Simd { op: OpSimd::Loads, args: vec![p.data, index] }, k);
                    return Some(self.coerce(v, repr));
                }
                Indexable::Core { .. } => {}
            }
        }
        let list = self.coerce(list, Type::Ref);
        let (list2, index2) = (list.clone(), index.clone());
        let mut fast = |s: &mut Self| match ix {
            Indexable::Typed(l) => {
                let address = s.typed_addresses(&list2);
                s.emit(Instruction::NativeLoad { address, index: index2.clone(), ty: l.element }, l.repr())
            }
            Indexable::Core { .. } => s.read_list_element(&list2, &index2, repr),
            Indexable::Simd { k, .. } => {
                let address = s.typed_addresses(&list2);
                s.emit(Instruction::Simd { op: OpSimd::Loads, args: vec![address, index2.clone()] }, k)
            }
        };
        let mut slow = |s: &mut Self| {
            // O receptor tem tipo estático (a lista indexável): o seletor
            // tipado (`typed_entry.rs`).
            s.call_by_name_with_typed_receiver(list2.clone(), true, super::sdk_source::AccessKind::Call, "[]", &[(None, index2.clone())])
        };
        Some(self.branch_indexed(&list, &index, ix, false, repr, &mut fast, &mut slow))
    }

    /// `list[index] = value`. `false` quando não há caminho rápido (o
    /// chamador faz o despacho).
    pub(super) fn store_indexed(&mut self, list: Operand, index: Operand, value: Operand, ix: Indexable) -> bool {
        let direct = match ix {
            Indexable::Typed(l) => l.direct_write(),
            Indexable::Core { write_op } => write_op.is_some(),
            Indexable::Simd { .. } => true,
        };
        let proven = self.access_proven.take();
        if !direct || self.operand_type(&index) != Type::I64 {
            return false;
        }
        // `list[i] = v` nas voltas rápidas de um laço versionado, com a
        // lista provada também para a gravação: a gravação, sem teste de
        // limites nem caminho lento. `Int32List` etc. guardam os bits baixos
        // (o `trunc` de `NativeWrite`), como o `[]=` da VM.
        if let Some(p) = proven.filter(|p| p.write) {
            match ix {
                Indexable::Typed(l) => {
                    let v = self.coerce(value, l.repr());
                    self.emit(Instruction::NativeWrite { address: p.data, index, ty: l.element, value: v }, Type::Void);
                    return true;
                }
                Indexable::Simd { k, .. } => {
                    let v = self.coerce(value, k);
                    self.emit(Instruction::Simd { op: OpSimd::Writes, args: vec![p.data, index, v] }, Type::Void);
                    return true;
                }
                Indexable::Core { .. } => {}
            }
        }
        let list = self.coerce(list, Type::Ref);
        let (list2, index2, value2) = (list.clone(), index.clone(), value.clone());
        let mut fast = |s: &mut Self| {
            match ix {
                Indexable::Typed(l) => {
                    let address = s.typed_addresses(&list2);
                    let v = s.coerce(value2.clone(), l.repr());
                    s.emit(
                        Instruction::NativeWrite { address, index: index2.clone(), ty: l.element, value: v },
                        Type::Void,
                    );
                }
                Indexable::Core { write_op: Some(t) } => {
                    let v = s.coerce(value2.clone(), t);
                    s.store_list_element(&list2, &index2, v, t);
                }
                Indexable::Core { write_op: None } => unreachable!("gravação direta conferida acima"),
                Indexable::Simd { k, .. } => {
                    let address = s.typed_addresses(&list2);
                    let v = s.coerce(value2.clone(), k);
                    s.emit(
                        Instruction::Simd { op: OpSimd::Writes, args: vec![address, index2.clone(), v] },
                        Type::Void,
                    );
                }
            }
            Operand::Constant(Constant::Int(0))
        };
        let mut slow = |s: &mut Self| {
            // Um vetor SIMD sem caixa só é encaixotado aqui, no caminho lento.
            let value = if s.operand_type(&value).is_vector() { s.coerce(value.clone(), Type::Ref) } else { value.clone() };
            s.call_by_name_with_typed_receiver(
                list2.clone(),
                true,
                super::sdk_source::AccessKind::Call,
                "[]=",
                &[(None, index2.clone()), (None, value)],
            );
            Operand::Constant(Constant::Int(0))
        };
        self.branch_indexed(&list, &index, ix, true, Type::I64, &mut fast, &mut slow);
        true
    }

    /// O armazenamento de uma lista do runtime (a própria `_List` ou
    /// `_ImmutableList`, ou os dados da `_GrowableList`): o endereço do
    /// elemento 0 (`a+22`) e a forma dele (`flags & (FORMA | ELEMENTO)`, o
    /// [`shape_code`]). O chamador sabe que `list` é lista do runtime.
    ///
    /// Sob ARC, o endereço é derivado da lista: a preparação segura a lista
    /// depois de cada acesso, que fica no bloco da leitura, sem chamada entre
    /// os dois (`optimize/arc/derived_set.rs`; o crescimento troca o armazenamento).
    pub(super) fn list_storage(&mut self, list: &Operand) -> (Operand, Operand) {
        let a = self.typed_helper("df.nucleo_armazenamento", vec![(list.clone(), Type::Ref)], Type::I64);
        let shape = self.typed_helper("df.nucleo_forma", vec![(a.clone(), Type::I64)], Type::I64);
        let data = self.emit(Instruction::Add(a, Operand::Constant(Constant::Int(ELEMENTOS_DO_ARMAZENAMENTO))), Type::I64);
        (data, shape)
    }

    /// O comprimento lógico de `list` se ela é lista do runtime, senão 0
    /// (`@df.nucleo_len`).
    pub(super) fn core_length(&mut self, list: &Operand) -> Operand {
        self.typed_helper("df.nucleo_len", vec![(list.clone(), Type::Ref)], Type::I64)
    }

    /// O comprimento de `list` para gravações diretas na forma `shape`
    /// (`@df.nucleo_len_gravavel`): 0 se ela não é `_List`/`_GrowableList`
    /// dessa forma.
    pub(super) fn core_writable_length(&mut self, list: &Operand, shape: i64) -> Operand {
        self.typed_helper(
            "df.nucleo_len_gravavel",
            vec![(list.clone(), Type::Ref), (Operand::Constant(Constant::Int(shape)), Type::I64)],
            Type::I64,
        )
    }

    /// O endereço do elemento 0 e a forma: os conferidos antes das voltas de
    /// N13 (forma conhecida na emissão), ou lidos em linha.
    ///
    /// Com a preparação ARC, o armazenamento é relido em cada acesso: o
    /// caminho lento do `[]` (dentro das voltas) é uma chamada, e um
    /// armazenamento lido antes delas não teria quem o segurasse
    /// (`optimize/arc/derived_set.rs`). A forma conferida antes vale.
    fn elements_and_shape(&mut self, list: &Operand) -> (Operand, Result<i64, Operand>) {
        if let Some(f) = &self.access_fixed {
            let shape = f.shape;
            if self.ctx.arc_memory && crate::alvo::arc_prepared().unwrap_or(false) {
                let (data, _) = self.list_storage(list);
                return (data, Ok(shape));
            }
            return (f.data.clone(), Ok(shape));
        }
        let (data, shape) = self.list_storage(list);
        (data, Err(shape))
    }

    /// O elemento `index` de um armazenamento da forma `k`, na
    /// representação natural dela: o `Ref` (`@df.palavra_ref`, que o
    /// emissor enraíza como qualquer `Ref`), ou os bits do escalar. Em ARC,
    /// a lista geral passa pela leitura conferida que produz um owner.
    fn load_in_shape(&mut self, list: &Operand, data: &Operand, index: &Operand, k: i64) -> Operand {
        match k {
            SHAPE_INT => self.emit(
                Instruction::NativeLoad { address: data.clone(), index: index.clone(), ty: CType::I64 },
                Type::I64,
            ),
            SHAPE_DOUBLE => self.emit(
                Instruction::NativeLoad { address: data.clone(), index: index.clone(), ty: CType::F64 },
                Type::F64,
            ),
            SHAPE_BOOL => {
                let b = self.emit(
                    Instruction::NativeLoad { address: data.clone(), index: index.clone(), ty: CType::I64 },
                    Type::I64,
                );
                self.emit(Instruction::ICmp(ICmpOp::Ne, b, Operand::Constant(Constant::Int(0))), Type::I1)
            }
            _ if self.ctx.arc_memory => self.emit(Instruction::CallRuntime {
                name: arc_list_ref_read().into(),
                args: vec![(list.clone(), Type::Ref), (index.clone(), Type::I64)],
                ret_ty: Type::Ref,
            }, Type::Ref),
            _ => self.typed_helper("df.palavra_ref", vec![(data.clone(), Type::I64), (index.clone(), Type::I64)], Type::Ref),
        }
    }

    /// O elemento `index` pelo `[]` do SDK (a forma que não serve à
    /// representação pedida), em `repr`.
    fn element_by_sdk(&mut self, list: &Operand, index: &Operand, repr: Type) -> Operand {
        let r = self.call_by_name_with_typed_receiver(
            list.clone(),
            true,
            super::sdk_source::AccessKind::Call,
            "[]",
            &[(None, index.clone())],
        );
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        self.coerce(r, repr)
    }

    /// O elemento `index` (já conferido) de uma lista do runtime, na
    /// representação `repr`.
    ///
    /// Na forma compacta de `repr` (`int`, `double`, `bool`), os 8 bytes dos
    /// bits em `data + 8·index`; na lista geral, o `Ref` (desencaixado se
    /// `repr` é escalar). Lido como `Ref`, um elemento compacto sai
    /// encaixotado. O que sobra (uma forma compacta que não é a de `repr`
    /// escalar) vai ao `[]` do SDK. A forma conhecida na emissão (as voltas
    /// de N13) poupa a conferência.
    pub(super) fn read_list_element(&mut self, list: &Operand, index: &Operand, repr: Type) -> Operand {
        let scalar = matches!(repr, Type::I64 | Type::F64 | Type::I1);
        let shapes: Vec<i64> = if scalar {
            vec![shape_code(repr), SHAPE_GENERAL]
        } else {
            vec![SHAPE_GENERAL, SHAPE_INT, SHAPE_DOUBLE, SHAPE_BOOL]
        };
        let (data, shape) = self.elements_and_shape(list);
        let f = match shape {
            Ok(k) if shapes.contains(&k) => {
                let v = self.load_in_shape(list, &data, index, k);
                return self.coerce(v, repr);
            }
            Ok(_) => return self.element_by_sdk(list, index, repr),
            Err(f) => f,
        };
        let join = self.new_block();
        let mut entries = Vec::with_capacity(shapes.len() + 1);
        for k in shapes {
            let ok = self.emit(Instruction::ICmp(ICmpOp::Eq, f.clone(), Operand::Constant(Constant::Int(k))), Type::I1);
            let yes = self.new_block();
            let node = self.new_block();
            self.terminate(Terminator::CondBranch { cond: ok, then_block: yes, else_block: node });
            self.set_block(yes);
            let v = self.load_in_shape(list, &data, index, k);
            let v = self.coerce(v, repr);
            if !self.is_terminated() {
                entries.push((self.current_block, v));
                self.terminate(Terminator::Branch(join));
            }
            self.set_block(node);
        }
        let v = self.element_by_sdk(list, index, repr);
        if !self.is_terminated() {
            entries.push((self.current_block, v));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match entries.len() {
            0 => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
            1 => entries.pop().expect("uma entrada").1,
            _ => self.emit(Instruction::Phi { incoming: entries, ty: repr }, repr),
        }
    }

    /// Grava `value` (`int`, `double` ou `bool`, sem caixa) no elemento
    /// `index` (já conferido) de uma lista do runtime cuja gravação direta
    /// foi conferida (`@df.nucleo_len_gravavel`): o armazenamento é compacto
    /// da forma de `repr`, e o elemento são os 8 bytes dos bits (escalar:
    /// sem barreira).
    fn store_list_element(&mut self, list: &Operand, index: &Operand, value: Operand, repr: Type) {
        let (typ, value) = match repr {
            Type::F64 => (CType::F64, value),
            Type::I1 => {
                let b = self.emit(Instruction::ZExt { op: value, from: Type::I1, to: Type::I64 }, Type::I64);
                (CType::I64, b)
            }
            _ => (CType::I64, value),
        };
        let (data, _) = self.elements_and_shape(list);
        self.emit(Instruction::NativeWrite { address: data, index: index.clone(), ty: typ, value }, Type::Void);
    }
}

/// A leitura ARC de um elemento `Ref` de lista geral: com a preparação
/// publicada, um owner que ela consome; sem ela (o ARC legado, em que
/// ninguém solta tokens do código), o empréstimo da lista.
pub(super) fn arc_list_ref_read() -> &'static str {
    if crate::alvo::arc_prepared().unwrap_or(false) {
        "dartforge_arc_lista_ref_owned_v1"
    } else {
        "dartforge_arc_lista_ref_v1"
    }
}

/// A forma de um armazenamento de lista do runtime: os bits `FORMA` e
/// `ELEMENTO` do `flags` (`layout::flags`, §2.3 e §2.16).
pub(super) const SHAPE_GENERAL: i64 = dartforge_runtime::layout::flags::REFS as i64;
const SHAPE_INT: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_INT) as i64;
const SHAPE_DOUBLE: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_DOUBLE) as i64;
const SHAPE_BOOL: i64 = (dartforge_runtime::layout::flags::BRUTO | dartforge_runtime::layout::flags::ELEMENTO_BOOL) as i64;

/// O elemento 0 de um armazenamento, a partir do handle (`b+24` = `h+22`).
const ELEMENTOS_DO_ARMAZENAMENTO: i64 =
    dartforge_runtime::layout::desl::ELEMENTOS as i64 - dartforge_runtime::layout::DESLOCAMENTO_DO_HANDLE;

/// O código da forma compacta dos elementos de representação `t` (`int`,
/// `double`, `bool`), o que `@df.nucleo_forma` lê do armazenamento; a forma
/// geral para as outras.
pub(super) fn shape_code(t: Type) -> i64 {
    match t {
        Type::I64 => SHAPE_INT,
        Type::F64 => SHAPE_DOUBLE,
        Type::I1 => SHAPE_BOOL,
        _ => SHAPE_GENERAL,
    }
}

/// O tipo de elemento e o tipo C de uma classe de lista tipada numérica do
/// `typed_data_patch.dart` (`_Int8List`, `_Int8ArrayView`,
/// `_UnmodifiableInt8ArrayView`…); `None` para SIMD e o resto.
fn class_element(class: &str) -> Option<(i64, CType)> {
    let base = class.strip_prefix("_Unmodifiable").or_else(|| class.strip_prefix('_'))?;
    let base = base.strip_suffix("ArrayView").or_else(|| base.strip_suffix("List"))?;
    element_of_name(base)
}

/// `Int8` … `Float64` → (o `TIPO_*`, o tipo C).
fn element_of_name(name_str: &str) -> Option<(i64, CType)> {
    Some(match name_str {
        "Int8" => (0, CType::I8),
        "Uint8" => (1, CType::U8),
        "Uint8Clamped" => (2, CType::U8),
        "Int16" => (3, CType::I16),
        "Uint16" => (4, CType::U16),
        "Int32" => (5, CType::I32),
        "Uint32" => (6, CType::U32),
        "Int64" => (7, CType::I64),
        "Uint64" => (8, CType::U64),
        "Float32" => (9, CType::F32),
        "Float64" => (10, CType::F64),
        _ => return None,
    })
}

impl FnBuilder<'_, '_> {
    /// O membro externo de lista tipada ou visão feito em linha (P4,
    /// docs/NATIVO-ESPACO-UNIFICADO.md §3.8), ou `None` (a chamada de sempre):
    ///
    /// * `TypedDataBase_length`, `TypedDataView_offsetInBytes`,
    ///   `TypedDataView_typedData`: a carga do bloco;
    /// * o `[]` reconhecido de cada lista numérica e visão
    ///   (`DartForge_typed_indexar_*`): o teste de limites e a carga, com o
    ///   runtime como caminho lento (que lança o `RangeError` da VM);
    /// * `_TypedList._getX`/`_setX` numéricos (os acessos por bytes do
    ///   `ByteData` e das cópias do patch): o teste `0 <= off`, `off + N <=
    ///   bytes` e a carga ou gravação, com o runtime como caminho lento.
    ///
    /// Os resultados saem na representação natural (`int` `I64`, `double`
    /// `F64`), como `inline_string`. SIMD e `_memMove*` ficam no runtime.
    pub(super) fn typed_inline(
        &mut self,
        member: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let this = this?.clone();
        if self.operand_type(&this) != Type::Ref {
            return None;
        }
        let helper_name = match (native, args.len()) {
            (Some("TypedDataBase_length"), 0) => Some(("df.tipada_len", Type::I64)),
            (Some("TypedDataView_offsetInBytes"), 0) => Some(("df.tipada_deslocamento", Type::I64)),
            (Some("TypedDataView_typedData"), 0) => Some(("df.tipada_base", Type::Ref)),
            (Some(_), _) => return None,
            (None, _) => None,
        };
        if let Some((name_str, ret)) = helper_name {
            return Some(self.typed_helper(name_str, vec![(this, Type::Ref)], ret));
        }
        let slow = crate::natives::intrinsic(member)?;
        if let Some(class) = member.strip_suffix(".[]") {
            let (_, tc) = class_element(class)?;
            if args.len() != 1 {
                return None;
            }
            return Some(self.index_typed_inline(this, args[0].clone(), tc, slow));
        }
        let (writes, name_str) = match member.strip_prefix("_TypedList._get") {
            Some(n) => (false, n),
            None => (true, member.strip_prefix("_TypedList._set")?),
        };
        let (_, tc) = element_of_name(name_str)?;
        if args.len() != usize::from(writes) + 1 {
            return None;
        }
        Some(self.inline_typed_bytes(this, args, tc, writes, slow))
    }

    /// `this[i]` de uma lista numérica ou visão (o elemento `tc`): em linha
    /// quando `0 <= i < length`, senão o intrínseco `slow` do runtime.
    fn index_typed_inline(&mut self, this: Operand, i: Operand, tc: CType, slow: &str) -> Operand {
        let repr = tc.hir_type();
        let i = self.coerce(i, Type::I64);
        let n = self.typed_helper("df.tipada_len", vec![(this.clone(), Type::Ref)], Type::I64);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let fast = self.new_block();
        let slowly = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fast, else_block: slowly });
        self.set_block(fast);
        let data = self.typed_helper("df.tipada_dados", vec![(this.clone(), Type::Ref)], Type::I64);
        let v = self.emit(Instruction::NativeLoad { address: data, index: i.clone(), ty: tc }, repr);
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(slowly);
        let s = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(slow),
                args: vec![(this, Type::Ref), (i, Type::I64)],
                ret_ty: repr,
            },
            repr,
        );
        let slow_end = self.current_block;
        let arrives = !self.is_terminated();
        if arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if arrives {
            self.emit(Instruction::Phi { incoming: vec![(fast_end, v), (slow_end, s)], ty: repr }, repr)
        } else {
            v
        }
    }

    /// `this._getX(off)` / `this._setX(off, v)` (o elemento `tc` de `N`
    /// bytes em `off`, sem alinhamento): em linha quando `off u< bytes` e
    /// `off + N <= bytes`, senão o intrínseco `slow` do runtime (que lança).
    fn inline_typed_bytes(&mut self, this: Operand, args: &[Operand], tc: CType, writes: bool, slow: &str) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let repr = tc.hir_type();
        let off = self.coerce(args[0].clone(), Type::I64);
        let value = if writes { Some(self.coerce(args[1].clone(), repr)) } else { None };
        let bytes = self.typed_helper("df.tipada_bytes", vec![(this.clone(), Type::Ref)], Type::I64);
        let inside = self.emit(Instruction::ICmp(ICmpOp::Ult, off.clone(), bytes.clone()), Type::I1);
        let block_fits = self.new_block();
        let fast = self.new_block();
        let slowly = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: inside, then_block: block_fits, else_block: slowly });
        self.set_block(block_fits);
        let end = self.emit(Instruction::Add(off.clone(), int(tc.c_size() as i64)), Type::I64);
        let fits = self.emit(Instruction::ICmp(ICmpOp::Sle, end, bytes), Type::I1);
        self.terminate(Terminator::CondBranch { cond: fits, then_block: fast, else_block: slowly });

        self.set_block(fast);
        let data = self.typed_helper("df.tipada_dados", vec![(this.clone(), Type::Ref)], Type::I64);
        let address = self.emit(Instruction::Add(data, off.clone()), Type::I64);
        let v = match &value {
            Some(x) => {
                self.emit(Instruction::NativeWrite { address, index: int(0), ty: tc, value: x.clone() }, Type::Void);
                None
            }
            None => Some(self.emit(Instruction::NativeLoad { address, index: int(0), ty: tc }, repr)),
        };
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));

        self.set_block(slowly);
        let mut a = vec![(this, Type::Ref), (off, Type::I64)];
        if let Some(x) = value {
            a.push((x, repr));
        }
        let ret = if writes { Type::Void } else { repr };
        let s = self.emit_call_with_check(Instruction::CallRuntime { name: crate::natives::symbol(slow), args: a, ret_ty: ret }, ret);
        let slow_end = self.current_block;
        let arrives = !self.is_terminated();
        if arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match v {
            None => Operand::Constant(Constant::Null),
            Some(v) if arrives => self.emit(Instruction::Phi { incoming: vec![(fast_end, v), (slow_end, s)], ty: repr }, repr),
            Some(v) => v,
        }
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    use crate::context::Context;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn source_read_produces_owner_only_with_arc() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "Object ler(List<Object> valores, int i) => valores[i]; void main() { ler([1], 0); }\n").unwrap();
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
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                let mut owned = 0; let mut raw_items = 0;
                for (_, inst, ty) in m.functions.iter().filter(|f| f.name == "ler").flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
                    if let Instruction::CallRuntime { name, args, ret_ty } = inst {
                        if name == "dartforge_arc_lista_ref_owned_v1" || name == "dartforge_arc_lista_ref_v1" {
                            owned += 1;
                            assert_eq!((*ret_ty, *ty), (Type::Ref, Type::Ref));
                            assert_eq!(args.iter().map(|a| a.1).collect::<Vec<_>>(), vec![Type::Ref, Type::I64]);
                        }
                        if name == "df.palavra_ref" { raw_items += 1; }
                    }
                }
                if arc { assert!(owned > 0); assert_eq!(raw_items, 0); }
                else { assert_eq!(owned, 0); assert!(raw_items > 0); }
            }
        }).unwrap().join().unwrap();
    }
}

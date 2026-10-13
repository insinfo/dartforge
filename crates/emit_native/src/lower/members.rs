//! Membros de classe no lowering nativo: layout de objetos, campos,
//! chamadas (diretas e com despacho pela classe), construtores e globais.
//!
//! Contrato: `docs/NATIVO-PLANO.md` §6 — R7 (layout e índices pelo elemento
//! resolvido), N5 (construtor como função) e N6 (globais preguiçosos).

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::Element;
use dartforge_elements::model::{
    ClassId, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId,
};
use dartforge_frontend::ast::{self, ExprId, FunctionBody, MemberKind, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::MemberRef;
use dartforge_types::table::{TypeId, TypeParamId};

/// Argumento já avaliado: nome (se nomeado) e valor.
pub type Evaluated = (Option<SymbolId>, Operand);

/// A classe é um `mixin` (não instanciável; os membros entram nas classes
/// que o aplicam).
pub fn is_mixin(ctx: &Context, cid: ClassId) -> bool {
    ctx.program.classes[cid.0 as usize].kind == dartforge_elements::model::ClassKind::Mixin
}

/// A linearização de uma classe do programa (P4, especificação §12.3 "Mixin
/// Application"): `class C extends B with M1, M2` é `C`, `M2`, `M1` e depois
/// a linearização de `B` — a ordem em que um membro é procurado. Para no
/// SDK. O elemento não sintetiza a classe `B&M1`: a aplicação é lida aqui.
pub fn linearization(ctx: &Context, cid: ClassId) -> Vec<ClassId> {
    let mut exit = Vec::new();
    let mut current = Some(cid);
    while let Some(c) = current {
        let class_ref = &ctx.program.classes[c.0 as usize];
        if !ctx.compiled_library(class_ref.library) || exit.contains(&c) {
            break;
        }
        exit.push(c);
        for m in class_ref.mixin_classes.iter().rev() {
            if ctx.compiled_library(ctx.program.classes[m.0 as usize].library) && !exit.contains(m) {
                exit.push(*m);
            }
        }
        current = class_ref.supertype_class;
    }
    exit
}

/// Campos **de instância** de toda a linearização (superclasses e mixins)
/// do usuário, da raiz para a folha (R7).
///
/// `ClassElement::fields` mistura campos de instância e estáticos; o layout
/// só tem os de instância. Superclasses do SDK não contribuem campos: o
/// runtime não tem os objetos do SDK no nosso layout. Os campos de um mixin
/// ficam entre os da superclasse e os da classe que o aplica.
pub fn layout(ctx: &Context, cid: ClassId) -> Vec<VariableId> {
    let chain = linearization(ctx, cid);
    let mut fields = Vec::new();
    for c in chain.into_iter().rev() {
        for &vid in &ctx.program.classes[c.0 as usize].fields {
            if !ctx.program.variables[vid.0 as usize].static_ {
                fields.push(vid);
            }
        }
    }
    fields
}

/// Tipo declarado (ou inferido) de uma variável/campo.
pub fn variable_type(ctx: &Context, vid: VariableId) -> TypeId {
    let data = &ctx.outline.variables[vid.0 as usize];
    data
        .declared_type
        .or(data.inferred)
        .unwrap_or(ctx.core.dynamic_)
}

/// `sub` é `sup` ou subtipo nominal dele (superclasse, mixin ou interface).
pub fn subclass_of(ctx: &Context, sub: ClassId, sup: ClassId) -> bool {
    if sub == sup {
        return true;
    }
    ctx.outline
        .hierarchy
        .get(sub)
        .is_some_and(|d| d.supertypes.contains_key(&sup))
}

/// A função tem corpo que o lowering compila?
pub fn has_body(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        // Um `external` do SDK é implementado pelo patch ou pelo native
        // (`sdk_source::call_external`).
        FunctionRef::Function { .. } if f.external => true,
        FunctionRef::Function { unit, function } => !matches!(
            ctx.program.unit(unit).ast.function(function).body,
            FunctionBody::Empty | FunctionBody::Native(_)
        ),
        FunctionRef::Constructor { .. } => true,
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representação de um tipo Dart (R1). `void` numa posição de valor
    /// vira `Ref` (o valor é sempre null).
    pub fn repr(&self, ty: TypeId) -> Type {
        match self.ctx.to_hir_type(ty) {
            Type::Void => Type::Ref,
            t => t,
        }
    }

    /// Valor neutro de uma representação: argumento opcional sem padrão
    /// (sempre de tipo anulável, logo `Ref` null) e campo não inicializado.
    pub fn zero_value(ty: Type) -> Operand {
        match ty {
            Type::I64 => Operand::Constant(Constant::Int(0)),
            Type::F64 => Operand::Constant(Constant::Double(0.0)),
            Type::I1 | Type::I8 => Operand::Constant(Constant::Bool(false)),
            _ => Operand::Constant(Constant::Null),
        }
    }

    /// Posição do campo no layout da classe que o declara (R7). O layout de
    /// uma subclasse estende o da superclasse, então o índice vale para
    /// qualquer receptor cuja classe estática seja subtipo da declarante.
    pub fn field_index(&self, vid: VariableId) -> Option<usize> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if is_mixin(self.ctx, owner) {
            // O índice de um campo de mixin depende da classe que o aplica
            // (`campo_de_mixin`).
            return None;
        }
        let base = super::enums::layout_base(self.ctx, owner);
        layout(self.ctx, owner).iter().position(|&v| v == vid).map(|i| i + base)
    }

    /// Representação de um campo no objeto: a do tipo (R1), exceto o campo
    /// `late` escalar com inicializador, que mora em caixa (`Ref`) — o null
    /// é o "ainda não inicializado" (P4; a VM usa o sentinela `_sentinel`).
    pub fn field_repr(&self, vid: VariableId) -> Type {
        super::layouts_arc::representation(self.ctx, vid)
    }

    /// Converte os bits `i64` lidos do heap para a representação `repr`.
    pub fn bits_to(&mut self, bits: Operand, repr: Type) -> Operand {
        match repr {
            Type::F64 => self.emit(
                Instruction::Bitcast {
                    op: bits,
                    to: Type::F64,
                },
                Type::F64,
            ),
            Type::I1 => self.emit(
                Instruction::ICmp(ICmpOp::Ne, bits, Operand::Constant(Constant::Int(0))),
                Type::I1,
            ),
            _ => bits,
        }
    }

    /// Bits `i64` de um valor, para gravar no heap, e se ele é referência
    /// (E1: `is_ref` vem da representação, nunca de um literal).
    pub fn to_bits(&mut self, op: Operand) -> (Operand, bool) {
        match self.operand_type(&op) {
            Type::F64 => (
                self.emit(Instruction::Bitcast { op, to: Type::I64 }, Type::I64),
                false,
            ),
            Type::I1 => (
                self.emit(
                    Instruction::ZExt {
                        op,
                        from: Type::I1,
                        to: Type::I64,
                    },
                    Type::I64,
                ),
                false,
            ),
            Type::I8 => (
                self.emit(
                    Instruction::ZExt {
                        op,
                        from: Type::I8,
                        to: Type::I64,
                    },
                    Type::I64,
                ),
                false,
            ),
            Type::Ref => (op, true),
            _ => (op, false),
        }
    }

    /// O índice de um campo de mixin em cada classe concreta que o aplica.
    fn mixin_field_indices(&self, vid: VariableId) -> Vec<(i64, usize)> {
        let mut exit = Vec::new();
        for (k, class_ref) in self.ctx.program.classes.iter().enumerate() {
            let kid = ClassId(k as u32);
            if !self.ctx.compiled_library(class_ref.library) || is_mixin(self.ctx, kid) {
                continue;
            }
            let base = super::enums::layout_base(self.ctx, kid);
            if let (Some(id), Some(i)) = (
                self.ctx.class_id_of(kid),
                layout(self.ctx, kid).iter().position(|&v| v == vid),
            ) {
                exit.push((id.into(), i + base));
            }
        }
        exit
    }

    /// Índice de um campo de mixin pela classe dinâmica do objeto (o código
    /// do mixin é compilado uma vez; a posição do campo é a da aplicação).
    fn dynamic_index(&mut self, obj: Operand, vid: VariableId, span: Span) -> Option<Operand> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if !is_mixin(self.ctx, owner) {
            return None;
        }
        let cases = self.mixin_field_indices(vid);
        if cases.is_empty() {
            // Nenhuma classe do programa aplica o mixin: não existe objeto
            // com este campo, e o acesso não executa (o `LocatorMixin` do
            // `state_notifier`, que o riverpod importa e ninguém aplica).
            // O programa é fechado — nenhuma classe nova aparece depois.
            let _ = span;
            self.terminate(Terminator::Unreachable);
            let dead = self.new_block();
            self.set_block(dead);
            return Some(Operand::Constant(Constant::Int(0)));
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(obj, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let blocks: Vec<(i64, usize, BlockId)> = cases.iter().map(|&(c, i)| (c, i, self.new_block())).collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocks[0].2,
            cases: blocks.iter().map(|&(c, _, b)| (c, b)).collect(),
        });
        let mut entries = Vec::new();
        for &(_, i, b) in &blocks {
            self.set_block(b);
            entries.push((b, Operand::Constant(Constant::Int(i as i64))));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: Type::I64,
            },
            Type::I64,
        ))
    }

    // Em ARC, escolhe o acesso físico no ramo de cada aplicação do mixin.
    // A junção carrega o resultado tipado, nunca um índice bruto cuja leitura
    // perderia o mapa Ref. As guardas/owners são produzidas depois pela HIR.
    fn access_arc_mixin_field(&mut self, obj: Operand, vid: VariableId, val_ref: Option<Operand>) -> Option<Operand> {
        let owner = self.ctx.program.variables[vid.0 as usize].class?;
        if !is_mixin(self.ctx, owner) { return None; }
        let cases = self.mixin_field_indices(vid);
        if cases.is_empty() {
            self.terminate(Terminator::Unreachable);
            let dead = self.new_block(); self.set_block(dead);
            return Some(Operand::Constant(Constant::Null));
        }
        let repr = self.field_repr(vid);
        let cls = self.emit(Instruction::CallRuntime {
            name: "dartforge_value_class".into(), args: vec![(obj.clone(), Type::Ref)], ret_ty: Type::I64,
        }, Type::I64);
        let join = self.new_block();
        let blocks: Vec<_> = cases.iter().map(|&(c, i)| (c, i, self.new_block())).collect();
        self.terminate(Terminator::Switch { val: cls, default: blocks[0].2,
            cases: blocks.iter().map(|&(c, _, b)| (c, b)).collect() });
        let mut entries = Vec::new();
        for &(_, index, b) in &blocks {
            self.set_block(b);
            if let Some(value) = &val_ref {
                self.emit(Instruction::SetField { object: obj.clone(), index, value: value.clone() }, Type::Void);
            } else {
                let v = self.emit(Instruction::GetField { object: obj.clone(), index }, repr);
                entries.push((b, v));
            }
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(if val_ref.is_some() { Operand::Constant(Constant::Null) }
            else { self.emit(Instruction::Phi { incoming: entries, ty: repr }, repr) })
    }

    /// Lê um campo de instância na representação do seu tipo declarado.
    pub fn read_field(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        if let Some(r) = self.read_ffi_field(obj.clone(), vid) {
            return r;
        }
        if self.ctx.arc_memory && !self.ctx.program.variables[vid.0 as usize].late
            && self.field_index(vid).is_none()
            && let Some(v) = self.access_arc_mixin_field(obj.clone(), vid, None)
        { return v; }
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => return self.unsupported("campo fora do layout do objeto", span),
            },
        };
        let repr = self.field_repr(vid);
        if self.ctx.arc_memory && self.ctx.program.variables[vid.0 as usize].late
            && repr == Type::Ref
        {
            // A sondagem anterior à guarda late pode encontrar zero sem marca
            // Ref. A ABI aceita somente esse estado vazio ou uma aresta real.
            return self.emit(Instruction::CallRuntime {
                name: "dartforge_arc_ler_campo_late_ref_v1".into(),
                args: vec![(obj, Type::Ref), (idx, Type::I64)],
                ret_ty: Type::Ref,
            }, Type::Ref);
        }
        if self.ctx.arc_memory && !self.ctx.program.variables[vid.0 as usize].late
            && let Operand::Constant(Constant::Int(i)) = &idx
            && let Ok(index) = usize::try_from(*i)
        {
            // Transporta a representação sem escolher uma ABI auditada:
            // origem do receiver e guardas ainda são conferidas depois.
            return self.emit(Instruction::GetField { object: obj, index }, repr);
        }
        let ret = if repr == Type::Ref {
            Type::Ref
        } else {
            Type::I64
        };
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (idx, Type::I64)],
                ret_ty: ret,
            },
            ret,
        );
        self.bits_to(bits, repr)
    }

    /// Grava um campo de instância; `is_ref` pela representação (E1).
    pub fn store_field(&mut self, obj: Operand, vid: VariableId, val: Operand, span: Span) {
        if self.store_ffi_field(obj.clone(), vid, val.clone()) {
            return;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if self.ctx.arc_memory && !var.late && self.field_index(vid).is_none()
            && var.class.is_some_and(|owner| is_mixin(self.ctx, owner))
        {
            let repr = self.field_repr(vid);
            let value = self.coerce(val.clone(), repr);
            if self.access_arc_mixin_field(obj.clone(), vid, Some(value)).is_some() { return; }
        }
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("campo fora do layout do objeto", span);
                    return;
                }
            },
        };
        let var = &self.ctx.program.variables[vid.0 as usize];
        let late_without_init = var.late && self.variable_initializer_in(vid).is_none();
        let late_final = late_without_init && var.final_;
        let name_str = self.ctx.symbol_name(var.name).to_string();
        if late_final {
            let already_initialized = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let already_initialized = self.emit(
                Instruction::ICmp(ICmpOp::Ne, already_initialized, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_error = self.new_block();
            let b_store = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: already_initialized,
                then_block: b_error,
                else_block: b_store,
            });
            self.set_block(b_error);
            self.throw_late_error(&name_str, 1);
            self.set_block(b_store);
        }
        let repr = self.field_repr(vid);
        let val = self.coerce(val, repr);
        if self.ctx.arc_memory
            && let Operand::Constant(Constant::Int(i)) = &idx
            && let Ok(index) = usize::try_from(*i)
        {
            // A escrita tipada também vale para late; o estado lateral só
            // muda depois de gravar, inclusive para null/false/zero.
            self.emit(Instruction::SetField { object: obj.clone(), index, value: val }, Type::Void);
        } else {
            let (bits, is_ref) = self.to_bits(val);
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_object_set".to_string(),
                    args: vec![
                        (obj.clone(), Type::Ref),
                        (idx.clone(), Type::I64),
                        (bits, Type::I64),
                        (
                            Operand::Constant(Constant::Int(i64::from(is_ref))),
                            Type::I8,
                        ),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        if var.late {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_mark_initialized".to_string(),
                    args: vec![(obj, Type::Ref), (idx, Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
    }

    /// Campo de instância pelo nome, na própria classe (alvo de `this.x` e
    /// da lista de inicialização).
    pub fn own_field(&self, cid: ClassId, name_str: SymbolId) -> Option<VariableId> {
        self.ctx.program.classes[cid.0 as usize]
            .fields
            .iter()
            .copied()
            .find(|&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == name_str
            })
    }

    /// Classe do usuário do tipo estático de uma expressão, se for uma.
    pub fn user_class_of(&self, e: ExprId) -> Option<ClassId> {
        let ty = self.ctx.get_type(self.unit_id, e)?;
        match self.ctx.table.get(ty) {
            dartforge_types::table::Type::Interface { class, .. } => {
                let class_ref = &self.ctx.program.classes[class.0 as usize];
                (self.ctx.compiled_library(class_ref.library)).then_some(*class)
            }
            _ => None,
        }
    }

    /// Membro de classe do usuário acessado em `recv.name` (R7).
    ///
    /// Primeiro o elemento que a inferência resolveu para o acesso; se ela
    /// não resolveu (receptor anulável de `?.` ou `x!`, que a busca de
    /// membros do `types` ainda não despe do `?`), a classe ESTÁTICA do
    /// receptor — nunca o primeiro membro com esse nome no programa.
    pub fn user_member(
        &self,
        access: ExprId,
        recv: ExprId,
        name_str: SymbolId,
        setter: bool,
    ) -> Option<(ClassId, MemberRef)> {
        if let Some(dartforge_types::resolved::Resolved::Member { class, member, .. }) =
            self.ctx.get_resolved(self.unit_id, access)
        {
            let class_ref = &self.ctx.program.classes[class.0 as usize];
            // Membro herdado do SDK (`index`/`name` de enum, `hashCode`,
            // `toString` de `Object`…) numa classe do programa: não é
            // membro compilado — vai pelos caminhos do SDK.
            let member_lib = match member {
                MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].library,
                MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].library,
            };
            if !self.ctx.compiled_library(member_lib) {
                return None;
            }
            // A resolução de `o.x = v` pode apontar para o getter `x`.
            // Nesse caso a escrita precisa procurar a entrada distinta
            // `x_=` (ou o setter implícito de um campo) na hierarquia.
            let is_getter = matches!(member, MemberRef::Function(f)
                if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                    && self.ctx.program.functions[f.0 as usize].variable.is_none());
            if !(setter && is_getter) {
                return (self.ctx.compiled_library(class_ref.library)).then_some((*class, *member));
            }
        }
        let cid = self.user_class_of(recv)?;
        for c in crate::lower::members::linearization(self.ctx, cid) {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.compiled_library(class_ref.library) {
                return None;
            }
            if let Some(&f) = class_ref.instance_members.get(&name_str) {
                let func = &self.ctx.program.functions[f.0 as usize];
                if !setter && func.kind != FunctionKind::Setter {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if setter {
                let setter_name = format!("{}_=", self.ctx.symbol_name(name_str));
                if let Some(s) = self.ctx.interner.lookup(&setter_name)
                    && let Some(&f) = class_ref.instance_members.get(&s)
                {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if let Some(&v) = class_ref.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == name_str
            }) {
                return Some((c, MemberRef::Variable(v)));
            }
        }
        None
    }

    /// `E` de uma `List<E>`/`Iterable<E>` (o primeiro argumento de tipo).
    pub fn element_type(&self, ty: TypeId) -> Option<TypeId> {
        let dartforge_types::table::Type::Interface { class, args, .. } = self.ctx.table.get(ty) else {
            return None;
        };
        let iter = self.ctx.core.iterable_class?;
        if *class == iter || self.ctx.core.list_class == Some(*class) {
            return args.first().copied();
        }
        // Subtipo de `Iterable<E>` (p. ex. `Runes`): o `E` vem do supertipo
        // instanciado; sem argumentos a substituir, ele já é o tipo final.
        let data = self.ctx.outline.hierarchy.get(*class)?;
        let sup = *data.supertypes.get(&iter)?;
        if !data.type_params.is_empty() {
            return None;
        }
        match self.ctx.table.get(sup) {
            dartforge_types::table::Type::Interface { args, .. } => args.first().copied(),
            _ => None,
        }
    }

    /// Membro de instância por nome, subindo a cadeia de superclasses.
    /// O operador `name` (`+`, `[]`, `unary-`…) declarado por uma classe do
    /// PROGRAMA (não do SDK) para o receptor de tipo estático `e`: a chamada
    /// vai por `call_member` (direta, ou o despacho só entre as
    /// implementações do programa), não pelo seletor dinâmico.
    pub fn program_operator(&self, e: ExprId, name_str: &str) -> Option<usize> {
        let cid = self.user_class_of(e)?;
        if self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk {
            return None;
        }
        let fid = self.member_in_class(cid, name_str)?;
        let f = &self.ctx.program.functions[fid];
        (crate::lower::user_function(self.ctx, fid) && !self.ctx.program.library(f.library).is_sdk).then_some(fid)
    }

    pub fn member_in_class(&self, cid: ClassId, name_str: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(name_str)?;
        for c in crate::lower::members::linearization(self.ctx, cid) {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class_ref.instance_members.get(&sym) {
                return Some(f.0 as usize);
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Leitura de identificadores

    /// Id de classe usado nos testes de tipo do runtime: o `class_id` de um
    /// objeto do usuário, ou o código que `dartforge_value_class` devolve
    /// para os valores que o runtime representa por conta própria.
    pub fn class_id_of(&self, cid: ClassId) -> Option<i64> {
        let class_ref = &self.ctx.program.classes[cid.0 as usize];
        if self.ctx.compiled_library(class_ref.library) {
            return self.ctx.class_id_of(cid).map(i64::from);
        }
        Some(match self.ctx.symbol_name(class_ref.name) {
            "String" => -2,
            "List" => -3,
            "Map" => -4,
            "Set" => -5,
            "Function" => -6,
            "Record" => -7,
            "StringBuffer" => -8,
            "int" => -9,
            "double" => -10,
            "bool" => -11,
            "Object" => 0,
            "Exception" => 1000,
            "FormatException" => 1001,
            "StateError" => 1002,
            "ArgumentError" => 1003,
            "RangeError" => 1004,
            "UnsupportedError" => 1005,
            "StackTrace" => 1006,
            "Error" => 1007,
            "UnimplementedError" => 1008,
            "AssertionError" => 1009,
            "ConcurrentModificationError" => 1010,
            "TypeError" => 1011,
            "NoSuchMethodError" => 1012,
            // O objeto `Type` do RTI (`rti.rs`, `CLASSE_TIPO`).
            "Type" => super::rti::CLASSE_TIPO,
            _ => return None,
        })
    }

    /// Identificador que a resolução diz ser membro da classe envolvente
    /// (`x` = `this.x`, ou membro estático).
    pub fn read_implicit_member(&mut self, member: MemberRef, span: Span) -> Operand {
        // `values` implícito de um enum do programa, lido sem o prefixo
        // `E.` no corpo de um membro do enum: a lista dos valores, como o
        // caminho explícito (`E.values`, `enum_values`). O getter
        // implícito não tem corpo para chamar — emitir a chamada deixava um
        // símbolo indefinido na ligação.
        if let MemberRef::Function(f) = member {
            let fid = f.0 as usize;
            let (is_static, name_str, cid) = {
                let func = &self.ctx.program.functions[fid];
                (func.static_, self.ctx.symbol_name(func.name).to_string(), func.class)
            };
            if is_static
                && name_str == "values"
                && !has_body(self.ctx, fid)
                && let Some(cid) = cid
                && super::enums::is_enum(self.ctx, cid)
            {
                return self.enum_values(cid, span);
            }
        }
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                return self.read_global(vid, span);
            }
            let Some(this) = self.this_param.clone() else {
                return self.unsupported("campo de instância fora de membro de instância", span);
            };
            return self.read_field_with_late(this, vid, span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Getter {
            if !super::user_function(self.ctx, fid) {
                return self.unsupported("tear-off de método do SDK", span);
            }
            if func.static_ {
                return self.function_tearoff(fid);
            }
            let Some(this) = self.this_param.clone() else {
                return self.unsupported("tear-off de método fora de membro de instância", span);
            };
            return self.method_tearoff(this, fid, span);
        }
        if func.static_ {
            if !super::user_function(self.ctx, fid) {
                return self.unsupported("getter estático do SDK", span);
            }
            return self.call_direct(fid, None, Vec::new());
        }
        let Some(this) = self.this_param.clone() else {
            return self.unsupported("getter de instância fora de membro de instância", span);
        };
        self.call_member(this, fid, &[], span)
    }

    /// `C.x` com `C` literal de classe: campo estático ou getter estático.
    pub fn read_static_member(&mut self, member: MemberRef, span: Span) -> Operand {
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                return self.read_global(vid, span);
            }
            return self.unsupported("constante de enum", span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        if self.ctx.program.functions[fid].kind == FunctionKind::Getter
            && super::user_function(self.ctx, fid)
        {
            return self.call_direct(fid, None, Vec::new());
        }
        if super::user_function(self.ctx, fid) && self.ctx.program.functions[fid].static_ {
            return self.function_tearoff(fid);
        }
        self.unsupported("tear-off de membro estático", span)
    }

    /// Identificador que resolve para um elemento de topo.
    pub fn read_element(&mut self, el: Element, span: Span) -> Operand {
        match el {
            Element::Variable(vid) => self.read_global(vid, span),
            Element::Function(f) => {
                let fid = f.0 as usize;
                let func = &self.ctx.program.functions[fid];
                if let Some(vid) = func.variable {
                    return self.read_global(vid, span);
                }
                if func.kind == FunctionKind::Getter && super::user_function(self.ctx, fid) {
                    return self.call_direct(fid, None, Vec::new());
                }
                if super::user_function(self.ctx, fid) && func.kind != FunctionKind::Setter {
                    return self.function_tearoff(fid);
                }
                self.unsupported("tear-off de função", span)
            }
            // Literal de tipo (`Peixe`, `List`): o objeto `Type` canônico do
            // tipo cru da classe (RTI).
            Element::Class(cid) => {
                let n = self.ctx.outline.classes.get(cid.0 as usize).map_or(0, |d| d.type_params.len());
                let mut r = super::rti::Recipe { text: format!("C{}", self.ctx.id_rti(cid)), variables: false };
                if n > 0 {
                    r.text.push('<');
                    r.text.push_str(&vec!["D"; n].join(","));
                    r.text.push('>');
                }
                let t = self.recipe_rti(&r);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_objeto_tipo".to_string(),
                        args: vec![(t, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
            _ => self.unsupported("elemento de topo", span),
        }
    }

    /// Campo lido; se `late` com inicializador, inicializa na primeira leitura.
    pub fn read_field_with_late(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        if let Some(r) = self.read_source_field(obj.clone(), vid) {
            return r;
        }
        self.read_field_with_late_direct(obj, vid, span)
    }

    /// `super.field` tem resolução lexical: preserva `late`, mas não pode
    /// voltar ao getter sobrescrito da classe dinâmica via seletor do SDK.
    pub fn read_field_with_late_direct(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.late {
            return self.read_field(obj, vid, span);
        }
        let dartforge_elements::model::VariableRef::Field { .. } = var.node else {
            return self.read_field(obj, vid, span);
        };
        if self.variable_initializer_in(vid).is_some() {
            // Via rápida em linha (ver `non_null_late_is_initialized`): o
            // valor não nulo sai direto; o getter (que inicializa) só é
            // chamado com o campo ainda nulo.
            let fast = self.non_null_late_is_initialized(vid).then(|| {
                let current = self.read_field(obj.clone(), vid, span);
                let non_null = self.emit(Instruction::ICmp(ICmpOp::Ne, current.clone(), Operand::Constant(Constant::Null)), Type::I1);
                let b_join = self.new_block();
                let b_getter = self.new_block();
                let de = self.current_block;
                self.terminate(Terminator::CondBranch { cond: non_null, then_block: b_join, else_block: b_getter });
                self.set_block(b_getter);
                (current, de, b_join)
            });
            let v = self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: super::late_field_getter_symbol(self.ctx, vid),
                    args: vec![obj],
                    ret_ty: self.field_repr(vid),
                },
                self.field_repr(vid),
            );
            let Some((current, de, b_join)) = fast else { return v };
            let incoming = if self.is_terminated() {
                vec![(de, current)]
            } else {
                let b = self.current_block;
                self.terminate(Terminator::Branch(b_join));
                vec![(de, current), (b, v)]
            };
            self.set_block(b_join);
            return self.emit(Instruction::Phi { incoming, ty: Type::Ref }, Type::Ref);
        }
        let current = self.read_field(obj.clone(), vid, span);
        if self.non_null_late_is_initialized(vid) {
            // Via rápida: valor não nulo ⇒ inicializado; só o nulo consulta
            // o runtime (que distingue "não inicializado").
            let non_null = self.emit(Instruction::ICmp(ICmpOp::Ne, current.clone(), Operand::Constant(Constant::Null)), Type::I1);
            let b_ready = self.new_block();
            let b_check = self.new_block();
            self.terminate(Terminator::CondBranch { cond: non_null, then_block: b_ready, else_block: b_check });
            self.set_block(b_check);
            self.check_late_initialized(obj, vid, span);
            if !self.is_terminated() {
                self.terminate(Terminator::Branch(b_ready));
            }
            self.set_block(b_ready);
            return current;
        }
        self.check_late_initialized(obj, vid, span);
        current
    }

    /// Um campo `late` sem inicializador, de tipo interface não anulável e
    /// guardado como referência: nele o valor gravado nunca é `null`, e o
    /// campo de um objeto novo é zero (`null`) — então "não nulo" já prova
    /// "inicializado", sem a tabela lateral do runtime (uma chamada com
    /// busca em `HashSet` por leitura, que ficava no corpo dos laços).
    fn non_null_late_is_initialized(&self, vid: VariableId) -> bool {
        let ty = variable_type(self.ctx, vid);
        // A representação natural do tipo (não a forçada do `late` com
        // inicializador): `int`/`double`/`bool` ficam de fora.
        if self.field_repr(vid) != Type::Ref || self.repr(ty) != Type::Ref {
            return false;
        }
        matches!(self.ctx.table.get(ty), dartforge_types::table::Type::Interface { nullable: false, .. })
            && ty != self.ctx.core.dynamic_
    }

    /// Confere no runtime que o campo `late` `vid` de `obj` foi inicializado
    /// e lança o `LateInitializationError` se não foi; termina no bloco da
    /// leitura, sem terminar.
    fn check_late_initialized(&mut self, obj: Operand, vid: VariableId, span: Span) {
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("campo late fora do layout", span);
                    return;
                }
            },
        };
        let initialized = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initialized".to_string(),
                args: vec![(obj, Type::Ref), (idx, Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let empty = self.emit(
            Instruction::ICmp(ICmpOp::Eq, initialized, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_error = self.new_block();
        let b_read = self.new_block();
        self.terminate(Terminator::CondBranch { cond: empty, then_block: b_error, else_block: b_read });
        self.set_block(b_error);
        let name_str = self.ctx.symbol_name(self.ctx.program.variables[vid.0 as usize].name).to_string();
        self.throw_late_error(&name_str, 0);
        self.set_block(b_read);
    }

    /// Corpo único do getter de um campo `late` com inicializador. O estado
    /// vive por objeto, inclusive quando o valor atribuído é `null`.
    pub fn lower_late_field_getter(&mut self, obj: Operand, vid: VariableId, span: Span) {
        // Corpo Dart chamado pelo leitor do campo: o retorno Ref é Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let Some(init) = self.variable_initializer_in(vid) else { unreachable!("getter late sem initializer") };
        let dartforge_elements::model::VariableRef::Field { unit, .. } = self.ctx.program.variables[vid.0 as usize].node else {
            unreachable!("getter late sem campo")
        };
        let idx = match self.field_index(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.dynamic_index(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.unsupported("campo late fora do layout", span);
                    self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                    return;
                }
            },
        };
        if self.non_null_late_is_initialized(vid) {
            // Via rápida (ver `non_null_late_is_initialized`): valor não nulo
            // já é o valor inicializado.
            let current = self.read_field(obj.clone(), vid, span);
            let non_null = self.emit(Instruction::ICmp(ICmpOp::Ne, current.clone(), Operand::Constant(Constant::Null)), Type::I1);
            let b_fast = self.new_block();
            let b_slow = self.new_block();
            self.terminate(Terminator::CondBranch { cond: non_null, then_block: b_fast, else_block: b_slow });
            self.set_block(b_fast);
            self.terminate(Terminator::Return(Some(current)));
            self.set_block(b_slow);
        }
        let ready = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initialized".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ready, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_verify = self.new_block();
        self.terminate(Terminator::CondBranch { cond: ready, then_block: b_read, else_block: b_verify });
        self.set_block(b_read);
        let current = self.read_field(obj.clone(), vid, span);
        self.terminate(Terminator::Return(Some(current)));

        self.set_block(b_verify);
        let starting = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let starting = self.emit(
            Instruction::ICmp(ICmpOp::Ne, starting, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_stack = self.new_block();
        let b_evaluate = self.new_block();
        self.terminate(Terminator::CondBranch { cond: starting, then_block: b_stack, else_block: b_evaluate });
        self.set_block(b_stack);
        let error = self.new_runtime_error("dartforge_stack_overflow_error_new", Vec::new());
        self.emit_throw_op(error);

        self.set_block(b_evaluate);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let b_fail = self.new_block();
        self.exception_targets.push(b_fail);
        let val_ref = self.lower_expr_of(unit, init);
        self.exception_targets.pop();
        let val_ref = self.coerce(val_ref, self.field_repr(vid));
        self.store_field(obj.clone(), vid, val_ref.clone(), span);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I8)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(val_ref)));
        self.set_block(b_fail);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![(obj, Type::Ref), (idx, Type::I64), (Operand::Constant(Constant::Int(0)), Type::I8)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
    }

    // ------------------------------------------------------------------
    // Argumentos

    /// Avalia os argumentos na ordem do texto (a ordem de avaliação do Dart).
    ///
    /// Cada argumento `dynamic` ganha, logo depois de avaliado, o cast
    /// implícito para o tipo do parâmetro na invocação armada para esta
    /// lista (`arm_arg_types`, `typed_entry.rs`), como o CFE.
    pub fn evaluate_args(&mut self, ast: &ast::Ast, args: &[ast::Argument]) -> Vec<Evaluated> {
        let types = match &self.arg_types {
            Some((key, _)) if *key == args.as_ptr() as usize && !args.is_empty() => {
                self.arg_types.take().map(|(_, t)| t)
            }
            _ => None,
        };
        let mut i_pos = 0;
        args.iter()
            .map(|a| {
                let v = self.lower_expr(ast, a.value);
                let name_str = a.name.map(|n| n.sym);
                if !self.is_terminated() {
                    self.implicit_argument_cast(types.as_ref(), a.value, name_str, i_pos, &v);
                }
                if name_str.is_none() {
                    i_pos += 1;
                }
                (name_str, v)
            })
            .collect()
    }

    /// Parâmetros como escritos na declaração (para valores padrão).
    fn ast_parameters(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
        let program = self.ctx.program;
        match program.functions[fid].node {
            FunctionRef::Function { unit, function } => program
                .unit(unit)
                .ast
                .function(function)
                .parameters
                .as_deref()
                .map(|p| (unit, p)),
            FunctionRef::Constructor { unit, member } => {
                match &program.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(c) => Some((unit, &c.parameters[..])),
                    _ => None,
                }
            }
            FunctionRef::None => None,
        }
    }

    /// Os parâmetros de `fid` na AST (a unidade e a lista).
    pub fn parameters_of(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
        self.ast_parameters(fid)
    }

    /// Baixa uma expressão de outra unidade (valor padrão de parâmetro).
    ///
    /// `lower_expr` usa `self.unit_id` para os tipos e resoluções; trocar a
    /// unidade enquanto a expressão é baixada é o que torna isso seguro
    /// (docs/NATIVO-PLANO.md §5.1). Valores padrão são constantes: não leem
    /// locais do chamado.
    pub fn lower_expr_of(&mut self, unit: UnitId, expr: ExprId) -> Operand {
        let saved = self.unit_id;
        self.unit_id = unit;
        let ast = &self.ctx.program.unit(unit).ast;
        let v = self.lower_expr(ast, expr);
        self.unit_id = saved;
        v
    }

    /// Valor padrão do parâmetro `i` de `fid`, ou o herdado do construtor da
    /// superclasse quando o parâmetro é `super.x` sem padrão próprio.
    pub fn default_value(&mut self, fid: usize, i: usize) -> Option<Operand> {
        let (unit, params) = self.ast_parameters(fid)?;
        let p = params.get(i)?;
        if let Some(e) = p.default_value {
            // O padrão de uma função do SDK da fonte chamada de fora do
            // módulo dela: as resoluções da unidade do SDK não existem aqui;
            // o módulo do SDK exporta o valor (`lower_sdk_patterns`).
            if exported_pattern(self.ctx, fid, unit, e) {
                return Some(self.emit_call_with_check(
                    Instruction::CallStatic { symbol: pattern_symbol(self.ctx, fid, i), args: Vec::new(), ret_ty: Type::Ref },
                    Type::Ref,
                ));
            }
            // O valor padrão é constante (§9.2.2): contexto const.
            let saved = std::mem::replace(&mut self.in_const_context, true);
            let v = self.lower_expr_of(unit, e);
            self.in_const_context = saved;
            return Some(v);
        }
        // Factory redirecionadora (`factory C({x}) = D;`): o parâmetro dela
        // não tem padrão próprio e vale o do parâmetro correspondente do
        // alvo (especificação, "Redirecting Factory Constructors"; o CFE
        // copia os padrões do alvo efetivo).
        if let Some(alvo) = self.redirected_target(fid) {
            let target_params = &self.ctx.outline.functions.get(alvo)?.parameters;
            let j = if p.kind == ParameterKind::Named {
                target_params.iter().position(|sp| sp.kind == ParameterKind::Named && sp.externo == p.name.map(|n| n.sym))?
            } else {
                // Os posicionais vêm antes dos nomeados, na mesma ordem.
                i
            };
            return self.default_value(alvo, j);
        }
        if !p.super_ {
            return None;
        }
        // `super.x` sem padrão herda o do parâmetro correspondente do
        // construtor da superclasse.
        let (sup_fid, explicit_positionals) = self.super_constructor_of(fid)?;
        let sup_params = &self.ctx.outline.functions.get(sup_fid)?.parameters;
        let alvo = if p.kind == ParameterKind::Named {
            sup_params
                .iter()
                .position(|sp| sp.externo == p.name.map(|n| n.sym))?
        } else {
            let k = params[..i]
                .iter()
                .filter(|q| q.super_ && q.kind != ParameterKind::Named)
                .count();
            let positionals: Vec<usize> = sup_params
                .iter()
                .enumerate()
                .filter(|(_, sp)| sp.kind != ParameterKind::Named)
                .map(|(j, _)| j)
                .collect();
            *positionals.get(explicit_positionals + k)?
        };
        self.default_value(sup_fid, alvo)
    }

    /// O construtor para o qual a factory `fid` redireciona
    /// (`factory C(…) = D.name;`), resolvido na unidade que a declara.
    fn redirected_target(&mut self, fid: usize) -> Option<usize> {
        let FunctionRef::Constructor { unit, member } = self.ctx.program.functions.get(fid)?.node else {
            return None;
        };
        let ast = &self.ctx.program.unit(unit).ast;
        let MemberKind::Constructor(c) = &ast.member(member).kind else { return None };
        let r = c.redirect.as_ref()?;
        let saved = self.unit_id;
        self.unit_id = unit;
        let alvo = self.type_constructor(ast.ty(r.ty), r.constructor.map(|n| n.sym));
        self.unit_id = saved;
        alvo.map(|f| f.0 as usize).filter(|&a| a != fid)
    }

    /// Construtor da superclasse chamado por um construtor (explícito na
    /// lista de inicialização ou o sem nome implícito), e quantos
    /// argumentos posicionais explícitos ele recebe.
    fn super_constructor_of(&self, fid: usize) -> Option<(usize, usize)> {
        let f = &self.ctx.program.functions[fid];
        let cid = f.class?;
        let sup = self.ctx.program.classes[cid.0 as usize].supertype_class?;
        let sup_class = &self.ctx.program.classes[sup.0 as usize];
        let FunctionRef::Constructor { unit, member } = f.node else {
            return None;
        };
        let MemberKind::Constructor(c) = &self.ctx.program.unit(unit).ast.member(member).kind
        else {
            return None;
        };
        let explicit = c.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super {
                constructor,
                arguments,
                ..
            } => Some((*constructor, arguments)),
            _ => None,
        });
        let empty = self.ctx.interner.lookup("")?;
        match explicit {
            Some((name_str, args)) => {
                let s = name_str.map(|n| n.sym).unwrap_or(empty);
                let sf = sup_class.constructors.get(&s)?;
                Some((
                    sf.0 as usize,
                    args.args.iter().filter(|a| a.name.is_none()).count(),
                ))
            }
            None => sup_class
                .constructors
                .get(&empty)
                .map(|sf| (sf.0 as usize, 0)),
        }
    }

    /// Casa argumentos avaliados com os parâmetros de `fid`: posicionais em
    /// ordem, nomeados pelo nome, faltantes pelo valor padrão (N5).
    pub fn match_args(&mut self, fid: usize, evaluated: &[Evaluated]) -> Vec<Operand> {
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return evaluated.iter().map(|(_, v)| v.clone()).collect();
        };
        let params: Vec<(Option<SymbolId>, TypeId, ParameterKind)> = data
            .parameters
            .iter()
            // O nome externo casa com o do argumento (nomeado privado da 3.12).
            .map(|p| (p.externo, p.ty, p.kind))
            .collect();
        let mut positionals = evaluated
            .iter()
            .filter(|(n, _)| n.is_none())
            .map(|(_, v)| v.clone());
        let mut exit = Vec::with_capacity(params.len());
        for (i, (name_str, ty, kind)) in params.into_iter().enumerate() {
            let datum = if kind == ParameterKind::Named {
                evaluated
                    .iter()
                    .find(|(n, _)| n.is_some() && *n == name_str)
                    .map(|(_, v)| v.clone())
            } else {
                positionals.next()
            };
            let repr = self.repr(ty);
            let v = match datum {
                Some(v) => v,
                None => self
                    .default_value(fid, i)
                    .unwrap_or_else(|| Self::zero_value(repr)),
            };
            exit.push(self.coerce(v, repr));
        }
        exit
    }

    /// Representação do retorno de uma função (Void para construtor).
    pub fn return_repr(&self, fid: usize) -> Type {
        if super::generative_constructor(self.ctx, fid) {
            return Type::Void;
        }
        // Construtor de tipo de extensão: a representação.
        let f = &self.ctx.program.functions[fid];
        if matches!(f.node, FunctionRef::Constructor { .. })
            && let Some(c) = f.class.filter(|&c| self.ctx.is_extension_type(c))
        {
            return self.ctx.te.this.get(&c).map_or(Type::Ref, |&t| self.repr(t));
        }
        self.ctx.hir_return(fid)
    }

    /// Chamada direta a uma função do usuário, com `this` quando é membro.
    pub fn call_direct(
        &mut self,
        fid: usize,
        this: Option<Operand>,
        args: Vec<Operand>,
    ) -> Operand {
        if let Some((alvo, shape)) = self.redirected_ffi_call(fid) {
            // `Pointer.fromFunction`, `NativeCallable.*`, `Struct.create`: o
            // ajudante da sobreposição tem os mesmos parâmetros
            // (`lower/ffi.rs`).
            let mut args = args;
            let saved = self.armed_tuple.clone();
            match shape {
                super::ffi::Redirection::DirectOnly => {}
                super::ffi::Redirection::ClassTuple => self.armed_tuple = args.pop(),
                super::ffi::Redirection::WithSite => {
                    // O sítio, estável entre compilações (símbolo da função
                    // e ordem no corpo).
                    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                    for b in self.func.symbol.bytes().chain(self.callback_sites.to_le_bytes()) {
                        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                    }
                    self.callback_sites += 1;
                    args.push(Operand::Constant(Constant::Int((h >> 1) as i64)));
                }
            }
            let r = self.call_direct(alvo, this, args);
            self.armed_tuple = saved;
            return r;
        }
        if let Some(r) = self.call_external(fid, this.clone(), &args) {
            return r;
        }
        let symbol = super::symbol_of(self.ctx, fid);
        let ret_ty = self.return_repr(fid);
        let mut all = Vec::with_capacity(args.len() + 2);
        all.extend(this);
        all.extend(args);
        // RTI: a função genérica recebe a tupla dos argumentos de tipo no
        // último parâmetro (a da chamada corrente, ou 0 = `dynamic`).
        if self.generic_function(fid) {
            all.push(self.armed_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0))));
        }
        let r = self.emit_call_with_check(
            Instruction::CallStatic {
                symbol,
                args: all,
                ret_ty,
            },
            ret_ty,
        );
        if ret_ty == Type::Void {
            Operand::Constant(Constant::Null)
        } else {
            r
        }
    }

    /// Implementações concretas de um membro de instância, por classe do
    /// usuário que é subtipo da classe declarante.
    fn implementations(&self, decl_fid: usize) -> Vec<(u32, usize)> {
        let ctx = self.ctx;
        let decl = &ctx.program.functions[decl_fid];
        let Some(cdecl) = decl.class else {
            return Vec::new();
        };
        // Getters e setters compartilham `FunctionElement.name`, mas o
        // outline guarda o setter na chave distinta `x_=`. Despachar por
        // `x` chamaria o getter e descartaria o valor atribuído.
        let name_str = if decl.kind == FunctionKind::Setter {
            let key = format!("{}_=", ctx.symbol_name(decl.name));
            let Some(sym) = ctx.interner.lookup(&key) else { return Vec::new() };
            sym
        } else {
            decl.name
        };
        let mut exit = Vec::new();
        for &kid in ctx.subtypes(cdecl).iter() {
            let class_ref = &ctx.program.classes[kid.0 as usize];
            if !ctx.compiled_library(class_ref.library) {
                continue;
            }
            if (class_ref.modifiers.abstract_ && kid != cdecl) || is_mixin(ctx, kid) {
                continue;
            }
            for c in crate::lower::members::linearization(self.ctx, kid) {
                let cl = &ctx.program.classes[c.0 as usize];
                if let Some(&f) = cl.instance_members.get(&name_str) {
                    let f = f.0 as usize;
                    if super::user_function(ctx, f) && has_body(ctx, f) {
                        if let Some(id) = ctx.class_id_of(kid) {
                            exit.push((id, f));
                        }
                        break;
                    }
                }
            }
        }
        exit
    }

    /// Chamada de método/getter/setter de instância com despacho pela classe
    /// dinâmica do receptor quando alguma subclasse sobrescreve o membro.
    ///
    /// Não há vtable ainda: um `switch` sobre `dartforge_value_class`
    /// escolhe a implementação. Cada alvo casa os argumentos com os SEUS
    /// parâmetros (padrões e representação podem diferir entre overrides) e
    /// o resultado é coagido para a representação do membro declarado.
    pub fn call_member(
        &mut self,
        recv: Operand,
        decl_fid: usize,
        evaluated: &[Evaluated],
        _span: Span,
    ) -> Operand {
        if let Some(r) = self.call_source_member(recv.clone(), decl_fid, evaluated) {
            return r;
        }
        let impls = self.implementations(decl_fid);
        let mut distinct: Vec<usize> = impls.iter().map(|(_, f)| *f).collect();
        distinct.sort_unstable();
        distinct.dedup();
        let ret = self.return_repr(decl_fid);
        if distinct.is_empty() {
            // Getter implícito `index`/`name` de um enum do programa
            // (especificação §13 "Enums"): o elemento resolvido não tem corpo
            // — o valor mora nos dois campos implícitos do objeto (posições 0
            // e 1, gravados pelo construtor do valor). O caminho explícito
            // (`E.a.name`, `enum_member`) lê os mesmos campos; o implícito
            // (`$name`, `index`, `this.name` no corpo de um membro do enum)
            // chegava aqui como "sem implementação compilada".
            if evaluated.is_empty()
                && let Some(cid) = self.ctx.program.functions[decl_fid].class
                && super::enums::is_enum(self.ctx, cid)
            {
                let name_str = self.ctx.symbol_name(self.ctx.program.functions[decl_fid].name).to_string();
                if (name_str == "index" || name_str == "name")
                    && let Some(r) = self.enum_member(cid, &name_str, recv.clone())
                {
                    return self.coerce(r, ret);
                }
            }
            if has_body(self.ctx, decl_fid) && super::user_function(self.ctx, decl_fid) {
                distinct.push(decl_fid);
            } else {
                // Nenhuma classe do programa implementa o membro: o que a
                // alcança só pode ser um encaminhador de `noSuchMethod`
                // (classe com `noSuchMethod` que não o declara). Pelo
                // seletor, a falta na tabela chega ao `noSuchMethod` do
                // receptor com o `Invocation`, como na VM.
                use super::sdk_source::AccessKind;
                let f = &self.ctx.program.functions[decl_fid];
                let ty = match f.kind {
                    FunctionKind::Getter => AccessKind::Read,
                    FunctionKind::Setter => AccessKind::Write,
                    FunctionKind::ImplicitAccessor if evaluated.len() == 1 => AccessKind::Write,
                    FunctionKind::ImplicitAccessor => AccessKind::Read,
                    _ => AccessKind::Call,
                };
                let name_str = self.ctx.symbol_name(f.name).to_string();
                let s = super::sdk_source::selector_string(self.ctx, ty, &name_str, f.library);
                let tuple = if self.generic_function(decl_fid) {
                    self.armed_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0)))
                } else {
                    Operand::Constant(Constant::Int(0))
                };
                // O encaminhador da VM passa todos os parâmetros do membro,
                // com os padrões dos opcionais omitidos.
                let complete_set: Vec<Evaluated> = if matches!(ty, AccessKind::Call) {
                    let values = self.match_args(decl_fid, evaluated);
                    self.ctx.outline.functions[decl_fid]
                        .parameters
                        .iter()
                        .zip(values)
                        .map(|(p, v)| (if p.kind == ParameterKind::Named { p.externo } else { None }, v))
                        .collect()
                } else {
                    evaluated.to_vec()
                };
                let r = self.call_by_selector_with_tuple(recv, s, &complete_set, tuple);
                return if ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) };
            }
        }
        if distinct.len() == 1 {
            let alvo = distinct[0];
            self.check_target_covariants(alvo, &recv, evaluated);
            let args = self.match_args(alvo, evaluated);
            let r = self.call_direct(alvo, Some(recv), args);
            return if ret == Type::Void {
                r
            } else {
                self.coerce(r, ret)
            };
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let pattern_fid = if distinct.contains(&decl_fid) {
            decl_fid
        } else {
            distinct[0]
        };
        let mut blocks: Vec<(usize, BlockId)> = Vec::new();
        for &f in &distinct {
            let b = self.new_block();
            blocks.push((f, b));
        }
        let block_of = |f: usize| {
            blocks
                .iter()
                .find(|(g, _)| *g == f)
                .map(|(_, b)| *b)
                .expect("bloco do alvo")
        };
        let cases: Vec<(i64, BlockId)> = impls
            .iter()
            .map(|(cid, f)| (i64::from(*cid), block_of(*f)))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: block_of(pattern_fid),
            cases,
        });
        let mut entries: Vec<(BlockId, Operand)> = Vec::new();
        for (f, b) in blocks.clone() {
            self.set_block(b);
            self.check_target_covariants(f, &recv, evaluated);
            let args = self.match_args(f, evaluated);
            let r = self.call_direct(f, Some(recv.clone()), args);
            let r = if ret == Type::Void {
                r
            } else {
                self.coerce(r, ret)
            };
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(join));
            }
        }
        self.set_block(join);
        if ret == Type::Void || entries.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: ret,
            },
            ret,
        )
    }

    // ------------------------------------------------------------------
    // Construtores (N5)

    /// `C(args)`: aloca com o layout da classe e chama o construtor.
    pub fn instantiate(
        &mut self,
        ast: &ast::Ast,
        ctor_fid: FunctionElementId,
        args: &[ast::Argument],
        span: Span,
    ) -> Operand {
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid) = f.class else {
            return self.unsupported("construtor sem classe", span);
        };
        // As factories `const` de ambiente são resolvidas pela CFE na
        // compilação. A CLI nativa ainda não aceita `-D`, portanto nenhuma
        // chave foi definida e vale o `defaultValue` especificado pelo SDK.
        // Nunca chamamos o native da VM para essas factories.
        if f.const_ && f.external {
            let name_str = self.ctx.symbol_name(f.name);
            let class_ref = if Some(cid) == self.ctx.core.bool_class {
                Some("bool")
            } else if Some(cid) == self.ctx.core.int_class {
                Some("int")
            } else if Some(cid) == self.ctx.core.string_class {
                Some("String")
            } else {
                None
            };
            if class_ref == Some("bool") && name_str == "hasEnvironment" {
                return self.emit(Instruction::Const(Constant::Bool(false)), Type::I1);
            }
            if name_str == "fromEnvironment" {
                if let Some(class_ref) = class_ref {
                    if let Some(val_ref) = args.iter().find(|a| {
                        a.name.as_ref().is_some_and(|n| self.ctx.symbol_name(n.sym) == "defaultValue")
                    }) {
                        return self.lower_in_const_context(ast, val_ref.value);
                    }
                    return match class_ref {
                        "bool" => self.emit(Instruction::Const(Constant::Bool(false)), Type::I1),
                        "int" => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                        _ => self.emit(Instruction::Const(Constant::String(String::new())), Type::Ref),
                    };
                }
            }
        }
        // RTI: o tipo estático da criação (`C<T…>`), gravado por quem chama.
        let ty = self.creation_type.take();
        if !super::user_function(self.ctx, fid) {
            let evaluated = self.evaluate_args(ast, args);
            self.creation_type = ty;
            let r = self.runtime_error_constructor(fid, &evaluated);
            self.creation_type = None;
            if let Some(op) = r {
                return op;
            }
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("construtor de classe do SDK ({name_str})"), span);
        }
        // A classe concreta (aplicação de mixin) vale para esta criação, não
        // para as que a avaliação dos argumentos fizer.
        let concrete = self.concrete_class.take();
        let armed = self.arm_constructor_types(fid, ty, args);
        let evaluated = self.evaluate_args(ast, args);
        self.arg_types = armed;
        self.creation_type = ty;
        self.concrete_class = concrete;
        self.instantiate_evaluated(ctor_fid, &evaluated, span)
    }

    /// O construtor `name` da classe `cid`. Uma aplicação de mixin
    /// (`class C = S with M;`, especificação §12.1) não declara construtores:
    /// ela tem um construtor de encaminhamento para cada construtor
    /// generativo da superclasse. Aí devolve o da superclasse (subindo pelas
    /// aplicações encadeadas) e anota `cid` como a classe concreta da criação
    /// seguinte — o objeto é de `cid`, e os campos dos mixins são
    /// inicializados antes do construtor da superclasse.
    pub fn constructor_of(&mut self, cid: ClassId, name_str: SymbolId) -> Option<FunctionElementId> {
        let mut c = cid;
        loop {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class_ref.constructors.get(&name_str) {
                if c != cid {
                    if self.ctx.program.functions[f.0 as usize].factory {
                        return None;
                    }
                    self.concrete_class = Some(cid);
                }
                return Some(f);
            }
            if class_ref.kind != dartforge_elements::model::ClassKind::MixinApplication || !class_ref.constructors.is_empty() {
                return None;
            }
            c = class_ref.supertype_class?;
        }
    }

    /// `C(args)` com os argumentos já avaliados.
    pub fn instantiate_evaluated(&mut self, ctor_fid: FunctionElementId, evaluated: &[Evaluated], span: Span) -> Operand {
        self.instantiate_evaluated_with_rti(ctor_fid, evaluated, span, None, None)
    }

    /// Uma factory redirecionadora pode mudar a classe e até permutar seus
    /// argumentos de tipo. Nesse caso, o RTI e a tupla do alvo vêm da
    /// anotação `= Destino<U, T>`, não do tipo de retorno da origem.
    pub fn instantiate_evaluated_with_rti(
        &mut self,
        ctor_fid: FunctionElementId,
        evaluated: &[Evaluated],
        span: Span,
        explicit_rti: Option<Operand>,
        explicit_tuple: Option<Operand>,
    ) -> Operand {
        let ty = self.creation_type.take();
        let concrete = self.concrete_class.take();
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid_ctor) = f.class else {
            return self.unsupported("construtor sem classe", span);
        };
        // O objeto é da classe concreta (aplicação de mixin) quando o
        // construtor é o da superclasse que ela encaminha.
        let cid = concrete.unwrap_or(cid_ctor);
        // A factory `core.Symbol` redireciona para a classe concreta da
        // biblioteca interna. O alvo é inequívoco e evita que a resolução
        // da factory volte à própria declaração abstrata.
        if Some(cid) == self.ctx.sdk_class("core", "Symbol") {
            if let Some(concrete) = self.ctx.sdk_class("_internal", "Symbol")
                && let Some(empty) = self.ctx.interner.lookup("")
                && let Some(&constructor) = self.ctx.program.classes[concrete.0 as usize].constructors.get(&empty)
            {
                return self.instantiate_evaluated_with_rti(constructor, evaluated, span, explicit_rti, explicit_tuple);
            }
        }
        if !super::user_function(self.ctx, fid) {
            if let Some(op) = self.runtime_error_constructor(fid, evaluated) {
                return op;
            }
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("construtor de classe do SDK ({name_str})"), span);
        }
        // O construtor de tipo de extensão (`extension_types.rs`) é uma
        // função que devolve a representação, chamada como uma fábrica.
        let factory = f.factory || self.ctx.is_extension_type(cid_ctor);
        let mut args = self.match_args(fid, evaluated);
        let generic = self.generic_class(cid);
        if factory {
            // RTI: a fábrica de uma classe genérica recebe os argumentos de
            // tipo da classe na tupla (o último parâmetro).
            if generic {
                let t = explicit_tuple.unwrap_or_else(|| self.creation_tuple(ty));
                args.push(t);
            }
            return self.call_direct(fid, None, args);
        }
        let fields = layout(self.ctx, cid).len() + super::enums::layout_base(self.ctx, cid);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (
                        Operand::Constant(Constant::Int(i64::from(self.ctx.class_id_of(cid).unwrap_or(0)))),
                        Type::I64,
                    ),
                    (Operand::Constant(Constant::Int(fields as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // RTI: a instância de classe genérica guarda o tipo (`C<T…>`).
        if generic && let Some(r) = explicit_rti.or_else(|| ty.map(|t| self.type_rti(t))) {
            self.define_rti(obj.clone(), r);
        }
        if concrete.is_some() {
            self.initialize_application_mixins(obj.clone(), cid, cid_ctor);
        }
        self.call_direct(fid, Some(obj.clone()), args);
        obj
    }

    /// Os construtores de encaminhamento das aplicações de mixin de `de` até
    /// `until` (exclusive): cada um inicializa os campos dos seus mixins, do
    /// último para o primeiro, antes do construtor da superclasse.
    fn initialize_application_mixins(&mut self, obj: Operand, de: ClassId, until: ClassId) {
        let saved = self.this_param.replace(obj);
        let saved_finalizable = self.finalizable_this;
        self.finalizable_this = self.ctx.classify_finalizable_class(de);
        let mut c = de;
        while c != until {
            let class_ref = &self.ctx.program.classes[c.0 as usize];
            let mixins = class_ref.mixin_classes.clone();
            let sup = class_ref.supertype_class;
            for m in mixins.into_iter().rev() {
                if self.ctx.compiled_library(self.ctx.program.classes[m.0 as usize].library) {
                    self.initialize_fields(m);
                }
            }
            let Some(s) = sup else { break };
            c = s;
        }
        self.this_param = saved;
        self.finalizable_this = saved_finalizable;
    }

    /// A função (não construtor) declara parâmetros de tipo: recebe a tupla.
    /// Um membro de instância de extensão genérica também recebe: os
    /// argumentos da extensão vêm primeiro (`type_params_of`).
    pub fn generic_function(&self, fid: usize) -> bool {
        !matches!(self.ctx.program.functions[fid].node, FunctionRef::Constructor { .. })
            && !self.type_params_of(fid).is_empty()
            && super::user_function(self.ctx, fid)
    }

    /// Os parâmetros de tipo que a tupla de `fid` carrega, na ordem: os da
    /// extensão (membro de instância; um estático não os vê) e os próprios.
    pub fn type_params_of(&self, fid: usize) -> Vec<TypeParamId> {
        let f = &self.ctx.program.functions[fid];
        let mut v = Vec::new();
        if let Some(e) = f.extension.filter(|_| !f.static_)
            && let Some(x) = self.ctx.outline.extensions.get(e.0 as usize)
        {
            v.extend(x.type_params.iter().copied());
        }
        // Membro de instância de tipo de extensão: os do tipo, como os de
        // uma extensão (não há objeto que os guarde).
        if let Some(c) = super::extension_types::instance_owner(self.ctx, fid)
            && let Some(d) = self.ctx.outline.classes.get(c.0 as usize)
        {
            v.extend(d.type_params.iter().copied());
        }
        if let Some(d) = self.ctx.outline.functions.get(fid) {
            v.extend(d.type_params.iter().copied());
        }
        v
    }

    /// A classe declara parâmetros de tipo.
    pub fn generic_class(&self, cid: ClassId) -> bool {
        self.ctx.outline.classes.get(cid.0 as usize).is_some_and(|d| !d.type_params.is_empty())
    }

    /// A tupla de argumentos de tipo (`L<…>`) do tipo de uma criação; sem
    /// tipo, 0 (os argumentos ficam `dynamic`).
    pub fn creation_tuple(&mut self, ty: Option<TypeId>) -> Operand {
        let Some(t) = ty else {
            return Operand::Constant(Constant::Int(0));
        };
        // (O de um tipo de extensão também: seus construtores recebem a
        // tupla como as fábricas.)
        let (dartforge_types::table::Type::Interface { args, .. } | dartforge_types::table::Type::ExtensionType { args, .. }) =
            self.ctx.table.get(t)
        else {
            return Operand::Constant(Constant::Int(0));
        };
        let args = args.clone();
        self.rti_type_tuple(&args)
    }

    /// A tupla `L<a…>` dos tipos `args`, como `I64`, no ambiente corrente.
    pub fn rti_type_tuple(&mut self, args: &[TypeId]) -> Operand {
        let mut r = super::rti::Recipe { text: "L<".to_string(), variables: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = self.type_recipe(*a);
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        self.recipe_rti(&r)
    }

    /// Inicializadores de campo da própria classe, na ordem de declaração.
    ///
    /// Os de um mixin (ou de uma superclasse sem construtor) rodam no
    /// construtor de quem o aplica, mas são código de `cid`: as variáveis de
    /// tipo deles são as de `cid`, e o `this` é visto como `cid` (o `reader =
    /// FlowLinkReader<PromotionInfo<Type>>()` do `FlowModelHelper<Type>` do
    /// analyzer, aplicado a uma classe cujo primeiro parâmetro é outro).
    fn initialize_fields(&mut self, cid: ClassId) {
        let saved = self.enclosing_class.replace(cid);
        self.initialize_fields_of(cid);
        self.enclosing_class = saved;
    }

    fn initialize_fields_of(&mut self, cid: ClassId) {
        let this = self.this_param.clone().expect("construtor tem this");
        let fields: Vec<VariableId> = self.ctx.program.classes[cid.0 as usize].fields.clone();
        for vid in fields {
            let var = &self.ctx.program.variables[vid.0 as usize];
            if var.static_ || var.late {
                continue;
            }
            let dartforge_elements::model::VariableRef::Field { unit, .. } = var.node else {
                continue;
            };
            let Some(init) = self.variable_initializer_in(vid) else {
                continue;
            };
            let v = self.lower_expr_of(unit, init);
            let span = self.ctx.program.unit(unit).ast.expr(init).span;
            self.store_field(this.clone(), vid, v, span);
        }
    }

    /// Chama o construtor da superclasse do usuário, se houver.
    fn chamar_super(
        &mut self,
        cid: ClassId,
        name_str: Option<SymbolId>,
        evaluated: Vec<Evaluated>,
        span: Span,
    ) {
        // Os mixins aplicados rodam os inicializadores dos seus campos antes
        // do construtor da superclasse, do último para o primeiro (cada
        // aplicação `S with M` é uma classe cujo construtor inicializa `M`
        // e chama o de `S`).
        let mixins: Vec<ClassId> = self.ctx.program.classes[cid.0 as usize].mixin_classes.clone();
        for m in mixins.into_iter().rev() {
            if self.ctx.compiled_library(self.ctx.program.classes[m.0 as usize].library) {
                self.initialize_fields(m);
            }
        }
        let Some(sup) = self.ctx.program.classes[cid.0 as usize].supertype_class else {
            return;
        };
        let sup_class = &self.ctx.program.classes[sup.0 as usize];
        if !self.ctx.compiled_library(sup_class.library) {
            // `Object()` e superclasses do SDK: nada a executar no nosso heap.
            return;
        }
        // `Object()` (`const Object();`): sem corpo, sem inicializadores e sem
        // superclasse — a chamada não faz nada (e custava uma chamada, uma
        // conferência de exceção e um quadro por objeto criado).
        if sup_class.supertype_class.is_none()
            && self.ctx.program.library(sup_class.library).uri == "dart:core"
            && self.ctx.symbol_name(sup_class.name) == "Object"
        {
            return;
        }
        let Some(empty) = self.ctx.interner.lookup("") else {
            return;
        };
        let s = name_str.unwrap_or(empty);
        if name_str.is_none() && sup_class.constructors.is_empty() {
            // Superclasse sem construtor declarado e sem o sintético (a
            // classe abstrata não o ganha no elemento): o construtor padrão
            // implícito — inicializadores de campo e o `super()` dela.
            self.initialize_fields(sup);
            self.chamar_super(sup, None, Vec::new(), span);
            return;
        }
        let Some(&sf) = sup_class.constructors.get(&s) else {
            self.unsupported("construtor da superclasse não encontrado", span);
            return;
        };
        let sf = sf.0 as usize;
        let args = self.match_args(sf, &evaluated);
        let this = self.this_param.clone().expect("construtor tem this");
        self.call_direct(sf, Some(this), args);
    }

    /// Corpo de um construtor generativo, na ordem da especificação (N5).
    pub fn lower_constructor(
        &mut self,
        ast: &ast::Ast,
        cid: ClassId,
        ctor: &ast::Constructor,
        span_ctor: Span,
    ) {
        let this = self.this_param.clone().expect("construtor tem this");
        let class_ref = &self.ctx.program.classes[cid.0 as usize];

        // `: this(…)` delega tudo para o outro construtor.
        if let Some((alvo, args, span)) = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Redirect {
                constructor,
                arguments,
                span,
            } => Some((*constructor, arguments, *span)),
            _ => None,
        }) {
            let Some(empty) = self.ctx.interner.lookup("") else {
                return;
            };
            let s = alvo.map(|n| n.sym).unwrap_or(empty);
            let Some(&tf) = class_ref.constructors.get(&s) else {
                self.unsupported("construtor redirecionado não encontrado", span);
                return;
            };
            let evaluated = self.evaluate_args(ast, &args.args);
            let args = self.match_args(tf.0 as usize, &evaluated);
            self.call_direct(tf.0 as usize, Some(this), args);
            self.terminate(Terminator::Return(None));
            return;
        }

        // 1. Inicializadores de campo da própria classe.
        self.initialize_fields(cid);

        // 2. Parâmetros `this.x`.
        for p in ctor.parameters.iter() {
            if !p.this_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(vid) = self.own_field(cid, n.sym) else {
                self.unsupported("parâmetro this.x sem campo", p.span);
                continue;
            };
            let v = self.read_local_by_name(n.sym)
                .expect("parâmetro declarado");
            self.store_field(this.clone(), vid, v, p.span);
        }

        // 3. Lista de inicialização, na ordem escrita; os argumentos do
        //    `super(…)` são avaliados na posição dele.
        let mut explicit_super: Option<(Option<SymbolId>, Vec<Evaluated>, Span)> = None;
        for init in ctor.initializers.iter() {
            match init {
                ast::Initializer::Field {
                    name, value, span, ..
                } => {
                    let Some(vid) = self.own_field(cid, name.sym) else {
                        self.unsupported("inicializador de campo inexistente", *span);
                        continue;
                    };
                    let v = self.lower_expr(ast, *value);
                    self.store_field(this.clone(), vid, v, *span);
                }
                ast::Initializer::Assert {
                    condition, message, ..
                } => {
                    self.lower_assert(ast, *condition, *message);
                }
                ast::Initializer::Super {
                    constructor,
                    arguments,
                    span,
                } => {
                    let evaluated = self.evaluate_args(ast, &arguments.args);
                    explicit_super = Some((constructor.map(|n| n.sym), evaluated, *span));
                }
                ast::Initializer::Redirect { .. } => {}
            }
        }

        // 4. `super(…)`: explícito ou implícito, mais os `super.x`.
        let (super_name, mut evaluated, span_super) =
            explicit_super.unwrap_or((None, Vec::new(), span_ctor));
        for p in ctor.parameters.iter().filter(|p| p.super_) {
            let Some(n) = p.name else { continue };
            let v = self.read_local_by_name(n.sym)
                .expect("parâmetro declarado");
            let name_str = if p.kind == ParameterKind::Named {
                Some(n.sym)
            } else {
                None
            };
            evaluated.push((name_str, v));
        }
        // Posicionais `super.x` entram depois dos explícitos, na ordem.
        evaluated.sort_by_key(|(n, _)| n.is_some());
        self.chamar_super(cid, super_name, evaluated, span_super);

        // 5. Corpo. Os `this.x` não estão no escopo do corpo: `x` ali é o
        //    campo, e a resolução (`Resolved::Member`) já diz isso.
        for p in ctor.parameters.iter() {
            if p.this_
                && let Some(n) = p.name
            {
                self.remove_local(n.sym);
            }
        }
        match &ctor.body {
            FunctionBody::Block(s) => self.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                self.lower_expr(ast, *e);
            }
            _ => {}
        }
    }

    /// Construtor sintético: inicializadores de campo e `super()`.
    pub fn lower_synthetic_constructor(&mut self, ast: &ast::Ast, cid: ClassId) {
        self.initialize_fields(cid);
        let span = self.ctx.program.classes[cid.0 as usize]
            .decl
            .map(|d| ast.decl(d.decl).span)
            .unwrap_or(Span { start: 0, end: 0 });
        self.chamar_super(cid, None, Vec::new(), span);
    }

    // ------------------------------------------------------------------
    // Globais (N6)

    /// Constrói o `LateError` do SDK e lança no fluxo de exceções do HIR.
    /// `code`: 0/1 campo não inicializado/já inicializado; 2/3 local.
    pub fn throw_late_error(&mut self, name_str: &str, code: i64) {
        let n = self.emit(Instruction::Const(Constant::String(name_str.to_string())), Type::Ref);
        let error = self.new_runtime_error("dartforge_late_error_new", vec![(n, Type::Ref), (Operand::Constant(Constant::Int(code)), Type::I64)]);
        self.emit_throw_op(error);
    }

    /// Corpo do getter preguiçoso de um global.
    pub fn lower_global_getter(&mut self, vid: VariableId, repr: Type) {
        // O getter devolve o valor como as funções Dart: Ref é Owned.
        if self.ctx.arc_memory && repr == Type::Ref {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let val_ref = super::global_value_symbol(self.ctx, vid);
        let flag = format!("{val_ref}$ok");
        let Some(init) = self.variable_initializer_in(vid) else {
            if self.ctx.program.variables[vid.0 as usize].late {
                let ok = self.emit(Instruction::LoadGlobal { symbol: flag, ty: Type::I8 }, Type::I8);
                let ready = self.emit(
                    Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_read = self.new_block();
                let b_error = self.new_block();
                self.terminate(Terminator::CondBranch { cond: ready, then_block: b_read, else_block: b_error });
                self.set_block(b_error);
                let name_str = self.ctx.symbol_name(self.ctx.program.variables[vid.0 as usize].name).to_string();
                self.throw_late_error(&name_str, 0);
                self.set_block(b_read);
            }
            let v = self.emit(
                Instruction::LoadGlobal {
                    symbol: val_ref,
                    ty: repr,
                },
                repr,
            );
            self.terminate(Terminator::Return(Some(v)));
            return;
        };
        let ok = self.emit(
            Instruction::LoadGlobal {
                symbol: flag.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok.clone(), Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ready,
            then_block: b_read,
            else_block: b_init,
        });
        self.set_block(b_read);
        let v = self.emit(
            Instruction::LoadGlobal {
                symbol: val_ref.clone(),
                ty: repr,
            },
            repr,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        let reentrant = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok, Operand::Constant(Constant::Int(2))),
            Type::I1,
        );
        let b_stack = self.new_block();
        let b_evaluate = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: reentrant,
            then_block: b_stack,
            else_block: b_evaluate,
        });
        self.set_block(b_stack);
        let error = self.new_runtime_error("dartforge_stack_overflow_error_new", Vec::new());
        self.emit_throw_op(error);
        self.set_block(b_evaluate);
        // 0 = não iniciado; 2 = avaliando; 1 = pronto. A leitura reentrante
        // produz `StackOverflowError` do SDK sem expandir a pilha nativa.
        self.emit(
            Instruction::StoreGlobal {
                symbol: flag,
                val: Operand::Constant(Constant::Int(2)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let unit = self.unit_id;
        let is_const = self.ctx.program.variables[vid.0 as usize].const_;
        let saved = std::mem::replace(&mut self.in_const_context, is_const);
        // Se o inicializador lança, a variável continua não inicializada
        // (§10 "Variables": a próxima leitura roda o inicializador de novo):
        // a bandeira volta a 0 e a exceção segue pendente.
        let b_fail = self.new_block();
        self.exception_targets.push(b_fail);
        let v = self.lower_expr_of(unit, init);
        self.exception_targets.pop();
        self.in_const_context = saved;
        let continues = self.current_block;
        self.set_block(b_fail);
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(0)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Self::zero_value(repr))));
        self.set_block(continues);
        let v = self.coerce(v, repr);
        // A `const` de string montada (`'a${'b'}'`) é o literal canônico de
        // mesmo conteúdo (`constants.rs`).
        let v = if is_const && repr == Type::Ref {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_constante_canonica".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            v
        };
        let root = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                symbol: val_ref,
                val: v.clone(),
                ty: repr,
                root,
            },
            Type::Void,
        );
        if is_const && repr == Type::Ref {
            self.emit(
                Instruction::CallRuntime {
                name: "dartforge_marcar_constante".to_string(),
                args: vec![(v.clone(), Type::Ref), (Operand::Constant(Constant::Function(self.func.symbol.clone())), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Lê um global pelo getter preguiçoso.
    pub fn read_global(&mut self, vid: VariableId, span: Span) -> Operand {
        if let Some(v) = self.read_native_variable(vid, span) {
            return v;
        }
        // `const` primitiva: a constante, sem o getter (`primitive_const.rs`).
        if let Some(v) = self.read_primitive_const(vid) {
            return v;
        }
        if !self.ctx.compiled_library(self.ctx.program.variables[vid.0 as usize].library)
        {
            let name_str = self
                .ctx
                .symbol_name(self.ctx.program.variables[vid.0 as usize].name)
                .to_string();
            return self.unsupported(&format!("global do SDK ({name_str})"), span);
        }
        let repr = self.repr(variable_type(self.ctx, vid));
        let v = self.emit_call_with_check(
            Instruction::CallStatic {
                symbol: super::global_symbol(self.ctx, vid),
                args: Vec::new(),
                ret_ty: repr,
            },
            repr,
        );
        if let Operand::Val(id) = &v {
            self.read_from_global.insert(*id);
        }
        v
    }

    /// Grava um global (e marca-o inicializado).
    pub fn store_global(&mut self, vid: VariableId, val: Operand, span: Span) -> Operand {
        if self.store_native_variable(vid, val.clone(), span) {
            return val;
        }
        if !self.ctx.compiled_library(self.ctx.program.variables[vid.0 as usize].library)
        {
            return self.unsupported("atribuição a global do SDK", span);
        }
        let repr = self.repr(variable_type(self.ctx, vid));
        let val = self.coerce(val, repr);
        if !self.ctx.library_in_module(self.ctx.program.variables[vid.0 as usize].library) {
            // O global mora em outro módulo (SDK da fonte): pelo setter dele.
            self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: format!("{}$set", super::global_symbol(self.ctx, vid)),
                    args: vec![val.clone()],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            return val;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if var.late && var.final_ {
            let flag = format!("{}$ok", super::global_value_symbol(self.ctx, vid));
            let ok = self.emit(Instruction::LoadGlobal { symbol: flag, ty: Type::I8 }, Type::I8);
            let already_initialized = self.emit(
                Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_error = self.new_block();
            let b_store = self.new_block();
            self.terminate(Terminator::CondBranch { cond: already_initialized, then_block: b_error, else_block: b_store });
            self.set_block(b_error);
            let name_str = self.ctx.symbol_name(var.name).to_string();
            self.throw_late_error(&name_str, 1);
            self.set_block(b_store);
        }
        self.emit(
            Instruction::StoreGlobal {
                symbol: format!("{}$ok", super::global_value_symbol(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let root = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                symbol: super::global_value_symbol(self.ctx, vid),
                val: val.clone(),
                ty: repr,
                root,
            },
            Type::Void,
        );
        val
    }

    /// O construtor `name` (ou o sem nome) da classe do programa nomeada por
    /// um tipo anotado.
    pub fn type_constructor(
        &self,
        ty: &dartforge_frontend::ast::TypeAnnotation,
        name_str: Option<dartforge_intern::SymbolId>,
    ) -> Option<FunctionElementId> {
        let dartforge_frontend::ast::TypeKind::Named { name, .. } = &ty.kind else {
            return None;
        };
        let lib = self.ctx.program.unit(self.unit_id).library;
        let local_class = |s: dartforge_intern::SymbolId| match self.ctx.program.lookup(lib, s).and_then(|b| b.getter) {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        };
        let prefixed_class = |p: dartforge_intern::SymbolId, s: dartforge_intern::SymbolId| {
            match self.ctx.program.lookup_prefixed(lib, p, s).and_then(|b| b.getter) {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        };
        // `= Alvo`, `= Alvo.name`, `= p.Alvo` e `= p.Alvo.name` (o parser lê
        // `Alvo.name` como um nome de tipo em duas partes, e `p.Alvo.name`
        // em três; o `= t.Template.fromSource` do `mustache_template`).
        let (c, name_str) = match (&name[..], name_str) {
            ([c, n], None) if local_class(c.sym).is_some() => (local_class(c.sym)?, Some(n.sym)),
            ([p, c], n) if prefixed_class(p.sym, c.sym).is_some() => (prefixed_class(p.sym, c.sym)?, n),
            ([p, c, n], None) => (prefixed_class(p.sym, c.sym)?, Some(n.sym)),
            (parts, n) => (local_class(parts.last()?.sym)?, n),
        };
        let key = name_str.or_else(|| self.ctx.interner.lookup(""))?;
        self.ctx.program.classes[c.0 as usize].constructors.get(&key).copied()
    }
}

/// Um valor padrão que não precisa de resolução (literal de null, bool,
/// número ou string sem interpolação): qualquer módulo o baixa.
pub fn literal_pattern(ast: &ast::Ast, e: ExprId) -> bool {
    use dartforge_frontend::ast::ExprKind;
    match &ast.expr(e).kind {
        ExprKind::Null | ExprKind::Bool(_) | ExprKind::Int(_) | ExprKind::Double(_) => true,
        ExprKind::String(s) => s.parts.iter().all(|p| matches!(p, ast::StringPart::Text(_))),
        ExprKind::Unary { op: ast::UnaryOp::Neg, operand } => {
            matches!(ast.expr(*operand).kind, ExprKind::Int(_) | ExprKind::Double(_))
        }
        _ => false,
    }
}

/// O padrão do parâmetro de uma função do SDK da fonte que o módulo dela
/// exporta: não literal, de uma biblioteca do SDK fora do módulo corrente.
pub fn exported_pattern(ctx: &crate::context::Context, fid: usize, unit: UnitId, e: ExprId) -> bool {
    let lib = ctx.program.functions[fid].library;
    ctx.program.library(lib).is_sdk
        && !ctx.library_in_module(lib)
        && !literal_pattern(&ctx.program.unit(unit).ast, e)
}

/// O símbolo da função que devolve o padrão do parâmetro `i` de `fid`.
pub fn pattern_symbol(ctx: &crate::context::Context, fid: usize, i: usize) -> String {
    format!("{}.padrao.{i}", super::symbol_of(ctx, fid))
}

#[cfg(test)]
mod arc_mixin_field_tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn arc_late_write_preserves_mark_and_final_guard_with_typed_representation() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry,
                "class C { late Object? ref; late final int inteiro; late bool booleano; late double real; late int preparado = 1; Object? ler() => ref; int inicializado() => preparado; void trocar() { ref = null; inteiro = 0; booleano = false; real = 0.0; } } void main() { C().trocar(); }\n").unwrap();
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
                let f = m.functions.iter().find(|f| f.name == "trocar").unwrap();
                let reads: Vec<_> = m.functions.iter().filter(|f| f.name == "ler" || f.symbol.contains("preparado"))
                    .flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).collect();
                let probes = reads.iter().filter(|(_, i, ty)| matches!(i,
                    Instruction::CallRuntime { name, ret_ty: Type::Ref, .. }
                        if name == "dartforge_arc_ler_campo_late_ref_v1") && *ty == Type::Ref).count();
                if arc {
                    assert_eq!(probes, 2, "late Ref e late escalar com inicializador usam a sondagem Ref");
                    assert!(!reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == "dartforge_object_get")));
                    assert!(reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == "dartforge_late_field_initialized")));
                } else {
                    assert_eq!(probes, 0);
                    assert!(reads.iter().any(|(_, i, _)| matches!(i,
                        Instruction::CallRuntime { name, .. } if name == "dartforge_object_get")));
                }
                let mut types = std::collections::HashSet::new();
                let mut marks = 0;
                let mut guards = 0;
                for b in &f.blocks {
                    for (pos, (_, i, _)) in b.instructions.iter().enumerate() {
                        if let Instruction::CallRuntime { name, args, .. } = i {
                            if name == "dartforge_late_field_initialized" { guards += 1; }
                            if name == "dartforge_late_field_mark_initialized" {
                                marks += 1;
                                let (obj, idx) = (&args[0].0, &args[1].0);
                                let previous_ones = &b.instructions[..pos];
                                if arc {
                                    let (_, Instruction::SetField { object, index, value }, _) = previous_ones.last().unwrap() else {
                                        panic!("marca late sem escrita tipada imediatamente anterior");
                                    };
                                    assert_eq!(object, obj);
                                    assert_eq!(*idx, Operand::Constant(Constant::Int(*index as i64)));
                                    let ty = match value {
                                        Operand::Constant(Constant::Null) => Type::Ref,
                                        Operand::Constant(Constant::Int(_)) => Type::I64,
                                        Operand::Constant(Constant::Bool(_)) => Type::I1,
                                        Operand::Constant(Constant::Double(_)) => Type::F64,
                                        Operand::Val(v) => f.blocks.iter().flat_map(|b| &b.instructions)
                                            .find(|(id, _, _)| id == v).unwrap().2,
                                        _ => panic!("valor late sem representação"),
                                    };
                                    types.insert(ty);
                                } else {
                                    assert!(matches!(&previous_ones.last().unwrap().1, Instruction::CallRuntime { name, .. }
                                        if name == "dartforge_object_set"));
                                }
                            }
                        }
                    }
                }
                assert_eq!(marks, 4);
                assert_eq!(guards, 1, "late final mantém a conferência antes da segunda escrita");
                if arc {
                    assert_eq!(types, std::collections::HashSet::from([Type::Ref, Type::I64, Type::I1, Type::F64]));
                    assert!(!f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                        matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_object_set")));
                } else {
                    assert!(!f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::SetField { .. })));
                }
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    fn applications_with_distinct_indices_carry_typed_accesses_in_arc() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "mixin M { Object? campo; int contador = 0; Object? ler() => campo; void trocar(Object? valor) { campo = valor; contador += 1; } } class Base { int a = 0; int b = 0; } class A with M {} class B extends Base with M {} void main() { A().trocar(1); B().trocar(2); A().ler(); B().ler(); }\n").unwrap();
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
                let methods: Vec<_> = m.functions.iter().filter(|f| matches!(f.name.as_str(), "ler" | "trocar")).collect();
                assert!(!methods.is_empty());
                let mut reads = std::collections::HashSet::new();
                let mut writes = std::collections::HashSet::new();
                let mut raw_items = 0;
                for (_, inst, ty) in methods.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
                    match inst {
                        Instruction::GetField { index, .. } if *ty == Type::Ref => { reads.insert(*index); }
                        Instruction::SetField { index, .. } => { writes.insert(*index); }
                        Instruction::CallRuntime { name, .. } if matches!(name.as_str(), "dartforge_object_get" | "dartforge_object_set") => raw_items += 1,
                        _ => {}
                    }
                }
                if arc { assert!(reads.len() >= 2, "{reads:?}"); assert!(writes.len() >= 2, "{writes:?}"); assert_eq!(raw_items, 0); }
                else { assert!(reads.is_empty()); assert!(writes.is_empty()); assert!(raw_items > 0); }
            }
        }).unwrap().join().unwrap();
    }
}

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
pub type Avaliado = (Option<SymbolId>, Operand);

/// A classe é um `mixin` (não instanciável; os membros entram nas classes
/// que o aplicam).
pub fn e_mixin(ctx: &Context, cid: ClassId) -> bool {
    ctx.program.classes[cid.0 as usize].kind == dartforge_elements::model::ClassKind::Mixin
}

/// A linearização de uma classe do programa (P4, especificação §12.3 "Mixin
/// Application"): `class C extends B with M1, M2` é `C`, `M2`, `M1` e depois
/// a linearização de `B` — a ordem em que um membro é procurado. Para no
/// SDK. O elemento não sintetiza a classe `B&M1`: a aplicação é lida aqui.
pub fn linearizacao(ctx: &Context, cid: ClassId) -> Vec<ClassId> {
    let mut saida = Vec::new();
    let mut atual = Some(cid);
    while let Some(c) = atual {
        let classe = &ctx.program.classes[c.0 as usize];
        if !ctx.biblioteca_compilada(classe.library) || saida.contains(&c) {
            break;
        }
        saida.push(c);
        for m in classe.mixin_classes.iter().rev() {
            if ctx.biblioteca_compilada(ctx.program.classes[m.0 as usize].library) && !saida.contains(m) {
                saida.push(*m);
            }
        }
        atual = classe.supertype_class;
    }
    saida
}

/// Campos **de instância** de toda a linearização (superclasses e mixins)
/// do usuário, da raiz para a folha (R7).
///
/// `ClassElement::fields` mistura campos de instância e estáticos; o layout
/// só tem os de instância. Superclasses do SDK não contribuem campos: o
/// runtime não tem os objetos do SDK no nosso layout. Os campos de um mixin
/// ficam entre os da superclasse e os da classe que o aplica.
pub fn layout(ctx: &Context, cid: ClassId) -> Vec<VariableId> {
    let cadeia = linearizacao(ctx, cid);
    let mut campos = Vec::new();
    for c in cadeia.into_iter().rev() {
        for &vid in &ctx.program.classes[c.0 as usize].fields {
            if !ctx.program.variables[vid.0 as usize].static_ {
                campos.push(vid);
            }
        }
    }
    campos
}

/// Tipo declarado (ou inferido) de uma variável/campo.
pub fn tipo_da_variavel(ctx: &Context, vid: VariableId) -> TypeId {
    let dados = &ctx.outline.variables[vid.0 as usize];
    dados
        .declared_type
        .or(dados.inferred)
        .unwrap_or(ctx.core.dynamic_)
}

/// `sub` é `sup` ou subtipo nominal dele (superclasse, mixin ou interface).
pub fn subclasse_de(ctx: &Context, sub: ClassId, sup: ClassId) -> bool {
    if sub == sup {
        return true;
    }
    ctx.outline
        .hierarchy
        .get(sub)
        .is_some_and(|d| d.supertypes.contains_key(&sup))
}

/// A função tem corpo que o lowering compila?
pub fn tem_corpo(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        // SDK da fonte (P5c): um `external` é implementado pelo patch ou
        // pelo native (`sdk_fonte::chamar_externo`).
        FunctionRef::Function { .. } if f.external && ctx.sdk_da_fonte => true,
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
    pub fn valor_zero(ty: Type) -> Operand {
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
    pub fn indice_campo(&self, vid: VariableId) -> Option<usize> {
        let dono = self.ctx.program.variables[vid.0 as usize].class?;
        if e_mixin(self.ctx, dono) {
            // O índice de um campo de mixin depende da classe que o aplica
            // (`campo_de_mixin`).
            return None;
        }
        let base = super::enums::base_do_layout(self.ctx, dono);
        layout(self.ctx, dono).iter().position(|&v| v == vid).map(|i| i + base)
    }

    /// Representação de um campo no objeto: a do tipo (R1), exceto o campo
    /// `late` escalar com inicializador, que mora em caixa (`Ref`) — o null
    /// é o "ainda não inicializado" (P4; a VM usa o sentinela `_sentinel`).
    pub fn repr_do_campo(&self, vid: VariableId) -> Type {
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        let var = &self.ctx.program.variables[vid.0 as usize];
        if var.late && repr != Type::Ref && self.variable_initializer_em(vid).is_some() {
            return Type::Ref;
        }
        repr
    }

    /// Converte os bits `i64` lidos do heap para a representação `repr`.
    pub fn bits_para(&mut self, bits: Operand, repr: Type) -> Operand {
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
    pub fn para_bits(&mut self, op: Operand) -> (Operand, bool) {
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
    fn indices_de_campo_de_mixin(&self, vid: VariableId) -> Vec<(i64, usize)> {
        let mut saida = Vec::new();
        for (k, classe) in self.ctx.program.classes.iter().enumerate() {
            let kid = ClassId(k as u32);
            if !self.ctx.biblioteca_compilada(classe.library) || e_mixin(self.ctx, kid) {
                continue;
            }
            let base = super::enums::base_do_layout(self.ctx, kid);
            if let (Some(id), Some(i)) = (
                self.ctx.id_de_classe(kid),
                layout(self.ctx, kid).iter().position(|&v| v == vid),
            ) {
                saida.push((id.into(), i + base));
            }
        }
        saida
    }

    /// Índice de um campo de mixin pela classe dinâmica do objeto (o código
    /// do mixin é compilado uma vez; a posição do campo é a da aplicação).
    fn indice_dinamico(&mut self, obj: Operand, vid: VariableId, span: Span) -> Option<Operand> {
        let dono = self.ctx.program.variables[vid.0 as usize].class?;
        if !e_mixin(self.ctx, dono) {
            return None;
        }
        let casos = self.indices_de_campo_de_mixin(vid);
        if casos.is_empty() {
            self.nao_suportado("campo de mixin sem classe que o aplique", span);
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
        let juncao = self.new_block();
        let blocos: Vec<(i64, usize, BlockId)> = casos.iter().map(|&(c, i)| (c, i, self.new_block())).collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocos[0].2,
            cases: blocos.iter().map(|&(c, _, b)| (c, b)).collect(),
        });
        let mut entradas = Vec::new();
        for &(_, i, b) in &blocos {
            self.set_block(b);
            entradas.push((b, Operand::Constant(Constant::Int(i as i64))));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        Some(self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty: Type::I64,
            },
            Type::I64,
        ))
    }

    /// Lê um campo de instância na representação do seu tipo declarado.
    pub fn ler_campo(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        let idx = match self.indice_campo(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.indice_dinamico(obj.clone(), vid, span) {
                Some(i) => i,
                None => return self.nao_suportado("campo fora do layout do objeto", span),
            },
        };
        let repr = self.repr_do_campo(vid);
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
        self.bits_para(bits, repr)
    }

    /// Grava um campo de instância; `is_ref` pela representação (E1).
    pub fn gravar_campo(&mut self, obj: Operand, vid: VariableId, val: Operand, span: Span) {
        let idx = match self.indice_campo(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.indice_dinamico(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.nao_suportado("campo fora do layout do objeto", span);
                    return;
                }
            },
        };
        let var = &self.ctx.program.variables[vid.0 as usize];
        let late_sem_init = var.late && self.variable_initializer_em(vid).is_none();
        let late_final = late_sem_init && var.final_;
        let nome = self.ctx.symbol_name(var.name).to_string();
        if late_final {
            let ja_inicializado = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let ja_inicializado = self.emit(
                Instruction::ICmp(ICmpOp::Ne, ja_inicializado, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_erro = self.new_block();
            let b_gravar = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: ja_inicializado,
                then_block: b_erro,
                else_block: b_gravar,
            });
            self.set_block(b_erro);
            self.lancar_erro_late(&nome, 1);
            self.set_block(b_gravar);
        }
        let repr = self.repr_do_campo(vid);
        let val = self.coagir(val, repr);
        let (bits, is_ref) = self.para_bits(val);
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
    pub fn campo_proprio(&self, cid: ClassId, nome: SymbolId) -> Option<VariableId> {
        self.ctx.program.classes[cid.0 as usize]
            .fields
            .iter()
            .copied()
            .find(|&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == nome
            })
    }

    /// Classe do usuário do tipo estático de uma expressão, se for uma.
    pub fn classe_do_usuario_de(&self, e: ExprId) -> Option<ClassId> {
        let ty = self.ctx.get_type(self.unit_id, e)?;
        match self.ctx.table.get(ty) {
            dartforge_types::table::Type::Interface { class, .. } => {
                let classe = &self.ctx.program.classes[class.0 as usize];
                (self.ctx.biblioteca_compilada(classe.library)).then_some(*class)
            }
            _ => None,
        }
    }

    /// Membro de classe do usuário acessado em `recv.nome` (R7).
    ///
    /// Primeiro o elemento que a inferência resolveu para o acesso; se ela
    /// não resolveu (receptor anulável de `?.` ou `x!`, que a busca de
    /// membros do `types` ainda não despe do `?`), a classe ESTÁTICA do
    /// receptor — nunca o primeiro membro com esse nome no programa.
    pub fn membro_do_usuario(
        &self,
        acesso: ExprId,
        recv: ExprId,
        nome: SymbolId,
        setter: bool,
    ) -> Option<(ClassId, MemberRef)> {
        if let Some(dartforge_types::resolved::Resolved::Member { class, member, .. }) =
            self.ctx.get_resolved(self.unit_id, acesso)
        {
            let classe = &self.ctx.program.classes[class.0 as usize];
            // Membro herdado do SDK (`index`/`name` de enum, `hashCode`,
            // `toString` de `Object`…) numa classe do programa: não é
            // membro compilado — vai pelos caminhos do SDK.
            let lib_do_membro = match member {
                MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].library,
                MemberRef::Variable(v) => self.ctx.program.variables[v.0 as usize].library,
            };
            if !self.ctx.biblioteca_compilada(lib_do_membro) {
                return None;
            }
            // A resolução de `o.x = v` pode apontar para o getter `x`.
            // Nesse caso a escrita precisa procurar a entrada distinta
            // `x_=` (ou o setter implícito de um campo) na hierarquia.
            let e_getter = matches!(member, MemberRef::Function(f)
                if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                    && self.ctx.program.functions[f.0 as usize].variable.is_none());
            if !(setter && e_getter) {
                return (self.ctx.biblioteca_compilada(classe.library)).then_some((*class, *member));
            }
        }
        let cid = self.classe_do_usuario_de(recv)?;
        for c in crate::lower::membros::linearizacao(self.ctx, cid) {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.biblioteca_compilada(classe.library) {
                return None;
            }
            if let Some(&f) = classe.instance_members.get(&nome) {
                let func = &self.ctx.program.functions[f.0 as usize];
                if !setter && func.kind != FunctionKind::Setter {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if setter {
                let nome_setter = format!("{}_=", self.ctx.symbol_name(nome));
                if let Some(s) = self.ctx.interner.lookup(&nome_setter)
                    && let Some(&f) = classe.instance_members.get(&s)
                {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if let Some(&v) = classe.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == nome
            }) {
                return Some((c, MemberRef::Variable(v)));
            }
        }
        None
    }

    /// `E` de uma `List<E>`/`Iterable<E>` (o primeiro argumento de tipo).
    pub fn tipo_elemento(&self, ty: TypeId) -> Option<TypeId> {
        let dartforge_types::table::Type::Interface { class, args, .. } = self.ctx.table.get(ty) else {
            return None;
        };
        let iter = self.ctx.core.iterable_class?;
        if *class == iter || self.ctx.core.list_class == Some(*class) {
            return args.first().copied();
        }
        // Subtipo de `Iterable<E>` (p. ex. `Runes`): o `E` vem do supertipo
        // instanciado; sem argumentos a substituir, ele já é o tipo final.
        let dados = self.ctx.outline.hierarchy.get(*class)?;
        let sup = *dados.supertypes.get(&iter)?;
        if !dados.type_params.is_empty() {
            return None;
        }
        match self.ctx.table.get(sup) {
            dartforge_types::table::Type::Interface { args, .. } => args.first().copied(),
            _ => None,
        }
    }

    /// `lista[i]` lido na representação `repr` (R5): referência pelo
    /// acessor que encaixota, escalar pelos bits. Índice fora da faixa lança
    /// `RangeError` (exceção pendente).
    pub fn ler_elemento_lista(&mut self, lista: Operand, idx: Operand, repr: Type) -> Operand {
        if repr == Type::Ref {
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_list_get_ref".to_string(),
                    args: vec![(lista, Type::Ref), (idx, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let bits = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_list_get_bits".to_string(),
                args: vec![(lista, Type::Ref), (idx, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_para(bits, repr)
    }

    /// `first`/`last`/`single` na representação do tipo da expressão.
    pub fn ler_extremo_lista(&mut self, lista: Operand, qual: &str, expr: ExprId) -> Operand {
        let repr = self.repr_da_expressao(expr).unwrap_or(Type::Ref);
        if repr == Type::Ref {
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: format!("dartforge_list_{qual}_ref"),
                    args: vec![(lista, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let bits = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: format!("dartforge_list_{qual}"),
                args: vec![(lista, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_para(bits, repr)
    }

    /// Membro de instância por nome, subindo a cadeia de superclasses.
    pub fn membro_na_classe(&self, cid: ClassId, nome: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(nome)?;
        for c in crate::lower::membros::linearizacao(self.ctx, cid) {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = classe.instance_members.get(&sym) {
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
    pub fn id_de_classe(&self, cid: ClassId) -> Option<i64> {
        let classe = &self.ctx.program.classes[cid.0 as usize];
        if self.ctx.biblioteca_compilada(classe.library) {
            return self.ctx.id_de_classe(cid).map(i64::from);
        }
        Some(match self.ctx.symbol_name(classe.name) {
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
    pub fn ler_membro_implicito(&mut self, member: MemberRef, span: Span) -> Operand {
        // `values` implícito de um enum do programa, lido sem o prefixo
        // `E.` no corpo de um membro do enum: a lista dos valores, como o
        // caminho explícito (`E.values`, `valores_do_enum`). O getter
        // implícito não tem corpo para chamar — emitir a chamada deixava um
        // símbolo indefinido na ligação.
        if let MemberRef::Function(f) = member {
            let fid = f.0 as usize;
            let (e_estatico, nome, cid) = {
                let func = &self.ctx.program.functions[fid];
                (func.static_, self.ctx.symbol_name(func.name).to_string(), func.class)
            };
            if e_estatico
                && nome == "values"
                && !tem_corpo(self.ctx, fid)
                && let Some(cid) = cid
                && super::enums::e_enum(self.ctx, cid)
            {
                return self.valores_do_enum(cid, span);
            }
        }
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::e_global(self.ctx, vid) {
                return self.ler_global(vid, span);
            }
            let Some(this) = self.this_param.clone() else {
                return self.nao_suportado("campo de instância fora de membro de instância", span);
            };
            return self.ler_campo_com_late(this, vid, span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Getter {
            if !super::funcao_do_usuario(self.ctx, fid) {
                return self.nao_suportado("tear-off de método do SDK", span);
            }
            if func.static_ {
                return self.tearoff_de_funcao(fid);
            }
            let Some(this) = self.this_param.clone() else {
                return self.nao_suportado("tear-off de método fora de membro de instância", span);
            };
            return self.tearoff_de_metodo(this, fid, span);
        }
        if func.static_ {
            if !super::funcao_do_usuario(self.ctx, fid) {
                return self.nao_suportado("getter estático do SDK", span);
            }
            return self.chamar_direto(fid, None, Vec::new());
        }
        let Some(this) = self.this_param.clone() else {
            return self.nao_suportado("getter de instância fora de membro de instância", span);
        };
        self.chamar_membro(this, fid, &[], span)
    }

    /// `C.x` com `C` literal de classe: campo estático ou getter estático.
    pub fn ler_membro_estatico(&mut self, member: MemberRef, span: Span) -> Operand {
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::e_global(self.ctx, vid) {
                return self.ler_global(vid, span);
            }
            return self.nao_suportado("constante de enum", span);
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        if self.ctx.program.functions[fid].kind == FunctionKind::Getter
            && super::funcao_do_usuario(self.ctx, fid)
        {
            return self.chamar_direto(fid, None, Vec::new());
        }
        if super::funcao_do_usuario(self.ctx, fid) && self.ctx.program.functions[fid].static_ {
            return self.tearoff_de_funcao(fid);
        }
        self.nao_suportado("tear-off de membro estático", span)
    }

    /// Identificador que resolve para um elemento de topo.
    pub fn ler_elemento(&mut self, el: Element, span: Span) -> Operand {
        match el {
            Element::Variable(vid) => self.ler_global(vid, span),
            Element::Function(f) => {
                let fid = f.0 as usize;
                let func = &self.ctx.program.functions[fid];
                if let Some(vid) = func.variable {
                    return self.ler_global(vid, span);
                }
                if func.kind == FunctionKind::Getter && super::funcao_do_usuario(self.ctx, fid) {
                    return self.chamar_direto(fid, None, Vec::new());
                }
                if super::funcao_do_usuario(self.ctx, fid) && func.kind != FunctionKind::Setter {
                    return self.tearoff_de_funcao(fid);
                }
                self.nao_suportado("tear-off de função", span)
            }
            // Literal de tipo (`Peixe`, `List`): o objeto `Type` canônico do
            // tipo cru da classe (RTI).
            Element::Class(cid) => {
                let n = self.ctx.outline.classes.get(cid.0 as usize).map_or(0, |d| d.type_params.len());
                let mut r = super::rti::Receita { texto: format!("C{}", self.ctx.id_rti(cid)), variaveis: false };
                if n > 0 {
                    r.texto.push('<');
                    r.texto.push_str(&vec!["D"; n].join(","));
                    r.texto.push('>');
                }
                let t = self.rti_da_receita(&r);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_objeto_tipo".to_string(),
                        args: vec![(t, Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
            _ => self.nao_suportado("elemento de topo", span),
        }
    }

    /// Campo lido; se `late` com inicializador, inicializa na primeira leitura.
    pub fn ler_campo_com_late(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        if let Some(r) = self.ler_campo_fonte(obj.clone(), vid) {
            return r;
        }
        self.ler_campo_com_late_direto(obj, vid, span)
    }

    /// `super.campo` tem resolução lexical: preserva `late`, mas não pode
    /// voltar ao getter sobrescrito da classe dinâmica via seletor do SDK.
    pub fn ler_campo_com_late_direto(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.late {
            return self.ler_campo(obj, vid, span);
        }
        let dartforge_elements::model::VariableRef::Field { .. } = var.node else {
            return self.ler_campo(obj, vid, span);
        };
        if self.variable_initializer_em(vid).is_some() {
            return self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: super::simbolo_getter_campo_late(self.ctx, vid),
                    args: vec![obj],
                    ret_ty: self.repr_do_campo(vid),
                },
                self.repr_do_campo(vid),
            );
        }
        let atual = self.ler_campo(obj.clone(), vid, span);
        {
            let idx = match self.indice_campo(vid) {
                Some(i) => Operand::Constant(Constant::Int(i as i64)),
                None => match self.indice_dinamico(obj.clone(), vid, span) {
                    Some(i) => i,
                    None => return self.nao_suportado("campo late fora do layout", span),
                },
            };
            let inicializado = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_late_field_initialized".to_string(),
                    args: vec![(obj, Type::Ref), (idx, Type::I64)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let vazio = self.emit(
                Instruction::ICmp(ICmpOp::Eq, inicializado, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_erro = self.new_block();
            let b_ler = self.new_block();
            self.terminate(Terminator::CondBranch { cond: vazio, then_block: b_erro, else_block: b_ler });
            self.set_block(b_erro);
            let nome = self.ctx.symbol_name(var.name).to_string();
            self.lancar_erro_late(&nome, 0);
            self.set_block(b_ler);
            return atual;
        }
    }

    /// Corpo único do getter de um campo `late` com inicializador. O estado
    /// vive por objeto, inclusive quando o valor atribuído é `null`.
    pub fn lower_getter_campo_late(&mut self, obj: Operand, vid: VariableId, span: Span) {
        let Some(init) = self.variable_initializer_em(vid) else { unreachable!("getter late sem initializer") };
        let dartforge_elements::model::VariableRef::Field { unit, .. } = self.ctx.program.variables[vid.0 as usize].node else {
            unreachable!("getter late sem campo")
        };
        let idx = match self.indice_campo(vid) {
            Some(i) => Operand::Constant(Constant::Int(i as i64)),
            None => match self.indice_dinamico(obj.clone(), vid, span) {
                Some(i) => i,
                None => {
                    self.nao_suportado("campo late fora do layout", span);
                    self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                    return;
                }
            },
        };
        let pronto = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initialized".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let pronto = self.emit(
            Instruction::ICmp(ICmpOp::Ne, pronto, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_ler = self.new_block();
        let b_verificar = self.new_block();
        self.terminate(Terminator::CondBranch { cond: pronto, then_block: b_ler, else_block: b_verificar });
        self.set_block(b_ler);
        let atual = self.ler_campo(obj.clone(), vid, span);
        self.terminate(Terminator::Return(Some(atual)));

        self.set_block(b_verificar);
        let iniciando = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let iniciando = self.emit(
            Instruction::ICmp(ICmpOp::Ne, iniciando, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_pilha = self.new_block();
        let b_avaliar = self.new_block();
        self.terminate(Terminator::CondBranch { cond: iniciando, then_block: b_pilha, else_block: b_avaliar });
        self.set_block(b_pilha);
        let erro = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_stack_overflow_error_new".to_string(),
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(erro);

        self.set_block(b_avaliar);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let b_falha = self.new_block();
        self.exception_targets.push(b_falha);
        let valor = self.lower_expr_de(unit, init);
        self.exception_targets.pop();
        let valor = self.coagir(valor, self.repr_do_campo(vid));
        self.gravar_campo(obj.clone(), vid, valor.clone(), span);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_field_set_initializing".to_string(),
                args: vec![(obj.clone(), Type::Ref), (idx.clone(), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I8)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(valor)));
        self.set_block(b_falha);
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
    pub fn avaliar_args(&mut self, ast: &ast::Ast, args: &[ast::Argument]) -> Vec<Avaliado> {
        args.iter()
            .map(|a| {
                let v = self.lower_expr(ast, a.value);
                (a.name.map(|n| n.sym), v)
            })
            .collect()
    }

    /// Parâmetros como escritos na declaração (para valores padrão).
    fn parametros_ast(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
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
    pub fn parametros_de(&self, fid: usize) -> Option<(UnitId, &'a [ast::Parameter])> {
        self.parametros_ast(fid)
    }

    /// Baixa uma expressão de outra unidade (valor padrão de parâmetro).
    ///
    /// `lower_expr` usa `self.unit_id` para os tipos e resoluções; trocar a
    /// unidade enquanto a expressão é baixada é o que torna isso seguro
    /// (docs/NATIVO-PLANO.md §5.1). Valores padrão são constantes: não leem
    /// locais do chamado.
    pub fn lower_expr_de(&mut self, unit: UnitId, expr: ExprId) -> Operand {
        let salvo = self.unit_id;
        self.unit_id = unit;
        let ast = &self.ctx.program.unit(unit).ast;
        let v = self.lower_expr(ast, expr);
        self.unit_id = salvo;
        v
    }

    /// Valor padrão do parâmetro `i` de `fid`, ou o herdado do construtor da
    /// superclasse quando o parâmetro é `super.x` sem padrão próprio.
    pub fn valor_padrao(&mut self, fid: usize, i: usize) -> Option<Operand> {
        let (unit, params) = self.parametros_ast(fid)?;
        let p = params.get(i)?;
        if let Some(e) = p.default_value {
            // O padrão de uma função do SDK da fonte chamada de fora do
            // módulo dela: as resoluções da unidade do SDK não existem aqui;
            // o módulo do SDK exporta o valor (`lower_padroes_do_sdk`).
            if padrao_exportado(self.ctx, fid, unit, e) {
                return Some(self.emit_call_with_check(
                    Instruction::CallStatic { symbol: simbolo_do_padrao(self.ctx, fid, i), args: Vec::new(), ret_ty: Type::Ref },
                    Type::Ref,
                ));
            }
            // O valor padrão é constante (§9.2.2): contexto const.
            let salvo = std::mem::replace(&mut self.em_contexto_const, true);
            let v = self.lower_expr_de(unit, e);
            self.em_contexto_const = salvo;
            return Some(v);
        }
        if !p.super_ {
            return None;
        }
        // `super.x` sem padrão herda o do parâmetro correspondente do
        // construtor da superclasse.
        let (sup_fid, pos_explicitos) = self.construtor_super_de(fid)?;
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
            let posicionais: Vec<usize> = sup_params
                .iter()
                .enumerate()
                .filter(|(_, sp)| sp.kind != ParameterKind::Named)
                .map(|(j, _)| j)
                .collect();
            *posicionais.get(pos_explicitos + k)?
        };
        self.valor_padrao(sup_fid, alvo)
    }

    /// Construtor da superclasse chamado por um construtor (explícito na
    /// lista de inicialização ou o sem nome implícito), e quantos
    /// argumentos posicionais explícitos ele recebe.
    fn construtor_super_de(&self, fid: usize) -> Option<(usize, usize)> {
        let f = &self.ctx.program.functions[fid];
        let cid = f.class?;
        let sup = self.ctx.program.classes[cid.0 as usize].supertype_class?;
        let sup_classe = &self.ctx.program.classes[sup.0 as usize];
        let FunctionRef::Constructor { unit, member } = f.node else {
            return None;
        };
        let MemberKind::Constructor(c) = &self.ctx.program.unit(unit).ast.member(member).kind
        else {
            return None;
        };
        let explicito = c.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super {
                constructor,
                arguments,
                ..
            } => Some((*constructor, arguments)),
            _ => None,
        });
        let vazio = self.ctx.interner.lookup("")?;
        match explicito {
            Some((nome, args)) => {
                let s = nome.map(|n| n.sym).unwrap_or(vazio);
                let sf = sup_classe.constructors.get(&s)?;
                Some((
                    sf.0 as usize,
                    args.args.iter().filter(|a| a.name.is_none()).count(),
                ))
            }
            None => sup_classe
                .constructors
                .get(&vazio)
                .map(|sf| (sf.0 as usize, 0)),
        }
    }

    /// Casa argumentos avaliados com os parâmetros de `fid`: posicionais em
    /// ordem, nomeados pelo nome, faltantes pelo valor padrão (N5).
    pub fn casar_args(&mut self, fid: usize, avaliados: &[Avaliado]) -> Vec<Operand> {
        let Some(dados) = self.ctx.outline.functions.get(fid) else {
            return avaliados.iter().map(|(_, v)| v.clone()).collect();
        };
        let params: Vec<(Option<SymbolId>, TypeId, ParameterKind)> = dados
            .parameters
            .iter()
            // O nome externo casa com o do argumento (nomeado privado da 3.12).
            .map(|p| (p.externo, p.ty, p.kind))
            .collect();
        let mut posicionais = avaliados
            .iter()
            .filter(|(n, _)| n.is_none())
            .map(|(_, v)| v.clone());
        let mut saida = Vec::with_capacity(params.len());
        for (i, (nome, ty, kind)) in params.into_iter().enumerate() {
            let dado = if kind == ParameterKind::Named {
                avaliados
                    .iter()
                    .find(|(n, _)| n.is_some() && *n == nome)
                    .map(|(_, v)| v.clone())
            } else {
                posicionais.next()
            };
            let repr = self.repr(ty);
            let v = match dado {
                Some(v) => v,
                None => self
                    .valor_padrao(fid, i)
                    .unwrap_or_else(|| Self::valor_zero(repr)),
            };
            saida.push(self.coagir(v, repr));
        }
        saida
    }

    /// Representação do retorno de uma função (Void para construtor).
    pub fn repr_retorno(&self, fid: usize) -> Type {
        if super::construtor_generativo(self.ctx, fid) {
            return Type::Void;
        }
        self.ctx
            .outline
            .functions
            .get(fid)
            .map_or(Type::Ref, |d| self.ctx.to_hir_type(d.return_type))
    }

    /// Chamada direta a uma função do usuário, com `this` quando é membro.
    pub fn chamar_direto(
        &mut self,
        fid: usize,
        this: Option<Operand>,
        args: Vec<Operand>,
    ) -> Operand {
        if let Some(r) = self.chamar_externo(fid, this.clone(), &args) {
            return r;
        }
        let symbol = super::simbolo_de(self.ctx, fid);
        let ret_ty = self.repr_retorno(fid);
        let mut todos = Vec::with_capacity(args.len() + 2);
        todos.extend(this);
        todos.extend(args);
        // RTI: a função genérica recebe a tupla dos argumentos de tipo no
        // último parâmetro (a da chamada corrente, ou 0 = `dynamic`).
        if self.funcao_generica(fid) {
            todos.push(self.tupla_armada.clone().unwrap_or(Operand::Constant(Constant::Int(0))));
        }
        let r = self.emit_call_with_check(
            Instruction::CallStatic {
                symbol,
                args: todos,
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
    fn implementacoes(&self, decl_fid: usize) -> Vec<(u32, usize)> {
        let ctx = self.ctx;
        let decl = &ctx.program.functions[decl_fid];
        let Some(cdecl) = decl.class else {
            return Vec::new();
        };
        // Getters e setters compartilham `FunctionElement.name`, mas o
        // outline guarda o setter na chave distinta `x_=`. Despachar por
        // `x` chamaria o getter e descartaria o valor atribuído.
        let nome = if decl.kind == FunctionKind::Setter {
            let chave = format!("{}_=", ctx.symbol_name(decl.name));
            let Some(sym) = ctx.interner.lookup(&chave) else { return Vec::new() };
            sym
        } else {
            decl.name
        };
        let mut saida = Vec::new();
        for (k, classe) in ctx.program.classes.iter().enumerate() {
            let kid = ClassId(k as u32);
            if !ctx.biblioteca_compilada(classe.library) || !subclasse_de(ctx, kid, cdecl) {
                continue;
            }
            if (classe.modifiers.abstract_ && kid != cdecl) || e_mixin(ctx, kid) {
                continue;
            }
            for c in crate::lower::membros::linearizacao(self.ctx, kid) {
                let cl = &ctx.program.classes[c.0 as usize];
                if let Some(&f) = cl.instance_members.get(&nome) {
                    let f = f.0 as usize;
                    if super::funcao_do_usuario(ctx, f) && tem_corpo(ctx, f) {
                        if let Some(id) = ctx.id_de_classe(kid) {
                            saida.push((id, f));
                        }
                        break;
                    }
                }
            }
        }
        saida
    }

    /// Chamada de método/getter/setter de instância com despacho pela classe
    /// dinâmica do receptor quando alguma subclasse sobrescreve o membro.
    ///
    /// Não há vtable ainda: um `switch` sobre `dartforge_value_class`
    /// escolhe a implementação. Cada alvo casa os argumentos com os SEUS
    /// parâmetros (padrões e representação podem diferir entre overrides) e
    /// o resultado é coagido para a representação do membro declarado.
    pub fn chamar_membro(
        &mut self,
        recv: Operand,
        decl_fid: usize,
        avaliados: &[Avaliado],
        span: Span,
    ) -> Operand {
        if let Some(r) = self.chamar_membro_fonte(recv.clone(), decl_fid, avaliados) {
            return r;
        }
        let impls = self.implementacoes(decl_fid);
        let mut distintos: Vec<usize> = impls.iter().map(|(_, f)| *f).collect();
        distintos.sort_unstable();
        distintos.dedup();
        let ret = self.repr_retorno(decl_fid);
        if distintos.is_empty() {
            // Getter implícito `index`/`name` de um enum do programa
            // (especificação §13 "Enums"): o elemento resolvido não tem corpo
            // — o valor mora nos dois campos implícitos do objeto (posições 0
            // e 1, gravados pelo construtor do valor). O caminho explícito
            // (`E.a.name`, `membro_de_enum`) lê os mesmos campos; o implícito
            // (`$name`, `index`, `this.name` no corpo de um membro do enum)
            // chegava aqui como "sem implementação compilada".
            if avaliados.is_empty()
                && let Some(cid) = self.ctx.program.functions[decl_fid].class
                && super::enums::e_enum(self.ctx, cid)
            {
                let nome = self.ctx.symbol_name(self.ctx.program.functions[decl_fid].name).to_string();
                if (nome == "index" || nome == "name")
                    && let Some(r) = self.membro_de_enum(cid, &nome, recv.clone())
                {
                    return self.coagir(r, ret);
                }
            }
            if tem_corpo(self.ctx, decl_fid) && super::funcao_do_usuario(self.ctx, decl_fid) {
                distintos.push(decl_fid);
            } else if self.ctx.sdk_da_fonte {
                // Nenhuma classe do programa implementa o membro: o que a
                // alcança só pode ser um encaminhador de `noSuchMethod`
                // (classe com `noSuchMethod` que não o declara). Pelo
                // seletor, a falta na tabela chega ao `noSuchMethod` do
                // receptor com o `Invocation`, como na VM.
                use super::sdk_fonte::Tipo;
                let f = &self.ctx.program.functions[decl_fid];
                let tipo = match f.kind {
                    FunctionKind::Getter => Tipo::Ler,
                    FunctionKind::Setter => Tipo::Gravar,
                    FunctionKind::ImplicitAccessor if avaliados.len() == 1 => Tipo::Gravar,
                    FunctionKind::ImplicitAccessor => Tipo::Ler,
                    _ => Tipo::Chamar,
                };
                let nome = self.ctx.symbol_name(f.name).to_string();
                let s = super::sdk_fonte::texto_seletor(self.ctx, tipo, &nome, f.library);
                let tupla = if self.funcao_generica(decl_fid) {
                    self.tupla_armada.clone().unwrap_or(Operand::Constant(Constant::Int(0)))
                } else {
                    Operand::Constant(Constant::Int(0))
                };
                // O encaminhador da VM passa todos os parâmetros do membro,
                // com os padrões dos opcionais omitidos.
                let completos: Vec<Avaliado> = if matches!(tipo, Tipo::Chamar) {
                    let valores = self.casar_args(decl_fid, avaliados);
                    self.ctx.outline.functions[decl_fid]
                        .parameters
                        .iter()
                        .zip(valores)
                        .map(|(p, v)| (if p.kind == ParameterKind::Named { p.externo } else { None }, v))
                        .collect()
                } else {
                    avaliados.to_vec()
                };
                let r = self.chamar_por_seletor_com_tupla(recv, s, &completos, tupla);
                return if ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coagir(r, ret) };
            } else {
                // Encaminhador `noSuchMethod` estático (casos 57, 216, 223):
                // a classe concreta tem `noSuchMethod` e o CFE sintetizaria o
                // encaminhador com a assinatura do membro. Só dispara onde
                // antes era erro; fora do escopo (setter, nomeado, genérico,
                // múltiplos nsm) mantém o diagnóstico antigo.
                if let Some(r) = super::nsm::encaminhar_metodo_para_nsm(self, recv.clone(), decl_fid, avaliados, span) {
                    return r;
                }
                if let Some(r) = super::nsm::encaminhar_getter_para_nsm(self, recv, decl_fid, avaliados, span) {
                    return r;
                }
                return self.nao_suportado("chamada de membro sem implementação compilada", span);
            }
        }
        if distintos.len() == 1 {
            let alvo = distintos[0];
            let args = self.casar_args(alvo, avaliados);
            let r = self.chamar_direto(alvo, Some(recv), args);
            return if ret == Type::Void {
                r
            } else {
                self.coagir(r, ret)
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
        let juncao = self.new_block();
        let padrao_fid = if distintos.contains(&decl_fid) {
            decl_fid
        } else {
            distintos[0]
        };
        let mut blocos: Vec<(usize, BlockId)> = Vec::new();
        for &f in &distintos {
            let b = self.new_block();
            blocos.push((f, b));
        }
        let bloco_de = |f: usize| {
            blocos
                .iter()
                .find(|(g, _)| *g == f)
                .map(|(_, b)| *b)
                .expect("bloco do alvo")
        };
        let cases: Vec<(i64, BlockId)> = impls
            .iter()
            .map(|(cid, f)| (i64::from(*cid), bloco_de(*f)))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: bloco_de(padrao_fid),
            cases,
        });
        let mut entradas: Vec<(BlockId, Operand)> = Vec::new();
        for (f, b) in blocos.clone() {
            self.set_block(b);
            let args = self.casar_args(f, avaliados);
            let r = self.chamar_direto(f, Some(recv.clone()), args);
            let r = if ret == Type::Void {
                r
            } else {
                self.coagir(r, ret)
            };
            if !self.is_terminated() {
                entradas.push((self.current_block, r));
                self.terminate(Terminator::Branch(juncao));
            }
        }
        self.set_block(juncao);
        if ret == Type::Void || entradas.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty: ret,
            },
            ret,
        )
    }

    // ------------------------------------------------------------------
    // Construtores (N5)

    /// `C(args)`: aloca com o layout da classe e chama o construtor.
    pub fn instanciar(
        &mut self,
        ast: &ast::Ast,
        ctor_fid: FunctionElementId,
        args: &[ast::Argument],
        span: Span,
    ) -> Operand {
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid) = f.class else {
            return self.nao_suportado("construtor sem classe", span);
        };
        // As factories `const` de ambiente são resolvidas pela CFE na
        // compilação. A CLI nativa ainda não aceita `-D`, portanto nenhuma
        // chave foi definida e vale o `defaultValue` especificado pelo SDK.
        // Nunca chamamos o native da VM para essas factories.
        if f.const_ && f.external {
            let nome = self.ctx.symbol_name(f.name);
            let classe = if Some(cid) == self.ctx.core.bool_class {
                Some("bool")
            } else if Some(cid) == self.ctx.core.int_class {
                Some("int")
            } else if Some(cid) == self.ctx.core.string_class {
                Some("String")
            } else {
                None
            };
            if classe == Some("bool") && nome == "hasEnvironment" {
                return self.emit(Instruction::Const(Constant::Bool(false)), Type::I1);
            }
            if nome == "fromEnvironment" {
                if let Some(classe) = classe {
                    if let Some(valor) = args.iter().find(|a| {
                        a.name.as_ref().is_some_and(|n| self.ctx.symbol_name(n.sym) == "defaultValue")
                    }) {
                        return self.lower_em_contexto_const(ast, valor.value);
                    }
                    return match classe {
                        "bool" => self.emit(Instruction::Const(Constant::Bool(false)), Type::I1),
                        "int" => self.emit(Instruction::Const(Constant::Int(0)), Type::I64),
                        _ => self.emit(Instruction::Const(Constant::String(String::new())), Type::Ref),
                    };
                }
            }
        }
        // RTI: o tipo estático da criação (`C<T…>`), gravado por quem chama.
        let tipo = self.tipo_da_criacao.take();
        if !super::funcao_do_usuario(self.ctx, fid) {
            let avaliados = self.avaliar_args(ast, args);
            self.tipo_da_criacao = tipo;
            let r = self.construtor_de_erro_do_runtime(fid, &avaliados);
            self.tipo_da_criacao = None;
            if let Some(op) = r {
                return op;
            }
            let nome = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.nao_suportado(&format!("construtor de classe do SDK ({nome})"), span);
        }
        // A classe concreta (aplicação de mixin) vale para esta criação, não
        // para as que a avaliação dos argumentos fizer.
        let concreta = self.classe_concreta.take();
        let avaliados = self.avaliar_args(ast, args);
        self.tipo_da_criacao = tipo;
        self.classe_concreta = concreta;
        self.instanciar_avaliados(ctor_fid, &avaliados, span)
    }

    /// O construtor `nome` da classe `cid`. Uma aplicação de mixin
    /// (`class C = S with M;`, especificação §12.1) não declara construtores:
    /// ela tem um construtor de encaminhamento para cada construtor
    /// generativo da superclasse. Aí devolve o da superclasse (subindo pelas
    /// aplicações encadeadas) e anota `cid` como a classe concreta da criação
    /// seguinte — o objeto é de `cid`, e os campos dos mixins são
    /// inicializados antes do construtor da superclasse.
    pub fn construtor_de(&mut self, cid: ClassId, nome: SymbolId) -> Option<FunctionElementId> {
        let mut c = cid;
        loop {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = classe.constructors.get(&nome) {
                if c != cid {
                    if self.ctx.program.functions[f.0 as usize].factory {
                        return None;
                    }
                    self.classe_concreta = Some(cid);
                }
                return Some(f);
            }
            if classe.kind != dartforge_elements::model::ClassKind::MixinApplication || !classe.constructors.is_empty() {
                return None;
            }
            c = classe.supertype_class?;
        }
    }

    /// `C(args)` com os argumentos já avaliados.
    pub fn instanciar_avaliados(&mut self, ctor_fid: FunctionElementId, avaliados: &[Avaliado], span: Span) -> Operand {
        self.instanciar_avaliados_com_rti(ctor_fid, avaliados, span, None, None)
    }

    /// Uma factory redirecionadora pode mudar a classe e até permutar seus
    /// argumentos de tipo. Nesse caso, o RTI e a tupla do alvo vêm da
    /// anotação `= Destino<U, T>`, não do tipo de retorno da origem.
    pub fn instanciar_avaliados_com_rti(
        &mut self,
        ctor_fid: FunctionElementId,
        avaliados: &[Avaliado],
        span: Span,
        rti_explicito: Option<Operand>,
        tupla_explicita: Option<Operand>,
    ) -> Operand {
        let tipo = self.tipo_da_criacao.take();
        let concreta = self.classe_concreta.take();
        let fid = ctor_fid.0 as usize;
        let f = &self.ctx.program.functions[fid];
        let Some(cid_ctor) = f.class else {
            return self.nao_suportado("construtor sem classe", span);
        };
        // O objeto é da classe concreta (aplicação de mixin) quando o
        // construtor é o da superclasse que ela encaminha.
        let cid = concreta.unwrap_or(cid_ctor);
        // A factory `core.Symbol` redireciona para a classe concreta da
        // biblioteca interna. O alvo é inequívoco e evita que a resolução
        // da factory volte à própria declaração abstrata.
        if self.ctx.sdk_da_fonte && Some(cid) == self.ctx.classe_do_sdk("core", "Symbol") {
            if let Some(concreta) = self.ctx.classe_do_sdk("_internal", "Symbol")
                && let Some(vazio) = self.ctx.interner.lookup("")
                && let Some(&construtor) = self.ctx.program.classes[concreta.0 as usize].constructors.get(&vazio)
            {
                return self.instanciar_avaliados_com_rti(construtor, avaliados, span, rti_explicito, tupla_explicita);
            }
        }
        if !super::funcao_do_usuario(self.ctx, fid) {
            if let Some(op) = self.construtor_de_erro_do_runtime(fid, avaliados) {
                return op;
            }
            let nome = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.nao_suportado(&format!("construtor de classe do SDK ({nome})"), span);
        }
        let factory = f.factory;
        let mut args = self.casar_args(fid, avaliados);
        let generica = self.classe_generica(cid);
        if factory {
            // RTI: a fábrica de uma classe genérica recebe os argumentos de
            // tipo da classe na tupla (o último parâmetro).
            if generica {
                let t = tupla_explicita.unwrap_or_else(|| self.tupla_da_criacao(tipo));
                args.push(t);
            }
            return self.chamar_direto(fid, None, args);
        }
        let campos = layout(self.ctx, cid).len() + super::enums::base_do_layout(self.ctx, cid);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (
                        Operand::Constant(Constant::Int(i64::from(self.ctx.id_de_classe(cid).unwrap_or(0)))),
                        Type::I64,
                    ),
                    (Operand::Constant(Constant::Int(campos as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // RTI: a instância de classe genérica guarda o tipo (`C<T…>`).
        if generica && let Some(r) = rti_explicito.or_else(|| tipo.map(|t| self.rti_de_tipo(t))) {
            self.definir_rti(obj.clone(), r);
        }
        if concreta.is_some() {
            self.inicializar_mixins_das_aplicacoes(obj.clone(), cid, cid_ctor);
        }
        self.chamar_direto(fid, Some(obj.clone()), args);
        obj
    }

    /// Os construtores de encaminhamento das aplicações de mixin de `de` até
    /// `ate` (exclusive): cada um inicializa os campos dos seus mixins, do
    /// último para o primeiro, antes do construtor da superclasse.
    fn inicializar_mixins_das_aplicacoes(&mut self, obj: Operand, de: ClassId, ate: ClassId) {
        let salvo = self.this_param.replace(obj);
        let mut c = de;
        while c != ate {
            let classe = &self.ctx.program.classes[c.0 as usize];
            let mixins = classe.mixin_classes.clone();
            let sup = classe.supertype_class;
            for m in mixins.into_iter().rev() {
                if self.ctx.biblioteca_compilada(self.ctx.program.classes[m.0 as usize].library) {
                    self.inicializar_campos(m);
                }
            }
            let Some(s) = sup else { break };
            c = s;
        }
        self.this_param = salvo;
    }

    /// A função (não construtor) declara parâmetros de tipo: recebe a tupla.
    /// Um membro de instância de extensão genérica também recebe: os
    /// argumentos da extensão vêm primeiro (`params_de_tipo_de`).
    pub fn funcao_generica(&self, fid: usize) -> bool {
        !matches!(self.ctx.program.functions[fid].node, FunctionRef::Constructor { .. })
            && !self.params_de_tipo_de(fid).is_empty()
            && super::funcao_do_usuario(self.ctx, fid)
    }

    /// Os parâmetros de tipo que a tupla de `fid` carrega, na ordem: os da
    /// extensão (membro de instância; um estático não os vê) e os próprios.
    pub fn params_de_tipo_de(&self, fid: usize) -> Vec<TypeParamId> {
        let f = &self.ctx.program.functions[fid];
        let mut v = Vec::new();
        if let Some(e) = f.extension.filter(|_| !f.static_)
            && let Some(x) = self.ctx.outline.extensions.get(e.0 as usize)
        {
            v.extend(x.type_params.iter().copied());
        }
        if let Some(d) = self.ctx.outline.functions.get(fid) {
            v.extend(d.type_params.iter().copied());
        }
        v
    }

    /// A classe declara parâmetros de tipo.
    pub fn classe_generica(&self, cid: ClassId) -> bool {
        self.ctx.outline.classes.get(cid.0 as usize).is_some_and(|d| !d.type_params.is_empty())
    }

    /// A tupla de argumentos de tipo (`L<…>`) do tipo de uma criação; sem
    /// tipo, 0 (os argumentos ficam `dynamic`).
    pub fn tupla_da_criacao(&mut self, tipo: Option<TypeId>) -> Operand {
        let Some(t) = tipo else {
            return Operand::Constant(Constant::Int(0));
        };
        let dartforge_types::table::Type::Interface { args, .. } = self.ctx.table.get(t) else {
            return Operand::Constant(Constant::Int(0));
        };
        let args = args.clone();
        self.tupla_de_tipos_rti(&args)
    }

    /// A tupla `L<a…>` dos tipos `args`, como `I64`, no ambiente corrente.
    pub fn tupla_de_tipos_rti(&mut self, args: &[TypeId]) -> Operand {
        let mut r = super::rti::Receita { texto: "L<".to_string(), variaveis: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.texto.push(',');
            }
            let x = self.receita_de_tipo(*a);
            r.texto.push_str(&x.texto);
            r.variaveis |= x.variaveis;
        }
        r.texto.push('>');
        self.rti_da_receita(&r)
    }

    /// Inicializadores de campo da própria classe, na ordem de declaração.
    fn inicializar_campos(&mut self, cid: ClassId) {
        let this = self.this_param.clone().expect("construtor tem this");
        let campos: Vec<VariableId> = self.ctx.program.classes[cid.0 as usize].fields.clone();
        for vid in campos {
            let var = &self.ctx.program.variables[vid.0 as usize];
            if var.static_ || var.late {
                continue;
            }
            let dartforge_elements::model::VariableRef::Field { unit, .. } = var.node else {
                continue;
            };
            let Some(init) = self.variable_initializer_em(vid) else {
                continue;
            };
            let v = self.lower_expr_de(unit, init);
            let span = self.ctx.program.unit(unit).ast.expr(init).span;
            self.gravar_campo(this.clone(), vid, v, span);
        }
    }

    /// Chama o construtor da superclasse do usuário, se houver.
    fn chamar_super(
        &mut self,
        cid: ClassId,
        nome: Option<SymbolId>,
        avaliados: Vec<Avaliado>,
        span: Span,
    ) {
        // Os mixins aplicados rodam os inicializadores dos seus campos antes
        // do construtor da superclasse, do último para o primeiro (cada
        // aplicação `S with M` é uma classe cujo construtor inicializa `M`
        // e chama o de `S`).
        let mixins: Vec<ClassId> = self.ctx.program.classes[cid.0 as usize].mixin_classes.clone();
        for m in mixins.into_iter().rev() {
            if self.ctx.biblioteca_compilada(self.ctx.program.classes[m.0 as usize].library) {
                self.inicializar_campos(m);
            }
        }
        let Some(sup) = self.ctx.program.classes[cid.0 as usize].supertype_class else {
            return;
        };
        let sup_classe = &self.ctx.program.classes[sup.0 as usize];
        if !self.ctx.biblioteca_compilada(sup_classe.library) {
            // `Object()` e superclasses do SDK: nada a executar no nosso heap.
            return;
        }
        let Some(vazio) = self.ctx.interner.lookup("") else {
            return;
        };
        let s = nome.unwrap_or(vazio);
        if nome.is_none() && sup_classe.constructors.is_empty() {
            // Superclasse sem construtor declarado e sem o sintético (a
            // classe abstrata não o ganha no elemento): o construtor padrão
            // implícito — inicializadores de campo e o `super()` dela.
            self.inicializar_campos(sup);
            self.chamar_super(sup, None, Vec::new(), span);
            return;
        }
        let Some(&sf) = sup_classe.constructors.get(&s) else {
            self.nao_suportado("construtor da superclasse não encontrado", span);
            return;
        };
        let sf = sf.0 as usize;
        let args = self.casar_args(sf, &avaliados);
        let this = self.this_param.clone().expect("construtor tem this");
        self.chamar_direto(sf, Some(this), args);
    }

    /// Corpo de um construtor generativo, na ordem da especificação (N5).
    pub fn lower_construtor(
        &mut self,
        ast: &ast::Ast,
        cid: ClassId,
        ctor: &ast::Constructor,
        span_ctor: Span,
    ) {
        let this = self.this_param.clone().expect("construtor tem this");
        let classe = &self.ctx.program.classes[cid.0 as usize];

        // `: this(…)` delega tudo para o outro construtor.
        if let Some((alvo, args, span)) = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Redirect {
                constructor,
                arguments,
                span,
            } => Some((*constructor, arguments, *span)),
            _ => None,
        }) {
            let Some(vazio) = self.ctx.interner.lookup("") else {
                return;
            };
            let s = alvo.map(|n| n.sym).unwrap_or(vazio);
            let Some(&tf) = classe.constructors.get(&s) else {
                self.nao_suportado("construtor redirecionado não encontrado", span);
                return;
            };
            let avaliados = self.avaliar_args(ast, &args.args);
            let args = self.casar_args(tf.0 as usize, &avaliados);
            self.chamar_direto(tf.0 as usize, Some(this), args);
            self.terminate(Terminator::Return(None));
            return;
        }

        // 1. Inicializadores de campo da própria classe.
        self.inicializar_campos(cid);

        // 2. Parâmetros `this.x`.
        for p in ctor.parameters.iter() {
            if !p.this_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(vid) = self.campo_proprio(cid, n.sym) else {
                self.nao_suportado("parâmetro this.x sem campo", p.span);
                continue;
            };
            let v = self.ler_local_por_nome(n.sym)
                .expect("parâmetro declarado");
            self.gravar_campo(this.clone(), vid, v, p.span);
        }

        // 3. Lista de inicialização, na ordem escrita; os argumentos do
        //    `super(…)` são avaliados na posição dele.
        let mut super_explicito: Option<(Option<SymbolId>, Vec<Avaliado>, Span)> = None;
        for init in ctor.initializers.iter() {
            match init {
                ast::Initializer::Field {
                    name, value, span, ..
                } => {
                    let Some(vid) = self.campo_proprio(cid, name.sym) else {
                        self.nao_suportado("inicializador de campo inexistente", *span);
                        continue;
                    };
                    let v = self.lower_expr(ast, *value);
                    self.gravar_campo(this.clone(), vid, v, *span);
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
                    let avaliados = self.avaliar_args(ast, &arguments.args);
                    super_explicito = Some((constructor.map(|n| n.sym), avaliados, *span));
                }
                ast::Initializer::Redirect { .. } => {}
            }
        }

        // 4. `super(…)`: explícito ou implícito, mais os `super.x`.
        let (nome_super, mut avaliados, span_super) =
            super_explicito.unwrap_or((None, Vec::new(), span_ctor));
        for p in ctor.parameters.iter().filter(|p| p.super_) {
            let Some(n) = p.name else { continue };
            let v = self.ler_local_por_nome(n.sym)
                .expect("parâmetro declarado");
            let nome = if p.kind == ParameterKind::Named {
                Some(n.sym)
            } else {
                None
            };
            avaliados.push((nome, v));
        }
        // Posicionais `super.x` entram depois dos explícitos, na ordem.
        avaliados.sort_by_key(|(n, _)| n.is_some());
        self.chamar_super(cid, nome_super, avaliados, span_super);

        // 5. Corpo. Os `this.x` não estão no escopo do corpo: `x` ali é o
        //    campo, e a resolução (`Resolved::Member`) já diz isso.
        for p in ctor.parameters.iter() {
            if p.this_
                && let Some(n) = p.name
            {
                self.remover_local(n.sym);
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
    pub fn lower_construtor_sintetico(&mut self, ast: &ast::Ast, cid: ClassId) {
        self.inicializar_campos(cid);
        let span = self.ctx.program.classes[cid.0 as usize]
            .decl
            .map(|d| ast.decl(d.decl).span)
            .unwrap_or(Span { start: 0, end: 0 });
        self.chamar_super(cid, None, Vec::new(), span);
    }

    // ------------------------------------------------------------------
    // Globais (N6)

    /// Constrói o `LateError` do SDK e lança no fluxo de exceções do HIR.
    /// `codigo`: 0/1 campo não inicializado/já inicializado; 2/3 local.
    pub fn lancar_erro_late(&mut self, nome: &str, codigo: i64) {
        let n = self.emit(Instruction::Const(Constant::String(nome.to_string())), Type::Ref);
        let erro = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_late_error_new".to_string(),
                args: vec![(n, Type::Ref), (Operand::Constant(Constant::Int(codigo)), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(erro);
    }

    /// Corpo do getter preguiçoso de um global.
    pub fn lower_getter_global(&mut self, vid: VariableId, repr: Type) {
        let valor = super::simbolo_valor_global(self.ctx, vid);
        let bandeira = format!("{valor}$ok");
        let Some(init) = self.variable_initializer_em(vid) else {
            if self.ctx.program.variables[vid.0 as usize].late {
                let ok = self.emit(Instruction::LoadGlobal { simbolo: bandeira, ty: Type::I8 }, Type::I8);
                let pronto = self.emit(
                    Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_ler = self.new_block();
                let b_erro = self.new_block();
                self.terminate(Terminator::CondBranch { cond: pronto, then_block: b_ler, else_block: b_erro });
                self.set_block(b_erro);
                let nome = self.ctx.symbol_name(self.ctx.program.variables[vid.0 as usize].name).to_string();
                self.lancar_erro_late(&nome, 0);
                self.set_block(b_ler);
            }
            let v = self.emit(
                Instruction::LoadGlobal {
                    simbolo: valor,
                    ty: repr,
                },
                repr,
            );
            self.terminate(Terminator::Return(Some(v)));
            return;
        };
        let ok = self.emit(
            Instruction::LoadGlobal {
                simbolo: bandeira.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let pronto = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok.clone(), Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_ler = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: pronto,
            then_block: b_ler,
            else_block: b_init,
        });
        self.set_block(b_ler);
        let v = self.emit(
            Instruction::LoadGlobal {
                simbolo: valor.clone(),
                ty: repr,
            },
            repr,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        let reentrante = self.emit(
            Instruction::ICmp(ICmpOp::Eq, ok, Operand::Constant(Constant::Int(2))),
            Type::I1,
        );
        let b_pilha = self.new_block();
        let b_avaliar = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: reentrante,
            then_block: b_pilha,
            else_block: b_avaliar,
        });
        self.set_block(b_pilha);
        let erro = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_stack_overflow_error_new".to_string(),
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(erro);
        self.set_block(b_avaliar);
        // 0 = não iniciado; 2 = avaliando; 1 = pronto. A leitura reentrante
        // produz `StackOverflowError` do SDK sem expandir a pilha nativa.
        self.emit(
            Instruction::StoreGlobal {
                simbolo: bandeira,
                val: Operand::Constant(Constant::Int(2)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        let unit = self.unit_id;
        let e_const = self.ctx.program.variables[vid.0 as usize].const_;
        let salvo = std::mem::replace(&mut self.em_contexto_const, e_const);
        // Se o inicializador lança, a variável continua não inicializada
        // (§10 "Variables": a próxima leitura roda o inicializador de novo):
        // a bandeira volta a 0 e a exceção segue pendente.
        let b_falha = self.new_block();
        self.exception_targets.push(b_falha);
        let v = self.lower_expr_de(unit, init);
        self.exception_targets.pop();
        self.em_contexto_const = salvo;
        let continua = self.current_block;
        self.set_block(b_falha);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: format!("{}$ok", super::simbolo_valor_global(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(0)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Self::valor_zero(repr))));
        self.set_block(continua);
        let v = self.coagir(v, repr);
        let raiz = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: valor,
                val: v.clone(),
                ty: repr,
                raiz,
            },
            Type::Void,
        );
        if e_const && repr == Type::Ref {
            self.emit(
                Instruction::CallRuntime {
                name: "dartforge_marcar_constante".to_string(),
                args: vec![(v.clone(), Type::Ref), (Operand::Constant(Constant::Funcao(self.func.symbol.clone())), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        self.emit(
            Instruction::StoreGlobal {
                simbolo: format!("{}$ok", super::simbolo_valor_global(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Lê um global pelo getter preguiçoso.
    pub fn ler_global(&mut self, vid: VariableId, span: Span) -> Operand {
        if !self.ctx.biblioteca_compilada(self.ctx.program.variables[vid.0 as usize].library)
        {
            let nome = self
                .ctx
                .symbol_name(self.ctx.program.variables[vid.0 as usize].name)
                .to_string();
            return self.nao_suportado(&format!("global do SDK ({nome})"), span);
        }
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        self.emit_call_with_check(
            Instruction::CallStatic {
                symbol: super::simbolo_global(self.ctx, vid),
                args: Vec::new(),
                ret_ty: repr,
            },
            repr,
        )
    }

    /// Grava um global (e marca-o inicializado).
    pub fn gravar_global(&mut self, vid: VariableId, val: Operand, span: Span) -> Operand {
        if !self.ctx.biblioteca_compilada(self.ctx.program.variables[vid.0 as usize].library)
        {
            return self.nao_suportado("atribuição a global do SDK", span);
        }
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        let val = self.coagir(val, repr);
        if self.ctx.sdk_da_fonte
            && !self.ctx.biblioteca_no_modulo(self.ctx.program.variables[vid.0 as usize].library)
        {
            // O global mora em outro módulo (SDK da fonte): pelo setter dele.
            self.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: format!("{}$set", super::simbolo_global(self.ctx, vid)),
                    args: vec![val.clone()],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            return val;
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        if var.late && var.final_ {
            let bandeira = format!("{}$ok", super::simbolo_valor_global(self.ctx, vid));
            let ok = self.emit(Instruction::LoadGlobal { simbolo: bandeira, ty: Type::I8 }, Type::I8);
            let ja_inicializado = self.emit(
                Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
                Type::I1,
            );
            let b_erro = self.new_block();
            let b_gravar = self.new_block();
            self.terminate(Terminator::CondBranch { cond: ja_inicializado, then_block: b_erro, else_block: b_gravar });
            self.set_block(b_erro);
            let nome = self.ctx.symbol_name(var.name).to_string();
            self.lancar_erro_late(&nome, 1);
            self.set_block(b_gravar);
        }
        self.emit(
            Instruction::StoreGlobal {
                simbolo: format!("{}$ok", super::simbolo_valor_global(self.ctx, vid)),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        let raiz = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: super::simbolo_valor_global(self.ctx, vid),
                val: val.clone(),
                ty: repr,
                raiz,
            },
            Type::Void,
        );
        val
    }

    /// O construtor `nome` (ou o sem nome) da classe do programa nomeada por
    /// um tipo anotado.
    pub fn construtor_do_tipo(
        &self,
        ty: &dartforge_frontend::ast::TypeAnnotation,
        nome: Option<dartforge_intern::SymbolId>,
    ) -> Option<FunctionElementId> {
        let dartforge_frontend::ast::TypeKind::Named { name, .. } = &ty.kind else {
            return None;
        };
        let lib = self.ctx.program.unit(self.unit_id).library;
        // `= Alvo` ou `= Alvo.nome` (o parser lê `Alvo.nome` como um nome
        // de tipo em duas partes).
        let (classe, nome) = match (&name[..], nome) {
            ([c, n], None)
                if matches!(
                    self.ctx.program.lookup(lib, c.sym).and_then(|b| b.getter),
                    Some(Element::Class(_))
                ) =>
            {
                (c.sym, Some(n.sym))
            }
            (partes, n) => (partes.last()?.sym, n),
        };
        let Some(Element::Class(c)) = self.ctx.program.lookup(lib, classe)?.getter else {
            return None;
        };
        let chave = nome.or_else(|| self.ctx.interner.lookup(""))?;
        self.ctx.program.classes[c.0 as usize].constructors.get(&chave).copied()
    }
}

/// Um valor padrão que não precisa de resolução (literal de null, bool,
/// número ou string sem interpolação): qualquer módulo o baixa.
pub fn padrao_literal(ast: &ast::Ast, e: ExprId) -> bool {
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
pub fn padrao_exportado(ctx: &crate::context::Context, fid: usize, unit: UnitId, e: ExprId) -> bool {
    let lib = ctx.program.functions[fid].library;
    ctx.sdk_da_fonte
        && ctx.program.library(lib).is_sdk
        && !ctx.biblioteca_no_modulo(lib)
        && !padrao_literal(&ctx.program.unit(unit).ast, e)
}

/// O símbolo da função que devolve o padrão do parâmetro `i` de `fid`.
pub fn simbolo_do_padrao(ctx: &crate::context::Context, fid: usize, i: usize) -> String {
    format!("{}.padrao.{i}", super::simbolo_de(ctx, fid))
}

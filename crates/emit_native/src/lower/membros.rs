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
use dartforge_types::table::TypeId;

/// Argumento já avaliado: nome (se nomeado) e valor.
pub type Avaliado = (Option<SymbolId>, Operand);

/// Campos **de instância** de toda a cadeia de superclasses do usuário, da
/// raiz para a folha (R7).
///
/// `ClassElement::fields` mistura campos de instância e estáticos; o layout
/// só tem os de instância. Superclasses do SDK não contribuem campos: o
/// runtime não tem os objetos do SDK no nosso layout.
pub fn layout(ctx: &Context, cid: ClassId) -> Vec<VariableId> {
    let mut cadeia = Vec::new();
    let mut atual = Some(cid);
    while let Some(c) = atual {
        let classe = &ctx.program.classes[c.0 as usize];
        if ctx.program.library(classe.library).is_sdk {
            break;
        }
        cadeia.push(c);
        atual = classe.supertype_class;
    }
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
        layout(self.ctx, dono).iter().position(|&v| v == vid)
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

    /// Lê um campo de instância na representação do seu tipo declarado.
    pub fn ler_campo(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        let Some(idx) = self.indice_campo(vid) else {
            return self.nao_suportado("campo fora do layout do objeto", span);
        };
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        let ret = if repr == Type::Ref {
            Type::Ref
        } else {
            Type::I64
        };
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![
                    (obj, Type::Ref),
                    (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
                ],
                ret_ty: ret,
            },
            ret,
        );
        self.bits_para(bits, repr)
    }

    /// Grava um campo de instância; `is_ref` pela representação (E1).
    pub fn gravar_campo(&mut self, obj: Operand, vid: VariableId, val: Operand, span: Span) {
        let Some(idx) = self.indice_campo(vid) else {
            self.nao_suportado("campo fora do layout do objeto", span);
            return;
        };
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        let val = self.coagir(val, repr);
        let (bits, is_ref) = self.para_bits(val);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj, Type::Ref),
                    (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
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
                (!self.ctx.program.library(classe.library).is_sdk).then_some(*class)
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
            return (!self.ctx.program.library(classe.library).is_sdk).then_some((*class, *member));
        }
        let cid = self.classe_do_usuario_de(recv)?;
        let mut atual = Some(cid);
        while let Some(c) = atual {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if self.ctx.program.library(classe.library).is_sdk {
                return None;
            }
            if let Some(&f) = classe.instance_members.get(&nome) {
                let func = &self.ctx.program.functions[f.0 as usize];
                if !setter && func.kind != FunctionKind::Setter {
                    return Some((c, MemberRef::Function(f)));
                }
            }
            if setter {
                let nome_setter = format!("{}=", self.ctx.symbol_name(nome));
                if let Some(s) = self.ctx.interner.lookup(&nome_setter) {
                    if let Some(&f) = classe.instance_members.get(&s) {
                        return Some((c, MemberRef::Function(f)));
                    }
                }
            }
            if let Some(&v) = classe.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == nome
            }) {
                return Some((c, MemberRef::Variable(v)));
            }
            atual = classe.supertype_class;
        }
        None
    }

    /// Membro de instância por nome, subindo a cadeia de superclasses.
    pub fn membro_na_classe(&self, cid: ClassId, nome: &str) -> Option<usize> {
        let sym = self.ctx.interner.lookup(nome)?;
        let mut atual = Some(cid);
        while let Some(c) = atual {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = classe.instance_members.get(&sym) {
                return Some(f.0 as usize);
            }
            atual = classe.supertype_class;
        }
        None
    }

    // ------------------------------------------------------------------
    // Leitura de identificadores

    /// Local (ou parâmetro) pelo nome, no escopo corrente.
    pub fn ler_local_por_nome(&mut self, sym: SymbolId) -> Option<Operand> {
        if let Some(ptr) = self.local_ptrs.get(&sym).cloned() {
            return Some(self.emit(Instruction::Load { ptr, ty: Type::I64 }, Type::I64));
        }
        self.named_locals.get(&sym).cloned()
    }

    /// Id de classe usado nos testes de tipo do runtime: o `class_id` de um
    /// objeto do usuário, ou o código que `dartforge_value_class` devolve
    /// para os valores que o runtime representa por conta própria.
    pub fn id_de_classe(&self, cid: ClassId) -> Option<i64> {
        let classe = &self.ctx.program.classes[cid.0 as usize];
        if !self.ctx.program.library(classe.library).is_sdk {
            return Some(i64::from(cid.0 + 1));
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
            _ => return None,
        })
    }

    /// Identificador que a resolução diz ser membro da classe envolvente
    /// (`x` = `this.x`, ou membro estático).
    pub fn ler_membro_implicito(&mut self, member: MemberRef, span: Span) -> Operand {
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
            return self.nao_suportado("tear-off de método", span);
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
                self.nao_suportado("tear-off de função", span)
            }
            Element::Class(cid) => match self.id_de_classe(cid) {
                Some(id) => Operand::Constant(Constant::Int(id)),
                None => self.nao_suportado("literal de classe do SDK", span),
            },
            _ => self.nao_suportado("elemento de topo", span),
        }
    }

    /// Campo lido; se `late` com inicializador, inicializa na primeira leitura.
    pub fn ler_campo_com_late(&mut self, obj: Operand, vid: VariableId, span: Span) -> Operand {
        let atual = self.ler_campo(obj.clone(), vid, span);
        let var = &self.ctx.program.variables[vid.0 as usize];
        if !var.late {
            return atual;
        }
        let dartforge_elements::model::VariableRef::Field { unit, .. } = var.node else {
            return atual;
        };
        let Some(init) = self.variable_initializer_em(vid) else {
            return atual;
        };
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        if repr != Type::Ref {
            // Sem valor sentinela para "não inicializado" num escalar.
            return self.nao_suportado("campo late escalar com inicializador", span);
        }
        let e_nulo = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                atual.clone(),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        );
        let b_init = self.new_block();
        let b_fim = self.new_block();
        let origem = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: e_nulo,
            then_block: b_init,
            else_block: b_fim,
        });
        self.set_block(b_init);
        let v = self.lower_expr_de(unit, init);
        self.gravar_campo(obj, vid, v.clone(), span);
        let v = self.coagir(v, repr);
        let fim_init = self.current_block;
        self.terminate(Terminator::Branch(b_fim));
        self.set_block(b_fim);
        self.emit(
            Instruction::Phi {
                incoming: vec![(origem, atual), (fim_init, v)],
                ty: repr,
            },
            repr,
        )
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
    fn valor_padrao(&mut self, fid: usize, i: usize) -> Option<Operand> {
        let (unit, params) = self.parametros_ast(fid)?;
        let p = params.get(i)?;
        if let Some(e) = p.default_value {
            return Some(self.lower_expr_de(unit, e));
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
                .position(|sp| sp.name == p.name.map(|n| n.sym))?
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
            .map(|p| (p.name, p.ty, p.kind))
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
        let s = super::simbolo_de(self.ctx, fid);
        if s.starts_with("df_ctor_") {
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
        let symbol = super::simbolo_de(self.ctx, fid);
        let ret_ty = self.repr_retorno(fid);
        let mut todos = Vec::with_capacity(args.len() + 1);
        todos.extend(this);
        todos.extend(args);
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
        let nome = decl.name;
        let mut saida = Vec::new();
        for (k, classe) in ctx.program.classes.iter().enumerate() {
            let kid = ClassId(k as u32);
            if ctx.program.library(classe.library).is_sdk || !subclasse_de(ctx, kid, cdecl) {
                continue;
            }
            if classe.modifiers.abstract_ && kid != cdecl {
                continue;
            }
            let mut atual = Some(kid);
            while let Some(c) = atual {
                let cl = &ctx.program.classes[c.0 as usize];
                if let Some(&f) = cl.instance_members.get(&nome) {
                    let f = f.0 as usize;
                    if super::funcao_do_usuario(ctx, f) && tem_corpo(ctx, f) {
                        saida.push((kid.0 + 1, f));
                        break;
                    }
                }
                atual = cl.supertype_class;
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
        let impls = self.implementacoes(decl_fid);
        let mut distintos: Vec<usize> = impls.iter().map(|(_, f)| *f).collect();
        distintos.sort_unstable();
        distintos.dedup();
        let ret = self.repr_retorno(decl_fid);
        if distintos.is_empty() {
            if tem_corpo(self.ctx, decl_fid) && super::funcao_do_usuario(self.ctx, decl_fid) {
                distintos.push(decl_fid);
            } else {
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
        if !super::funcao_do_usuario(self.ctx, fid) {
            let nome = self
                .ctx
                .symbol_name(self.ctx.program.classes[cid.0 as usize].name)
                .to_string();
            return self.nao_suportado(&format!("construtor de classe do SDK ({nome})"), span);
        }
        let factory = f.factory;
        let avaliados = self.avaliar_args(ast, args);
        let args = self.casar_args(fid, &avaliados);
        if factory {
            return self.chamar_direto(fid, None, args);
        }
        let campos = layout(self.ctx, cid).len();
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (
                        Operand::Constant(Constant::Int(i64::from(cid.0 + 1))),
                        Type::I64,
                    ),
                    (Operand::Constant(Constant::Int(campos as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.chamar_direto(fid, Some(obj.clone()), args);
        obj
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
        let Some(sup) = self.ctx.program.classes[cid.0 as usize].supertype_class else {
            return;
        };
        let sup_classe = &self.ctx.program.classes[sup.0 as usize];
        if self.ctx.program.library(sup_classe.library).is_sdk {
            // `Object()` e superclasses do SDK: nada a executar no nosso heap.
            return;
        }
        let Some(vazio) = self.ctx.interner.lookup("") else {
            return;
        };
        let s = nome.unwrap_or(vazio);
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
            let v = self
                .named_locals
                .get(&n.sym)
                .cloned()
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
            let v = self
                .named_locals
                .get(&n.sym)
                .cloned()
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
            if p.this_ {
                if let Some(n) = p.name {
                    self.named_locals.remove(&n.sym);
                }
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

    /// Corpo do getter preguiçoso de um global.
    pub fn lower_getter_global(&mut self, vid: VariableId, repr: Type) {
        let valor = format!("dfg_{}", vid.0);
        let bandeira = format!("dfg_{}_ok", vid.0);
        let Some(init) = self.variable_initializer_em(vid) else {
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
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
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
        // A bandeira sobe antes do inicializador: é assim que a VM trata a
        // leitura reentrante (e a nossa não recursa para sempre).
        self.emit(
            Instruction::StoreGlobal {
                simbolo: bandeira,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        let unit = self.unit_id;
        let v = self.lower_expr_de(unit, init);
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
        self.terminate(Terminator::Return(Some(v)));
    }

    /// Lê um global pelo getter preguiçoso.
    pub fn ler_global(&mut self, vid: VariableId, span: Span) -> Operand {
        if self
            .ctx
            .program
            .library(self.ctx.program.variables[vid.0 as usize].library)
            .is_sdk
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
                symbol: super::simbolo_global(vid),
                args: Vec::new(),
                ret_ty: repr,
            },
            repr,
        )
    }

    /// Grava um global (e marca-o inicializado).
    pub fn gravar_global(&mut self, vid: VariableId, val: Operand, span: Span) -> Operand {
        if self
            .ctx
            .program
            .library(self.ctx.program.variables[vid.0 as usize].library)
            .is_sdk
        {
            return self.nao_suportado("atribuição a global do SDK", span);
        }
        let repr = self.repr(tipo_da_variavel(self.ctx, vid));
        let val = self.coagir(val, repr);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: format!("dfg_{}_ok", vid.0),
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                raiz: None,
            },
            Type::Void,
        );
        let raiz = (repr == Type::Ref).then_some(vid.0);
        self.emit(
            Instruction::StoreGlobal {
                simbolo: format!("dfg_{}", vid.0),
                val: val.clone(),
                ty: repr,
                raiz,
            },
            Type::Void,
        );
        val
    }
}

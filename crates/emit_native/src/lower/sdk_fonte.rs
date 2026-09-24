//! O SDK compilado da fonte no lowering (P5c/P5d, δ; docs/NATIVO-PLANO.md §7).
//!
//! Com `Context::sdk_da_fonte`, as sete bibliotecas de
//! `sdk_modulo::BIBLIOTECAS_DA_FONTE` são código Dart compilado como o do
//! programa, cada uma no seu módulo (objeto em cache), e o programa as chama
//! pelos símbolos estáveis. Três coisas mudam em relação ao mundo fechado:
//!
//! 1. **Chamada por seletor.** Um membro público de instância de uma classe
//!    do SDK pode ser sobrescrito por uma classe do programa, que o SDK não
//!    conhece quando é compilado. A chamada vai pelo seletor (`c:m`, `g:x`,
//!    `s:x`; o nome privado leva `@<biblioteca>`), que a tabela de métodos da
//!    classe dinâmica resolve (`llvm/seletores.rs`, `runtime/seletores.rs`).
//!    Continua direta (ou pelo `switch` do mundo fechado de `membros.rs`) a
//!    chamada que só a biblioteca da classe pode sobrescrever: membro
//!    privado, ou classe privada cujos subtipos na biblioteca são todos
//!    privados.
//! 2. **Adaptadores.** Cada membro de instância ganha as entradas uniformes
//!    que a tabela de métodos aponta: `<símbolo>$c` (chamar), `$g` (ler: o
//!    getter, o campo, ou o tear-off de um método) e `$s` (gravar), com a
//!    convenção das closures (`i64 (i64 receptor, ptr args, ptr desc)`).
//! 3. **`external`.** O membro de patch (`patched_by`) é o corpo; um native
//!    (`vm:external-name`) é a função do runtime da tabela `nativos.rs`; o
//!    que não tem implementação (native pendente, intrínseco da VM) é
//!    diagnóstico — o membro que o chama é **recusado** no módulo do SDK
//!    (`sdk_modulo`), com o motivo, e nunca emitido errado.

use super::fn_builder::FnBuilder;
use super::membros::{Avaliado, linearizacao, subclasse_de};
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::ParameterKind;

/// O que o seletor faz com o membro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    /// `o.m(…)`.
    Chamar,
    /// `o.x`.
    Ler,
    /// `o.x = v`.
    Gravar,
}

/// O texto do seletor: `c:m`, `g:x`, `s:x`; um nome privado leva
/// `@<biblioteca>` (dois `_m` de bibliotecas diferentes são membros
/// diferentes). O `_=` da chave de um setter nas tabelas do elemento sai
/// (`==` e `[]=` são operadores, não setters).
pub fn texto_seletor(ctx: &Context, tipo: Tipo, nome: &str, lib: LibraryId) -> String {
    let p = match tipo {
        Tipo::Chamar => "c",
        Tipo::Ler => "g",
        Tipo::Gravar => "s",
    };
    let nome = nome.strip_suffix("_=").unwrap_or(nome);
    if nome.starts_with('_') {
        format!("{p}:{nome}@{}", ctx.nome_da_biblioteca(lib))
    } else {
        format!("{p}:{nome}")
    }
}

/// Nenhuma classe fora da biblioteca de `cid` pode ser subtipo dela: ela e
/// todos os seus subtipos na biblioteca são privados.
pub fn classe_fechada(ctx: &Context, cid: ClassId) -> bool {
    let classe = &ctx.program.classes[cid.0 as usize];
    if !ctx.symbol_name(classe.name).starts_with('_') {
        return false;
    }
    ctx.program.classes.iter().enumerate().all(|(k, c)| {
        c.library != classe.library
            || !subclasse_de(ctx, ClassId(k as u32), cid)
            || ctx.symbol_name(c.name).starts_with('_')
    })
}

/// O membro `nome` de `cid` só pode ser sobrescrito dentro da biblioteca
/// dela (o mundo fechado de `membros.rs` vale para ele).
pub fn membro_fechado(ctx: &Context, cid: ClassId, nome: &str) -> bool {
    let classe = &ctx.program.classes[cid.0 as usize];
    if !ctx.program.library(classe.library).is_sdk {
        // Classe do programa: o programa conhece todos os subtipos dela.
        return true;
    }
    nome.starts_with('_') || classe_fechada(ctx, cid)
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.<seletor>(avaliados)` pela convenção uniforme; o resultado é
    /// `Ref`.
    pub fn chamar_por_seletor(&mut self, recv: Operand, seletor: String, avaliados: &[Avaliado]) -> Operand {
        let recv = self.coagir(recv, Type::Ref);
        let mut args = Vec::with_capacity(avaliados.len());
        for (n, v) in avaliados {
            if n.is_none() {
                let v = self.coagir(v.clone(), Type::Ref);
                args.push(v);
            }
        }
        let mut nomeados: Vec<(String, Operand)> = avaliados
            .iter()
            .filter_map(|(n, v)| n.map(|n| (self.ctx.symbol_name(n).to_string(), v.clone())))
            .collect();
        nomeados.sort_by(|a, b| a.0.cmp(&b.0));
        let mut nomes = Vec::with_capacity(nomeados.len());
        for (n, v) in nomeados {
            let v = self.coagir(v, Type::Ref);
            args.push(v);
            nomes.push(n);
        }
        self.emit_call_with_check(Instruction::CallSeletor { seletor, recv, args, nomes }, Type::Ref)
    }

    /// `recv.nome(avaliados)` pelo seletor, com o nome privado da biblioteca
    /// da unidade corrente.
    pub fn chamar_por_nome(&mut self, recv: Operand, tipo: Tipo, nome: &str, avaliados: &[Avaliado]) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = texto_seletor(self.ctx, tipo, nome, lib);
        self.chamar_por_seletor(recv, s, avaliados)
    }

    /// Hook de `chamar_membro` (SDK da fonte): o membro de instância de uma
    /// classe do SDK vai pelo seletor, a não ser que só a biblioteca dela o
    /// possa sobrescrever e ele tenha **uma** implementação, que é um método
    /// (chamada direta). `None`: classe do programa (o mundo fechado de
    /// `membros.rs` vale).
    pub fn chamar_membro_fonte(&mut self, recv: Operand, decl_fid: usize, avaliados: &[Avaliado]) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte || self.em_adaptador {
            return None;
        }
        let f = &self.ctx.program.functions[decl_fid];
        let cid = f.class?;
        if f.static_ || !self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk {
            return None;
        }
        let nome = self.ctx.symbol_name(f.name).to_string();
        let chave = if f.kind == FunctionKind::Setter { format!("{nome}_=") } else { nome.clone() };
        if membro_fechado(self.ctx, cid, &nome)
            && let [Implementacao::Funcao(alvo)] = implementacoes(self.ctx, cid, &chave)[..]
        {
            let args = self.casar_args(alvo, avaliados);
            let r = self.chamar_direto(alvo, Some(recv), args);
            let ret = self.repr_retorno(decl_fid);
            return Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coagir(r, ret) });
        }
        let tipo = match f.kind {
            FunctionKind::Getter => Tipo::Ler,
            FunctionKind::Setter => Tipo::Gravar,
            FunctionKind::ImplicitAccessor if avaliados.len() == 1 => Tipo::Gravar,
            FunctionKind::ImplicitAccessor => Tipo::Ler,
            _ => Tipo::Chamar,
        };
        let s = texto_seletor(self.ctx, tipo, &nome, f.library);
        let r = self.chamar_por_seletor(recv, s, avaliados);
        let ret = self.repr_retorno(decl_fid);
        Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coagir(r, ret) })
    }

    /// Hook da leitura de campo (SDK da fonte): o campo público de uma
    /// classe do SDK aberta pode ser sobrescrito por um getter do programa.
    pub fn ler_campo_fonte(&mut self, obj: Operand, vid: VariableId) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte || self.em_adaptador {
            return None;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let cid = v.class?;
        if !self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk {
            return None;
        }
        let nome = self.ctx.symbol_name(v.name).to_string();
        if membro_fechado(self.ctx, cid, &nome)
            && implementacoes(self.ctx, cid, &nome).iter().all(|i| *i == Implementacao::Campo(vid))
        {
            return None;
        }
        let s = texto_seletor(self.ctx, Tipo::Ler, &nome, v.library);
        let r = self.chamar_por_seletor(obj, s, &[]);
        let repr = self.repr_do_campo(vid);
        Some(self.coagir(r, repr))
    }

    /// Hook da gravação de campo por atribuição (SDK da fonte): o campo de
    /// uma classe do SDK que um setter pode sobrescrever vai pelo seletor
    /// `s:x`. `false`: a gravação direta de sempre vale.
    pub fn gravar_campo_fonte(&mut self, obj: Operand, vid: VariableId, valor: Operand) -> bool {
        if !self.ctx.sdk_da_fonte || self.em_adaptador {
            return false;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let Some(cid) = v.class else { return false };
        if !self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk {
            return false;
        }
        let nome = self.ctx.symbol_name(v.name).to_string();
        if membro_fechado(self.ctx, cid, &nome)
            && implementacoes(self.ctx, cid, &format!("{nome}_=")).iter().all(|i| *i == Implementacao::Campo(vid))
        {
            return false;
        }
        let s = texto_seletor(self.ctx, Tipo::Gravar, &nome, v.library);
        self.chamar_por_seletor(obj, s, &[(None, valor)]);
        true
    }

    /// `a == b` com o SDK da fonte (§17.26 "Equality"): com um lado null,
    /// `identical(a, b)`; senão `a.==(b)` pela classe dinâmica de `a`.
    pub fn igualdade_fonte(&mut self, a: Operand, b: Operand) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let na = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), zero.clone()), Type::I1);
        let nb = self.emit(Instruction::ICmp(ICmpOp::Eq, b.clone(), zero), Type::I1);
        let algum = self.emit(Instruction::Or(na, nb), Type::I1);
        let algum = self.emit(Instruction::ICmp(ICmpOp::Ne, algum, Operand::Constant(Constant::Int(0))), Type::I1);
        let b_id = self.new_block();
        let b_din = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: algum, then_block: b_id, else_block: b_din });
        self.set_block(b_id);
        let r1 = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b.clone()), Type::I1);
        let fim1 = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(b_din);
        let r = self.chamar_por_seletor(a, "c:==".to_string(), &[(None, b)]);
        let r2 = self.coagir(r, Type::I1);
        let fim2 = self.current_block;
        self.terminate(Terminator::Branch(juncao));
        self.set_block(juncao);
        self.emit(Instruction::Phi { incoming: vec![(fim1, r1), (fim2, r2)], ty: Type::I1 }, Type::I1)
    }

    /// `op is T` com o SDK da fonte (hook de `testar_tipo`): toda classe
    /// (`int`, `num`, `String`, `Object`…) é uma classe compilada com id, e
    /// a resposta é a do grafo de subtipos que as bibliotecas registram; null
    /// só é um `T` quando `T` é `Null` ou anulável (§20.3). Um nome que não é
    /// classe (parâmetro de tipo, `typedef`) num `as` confere só a classe,
    /// como os argumentos de tipo (`checar_tipo_ou_lancar`); num `is`,
    /// `None` (o diagnóstico de sempre). `None` também sem o SDK da fonte.
    pub fn testar_tipo_fonte(
        &mut self,
        ast_ty: &dartforge_frontend::ast::TypeAnnotation,
        op: Operand,
    ) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        // No código do SDK, os testes com argumentos de tipo (`is List<E>`)
        // escolhem um caminho rápido equivalente para um programa correto
        // (`ListBase.setRange`, `List.from`, `ListQueue.addAll`…): até a RTI,
        // eles conferem a classe. No programa, só o cast (`as`) confere a
        // classe; o `is` com argumentos continua diagnóstico.
        let no_sdk = self.ctx.program.library(self.ctx.program.unit(self.unit_id).library).is_sdk;
        let so_classe = self.cast_so_pela_classe || no_sdk;
        // O que a classe não responde (variável de tipo, tipo de função ou de
        // record) num `is` do código do SDK: recusado. A RTI (`rti.rs`) não
        // vale ali — a entrada uniforme da tabela de métodos não leva a tupla
        // dos argumentos de tipo, e o `T` de um método genérico chamado pelo
        // seletor seria `dynamic` (um `whereType<int>` deixaria passar tudo).
        let sem_classe = |b: &mut Self| -> Option<Operand> {
            if b.cast_so_pela_classe {
                Some(Operand::Constant(Constant::Bool(true)))
            } else if no_sdk {
                Some(b.nao_suportado("teste de tipo sem classe no código do SDK (RTI pelo seletor)", ast_ty.span))
            } else {
                None
            }
        };
        let dartforge_frontend::ast::TypeKind::Named { name, args } = &ast_ty.kind else {
            // Tipo de função/record num cast: confere só... nada (a classe
            // de uma função é `_Closure`).
            return sem_classe(self);
        };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let trivial = |t: &dartforge_frontend::ast::TypeAnnotation| match &t.kind {
            dartforge_frontend::ast::TypeKind::Named { name, args } if args.is_empty() => name.last().is_some_and(|n| {
                let s = self.ctx.symbol_name(n.sym);
                s == "dynamic" || (s == "Object" && t.nullable)
            }),
            _ => false,
        };
        if !args.is_empty() && !args.iter().all(|a| trivial(unit_ast.ty(*a))) && !so_classe {
            return None;
        }
        let ultimo = name.last()?;
        let nome = self.ctx.symbol_name(ultimo.sym).to_string();
        let mut op = op;
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            // Escalar: verdadeiro já em compilação para os supertipos óbvios;
            // o resto (`_Smi`, `_IntegerImplementation`, `Pattern`…) pela
            // classe da caixa — um `int` é `_Smi` ou `_Mint` conforme o valor.
            let certo = matches!(
                (nome.as_str(), repr),
                ("int", Type::I64)
                    | ("double", Type::F64)
                    | ("bool", Type::I1 | Type::I8)
                    | ("num", Type::I64 | Type::F64)
                    | ("Object" | "dynamic" | "Comparable", _)
            );
            if certo {
                return Some(Operand::Constant(Constant::Bool(true)));
            }
            op = self.coagir(op, Type::Ref);
        }
        if nome == "dynamic" {
            return Some(Operand::Constant(Constant::Bool(true)));
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let binding = match &name[..] {
            [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
            _ => self.ctx.program.lookup(lib, ultimo.sym),
        };
        let cid = binding.and_then(|b| match b.getter {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            _ => None,
        });
        let Some(id) = cid.and_then(|c| self.ctx.id_de_classe(c)) else {
            return sem_classe(self);
        };
        let _ = so_classe;
        let zero = Operand::Constant(Constant::Int(0));
        let nulo = self.emit(Instruction::ICmp(ICmpOp::Eq, op.clone(), zero.clone()), Type::I1);
        if nome == "Null"
            && cid.is_some_and(|c| self.ctx.program.library(self.ctx.program.classes[c.0 as usize].library).uri == "dart:core")
        {
            return Some(nulo);
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
        let nao_nulo = self.emit(Instruction::LNot(nulo.clone()), Type::I1);
        let base = self.emit(Instruction::And(sub, nao_nulo), Type::I64);
        let base = self.emit(Instruction::ICmp(ICmpOp::Ne, base, Operand::Constant(Constant::Int(0))), Type::I1);
        if !ast_ty.nullable {
            return Some(base);
        }
        let r = self.emit(Instruction::Or(base, nulo), Type::I64);
        Some(self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1))
    }

    /// `toString()` de um valor pelo seletor (interpolação com o SDK da
    /// fonte): o texto de null é `"null"` (o `toString` de `Null`).
    pub fn texto_por_seletor(&mut self, op: Operand) -> Operand {
        let r = self.chamar_por_seletor(op, "c:toString".to_string(), &[]);
        r
    }

    /// Hook de `chamar_direto`: a chamada a um `external` do SDK da fonte.
    /// `None` quando a função tem corpo (a chamada direta de sempre).
    pub fn chamar_externo(&mut self, fid: usize, this: Option<Operand>, args: &[Operand]) -> Option<Operand> {
        if !self.ctx.sdk_da_fonte {
            return None;
        }
        let f = &self.ctx.program.functions[fid];
        if !f.external || f.variable.is_some() {
            return None;
        }
        if let Some(p) = f.patched_by
            && p.0 as usize != fid
        {
            return Some(self.chamar_direto(p.0 as usize, this, args.to_vec()));
        }
        let (native, reconhecido) = crate::nativos::pragmas(self.ctx.program, self.ctx.interner, f);
        let span = Span { start: 0, end: 0 };
        let dono = f
            .class
            .map(|c| format!("{}.", self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name)))
            .unwrap_or_default();
        let membro = format!("{dono}{}", self.ctx.symbol_name(f.name));
        let nome = match native {
            Some(n) => {
                match crate::nativos::nativo(&n).map(|x| x.estado) {
                    Some(crate::nativos::Estado::Runtime) => {}
                    Some(crate::nativos::Estado::Embutido) => return Some(self.nativo_embutido(&n, fid, args)),
                    Some(crate::nativos::Estado::Pendente) => {
                        return Some(self.nao_suportado(&format!("native pendente `{n}` ({membro})"), span));
                    }
                    None => return Some(self.nao_suportado(&format!("native desconhecido `{n}` ({membro})"), span)),
                }
                n
            }
            None if reconhecido => match crate::nativos::intrinseco(&membro) {
                Some(n) => n.to_string(),
                None => return Some(self.nao_suportado(&format!("intrínseco da VM `{membro}`"), span)),
            },
            None => return Some(self.nao_suportado(&format!("external sem implementação `{membro}`"), span)),
        };
        // A assinatura do native é a representação dos tipos declarados;
        // `bool` cruza a fronteira como `i8` (nativos.rs).
        let mut a = Vec::with_capacity(args.len() + 1);
        if let Some(t) = this {
            // O receptor de um native de `int`/`double`/`bool` é o valor (as
            // funções do runtime recebem `i64`/`double`, como a VM, que
            // desencaixota o `Smi`); o de qualquer outra classe, o `Ref`.
            let r = f.class.map_or(Type::Ref, |c| self.repr_do_receptor(c));
            let t = self.coagir(t, r);
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
        let tipos: Vec<Type> = self
            .ctx
            .outline
            .functions
            .get(fid)
            .map(|d| d.parameters.iter().map(|p| self.repr_nativo(p.ty)).collect())
            .unwrap_or_default();
        for (i, v) in args.iter().enumerate() {
            let t = tipos.get(i).copied().unwrap_or_else(|| self.operand_type(v));
            let t = if t == Type::Void { Type::Ref } else { t };
            let v = self.coagir(v.clone(), t);
            if t == Type::I1 {
                let b = self.emit(Instruction::ZExt { op: v, from: Type::I1, to: Type::I8 }, Type::I8);
                a.push((b, Type::I8));
            } else {
                a.push((v, t));
            }
        }
        let ret = self.repr_retorno(fid);
        let ret_valor = if super::construtor_generativo(self.ctx, fid) {
            Type::Void
        } else {
            self.ctx.outline.functions.get(fid).map_or(ret, |d| self.repr_nativo(d.return_type))
        };
        let ret_nativo = if ret_valor == Type::I1 { Type::I8 } else { ret_valor };
        let r = self.emit_call_with_check(
            Instruction::CallRuntime { name: crate::nativos::simbolo(&nome), args: a, ret_ty: ret_nativo },
            ret_nativo,
        );
        let r = match ret_valor {
            Type::Void => return Some(Operand::Constant(Constant::Null)),
            Type::I1 => self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1),
            _ => r,
        };
        Some(if ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coagir(r, ret) })
    }

    /// A representação de um tipo na fronteira com um native: a de valor
    /// (`i64`, `double`, `i1`) para as classes de `int`, `double` e `bool`
    /// não anuláveis (inclusive `_Smi`, `_Mint`, `_Double`), senão a da HIR.
    fn repr_nativo(&self, ty: dartforge_types::table::TypeId) -> Type {
        if self.ctx.is_void(ty) {
            return Type::Void;
        }
        if let dartforge_types::table::Type::Interface { class, nullable: false, .. } = self.ctx.table.get(ty) {
            let r = self.repr_do_receptor(*class);
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
    fn repr_do_receptor(&self, cid: ClassId) -> Type {
        let core = self.ctx.core;
        let de = |c: Option<ClassId>| c.is_some_and(|c| subclasse_de(self.ctx, cid, c));
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
    /// (`nativos::Estado::Embutido`).
    fn nativo_embutido(&mut self, nome: &str, fid: usize, args: &[Operand]) -> Operand {
        let ret = self.repr_retorno(fid);
        match nome {
            // `unsafeCast<T>(v)`: o próprio valor, sem checagem.
            "Internal_unsafeCast" => {
                let v = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                if ret == Type::Void { v } else { self.coagir(v, ret) }
            }
            _ => self.nao_suportado(&format!("native embutido `{nome}`"), Span { start: 0, end: 0 }),
        }
    }
}

/// Uma implementação concreta de um membro de instância.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Implementacao {
    /// Método, getter, setter ou operador (com corpo, patch ou native).
    Funcao(usize),
    /// Campo (o getter/setter implícito).
    Campo(VariableId),
}

/// As implementações distintas de `chave` (o nome; `x_=` para o setter)
/// nas classes concretas compiladas que são subtipos de `cid`, cada uma
/// achada pela linearização da classe (a primeira que implementa).
pub fn implementacoes(ctx: &Context, cid: ClassId, chave: &str) -> Vec<Implementacao> {
    let Some(sym) = ctx.interner.lookup(chave) else { return Vec::new() };
    let mut saida = Vec::new();
    for (k, classe) in ctx.program.classes.iter().enumerate() {
        let kid = ClassId(k as u32);
        if !ctx.biblioteca_compilada(classe.library)
            || classe.modifiers.abstract_
            || super::membros::e_mixin(ctx, kid)
            || !subclasse_de(ctx, kid, cid)
        {
            continue;
        }
        for c in linearizacao(ctx, kid) {
            let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&sym) else { continue };
            let f = f.0 as usize;
            if !implementado(ctx, f) {
                continue;
            }
            let i = match ctx.program.functions[f].variable {
                Some(v) => Implementacao::Campo(v),
                None => Implementacao::Funcao(f),
            };
            if !saida.contains(&i) {
                saida.push(i);
            }
            break;
        }
    }
    saida
}

/// O que uma entrada da tabela de métodos faz.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Adaptador {
    /// Chama o método; lê e chama o valor de um getter.
    Chamar,
    /// Lê o getter; tira o tear-off de um método.
    Ler,
    /// Chama o setter.
    Gravar,
}

impl Adaptador {
    fn sufixo(self) -> &'static str {
        match self {
            Adaptador::Chamar => "$c",
            Adaptador::Ler => "$g",
            Adaptador::Gravar => "$s",
        }
    }
}

/// O símbolo base de um campo de instância: `df.<lib>.<Classe>.<campo>`.
fn simbolo_do_campo(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    let dono = v.class.map(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string()).unwrap_or_default();
    format!(
        "df.{}.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(v.library)),
        crate::context::escapar(&dono),
        crate::context::escapar(ctx.symbol_name(v.name))
    )
}

/// O membro de instância implementado (tem corpo, é campo, ou é `external`
/// com patch ou native).
fn implementado(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    if f.variable.is_some() {
        return !f.abstract_;
    }
    if f.external {
        return true;
    }
    super::membros::tem_corpo(ctx, fid)
}

/// A tabela de métodos de uma classe concreta: para cada seletor que ela
/// responde (os membros dela e os herdados, pela linearização, o primeiro
/// que implementa), a entrada uniforme.
pub fn tabela_de_metodos(ctx: &Context, cid: ClassId) -> Vec<(String, String)> {
    let mut vistos = std::collections::HashSet::new();
    let mut saida = Vec::new();
    let mut cadeia = linearizacao(ctx, cid);
    // A hierarquia de certos elementos sintéticos (notadamente enums) não
    // inclui `Object` em `supertype_class`. Seus membros continuam herdando
    // `Object.==`, `hashCode` e os outros acessores na semântica Dart.
    if let Some(objeto) = ctx.core.object_class && !cadeia.contains(&objeto) {
        cadeia.push(objeto);
    }
    for c in cadeia {
        let classe = &ctx.program.classes[c.0 as usize];
        let mut membros: Vec<(&str, usize)> = classe
            .instance_members
            .iter()
            .map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize))
            .collect();
        membros.sort();
        for (chave, fid) in membros {
            if !implementado(ctx, fid) {
                continue;
            }
            let f = &ctx.program.functions[fid];
            let e_setter = chave.ends_with("_=");
            let nome = chave.strip_suffix("_=").unwrap_or(chave);
            let mut por = |tipo: Tipo, a: Adaptador, base: &str| {
                let s = texto_seletor(ctx, tipo, nome, f.library);
                if vistos.insert(s.clone()) {
                    saida.push((s, format!("{base}{}", a.sufixo())));
                }
            };
            if let Some(v) = f.variable {
                let base = simbolo_do_campo(ctx, v);
                if e_setter {
                    por(Tipo::Gravar, Adaptador::Gravar, &base);
                } else {
                    por(Tipo::Ler, Adaptador::Ler, &base);
                    por(Tipo::Chamar, Adaptador::Chamar, &base);
                }
                continue;
            }
            let base = super::simbolo_de(ctx, fid);
            match f.kind {
                FunctionKind::Setter => por(Tipo::Gravar, Adaptador::Gravar, &base),
                FunctionKind::Getter => {
                    por(Tipo::Ler, Adaptador::Ler, &base);
                    por(Tipo::Chamar, Adaptador::Chamar, &base);
                }
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {}
                _ => {
                    por(Tipo::Chamar, Adaptador::Chamar, &base);
                    por(Tipo::Ler, Adaptador::Ler, &base);
                }
            }
        }
    }
    saida
}

/// As entradas uniformes de um membro de instância (ou campo) do módulo.
pub fn lower_adaptadores_da_funcao(ctx: &Context, module: &mut Module, fid: usize) {
    let f = &ctx.program.functions[fid];
    if f.static_ || f.class.is_none() || !implementado(ctx, fid) {
        return;
    }
    if matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) || f.factory {
        return;
    }
    if f.variable.is_some() {
        return;
    }
    let base = super::simbolo_de(ctx, fid);
    let adaptadores: &[Adaptador] = match f.kind {
        FunctionKind::Setter => &[Adaptador::Gravar],
        FunctionKind::Getter => &[Adaptador::Ler, Adaptador::Chamar],
        _ => &[Adaptador::Chamar, Adaptador::Ler],
    };
    for &a in adaptadores {
        let simbolo = format!("{base}{}", a.sufixo());
        let unit = unidade_de(ctx, f.class.expect("membro de classe"));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, simbolo, ctx.symbol_name(f.name).to_string(), Type::Ref);
        b.em_adaptador = true;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param("args".to_string(), Type::Ptr));
        let desc = Operand::Val(b.add_param("desc".to_string(), Type::Ptr));
        let span = Span { start: 0, end: 0 };
        let r = match (a, f.kind) {
            (Adaptador::Ler, FunctionKind::Getter) => {
                if b.desempacotar(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.chamar_direto(fid, Some(recv), Vec::new())
            }
            (Adaptador::Chamar, FunctionKind::Getter) => {
                let v = b.chamar_direto(fid, Some(recv), Vec::new());
                let v = b.coagir(v, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureRepasse { closure: v, args, desc }, Type::Ref)
            }
            (Adaptador::Ler, _) => {
                if b.desempacotar(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.tearoff_de_metodo(recv, fid, span)
            }
            _ => {
                let infos = b.params_da_funcao(fid);
                let Some(vals) = b.desempacotar(&infos, args, desc) else {
                    b.finalizar(module);
                    continue;
                };
                let reprs: Vec<Type> = ctx.outline.functions[fid].parameters.iter().map(|p| b.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| b.coagir(v, r)).collect();
                b.chamar_direto(fid, Some(recv), vals)
            }
        };
        let r = if matches!(b.operand_type(&r), Type::Void) { Operand::Constant(Constant::Null) } else { b.coagir(r, Type::Ref) };
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
    }
}

/// As entradas uniformes de um campo de instância: `$g`, `$c` e, se ele tem
/// setter, `$s`.
pub fn lower_adaptadores_do_campo(ctx: &Context, module: &mut Module, vid: VariableId) {
    let v = &ctx.program.variables[vid.0 as usize];
    let Some(cid) = v.class else { return };
    if v.static_ {
        return;
    }
    let Some(unit) = unidade_de(ctx, cid) else { return };
    let base = simbolo_do_campo(ctx, vid);
    let span = Span { start: 0, end: 0 };
    let mut tipos = vec![Adaptador::Ler, Adaptador::Chamar];
    if v.setter.is_some() {
        tipos.push(Adaptador::Gravar);
    }
    for a in tipos {
        let mut b = FnBuilder::new(ctx, unit, format!("{base}{}", a.sufixo()), ctx.symbol_name(v.name).to_string(), Type::Ref);
        b.em_adaptador = true;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param("args".to_string(), Type::Ptr));
        let desc = Operand::Val(b.add_param("desc".to_string(), Type::Ptr));
        let r = match a {
            Adaptador::Ler => {
                if b.desempacotar(&[], args, desc).is_none() {
                    b.finalizar(module);
                    continue;
                }
                b.ler_campo_com_late(recv, vid, span)
            }
            Adaptador::Chamar => {
                let x = b.ler_campo_com_late(recv, vid, span);
                let x = b.coagir(x, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureRepasse { closure: x, args, desc }, Type::Ref)
            }
            Adaptador::Gravar => {
                let um = [super::closures::ParamEntrada {
                    nome: None,
                    kind: ParameterKind::Required,
                    required: true,
                    padrao: super::closures::Padrao::Nenhum,
                }];
                let Some(vals) = b.desempacotar(&um, args, desc) else {
                    b.finalizar(module);
                    continue;
                };
                b.gravar_campo(recv, vid, vals[0].clone(), span);
                Operand::Constant(Constant::Null)
            }
        };
        let r = b.coagir(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
    }
}

/// A unidade que declara a classe (onde os adaptadores dela são baixados).
fn unidade_de(ctx: &Context, cid: ClassId) -> Option<dartforge_elements::model::UnitId> {
    let c = &ctx.program.classes[cid.0 as usize];
    c.decl.map(|d| d.unit).or_else(|| ctx.program.library(c.library).units.first().copied())
}

/// A função no lugar de um membro do SDK recusado: a mesma assinatura, e o
/// corpo avisa em tempo de execução qual membro e por quê
/// (`dartforge_membro_recusado`) — nunca uma saída errada.
pub fn funcao_recusada(ctx: &Context, fid: usize, motivo: &str) -> Function {
    let f = &ctx.program.functions[fid];
    let unit = match f.node {
        dartforge_elements::model::FunctionRef::Function { unit, .. }
        | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => Some(unit),
        dartforge_elements::model::FunctionRef::None => f.class.and_then(|c| unidade_de(ctx, c)),
    }
    .or_else(|| ctx.program.library(f.library).units.first().copied())
    .expect("biblioteca com unidade");
    let generativo = super::construtor_generativo(ctx, fid);
    let ret = if generativo {
        Type::Void
    } else {
        ctx.outline.functions.get(fid).map_or(Type::Ref, |d| ctx.to_hir_type(d.return_type))
    };
    let simbolo = super::simbolo_de(ctx, fid);
    let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), ctx.symbol_name(f.name).to_string(), ret);
    let com_this = (f.class.is_some() && !f.static_ && !f.factory) || generativo;
    if com_this {
        b.add_param("this".to_string(), Type::Ref);
    }
    if let Some(d) = ctx.outline.functions.get(fid) {
        for p in &d.parameters {
            let t = b.repr(p.ty);
            b.add_param("p".to_string(), t);
        }
    }
    let texto = format!("{simbolo} ({motivo})");
    let t = b.emit(Instruction::Const(Constant::String(texto)), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// Baixa uma função do SDK da fonte num módulo à parte; se ela não baixa
/// (diagnóstico de construto, native pendente, intrínseco, pânico do
/// lowering, problema do verificador), o módulo recebe no lugar dela a
/// função recusada (`funcao_recusada`), e o motivo vai para
/// `Module::recusados`.
pub fn lower_funcao_ou_recusa(ctx: &Context, module: &mut Module, fid: usize) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.modo_sdk = true;
        super::lower_funcao(ctx, &mut m, fid);
        if m.erros.is_empty() {
            let problemas = super::verificador::verificar(&m);
            m.erros.extend(problemas);
        }
        m
    }));
    let motivo = match &r {
        Ok(m) if m.erros.is_empty() => None,
        Ok(m) => Some(
            m.erros[0]
                .strip_prefix(crate::PREFIXO_NAO_SUPORTADO)
                .unwrap_or(&m.erros[0])
                .to_string(),
        ),
        Err(p) => Some(format!(
            "pânico do lowering: {}",
            p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        )),
    };
    match (r, motivo) {
        (Ok(m), None) => {
            module.functions.extend(m.functions);
            module.globais.extend(m.globais);
        }
        (_, Some(motivo)) => {
            if !super::membros::tem_corpo(ctx, fid) {
                return;
            }
            let curto = motivo.rsplit_once(" (").map_or(motivo.as_str(), |(a, _)| a).to_string();
            let simbolo = super::simbolo_de(ctx, fid);
            module.functions.push(funcao_recusada(ctx, fid, &curto));
            // No resumo vai o diagnóstico inteiro, com a posição.
            module.recusados.push((simbolo, motivo));
        }
        (Err(_), None) => unreachable!(),
    }
}

/// Os adaptadores dos membros de instância das classes do módulo e as
/// tabelas de métodos das classes concretas dele (SDK da fonte).
pub fn lower_adaptadores_e_tabelas(ctx: &Context, module: &mut Module) {
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        let nome = ctx.symbol_name(f.name);
        if f.class.is_none() && nome.starts_with("_dartforge") && ctx.biblioteca_no_modulo(f.library) {
            module.ajudantes.push((nome.to_string(), super::simbolo_de(ctx, fid)));
        }
    }
    for (k, classe) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.biblioteca_no_modulo(classe.library) {
            continue;
        }
        let mut fids: Vec<usize> = classe.instance_members.values().map(|f| f.0 as usize).collect();
        fids.sort_unstable();
        fids.dedup();
        for fid in fids {
            if ctx.program.functions[fid].class != Some(cid) {
                continue;
            }
            adaptadores_ou_recusa(ctx, module, |m| lower_adaptadores_da_funcao(ctx, m, fid));
        }
        for &vid in &classe.fields {
            if ctx.program.variables[vid.0 as usize].static_ {
                continue;
            }
            adaptadores_ou_recusa(ctx, module, |m| lower_adaptadores_do_campo(ctx, m, vid));
        }
        let concreta = !classe.modifiers.abstract_ && !super::membros::e_mixin(ctx, cid);
        if concreta && let Some(id) = ctx.id_de_classe(cid) {
            let mut tabela = tabela_de_metodos(ctx, cid);
            // A VM implementa `_StackTrace.toString` em C++, sem declaração
            // Dart na classe. Nosso objeto guarda a string no campo zero.
            if ctx.symbol_name(classe.name) == "_StackTrace"
                && ctx.program.library(classe.library).uri == "dart:core"
                && let Some(u) = unidade_de(ctx, cid)
            {
                let simbolo = "df.$stackTrace.toString$c".to_string();
                let mut b = FnBuilder::new(ctx, u, simbolo.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param("args".to_string(), Type::Ptr);
                b.add_param("desc".to_string(), Type::Ptr);
                let texto = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![(this, Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(texto)));
                b.finalizar(module);
                tabela.retain(|(s, _)| s != "c:toString");
                tabela.push(("c:toString".to_string(), simbolo));
            }
            module.tabelas_de_metodos.push((id, simbolo_de_tabela(ctx, cid), tabela));
        }
    }
    // A função da tabela de toda classe concreta compilada (a alocação, em
    // qualquer módulo, registra a tabela da classe).
    for (k, classe) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.biblioteca_compilada(classe.library) || classe.modifiers.abstract_ || super::membros::e_mixin(ctx, cid) {
            continue;
        }
        if let Some(id) = ctx.id_de_classe(cid) {
            module.funcoes_de_tabela.insert(id, simbolo_de_tabela(ctx, cid));
        }
    }
}

/// A função que devolve a tabela de métodos de uma classe:
/// `df.mt.<biblioteca>.<Classe>`.
pub fn simbolo_de_tabela(ctx: &Context, cid: ClassId) -> String {
    let c = &ctx.program.classes[cid.0 as usize];
    format!(
        "df.mt.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(c.library)),
        crate::context::escapar(ctx.symbol_name(c.name))
    )
}

/// Os adaptadores gerados por `gerar`; os que não baixam viram entradas que
/// avisam em tempo de execução.
fn adaptadores_ou_recusa(ctx: &Context, module: &mut Module, gerar: impl FnOnce(&mut Module)) {
    let mut m = Module::new();
    m.modo_sdk = true;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gerar(&mut m)));
    if r.is_ok() && m.erros.is_empty() {
        module.functions.extend(m.functions);
        module.globais.extend(m.globais);
        return;
    }
    let motivo = m
        .erros
        .first()
        .map(|e| e.strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(e).to_string())
        .unwrap_or_else(|| "pânico do lowering".to_string());
    let motivo = motivo.rsplit_once(" (").map_or(motivo.as_str(), |(a, _)| a).to_string();
    for f in &m.functions {
        if !(f.symbol.ends_with("$c") || f.symbol.ends_with("$g") || f.symbol.ends_with("$s")) {
            continue;
        }
        let unit = ctx.program.units.iter().position(|_| true).map(|u| dartforge_elements::model::UnitId(u as u32));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, f.symbol.clone(), f.name.clone(), Type::Ref);
        b.add_param("this".to_string(), Type::Ref);
        b.add_param("args".to_string(), Type::Ptr);
        b.add_param("desc".to_string(), Type::Ptr);
        let t = b.emit(Instruction::Const(Constant::String(format!("{} ({motivo})", f.symbol))), Type::Ref);
        b.emit(
            Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
            Type::Void,
        );
        b.terminate(Terminator::Unreachable);
        module.recusados.push((f.symbol.clone(), motivo.clone()));
        module.functions.push(b.func);
    }
}
/// O getter preguiçoso de um global do SDK da fonte (ou a recusa dele) e o
/// setter `<getter>$set`, que outro módulo chama para gravar o global.
pub fn lower_global_ou_recusa(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let nome = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::simbolo_global(ctx, vid);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.modo_sdk = true;
        let mut b = FnBuilder::new(ctx, unit, getter.clone(), nome.clone(), repr);
        b.lower_getter_global(vid, repr);
        b.finalizar(&mut m);
        if m.erros.is_empty() {
            let problemas = super::verificador::verificar(&m);
            m.erros.extend(problemas);
        }
        m
    }));
    match r {
        Ok(m) if m.erros.is_empty() => {
            module.functions.extend(m.functions);
            module.globais.extend(m.globais);
        }
        r => {
            let motivo = match r {
                Ok(m) => m.erros[0].strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(&m.erros[0]).to_string(),
                Err(_) => "pânico do lowering".to_string(),
            };
            let motivo = motivo.rsplit_once(" (").map_or(motivo.as_str(), |(a, _)| a).to_string();
            let mut b = FnBuilder::new(ctx, unit, getter.clone(), nome.clone(), repr);
            let t = b.emit(Instruction::Const(Constant::String(format!("{getter} ({motivo})"))), Type::Ref);
            b.emit(
                Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
                Type::Void,
            );
            b.terminate(Terminator::Unreachable);
            module.functions.push(b.func);
            module.recusados.push((getter.clone(), motivo));
        }
    }
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), nome, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    s.gravar_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finalizar(module);
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `{}` com o SDK da fonte: um `_Map` (ou `_Set`) novo da fonte, o
    /// `LinkedHashMap`/`LinkedHashSet` padrão que o literal constrói.
    pub fn colecao_vazia_fonte(&mut self, mapa: bool) -> Operand {
        let nome = if mapa { "_Map" } else { "_Set" };
        let span = Span { start: 0, end: 0 };
        let Some(cid) = self.ctx.classe_do_sdk("_compact_hash", nome) else {
            return self.nao_suportado(&format!("literal de coleção sem `{nome}`"), span);
        };
        let vazio = self.ctx.interner.lookup("");
        let Some(ctor) = vazio.and_then(|v| self.ctx.program.classes[cid.0 as usize].constructors.get(&v).copied()) else {
            return self.nao_suportado(&format!("`{nome}()` ausente"), span);
        };
        self.instanciar_avaliados(ctor, &[], span)
    }

    /// `for (x in fonte) f(x)` pelo protocolo do `Iterator` (§17.7.3):
    /// `iterator`, `moveNext()` e `current`, pelos seletores.
    pub fn iterar_fonte(&mut self, fonte: Operand, f: &mut dyn FnMut(&mut Self, Operand)) {
        let it = self.chamar_por_seletor(fonte, "g:iterator".to_string(), &[]);
        let cabeca = self.new_block();
        let corpo = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::Branch(cabeca));
        self.set_block(cabeca);
        let ok = self.chamar_por_seletor(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coagir(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: corpo, else_block: fim });
        self.set_block(corpo);
        let x = self.chamar_por_seletor(it, "g:current".to_string(), &[]);
        f(self, x);
        self.terminate(Terminator::Branch(cabeca));
        self.set_block(fim);
    }

    /// Liga o elemento corrente de um `for-in` ao alvo dele (variável
    /// declarada, local existente ou padrão), na representação do alvo.
    pub fn ligar_alvo_de_for_in(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        x: Operand,
        iterable: dartforge_frontend::ast::ExprId,
        span: Span,
    ) {
        use dartforge_frontend::ast::{ExprKind, ForInTarget};
        match target {
            ForInTarget::Declared { name, .. } => {
                let ty = self.repr_do_local(name.span.start as usize);
                let x = self.coagir(x, ty);
                self.declarar_variavel(name.sym, name.span.start as usize, ty, x);
            }
            ForInTarget::Expression(e) => {
                if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                    let ty = self.buscar_local(id.sym).map_or(Type::Ref, |l| l.ty);
                    let x = self.coagir(x, ty);
                    self.gravar_local(id.sym, x);
                } else {
                    self.nao_suportado("alvo de for-in", span);
                }
            }
            ForInTarget::Pattern { pattern, .. } => {
                self.casar_irrefutavel(ast, *pattern, x, super::padroes::Ligacao::Declarar, iterable);
            }
        }
    }

    /// `for (alvo in iterável) corpo` com o SDK da fonte: o protocolo do
    /// `Iterator`, com `break`/`continue` (e os rótulos) como o `for-in` de
    /// sempre.
    pub fn lower_for_in_fonte(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
    ) {
        let fonte = self.lower_expr(ast, iterable);
        let it = self.chamar_por_seletor(fonte, "g:iterator".to_string(), &[]);
        let cabeca = self.new_block();
        let corpo = self.new_block();
        let fim = self.new_block();
        let rotulos = std::mem::take(&mut self.pending_labels);
        for &r in &rotulos {
            self.labeled_break_targets.insert(r, fim);
            self.labeled_continue_targets.insert(r, cabeca);
        }
        self.terminate(Terminator::Branch(cabeca));
        self.set_block(cabeca);
        let ok = self.chamar_por_seletor(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coagir(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: corpo, else_block: fim });
        self.set_block(corpo);
        self.break_targets.push(fim);
        self.continue_targets.push(cabeca);
        self.abrir_escopo();
        let x = self.chamar_por_seletor(it, "g:current".to_string(), &[]);
        self.ligar_alvo_de_for_in(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.fechar_escopo();
        self.terminate(Terminator::Branch(cabeca));
        self.break_targets.pop();
        self.continue_targets.pop();
        for &r in &rotulos {
            self.labeled_break_targets.remove(&r);
            self.labeled_continue_targets.remove(&r);
        }
        self.set_block(fim);
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `v is <nome>` para uma classe de `dart:core` (`List`, `Map`,
    /// `Record`), pelo grafo de classes do SDK da fonte; null não é.
    pub fn e_instancia_do_core(&mut self, v: Operand, nome: &str) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let Some(id) = self.ctx.classe_do_sdk("core", nome).and_then(|c| self.ctx.id_de_classe(c)) else {
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
        let nao_nulo = self.emit(Instruction::ICmp(ICmpOp::Ne, v, zero.clone()), Type::I1);
        let r = self.emit(Instruction::And(sub, nao_nulo), Type::I64);
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, zero), Type::I1)
    }

    /// Segue se `cond`, senão desvia para `falha`.
    fn exigir_fonte(&mut self, cond: Operand, falha: BlockId) {
        let cond = self.para_bool(cond);
        let ok = self.new_block();
        self.terminate(Terminator::CondBranch { cond, then_block: ok, else_block: falha });
        self.set_block(ok);
    }

    /// Padrão de lista com o SDK da fonte (§19.4.6 "List pattern"): o valor é
    /// um `List`, e `length`, `[]` e `sublist` vão pelo seletor.
    #[allow(clippy::too_many_arguments)]
    pub fn casar_lista_fonte(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        elements: &[dartforge_frontend::ast::ListPatternElement],
        valor: Operand,
        falha: BlockId,
        ligacao: super::padroes::Ligacao,
        ligados: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origem: dartforge_frontend::ast::ExprId,
    ) {
        use dartforge_frontend::ast::ListPatternElement;
        let v = self.coagir(valor, Type::Ref);
        let e = self.e_instancia_do_core(v.clone(), "List");
        self.exigir_fonte(e, falha);
        let len = self.chamar_por_seletor(v.clone(), "g:length".to_string(), &[]);
        let len = self.coagir(len, Type::I64);
        let resto = elements.iter().position(|e| matches!(e, ListPatternElement::Rest(_)));
        let n_fixos = elements.len() - usize::from(resto.is_some());
        let ok = self.emit(
            Instruction::ICmp(
                if resto.is_some() { ICmpOp::Sge } else { ICmpOp::Eq },
                len.clone(),
                Operand::Constant(Constant::Int(n_fixos as i64)),
            ),
            Type::I1,
        );
        self.exigir_fonte(ok, falha);
        for (i, el) in elements.iter().enumerate() {
            match (el, resto) {
                (ListPatternElement::Pattern(sp), r) => {
                    let idx = match r {
                        Some(r) if i > r => {
                            let depois = (elements.len() - i) as i64;
                            self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(depois))), Type::I64)
                        }
                        _ => Operand::Constant(Constant::Int(i as i64)),
                    };
                    let x = self.chamar_por_seletor(v.clone(), "c:[]".to_string(), &[(None, idx)]);
                    self.casar(ast, *sp, x, falha, ligacao, ligados, origem);
                }
                (ListPatternElement::Rest(Some(sp)), _) => {
                    let depois = (elements.len() - i - 1) as i64;
                    let fim = self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(depois))), Type::I64);
                    let sub = self.chamar_por_seletor(
                        v.clone(),
                        "c:sublist".to_string(),
                        &[(None, Operand::Constant(Constant::Int(i as i64))), (None, fim)],
                    );
                    self.casar(ast, *sp, sub, falha, ligacao, ligados, origem);
                }
                (ListPatternElement::Rest(None), _) => {}
            }
        }
    }

    /// Padrão de mapa com o SDK da fonte (§19.4.7 "Map pattern"): o valor é
    /// um `Map`, e cada chave existe (`containsKey`) e casa (`[]`).
    #[allow(clippy::too_many_arguments)]
    pub fn casar_mapa_fonte(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        entries: &[dartforge_frontend::ast::MapPatternEntry],
        valor: Operand,
        falha: BlockId,
        ligacao: super::padroes::Ligacao,
        ligados: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origem: dartforge_frontend::ast::ExprId,
    ) {
        let v = self.coagir(valor, Type::Ref);
        let e = self.e_instancia_do_core(v.clone(), "Map");
        self.exigir_fonte(e, falha);
        for en in entries {
            let k = self.lower_expr(ast, en.key);
            let tem = self.chamar_por_seletor(v.clone(), "c:containsKey".to_string(), &[(None, k.clone())]);
            let tem = self.coagir(tem, Type::I1);
            self.exigir_fonte(tem, falha);
            let x = self.chamar_por_seletor(v.clone(), "c:[]".to_string(), &[(None, k)]);
            self.casar(ast, en.value, x, falha, ligacao, ligados, origem);
        }
    }
}
/// As formas de record com campo nomeado do programa (`registros.rs`) com o
/// SDK da fonte: cada uma ganha a tabela de métodos — `toString` e `==`
/// (os gerados), e os membros de `Object` — com os adaptadores.
pub fn tabelas_das_formas_de_record(ctx: &Context, module: &mut Module) {
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else { return };
    let objeto = ctx.core.object_class.map(|c| tabela_de_metodos(ctx, c)).unwrap_or_default();
    for k in 0..ctx.formas_de_record.len() {
        let id = crate::context::ID_BASE_DE_FORMA + k as u32;
        let base = format!("df.$registro.{k}");
        // toString$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.toString$c"), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param("args".to_string(), Type::Ptr);
        b.add_param("desc".to_string(), Type::Ptr);
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.toString"), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        // ==$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.$3d$3d$c"), "==".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param("args".to_string(), Type::Ptr));
        b.add_param("desc".to_string(), Type::Ptr);
        let outro = b.emit(Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
        let r = b.emit_call_with_check(
            Instruction::CallStatic {
                symbol: super::registros::SIMBOLO_IGUAL.to_string(),
                args: vec![recv, outro],
                ret_ty: Type::I1,
            },
            Type::I1,
        );
        let r = b.coagir(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        // hashCode$g: o hash acompanha a igualdade estrutural do record.
        let mut b = FnBuilder::new(ctx, u, format!("{base}.hashCode$g"), "hashCode".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param("args".to_string(), Type::Ptr);
        b.add_param("desc".to_string(), Type::Ptr);
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.hashCode"), args: vec![recv], ret_ty: Type::I64 },
            Type::I64,
        );
        let r = b.coagir(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(module);
        let mut tabela: Vec<(String, String)> = vec![
            ("c:toString".to_string(), format!("{base}.toString$c")),
            ("c:==".to_string(), format!("{base}.$3d$3d$c")),
            ("g:hashCode".to_string(), format!("{base}.hashCode$g")),
            ("c:hashCode".to_string(), format!("{base}.hashCode$g")),
        ];
        for (s, f) in &objeto {
            if !tabela.iter().any(|(x, _)| x == s) {
                tabela.push((s.clone(), f.clone()));
            }
        }
        module.tabelas_de_metodos.push((id, format!("df.mt.$registro.{k}"), tabela));
    }
}

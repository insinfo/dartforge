//! O SDK compilado da fonte no lowering (P5c/P5d, δ; docs/NATIVO-PLANO.md §7).
//!
//! As sete bibliotecas de
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
use dartforge_elements::model::{ClassId, FunctionKind, LibraryId, UnitId, VariableId};
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
    ctx.subtipos(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library != classe.library || ctx.symbol_name(c.name).starts_with('_')
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
    nome.starts_with('_') || classe_fechada(ctx, cid) || classe_fechada_por_modificador(ctx, cid)
}

/// Os modificadores de classe do Dart 3 fecham `cid` na biblioteca dela:
/// ela é `final` ou `sealed`, e todo subtipo dela no programa é da mesma
/// biblioteca e `final`, `sealed` ou privado — nenhum código de fora pode
/// estendê-la, implementá-la nem sobrescrever um membro dela. É o caso de
/// `String`, `int`, `double`, `num` e `bool`: sem isto, `s.codeUnitAt(i)`
/// com `s` estático `String` ia pelo seletor e pela entrada uniforme, que
/// confere os argumentos no RTI (a escrita dos cabeçalhos do `dart:_http`
/// fazia centenas dessas chamadas por requisição).
pub fn classe_fechada_por_modificador(ctx: &Context, cid: ClassId) -> bool {
    let classe = &ctx.program.classes[cid.0 as usize];
    if !(classe.modifiers.final_ || classe.modifiers.sealed) {
        return false;
    }
    ctx.subtipos(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library == classe.library
            && (c.modifiers.final_ || c.modifiers.sealed || ctx.symbol_name(c.name).starts_with('_'))
    })
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.<seletor>(avaliados)` pela convenção uniforme; o resultado é
    /// `Ref`.
    pub fn chamar_por_seletor(&mut self, recv: Operand, seletor: String, avaliados: &[Avaliado]) -> Operand {
        self.chamar_por_seletor_com_tupla(recv, seletor, avaliados, Operand::Constant(Constant::Int(0)))
    }

    pub(super) fn chamar_por_seletor_com_tupla(
        &mut self,
        recv: Operand,
        seletor: String,
        avaliados: &[Avaliado],
        tupla_tipos: Operand,
    ) -> Operand {
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
        self.emit_call_with_check(Instruction::CallSeletor { seletor, recv, args, nomes, tupla_tipos }, Type::Ref)
    }

    /// `recv.nome(avaliados)` pelo seletor, com o nome privado da biblioteca
    /// da unidade corrente.
    pub fn chamar_por_nome(&mut self, recv: Operand, tipo: Tipo, nome: &str, avaliados: &[Avaliado]) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = texto_seletor(self.ctx, tipo, nome, lib);
        self.chamar_por_seletor(recv, s, avaliados)
    }

    /// [`Self::chamar_por_nome`] com o receptor `alvo` de tipo estático
    /// conhecido: se os argumentos têm todos o tipo garantido, pelo seletor
    /// tipado (`entrada_tipada.rs`), que só confere os covariantes.
    pub fn chamar_por_nome_tipado(
        &mut self,
        recv: Operand,
        alvo: dartforge_frontend::ast::ExprId,
        tipo: Tipo,
        nome: &str,
        avaliados: &[Avaliado],
    ) -> Operand {
        let tipado = self.ctx.get_type(self.unit_id, alvo).is_some_and(|t| t != self.ctx.core.dynamic_);
        self.chamar_por_nome_com_receptor_tipado(recv, tipado, tipo, nome, avaliados)
    }

    /// [`Self::chamar_por_nome_tipado`] com a decisão sobre o receptor já
    /// tomada (`receptor_tipado`: o tipo estático dele é conhecido).
    pub fn chamar_por_nome_com_receptor_tipado(
        &mut self,
        recv: Operand,
        receptor_tipado: bool,
        tipo: Tipo,
        nome: &str,
        avaliados: &[Avaliado],
    ) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = texto_seletor(self.ctx, tipo, nome, lib);
        let s = if receptor_tipado && matches!(tipo, Tipo::Chamar | Tipo::Gravar) && self.todos_conferidos(avaliados) {
            super::entrada_tipada::seletor_tipado(&s)
        } else {
            s
        };
        self.chamar_por_seletor(recv, s, avaliados)
    }

    /// Hook de `chamar_membro` (SDK da fonte): o membro de instância de uma
    /// classe do SDK vai pelo seletor, a não ser que só a biblioteca dela o
    /// possa sobrescrever e ele tenha **uma** implementação, que é um método
    /// (chamada direta). Um getter de interface do programa também usa o
    /// seletor quando alguma implementação concreta é um campo: o despacho
    /// por funções de `membros.rs` não pode representar esse getter implícito.
    pub fn chamar_membro_fonte(&mut self, recv: Operand, decl_fid: usize, avaliados: &[Avaliado]) -> Option<Operand> {
        if self.em_adaptador {
            return None;
        }
        let f = &self.ctx.program.functions[decl_fid];
        let cid = f.class?;
        if f.static_ {
            return None;
        }
        let nome = self.ctx.symbol_name(f.name).to_string();
        let chave = if f.kind == FunctionKind::Setter { format!("{nome}_=") } else { nome.clone() };
        let e_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !e_sdk && (!matches!(f.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor)
            || !implementacoes(self.ctx, cid, &chave).iter().any(|i| matches!(i, Implementacao::Campo(_))))
        {
            return None;
        }
        if e_sdk && membro_fechado(self.ctx, cid, &nome)
            && let [Implementacao::Funcao(alvo)] = implementacoes(self.ctx, cid, &chave)[..]
        {
            // O `this` de um membro do SDK é sempre `Ref`: um `int` escalar
            // vira `Smi` ou `_Mint` aqui (o `int` fechado por modificador
            // chega com o receptor em `i64`; sem a caixa, um valor par ou
            // fora do `Smi` seria lido como handle).
            let recv = self.coagir(recv, Type::Ref);
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
        if e_sdk && membro_fechado(self.ctx, cid, &nome) {
            let ret = match self.repr_retorno(decl_fid) {
                Type::Void => Type::Ref,
                t => t,
            };
            if let Some(r) = self.despacho_por_classe(&recv, cid, &chave, tipo, f.library, avaliados, ret) {
                let void = matches!(self.repr_retorno(decl_fid), Type::Void);
                return Some(if void { Operand::Constant(Constant::Null) } else { r });
            }
        }
        let s = texto_seletor(self.ctx, tipo, &nome, f.library);
        // Receptor tipado e argumentos garantidos: a entrada que só confere
        // os covariantes (`entrada_tipada.rs`).
        let s = if matches!(tipo, Tipo::Chamar | Tipo::Gravar) && self.todos_conferidos(avaliados) {
            super::entrada_tipada::seletor_tipado(&s)
        } else {
            s
        };
        let tupla = if self.funcao_generica(decl_fid) {
            self.tupla_armada.clone().unwrap_or(Operand::Constant(Constant::Int(0)))
        } else {
            Operand::Constant(Constant::Int(0))
        };
        let r = self.chamar_por_seletor_com_tupla(recv, s, avaliados, tupla);
        let ret = self.repr_retorno(decl_fid);
        Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coagir(r, ret) })
    }

    /// Despacho de poucos alvos (P2) de um membro fechado (só a biblioteca
    /// dele o implementa, e o programa inteiro dela está compilado) com 2 a
    /// [`POUCOS_ALVOS`] implementações: um `switch` pela classe do receptor
    /// escolhe a chamada direta ou o acesso ao campo de cada uma — o mesmo
    /// que a chamada direta do caso de uma implementação só, repetido por
    /// classe —, e o seletor fica no `default` (uma classe que o módulo não
    /// conhece). Sem isto, cada acesso ia pelo seletor e pela entrada
    /// uniforme, que confere os argumentos (os `_index`, `_data`,
    /// `_usedData` dos mapas e conjuntos: duas hierarquias no
    /// `compact_hash.dart`). `None`: sem despacho por classe (o chamador
    /// segue pelo seletor).
    #[allow(clippy::too_many_arguments)]
    fn despacho_por_classe(
        &mut self,
        recv: &Operand,
        cid: ClassId,
        chave: &str,
        tipo: Tipo,
        biblioteca: dartforge_elements::model::LibraryId,
        avaliados: &[Avaliado],
        ret: Type,
    ) -> Option<Operand> {
        let por_classe = implementacoes_por_classe(self.ctx, cid, chave)?;
        let mut distintas: Vec<Implementacao> = Vec::new();
        for (_, i) in &por_classe {
            if !distintas.contains(i) {
                distintas.push(*i);
            }
        }
        if distintas.len() < 2 || distintas.len() > POUCOS_ALVOS {
            return None;
        }
        // Chamar um campo (uma closure guardada) fica com o seletor.
        if tipo == Tipo::Chamar && distintas.iter().any(|i| matches!(i, Implementacao::Campo(_))) {
            return None;
        }
        let recv = self.coagir(recv.clone(), Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let blocos: Vec<BlockId> = distintas.iter().map(|_| self.new_block()).collect();
        let padrao = self.new_block();
        let juncao = self.new_block();
        let cases = por_classe
            .iter()
            .map(|(id, i)| (*id, blocos[distintas.iter().position(|d| d == i).expect("implementação listada")]))
            .collect();
        self.terminate(Terminator::Switch { val: cls, default: padrao, cases });
        let span = dartforge_diagnostics::Span { start: 0, end: 0 };
        let mut entradas = Vec::new();
        for (i, &b) in distintas.iter().zip(&blocos) {
            self.set_block(b);
            let r = match (*i, tipo) {
                (Implementacao::Campo(vid), Tipo::Gravar) => {
                    let v = avaliados.first().map(|(_, v)| v.clone()).unwrap_or(Operand::Constant(Constant::Null));
                    let v = self.coagir(v, self.repr_do_campo(vid));
                    self.gravar_campo(recv.clone(), vid, v, span);
                    Operand::Constant(Constant::Null)
                }
                (Implementacao::Campo(vid), _) => self.ler_campo_com_late_direto(recv.clone(), vid, span),
                (Implementacao::Funcao(alvo), _) => {
                    let args = self.casar_args(alvo, avaliados);
                    let r = self.chamar_direto(alvo, Some(recv.clone()), args);
                    if matches!(self.repr_retorno(alvo), Type::Void) { Operand::Constant(Constant::Null) } else { r }
                }
            };
            if self.is_terminated() {
                continue;
            }
            let r = self.coagir(r, ret);
            entradas.push((self.current_block, r));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(padrao);
        let nome = chave.strip_suffix("_=").unwrap_or(chave);
        let s = texto_seletor(self.ctx, tipo, nome, biblioteca);
        let s = if matches!(tipo, Tipo::Chamar | Tipo::Gravar) && self.todos_conferidos(avaliados) {
            super::entrada_tipada::seletor_tipado(&s)
        } else {
            s
        };
        let r = self.chamar_por_seletor(recv, s, avaliados);
        if !self.is_terminated() {
            let r = self.coagir(r, ret);
            entradas.push((self.current_block, r));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        if entradas.is_empty() {
            // Todo caminho lança: a junção é inalcançável.
            return Some(Self::valor_zero(ret));
        }
        Some(self.emit(Instruction::Phi { incoming: entradas, ty: ret }, ret))
    }

    /// Hook da leitura de campo: um getter de subclasse pode sobrescrever
    /// inclusive o getter implícito de um campo da classe base.
    pub fn ler_campo_fonte(&mut self, obj: Operand, vid: VariableId) -> Option<Operand> {
        if self.em_adaptador {
            return None;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let cid = v.class?;
        let nome = self.ctx.symbol_name(v.name).to_string();
        let e_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !e_sdk {
            // A leitura direta do campo só vale se toda classe concreta do
            // tipo tem ESTE campo: um getter que o sobrescreve, ou outro
            // campo de mesmo nome numa classe que só implementa a interface
            // (`class ScalarToken implements Token { final FileSpan span; }`
            // do `package:yaml`, noutra posição do objeto), vão pelo seletor.
            let so_este_campo =
                implementacoes(self.ctx, cid, &nome).iter().all(|i| *i == Implementacao::Campo(vid));
            if so_este_campo {
                return None;
            }
        }
        if e_sdk && membro_fechado(self.ctx, cid, &nome)
            && implementacoes(self.ctx, cid, &nome).iter().all(|i| *i == Implementacao::Campo(vid))
        {
            return None;
        }
        let repr = self.repr_do_campo(vid);
        if e_sdk && membro_fechado(self.ctx, cid, &nome)
            && let Some(r) = self.despacho_por_classe(&obj, cid, &nome, Tipo::Ler, v.library, &[], repr)
        {
            return Some(r);
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
        if self.em_adaptador {
            return false;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let Some(cid) = v.class else { return false };
        let e_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        let nome = self.ctx.symbol_name(v.name).to_string();
        // Como na leitura: direto só se toda classe concreta grava ESTE
        // campo (no SDK, também se o membro é fechado).
        let so_este_campo =
            implementacoes(self.ctx, cid, &format!("{nome}_=")).iter().all(|i| *i == Implementacao::Campo(vid));
        if so_este_campo && (!e_sdk || membro_fechado(self.ctx, cid, &nome)) {
            return false;
        }
        if e_sdk && membro_fechado(self.ctx, cid, &nome)
            && self
                .despacho_por_classe(&obj, cid, &format!("{nome}_="), Tipo::Gravar, v.library, &[(None, valor.clone())], Type::Ref)
                .is_some()
        {
            return true;
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

    /// `a == b` (ou `a != b`, com `negar`) quando o tipo estático de `a` é
    /// uma classe cujo `==` é fechado ([`membro_fechado`]: `String`, `int`,
    /// `double`, `bool`…): a regra do null de [`Self::igualdade_fonte`] e,
    /// com os dois lados não nulos, o `==` pelo despacho de
    /// [`Self::chamar_membro_fonte`] — a chamada direta, ou o `switch` pela
    /// classe com o seletor no `default` —, sem a entrada uniforme e a
    /// conferência dos argumentos do seletor `c:==`. É o `char == " "` dos
    /// analisadores do `dart:_http`. `None`: o caminho geral.
    pub fn igualdade_de_classe_fechada(
        &mut self,
        tipo_de_a: Option<dartforge_types::table::TypeId>,
        a: &Operand,
        b: &Operand,
        negar: bool,
    ) -> Option<Operand> {
        if self.em_adaptador || self.operand_type(a) != Type::Ref {
            return None;
        }
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(tipo_de_a?) else {
            return None;
        };
        let fid = self.membro_na_classe(*class, "==")?;
        let dono = self.ctx.program.functions[fid].class?;
        if !self.ctx.program.library(self.ctx.program.classes[dono.0 as usize].library).is_sdk
            || !membro_fechado(self.ctx, dono, "==")
        {
            return None;
        }
        let a = a.clone();
        let b = self.coagir(b.clone(), Type::Ref);
        // `String`: o `==` das duas classes concretas é o mesmo native
        // (`_StringBase._igualA`, que já trata `null` e o outro lado que
        // não é `String`): uma chamada, sem a regra do null nem o `switch`
        // pela classe — o intrínseco `String_equality` da VM.
        if self.ctx.core.string_class == Some(*class) {
            let r = self.emit(
                Instruction::CallRuntime {
                    name: crate::nativos::simbolo("DartForge_string_igual_a"),
                    args: vec![(a, Type::Ref), (b, Type::Ref)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let e = self.coagir(r, Type::I1);
            return Some(if negar { self.emit(Instruction::LNot(e), Type::I1) } else { e });
        }
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
        // As condições de `chamar_membro_fonte` já valem (membro de
        // instância de classe do SDK, fora de adaptador); o seletor fica só
        // por garantia, sem deixar blocos pela metade.
        let r = match self.chamar_membro_fonte(a.clone(), fid, &[(None, b.clone())]) {
            Some(r) => r,
            None => self.chamar_por_seletor(a, "c:==".to_string(), &[(None, b)]),
        };
        let r2 = self.coagir(r, Type::I1);
        let mut entradas = vec![(fim1, r1)];
        if !self.is_terminated() {
            entradas.push((self.current_block, r2));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        let e = self.emit(Instruction::Phi { incoming: entradas, ty: Type::I1 }, Type::I1);
        Some(if negar { self.emit(Instruction::LNot(e), Type::I1) } else { e })
    }

    /// `op is T` com o SDK da fonte (hook de `testar_tipo`): toda classe
    /// (`int`, `num`, `String`, `Object`…) é uma classe compilada com id, e
    /// a resposta é a do grafo de subtipos que as bibliotecas registram; null
    /// só é um `T` quando `T` é `Null` ou anulável (§20.3). Parâmetros de tipo
    /// são resolvidos pelo RTI da instância ou pela tupla do método.
    pub fn testar_tipo_fonte(
        &mut self,
        ast_ty: &dartforge_frontend::ast::TypeAnnotation,
        op: Operand,
    ) -> Option<Operand> {
        // No código do SDK, os testes com argumentos de tipo (`is List<E>`)
        // escolhem um caminho rápido equivalente para um programa correto
        // (`ListBase.setRange`, `List.from`, `ListQueue.addAll`…): até a RTI,
        // eles conferem a classe. No programa, só o cast (`as`) confere a
        // classe; o `is` com argumentos continua diagnóstico.
        let no_sdk = self.ctx.program.library(self.ctx.program.unit(self.unit_id).library).is_sdk;
        let so_classe = self.cast_so_pela_classe || no_sdk;
        // Tipos sem classe que não têm receita resolvida continuam recusados.
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
            // `Iterable.generate<E>` testa `_id is E Function(int)` antes de
            // aceitar o gerador implícito. A RTI representa a assinatura e
            // substitui `E` pela tupla da factory do SDK.
            if let Some(receita) = self.receita_da_anotacao(ast_ty) {
                let tipo = self.rti_da_receita(&receita);
                return Some(self.testar_rti(op, tipo));
            }
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
        let argumentos = !args.is_empty() && !args.iter().all(|a| trivial(unit_ast.ty(*a)));
        if argumentos && !so_classe {
            return None;
        }
        // No SDK, `is C<…>` com argumentos confere a RTI inteira (o
        // `value is Future<T>` do `_Future._asyncComplete` decide entre
        // encadear e completar com o próprio `Future`); o `as` segue pela
        // classe.
        if argumentos
            && no_sdk
            && !self.cast_so_pela_classe
            && let Some(receita) = self.receita_da_anotacao(ast_ty)
        {
            let tipo = self.rti_da_receita(&receita);
            return Some(self.testar_rti(op, tipo));
        }
        let ultimo = name.last()?;
        let nome = self.ctx.symbol_name(ultimo.sym).to_string();
        // Parâmetros da classe vêm do RTI de `this` (`P<i>`). Os de método
        // vêm da tupla oculta repassada pelo adaptador do seletor (`M<i>`).
        if no_sdk
            && !self.cast_so_pela_classe
            && args.is_empty()
            && (self.params_de_tipo_da_funcao.contains(&ultimo.sym)
                || self.indice_local_de_tipo(ultimo.sym).is_some()
                || self.params_da_classe().contains(&ultimo.sym))
        {
            let receita = self.receita_da_anotacao(ast_ty)?;
            let tipo = self.rti_da_receita(&receita);
            return Some(self.testar_rti(op, tipo));
        }
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
        if let Some(r) = self.texto_em_linha(&membro, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        // Os ganchos de cada pacote do espaço unificado
        // (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, passo 4): listas do núcleo
        // (P3), listas tipadas e SIMD (P4), caixas, closures e records (P2).
        if let Some(r) = self.lista_em_linha(&membro, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.tipada_em_linha(&membro, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.caixa_em_linha(&membro, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
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
            None if reconhecido && let Some(op) = self.fabrica_tipada(&membro, args) => return Some(op),
            None if reconhecido => match crate::nativos::intrinseco(&membro) {
                Some(n) => n.to_string(),
                None => return Some(self.nao_suportado(&format!("intrínseco da VM `{membro}`"), span)),
            },
            None => {
                if let Some(r) = self.chamar_native_anotado(fid, args, span) {
                    return Some(r);
                }
                return Some(self.nao_suportado(&format!("external sem implementação `{membro}`"), span));
            }
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
        // `RETORNO_REF`: o runtime devolve a caixa, e o valor declarado sai
        // da coerção (nativos.rs).
        let ret_valor = if crate::nativos::RETORNO_REF.contains(&nome.as_str()) && ret_valor != Type::Void {
            Type::Ref
        } else {
            ret_valor
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

    /// As fábricas `vm:recognized` do `typed_data_patch.dart` da VM: a
    /// lista pública (`Uint8List(n)`) aloca a lista interna da classe
    /// correspondente (`_Uint8List`), e `_XArrayView._(base, desloc, n)` a
    /// visão (`typed_data.rs` do runtime). A alocação registra a tabela de
    /// métodos da classe, como a de um objeto comum (emissor LLVM).
    fn fabrica_tipada(&mut self, membro: &str, args: &[Operand]) -> Option<Operand> {
        const LISTAS: &[(&str, &str, i64)] = &[
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
        const VISOES: &[(&str, i64)] = &[
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
        let id_de = |b: &Self, nome: &str| -> Option<i64> {
            b.ctx.classe_do_sdk("typed_data", nome).and_then(|c| b.ctx.id_de_classe(c)).map(i64::from)
        };
        if let Some(&(_, interna, tipo)) = LISTAS.iter().find(|(m, _, _)| *m == membro) {
            let cid = id_de(self, interna)?;
            let n = self.coagir(args.first()?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_novo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(tipo)), Type::I64),
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
        const EXTERNAS: &[(&str, &str, i64)] = &[
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
        if let Some(&(_, interna, tipo)) = EXTERNAS.iter().find(|(m, _, _)| *m == membro) {
            let cid = id_de(self, interna)?;
            let ponteiro = self.coagir(args.first()?.clone(), Type::Ref);
            let n = self.coagir(args.get(1)?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_externo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(tipo)), Type::I64),
                        (ponteiro, Type::Ref),
                        (n, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        let (classe, fabrica) = membro.split_once('.')?;
        if fabrica != "_" {
            return None;
        }
        // `_UnmodifiableXArrayView` estende `_XArrayView`: o mesmo tipo.
        let modificavel = classe.strip_prefix("_Unmodifiable").map(|r| format!("_{r}"));
        let procurada = modificavel.as_deref().unwrap_or(classe);
        let &(_, tipo) = VISOES.iter().find(|(c, _)| *c == procurada)?;
        // O bit `VISAO_IMUTAVEL` do runtime: a visão não modificável recusa
        // o caminho rápido de `[]=` (o `[]=` dela lança, como na VM).
        let tipo = if modificavel.is_some() { tipo | 0x100 } else { tipo };
        let cid = id_de(self, classe)?;
        let base = self.coagir(args.first()?.clone(), Type::Ref);
        let desloc = self.coagir(args.get(1)?.clone(), Type::I64);
        let n = self.coagir(args.get(2)?.clone(), Type::I64);
        Some(self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_view_nova".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(cid)), Type::I64),
                    (Operand::Constant(Constant::Int(tipo)), Type::I64),
                    (base, Type::Ref),
                    (desloc, Type::I64),
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
            // `ClassID.getID(o)`: a classe do valor em linha (`@df.classe`,
            // docs/NATIVO-ESPACO-UNIFICADO.md §3.8), sem chamar o runtime.
            "ClassID_getID" => {
                let o = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                let o = self.coagir(o, Type::Ref);
                let c = self.emit(
                    Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(o, Type::Ref)], ret_ty: Type::I64 },
                    Type::I64,
                );
                if ret == Type::Void { c } else { self.coagir(c, ret) }
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
    if let Some(r) = ctx.memoria_implementacoes.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().clone();
    }
    let r = implementacoes_sem_memoria(ctx, cid, sym);
    ctx.memoria_implementacoes.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), std::sync::Arc::new(r.clone()));
    r
}

fn implementacoes_sem_memoria(ctx: &Context, cid: ClassId, sym: dartforge_intern::SymbolId) -> Vec<Implementacao> {
    let mut saida = Vec::new();
    for &kid in ctx.subtipos(cid).iter() {
        let classe = &ctx.program.classes[kid.0 as usize];
        if !ctx.biblioteca_compilada(classe.library)
            || classe.modifiers.abstract_
            || super::membros::e_mixin(ctx, kid)
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

/// Cada classe concreta compilada, subtipo de `cid`, com a implementação
/// de `chave` que a linearização dela acha (como em [`implementacoes`]), pelo
/// id de classe do runtime; `None` se alguma não tem id.
pub fn implementacoes_por_classe(ctx: &Context, cid: ClassId, chave: &str) -> Option<Vec<(i64, Implementacao)>> {
    let sym = ctx.interner.lookup(chave)?;
    if let Some(r) = ctx.memoria_implementacoes_por_classe.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().map(|v| v.as_ref().clone());
    }
    let r = implementacoes_por_classe_sem_memoria(ctx, cid, sym);
    ctx.memoria_implementacoes_por_classe.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), r.clone().map(std::sync::Arc::new));
    r
}

fn implementacoes_por_classe_sem_memoria(
    ctx: &Context,
    cid: ClassId,
    sym: dartforge_intern::SymbolId,
) -> Option<Vec<(i64, Implementacao)>> {
    let mut saida = Vec::new();
    for &kid in ctx.subtipos(cid).iter() {
        let classe = &ctx.program.classes[kid.0 as usize];
        if !ctx.biblioteca_compilada(classe.library)
            || classe.modifiers.abstract_
            || super::membros::e_mixin(ctx, kid)
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
            saida.push((i64::from(ctx.id_de_classe(kid)?), i));
            break;
        }
    }
    Some(saida)
}

/// Quantas implementações distintas o despacho por classe
/// (`despacho_por_classe`) aceita; acima disso, o seletor.
const POUCOS_ALVOS: usize = 4;

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
            // A entrada tipada do método (`entrada_tipada.rs`), quando ele
            // a tem, no seletor `t…` — só se o `c:`/`s:` dele venceu aqui.
            let tipada = f.variable.is_none()
                && !matches!(f.kind, FunctionKind::Getter)
                && super::entrada_tipada::precisa_entrada_tipada(ctx, fid);
            // Fora do mundo fechado do programa (C7, `mundo_nativo.rs`): o
            // seletor fica tomado (uma superclasse não preenche o lugar com
            // outro membro), mas sem entrada nem adaptador — a chamada
            // dinâmica a ele cai no `noSuchMethod`, como a que o mundo diz
            // que não acontece.
            let vivo = !ctx.membro_podado(fid);
            let mut por = |tipo: Tipo, a: Adaptador, base: &str| {
                let s = texto_seletor(ctx, tipo, nome, f.library);
                if vistos.insert(s.clone()) && vivo {
                    if tipada && a != Adaptador::Ler {
                        saida.push((super::entrada_tipada::seletor_tipado(&s), format!("{base}$t{}", &a.sufixo()[1..])));
                    }
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

/// A classe (ou uma superclasse do programa) declara `noSuchMethod`: o de
/// `Object` não conta.
fn tem_no_such_method_proprio(ctx: &Context, cid: ClassId) -> bool {
    let Some(nsm) = ctx.interner.lookup("noSuchMethod") else { return false };
    for c in linearizacao(ctx, cid) {
        if Some(c) == ctx.core.object_class {
            return false;
        }
        if let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&nsm)
            && implementado(ctx, f.0 as usize)
        {
            return true;
        }
    }
    false
}

/// Os membros de instância das superinterfaces de `cid` (superclasses,
/// mixins, `implements`, `on`, transitivamente) cujo seletor a tabela não
/// tem: (seletor, uso, membro declarado).
fn membros_para_encaminhar(ctx: &Context, cid: ClassId, tabela: &[(String, String)]) -> Vec<(String, Tipo, usize)> {
    let mut vistos: std::collections::HashSet<String> = tabela.iter().map(|(s, _)| s.clone()).collect();
    let mut saida = Vec::new();
    let mut pilha = vec![cid];
    let mut classes = std::collections::HashSet::new();
    while let Some(c) = pilha.pop() {
        if !classes.insert(c) || Some(c) == ctx.core.object_class {
            continue;
        }
        let classe = &ctx.program.classes[c.0 as usize];
        pilha.extend(classe.supertype_class);
        pilha.extend(classe.mixin_classes.iter().copied());
        pilha.extend(classe.interface_classes.iter().copied());
        pilha.extend(classe.on_classes.iter().copied());
        let mut membros: Vec<(&str, usize)> =
            classe.instance_members.iter().map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize)).collect();
        membros.sort();
        for (chave, fid) in membros {
            let f = &ctx.program.functions[fid];
            if f.static_ || matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
                continue;
            }
            let nome = chave.strip_suffix("_=").unwrap_or(chave);
            let usos: &[Tipo] = match f.kind {
                FunctionKind::Setter => &[Tipo::Gravar],
                FunctionKind::Getter => &[Tipo::Ler],
                FunctionKind::ImplicitAccessor if chave.ends_with("_=") => &[Tipo::Gravar],
                FunctionKind::ImplicitAccessor => &[Tipo::Ler],
                _ => &[Tipo::Chamar],
            };
            for &uso in usos {
                let s = texto_seletor(ctx, uso, nome, f.library);
                if vistos.insert(s.clone()) {
                    saida.push((s, uso, fid));
                }
            }
        }
    }
    saida
}

/// O encaminhador de `noSuchMethod` do membro `fid` (entrada uniforme):
/// desempacota os argumentos como o membro declara (os padrões dos
/// opcionais omitidos), e o runtime monta o `Invocation` na ordem da
/// declaração e chama o `noSuchMethod` do receptor.
fn gerar_encaminhador_nsm(ctx: &Context, module: &mut Module, unit: UnitId, simbolo: &str, fid: usize, tipo: Tipo) {
    let f = &ctx.program.functions[fid];
    let nome = ctx.symbol_name(f.name).to_string();
    let mut b = FnBuilder::new(ctx, unit, simbolo.to_string(), nome.clone(), Type::Ref);
    let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
    let args = Operand::Val(b.add_param("args".to_string(), Type::Ptr));
    let desc = Operand::Val(b.add_param("desc".to_string(), Type::Ptr));
    let (codigo, valores, nomes, tupla) = match tipo {
        Tipo::Chamar => {
            let infos = b.params_da_funcao(fid);
            let Some(vals) = b.desempacotar(&infos, args.clone(), desc.clone()) else {
                b.finalizar(module);
                return;
            };
            let nomes: Vec<String> = infos
                .iter()
                .filter(|p| p.kind == ParameterKind::Named)
                .map(|p| p.nome.clone().unwrap_or_default())
                .collect();
            // A tupla do método genérico vem depois dos argumentos.
            let tupla = if b.funcao_generica(fid) {
                let npos = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
                let nnom = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
                let i = b.emit(Instruction::Add(npos, nnom), Type::I64);
                b.emit(Instruction::LoadIndexed { base: args.clone(), index: i }, Type::I64)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            (0, vals, nomes, tupla)
        }
        Tipo::Ler => (1, Vec::new(), Vec::new(), Operand::Constant(Constant::Int(0))),
        Tipo::Gravar => {
            let v = b.emit(Instruction::LoadIndexed { base: args.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
            (2, vec![v], Vec::new(), Operand::Constant(Constant::Int(0)))
        }
    };
    let npos = valores.len() - nomes.len();
    let nnom = nomes.len();
    // `dartforge_encaminhar_nsm` lê todas as capturas como `Ref`: os valores
    // vão encaixotados (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 48).
    let mut campos: Vec<Operand> = valores.into_iter().map(|v| b.coagir(v, Type::Ref)).collect();
    for n in nomes {
        campos.push(b.emit(Instruction::Const(Constant::String(n)), Type::Ref));
    }
    let ambiente = b.emit(Instruction::AllocEnv { values: campos }, Type::Ref);
    let texto = b.emit(Instruction::Const(Constant::String(nome)), Type::Ref);
    let r = b.emit_call_with_check(
        Instruction::CallRuntime {
            name: "dartforge_encaminhar_nsm".to_string(),
            args: vec![
                (this, Type::Ref),
                (Operand::Constant(Constant::Int(codigo)), Type::I64),
                (texto, Type::Ref),
                (ambiente, Type::Ref),
                (Operand::Constant(Constant::Int(npos as i64)), Type::I64),
                (Operand::Constant(Constant::Int(nnom as i64)), Type::I64),
                (tupla, Type::I64),
            ],
            ret_ty: Type::Ref,
        },
        Type::Ref,
    );
    b.terminate(Terminator::Return(Some(r)));
    b.finalizar(module);
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
    // A entrada tipada (`$tc`, `$ts`): só confere os covariantes
    // (`entrada_tipada.rs`), e só existe quando dispensa alguma conferência.
    let mut lista: Vec<(Adaptador, bool)> = adaptadores.iter().map(|&a| (a, false)).collect();
    if !matches!(f.kind, FunctionKind::Getter) && super::entrada_tipada::precisa_entrada_tipada(ctx, fid) {
        lista.extend(adaptadores.iter().filter(|a| **a != Adaptador::Ler).map(|&a| (a, true)));
    }
    for (a, tipada) in lista {
        let simbolo = if tipada { format!("{base}$t{}", &a.sufixo()[1..]) } else { format!("{base}{}", a.sufixo()) };
        let unit = unidade_de(ctx, f.class.expect("membro de classe"));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, simbolo, ctx.symbol_name(f.name).to_string(), Type::Ref);
        b.em_adaptador = true;
        b.aridade_garantida = tipada;
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
                let Some(vals) = b.desempacotar(&infos, args.clone(), desc.clone()) else {
                    b.finalizar(module);
                    continue;
                };
                // A VM confere os argumentos na entrada do método, antes do
                // corpo (e do native, no SDK: `String.+(String)` não pode
                // receber um Smi como handle de String via `dynamic`): os
                // covariantes de classe e os de tipo nominal, com a mensagem
                // " of 'nome'" — antes de converter cada um à representação.
                b.this_param = Some(recv.clone());
                b.this_finalizavel = ctx.classificar_this(fid);
                b.enclosing_class = f.class;
                b.conferir_argumentos_da_entrada_com(fid, &vals, tipada);
                let reprs: Vec<Type> = ctx.outline.functions[fid].parameters.iter().map(|p| b.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| b.coagir(v, r)).collect();
                if b.funcao_generica(fid) {
                    let npos = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) },
                        Type::I64,
                    );
                    let nnom = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) },
                        Type::I64,
                    );
                    let indice = b.emit(Instruction::Add(npos, nnom), Type::I64);
                    b.tupla_armada = Some(b.emit(Instruction::LoadIndexed { base: args.clone(), index: indice }, Type::I64));
                }
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
        ctx.retorno_hir(fid)
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
        module.parametros_escalares_dart.extend(m.parametros_escalares_dart);
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

/// As funções que devolvem os valores padrão não literais dos parâmetros
/// das funções do módulo (`membros::simbolo_do_padrao`): quem chama de fora
/// do módulo (o programa) não tem as resoluções das unidades do SDK e chama
/// esta função no lugar de baixar a expressão.
pub fn lower_padroes_do_sdk(ctx: &Context, module: &mut Module) {
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        if !ctx.biblioteca_no_modulo(f.library) {
            continue;
        }
        let unit = match f.node {
            dartforge_elements::model::FunctionRef::Function { unit, .. }
            | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => unit,
            dartforge_elements::model::FunctionRef::None => continue,
        };
        let simbolo_base = super::simbolo_de(ctx, fid);
        let b = FnBuilder::new(ctx, unit, String::new(), String::new(), Type::Ref);
        let Some((unidade, params)) = b.parametros_de(fid) else { continue };
        let exportados: Vec<usize> = params
            .iter()
            .enumerate()
            .filter(|(_, p)| p.default_value.is_some_and(|e| !super::membros::padrao_literal(&ctx.program.unit(unidade).ast, e)))
            .map(|(i, _)| i)
            .collect();
        drop(b);
        for i in exportados {
            let simbolo = format!("{simbolo_base}.padrao.{i}");
            let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), simbolo.clone(), Type::Ref);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let v = b.valor_padrao(fid, i).unwrap_or(Operand::Constant(Constant::Null));
                let v = b.coagir(v, Type::Ref);
                b.terminate(Terminator::Return(Some(v)));
                b
            }));
            match r {
                Ok(b) if b.erros.is_empty() => b.finalizar(module),
                Ok(b) => {
                    let motivo = b.erros[0].clone();
                    module.functions.push(padrao_recusado(ctx, unit, &simbolo, &motivo));
                    module.recusados.push((simbolo, motivo));
                }
                Err(_) => {
                    let motivo = "pânico ao baixar o valor padrão".to_string();
                    module.functions.push(padrao_recusado(ctx, unit, &simbolo, &motivo));
                    module.recusados.push((simbolo, motivo));
                }
            }
        }
    }
}

/// A função de padrão que não baixa: avisa em tempo de execução, como as
/// funções recusadas.
fn padrao_recusado(ctx: &Context, unit: dartforge_elements::model::UnitId, simbolo: &str, motivo: &str) -> Function {
    let mut b = FnBuilder::new(ctx, unit, simbolo.to_string(), simbolo.to_string(), Type::Ref);
    let curto = motivo.strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(motivo);
    let t = b.emit(Instruction::Const(Constant::String(format!("{simbolo} ({curto})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// Os adaptadores dos membros de instância das classes do módulo e as
/// tabelas de métodos das classes concretas dele (SDK da fonte).
pub fn lower_adaptadores_e_tabelas(ctx: &Context, module: &mut Module) {
    lower_padroes_do_sdk(ctx, module);
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
            // Fora do mundo fechado (C7): sem entrada na tabela, sem adaptador.
            if ctx.program.functions[fid].class != Some(cid) || ctx.membro_podado(fid) {
                continue;
            }
            adaptadores_ou_recusa(ctx, module, |m| lower_adaptadores_da_funcao(ctx, m, fid));
        }
        for &vid in &classe.fields {
            if ctx.program.variables[vid.0 as usize].static_ || ctx.campo_podado(vid) {
                continue;
            }
            adaptadores_ou_recusa(ctx, module, |m| lower_adaptadores_do_campo(ctx, m, vid));
        }
        let concreta = !classe.modifiers.abstract_ && !super::membros::e_mixin(ctx, cid);
        if concreta && let Some(id) = ctx.id_de_classe(cid) {
            let mut tabela = tabela_de_metodos(ctx, cid);
            // A VM implementa `_StackTrace.toString` em C++, sem declaração
            // Dart na classe. Nosso objeto guarda a string no campo zero, ou
            // os endereços do `throw` que o runtime simboliza no primeiro
            // pedido (o rastro simbólico, §13.14).
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
                    name: "dartforge_rastro_texto".to_string(),
                    args: vec![(this, Type::Ref)],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(texto)));
                b.finalizar(module);
                tabela.retain(|(s, _)| s != "c:toString");
                tabela.push(("c:toString".to_string(), simbolo));
            }
            // O layout gerado para enums guarda `_Enum._name` no campo 1.
            // Extensões do SDK (byName/asNameMap) leem esse campo pelo
            // seletor privado de dart:core, inclusive em enums do usuário.
            if super::enums::e_enum(ctx, cid)
                && let Some(decl) = classe.decl
            {
                let simbolo = format!(
                    "df.{}.{}.$enumName$g",
                    crate::context::escapar(&ctx.nome_da_biblioteca(classe.library)),
                    crate::context::escapar(ctx.symbol_name(classe.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, simbolo.clone(), "_name".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param("args".to_string(), Type::Ptr);
                b.add_param("desc".to_string(), Type::Ptr);
                let nome = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(1)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(nome)));
                b.finalizar(module);
                tabela.retain(|(s, _)| s != "g:_name@dart:core");
                tabela.push(("g:_name@dart:core".to_string(), simbolo));
                // `index` (campo 0) pelo seletor: `x.index` com `x` de tipo
                // `Enum` ou `T extends Enum` (o `EnumSet.updated` do
                // analyzer) chega pela tabela, não pelo tipo estático.
                let simbolo = format!(
                    "df.{}.{}.$enumIndex$g",
                    crate::context::escapar(&ctx.nome_da_biblioteca(classe.library)),
                    crate::context::escapar(ctx.symbol_name(classe.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, simbolo.clone(), "index".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param("args".to_string(), Type::Ptr);
                b.add_param("desc".to_string(), Type::Ptr);
                let indice = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::I64,
                }, Type::I64);
                let indice = b.coagir(indice, Type::Ref);
                b.terminate(Terminator::Return(Some(indice)));
                b.finalizar(module);
                tabela.retain(|(s, _)| s != "g:index");
                tabela.push(("g:index".to_string(), simbolo));
            }
            // Um enum sem override recebe `Enum.nome` do emissor. A tabela
            // por seletor deve usar esse mesmo corpo, inclusive quando o
            // valor é impresso por `List.toString` da fonte.
            if super::enums::e_enum(ctx, cid)
                && module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_none())
                && let Some(decl) = classe.decl
            {
                let base = format!(
                    "df.{}.{}.toString",
                    crate::context::escapar(&ctx.nome_da_biblioteca(classe.library)),
                    crate::context::escapar(ctx.symbol_name(classe.name))
                );
                let simbolo = format!("{base}$c");
                let mut b = FnBuilder::new(ctx, decl.unit, simbolo.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param("args".to_string(), Type::Ptr);
                b.add_param("desc".to_string(), Type::Ptr);
                let texto = b.emit_call_with_check(
                    Instruction::CallStatic { symbol: base, args: vec![this], ret_ty: Type::Ref },
                    Type::Ref,
                );
                b.terminate(Terminator::Return(Some(texto)));
                b.finalizar(module);
                tabela.retain(|(s, _)| s != "c:toString");
                tabela.push(("c:toString".to_string(), simbolo));
            }
            // Encaminhadores de `noSuchMethod`: a classe com `noSuchMethod`
            // próprio responde a todo membro das suas interfaces que não
            // implementa (a VM gera o encaminhador no kernel).
            if let Some(decl) = classe.decl
                && tem_no_such_method_proprio(ctx, cid)
            {
                for (seletor, tipo, fid) in membros_para_encaminhar(ctx, cid, &tabela) {
                    let simbolo = format!(
                        "df.{}.{}.$nsm.{}",
                        crate::context::escapar(&ctx.nome_da_biblioteca(classe.library)),
                        crate::context::escapar(ctx.symbol_name(classe.name)),
                        crate::context::escapar(&seletor)
                    );
                    gerar_encaminhador_nsm(ctx, module, decl.unit, &simbolo, fid, tipo);
                    tabela.push((seletor, simbolo));
                }
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
        module.parametros_escalares_dart.extend(m.parametros_escalares_dart);
        module.globais.extend(m.globais);
        return;
    }
    let motivo = m
        .erros
        .first()
        .map(|e| e.strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(e).to_string())
        .unwrap_or_else(|| "pânico do lowering".to_string());
    let motivo = motivo.rsplit_once(" (").map_or(motivo.as_str(), |(a, _)| a).to_string();
    // Cada entrada uniforme construída e a tipada correspondente
    // (`$tc`/`$ts`, `entrada_tipada.rs`), que a tabela pode citar mesmo sem
    // ter sido construída antes do erro.
    let mut entradas: Vec<(String, String)> = Vec::new();
    for f in &m.functions {
        let s = &f.symbol;
        if !(s.ends_with("$c") || s.ends_with("$g") || s.ends_with("$s") || s.ends_with("$tc") || s.ends_with("$ts")) {
            continue;
        }
        let tipada = (s.ends_with("$c") || s.ends_with("$s")) && !s.ends_with("$tc") && !s.ends_with("$ts");
        for simbolo in [Some(s.clone()), tipada.then(|| format!("{}$t{}", &s[..s.len() - 2], &s[s.len() - 1..]))].into_iter().flatten() {
            if !entradas.iter().any(|(x, _)| *x == simbolo) {
                entradas.push((simbolo, f.name.clone()));
            }
        }
    }
    for (simbolo, nome) in entradas {
        let unit = ctx.program.units.iter().position(|_| true).map(|u| dartforge_elements::model::UnitId(u as u32));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), nome, Type::Ref);
        b.add_param("this".to_string(), Type::Ref);
        b.add_param("args".to_string(), Type::Ptr);
        b.add_param("desc".to_string(), Type::Ptr);
        let t = b.emit(Instruction::Const(Constant::String(format!("{simbolo} ({motivo})"))), Type::Ref);
        b.emit(
            Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
            Type::Void,
        );
        b.terminate(Terminator::Unreachable);
        module.recusados.push((simbolo, motivo.clone()));
        module.functions.push(b.func);
    }
}
/// O getter de um campo `late` com inicializador do SDK da fonte, ou a
/// recusa dele: `construir` baixa o getter num módulo à parte; se ele tem
/// diagnóstico (ou o lowering entra em pânico), fica no lugar a função que
/// avisa em tempo de execução.
pub fn lower_getter_late_ou_recusa(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    construir: impl Fn(&mut Module),
) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.modo_sdk = true;
        construir(&mut m);
        if m.erros.is_empty() {
            let problemas = super::verificador::verificar(&m);
            m.erros.extend(problemas);
        }
        m
    }));
    let simbolo = super::simbolo_getter_campo_late(ctx, vid);
    let motivo = match r {
        Ok(m) if m.erros.is_empty() => {
            module.functions.extend(m.functions);
        module.parametros_escalares_dart.extend(m.parametros_escalares_dart);
            module.globais.extend(m.globais);
            return;
        }
        Ok(m) => m.erros[0].strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(&m.erros[0]).to_string(),
        Err(_) => "pânico do lowering".to_string(),
    };
    let motivo = motivo.rsplit_once(" (").map_or(motivo.as_str(), |(a, _)| a).to_string();
    let nome = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let mut b = FnBuilder::new(ctx, unit, simbolo.clone(), nome, Type::Ref);
    b.add_param("this".to_string(), Type::Ref);
    let t = b.emit(Instruction::Const(Constant::String(format!("{simbolo} ({motivo})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    module.functions.push(b.func);
    module.recusados.push((simbolo, motivo));
}

/// O getter preguiçoso de um global do SDK da fonte (ou a recusa dele) e o
/// setter `<getter>$set`, que outro módulo chama para gravar o global.
/// O global do programa fora do mundo fechado (C7, `mundo_nativo.rs`): o
/// getter lança (`FnBuilder::corpo_podado`); o setter é o de sempre.
pub fn lower_global_podado(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let nome = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::simbolo_global(ctx, vid);
    let mut b = FnBuilder::new(ctx, unit, getter.clone(), nome.clone(), repr);
    b.corpo_podado();
    b.finalizar(module);
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), nome, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    s.gravar_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finalizar(module);
}

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
        module.parametros_escalares_dart.extend(m.parametros_escalares_dart);
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
        self.emitir_ponto_seguro();
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
        // Elemento `dynamic` num alvo tipado: o cast implícito de cada
        // elemento (o CFE o põe na atribuição à variável do laço), com a
        // mensagem do cast, antes de converter à representação.
        let elemento_dinamico = match self.ctx.get_type(self.unit_id, iterable) {
            Some(t) if t == self.ctx.core.dynamic_ => true,
            Some(t) => self.tipo_elemento(t) == Some(self.ctx.core.dynamic_),
            None => false,
        };
        match target {
            ForInTarget::Declared { name, ty: anotado, .. } => {
                if elemento_dinamico && let Some(tid) = anotado {
                    self.checar_tipo_ou_lancar(ast.ty(*tid), x.clone(), super::rti::ContextoDoCast::Implicito);
                    if self.is_terminated() {
                        return;
                    }
                }
                let ty = self.repr_do_local(name.span.start as usize);
                let x = self.coagir(x, ty);
                self.declarar_variavel(name.sym, name.span.start as usize, ty, x);
            }
            ForInTarget::Expression(e) => {
                if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                    if elemento_dinamico
                        && let Some(t) = self.ctx.get_type(self.unit_id, *e)
                        && t != self.ctx.core.dynamic_
                        && t != self.ctx.core.object_nullable
                    {
                        let rti = self.rti_de_tipo(t);
                        self.cast_rti_em(x.clone(), rti, super::rti::ContextoDoCast::Implicito);
                        if self.is_terminated() {
                            return;
                        }
                    }
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
        // Os rótulos deste laço (os dois caminhos abaixo os usam; um laço
        // aninhado no corpo não pode vê-los como seus).
        let rotulos = std::mem::take(&mut self.pending_labels);
        // `List<E>`: a lista do runtime percorrida pelo índice, com a
        // conferência de comprimento do `ListIterator`; outra classe que
        // implementa `List` segue pelo `Iterator` dela (abaixo).
        let mut fim_direto = None;
        if let Some(super::tipados::Indexavel::Nucleo { gravacao }) = self.indexavel(self.ctx.get_type(self.unit_id, iterable)) {
            let fonte = self.coagir(fonte.clone(), Type::Ref);
            if let Some(fim) = self.for_in_de_lista(ast, target, iterable, body, span, fonte, gravacao.unwrap_or(Type::Ref), &rotulos) {
                fim_direto = Some(fim);
            }
        }
        let it = self.chamar_por_seletor(fonte, "g:iterator".to_string(), &[]);
        let cabeca = self.new_block();
        let corpo = self.new_block();
        let fim = self.new_block();
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
        self.emitir_ponto_seguro();
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
        // A saída do laço pelo `Iterator` encontra a do laço direto.
        if let Some(depois) = fim_direto {
            self.terminate(Terminator::Branch(depois));
            self.set_block(depois);
        }
    }

    /// O `for-in` sobre uma lista do runtime pelo índice. Deixa o bloco
    /// corrente no caminho do `Iterator` (lista de outra classe) e devolve o
    /// bloco depois do laço, aonde os dois caminhos chegam.
    ///
    /// É o que o `ListIterator` (e o `_FixedSizeArrayIterator`, que nunca vê
    /// o comprimento mudar) faz: a cada volta, comprimento diferente do
    /// inicial lança `ConcurrentModificationError(lista)`; índice no fim
    /// termina; senão o elemento `lista[i]`.
    #[allow(clippy::too_many_arguments)]
    fn for_in_de_lista(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
        lista: Operand,
        repr: Type,
        rotulos: &[dartforge_intern::SymbolId],
    ) -> Option<BlockId> {
        let classe = self.ctx.classe_do_sdk("core", "ConcurrentModificationError")?;
        let vazio = self.ctx.interner.lookup("")?;
        let ctor = *self.ctx.program.classes[classe.0 as usize].constructors.get(&vazio)?;
        // Lista do núcleo pela classe (`@df.classe`, cids 8–10 fixos,
        // docs/NATIVO-ESPACO-UNIFICADO.md §2.4): `_List`, `_ImmutableList` ou
        // `_GrowableList`; o comprimento em linha (`@df.lista_len`).
        let cid = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(lista.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let desde = self.emit(
            Instruction::Sub(cid, Operand::Constant(Constant::Int(i64::from(dartforge_runtime::layout::cid::LIST)))),
            Type::I64,
        );
        let e_lista = self.emit(Instruction::ICmp(ICmpOp::Ult, desde, Operand::Constant(Constant::Int(3))), Type::I1);
        let direto = self.new_block();
        let iterador = self.new_block();
        let depois = self.new_block();
        self.terminate(Terminator::CondBranch { cond: e_lista, then_block: direto, else_block: iterador });

        self.set_block(direto);
        let n0 = self.lista_len_em_linha(lista.clone());
        let indice = self.emit(Instruction::Alloca(Type::I64), Type::Ptr);
        self.emit(Instruction::Store { ptr: indice.clone(), val: Operand::Constant(Constant::Int(0)) }, Type::Void);
        let cabeca = self.new_block();
        let mudou = self.new_block();
        let confere = self.new_block();
        let corpo = self.new_block();
        self.terminate(Terminator::Branch(cabeca));

        self.set_block(cabeca);
        // Dentro do laço, o comprimento lido do bloco a cada volta
        // (`@df.lista_len`, o mesmo deslocamento nas três classes).
        let n = self.lista_len_em_linha(lista.clone());
        let igual = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), n0), Type::I1);
        self.terminate(Terminator::CondBranch { cond: igual, then_block: confere, else_block: mudou });

        self.set_block(mudou);
        let erro = self.instanciar_avaliados(ctor, &[(None, lista.clone())], span);
        if !self.is_terminated() {
            self.emit_throw_op(erro);
        }
        if !self.is_terminated() {
            self.terminate(Terminator::Unreachable);
        }

        self.set_block(confere);
        let i = self.emit(Instruction::Load { ptr: indice.clone(), ty: Type::I64 }, Type::I64);
        let dentro = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch { cond: dentro, then_block: corpo, else_block: depois });

        self.set_block(corpo);
        self.emitir_ponto_seguro();
        let x = self.ler_elemento_da_lista(&lista, &i, repr);
        let prox = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: indice, val: prox }, Type::Void);
        for &r in rotulos {
            self.labeled_break_targets.insert(r, depois);
            self.labeled_continue_targets.insert(r, cabeca);
        }
        self.break_targets.push(depois);
        self.continue_targets.push(cabeca);
        self.abrir_escopo();
        self.ligar_alvo_de_for_in(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.fechar_escopo();
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(cabeca));
        }
        self.break_targets.pop();
        self.continue_targets.pop();
        for r in rotulos {
            self.labeled_break_targets.remove(r);
            self.labeled_continue_targets.remove(r);
        }
        self.set_block(iterador);
        Some(depois)
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

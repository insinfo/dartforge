//! `super` (P4): `super.m(…)`, `super.x`, `super.x = v` e `super op e`.
//!
//! A especificação (§17.21.3 "Super Invocation") procura o membro a partir
//! da superclasse da classe **lexicamente** envolvente, sem despacho pela
//! classe dinâmica: a chamada é direta. Quando a busca chega ao SDK
//! (`Object`), os membros de `Object` são os do runtime: `toString()` é o
//! `Instance of 'C'` e `==` é a identidade.

use super::fn_builder::FnBuilder;
use super::membros::{Avaliado, tem_corpo};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionKind, VariableId};
use dartforge_intern::SymbolId;

/// O que `super.nome` encontrou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembroSuper {
    Funcao(usize),
    Campo(VariableId),
    /// A busca chegou ao SDK.
    Sdk,
    /// `super` dentro de um `mixin`: o alvo depende da aplicação.
    DeMixin,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Procura `nome` (ou o setter `nome=`) a partir da superclasse da
    /// classe envolvente.
    pub fn membro_de_super(&self, nome: SymbolId, setter: bool) -> MembroSuper {
        let Some(cid) = self.enclosing_class else {
            return MembroSuper::Sdk;
        };
        let texto = self.ctx.symbol_name(nome).to_string();
        let chave = if setter {
            self.ctx.interner.lookup(&format!("{texto}="))
        } else {
            Some(nome)
        };
        // A superclasse de `C` na linearização é o que vem depois dele (os
        // mixins aplicados e depois a superclasse); num `mixin` o `super`
        // depende da classe que o aplica (`MembroSuper::DeMixin`).
        if super::membros::e_mixin(self.ctx, cid) {
            return MembroSuper::DeMixin;
        }
        self.procurar_depois(cid, cid, nome, chave)
    }

    /// Procura `nome` (`chave`: o nome ou o do setter) na linearização de
    /// `classe`, depois de `apos`.
    fn procurar_depois(
        &self,
        classe: dartforge_elements::model::ClassId,
        apos: dartforge_elements::model::ClassId,
        nome: SymbolId,
        chave: Option<SymbolId>,
    ) -> MembroSuper {
        let lin = super::membros::linearizacao(self.ctx, classe);
        let Some(pos) = lin.iter().position(|c| *c == apos) else {
            return MembroSuper::Sdk;
        };
        for &c in lin.iter().skip(pos + 1) {
            let cl = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = chave.and_then(|k| cl.instance_members.get(&k)) {
                let f = f.0 as usize;
                if let Some(v) = self.ctx.program.functions[f].variable {
                    return MembroSuper::Campo(v);
                }
                if tem_corpo(self.ctx, f) {
                    return MembroSuper::Funcao(f);
                }
            }
            if let Some(&v) = cl.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == nome
            }) {
                return MembroSuper::Campo(v);
            }
        }
        MembroSuper::Sdk
    }


    /// `super.nome` dentro de um `mixin` M: o alvo é o que vem depois de M na
    /// linearização da classe **dinâmica** de `this` (especificação §12.3:
    /// cada aplicação de M tem a sua superclasse). O código do mixin é
    /// compilado uma vez, então o alvo é escolhido pela classe (o mesmo
    /// mecanismo dos campos de mixin): um alvo por classe concreta que
    /// aplica M.
    fn alvos_super_de_mixin(&self, nome: SymbolId, setter: bool) -> Vec<(i64, MembroSuper)> {
        let Some(mixin) = self.enclosing_class else {
            return Vec::new();
        };
        let texto = self.ctx.symbol_name(nome).to_string();
        let chave = if setter {
            self.ctx.interner.lookup(&format!("{texto}="))
        } else {
            Some(nome)
        };
        let mut saida = Vec::new();
        for (k, classe) in self.ctx.program.classes.iter().enumerate() {
            let kid = dartforge_elements::model::ClassId(k as u32);
            // As classes do SDK da fonte também aplicam mixins com `super`
            // (`_ConstMap` com `_ImmutableLinkedHashMapMixin`).
            if !self.ctx.biblioteca_compilada(classe.library)
                || classe.modifiers.abstract_
                || super::membros::e_mixin(self.ctx, kid)
            {
                continue;
            }
            let Some(id) = self.ctx.id_de_classe(kid) else { continue };
            if !super::membros::linearizacao(self.ctx, kid).contains(&mixin) {
                continue;
            }
            saida.push((i64::from(id), self.procurar_depois(kid, mixin, nome, chave)));
        }
        saida
    }

    fn this_para_super(&mut self, span: Span) -> Option<Operand> {
        let t = self.this_param.clone();
        if t.is_none() {
            self.nao_suportado("`super` fora de membro de instância", span);
        }
        t
    }

    /// Faz `uso` com o alvo de `super.nome`: direto, ou — dentro de um
    /// mixin — pela classe dinâmica de `this` (`switch`), com o resultado
    /// `Ref` numa junção.
    fn com_alvo_super(
        &mut self,
        this: Operand,
        nome: SymbolId,
        setter: bool,
        span: Span,
        uso: &mut dyn FnMut(&mut Self, MembroSuper, Operand) -> Operand,
    ) -> Operand {
        let alvo = self.membro_de_super(nome, setter);
        if alvo != MembroSuper::DeMixin {
            return uso(self, alvo, this);
        }
        let alvos = self.alvos_super_de_mixin(nome, setter);
        if alvos.is_empty() {
            return self.nao_suportado("`super` dentro de mixin sem classe que o aplique", span);
        }
        let this = self.coagir(this, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(this.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let mut distintos: Vec<MembroSuper> = Vec::new();
        for (_, a) in &alvos {
            if !distintos.contains(a) {
                distintos.push(*a);
            }
        }
        let blocos: Vec<(MembroSuper, BlockId)> = distintos.iter().map(|a| (*a, self.new_block())).collect();
        let bloco_de = |a: MembroSuper| blocos.iter().find(|(x, _)| *x == a).map(|(_, b)| *b).expect("bloco");
        let juncao = self.new_block();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocos[0].1,
            cases: alvos.iter().map(|(id, a)| (*id, bloco_de(*a))).collect(),
        });
        let mut entradas = Vec::new();
        for (a, b) in blocos.clone() {
            self.set_block(b);
            let r = uso(self, a, this.clone());
            let r = if matches!(self.operand_type(&r), Type::Void | Type::Ptr) {
                Operand::Constant(Constant::Null)
            } else {
                self.coagir(r, Type::Ref)
            };
            if !self.is_terminated() {
                entradas.push((self.current_block, r));
                self.terminate(Terminator::Branch(juncao));
            }
        }
        self.set_block(juncao);
        if entradas.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(Instruction::Phi { incoming: entradas, ty: Type::Ref }, Type::Ref)
    }

    /// `super.nome(args)`.
    pub fn chamar_super_metodo(&mut self, nome: SymbolId, avaliados: &[Avaliado], span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        self.com_alvo_super(this, nome, false, span, &mut |s, alvo, this| match alvo {
            MembroSuper::Funcao(f) => {
                if s.ctx.program.functions[f].kind == FunctionKind::Getter {
                    let v = s.chamar_direto(f, Some(this), Vec::new());
                    return s.chamar_valor_funcao(v, avaliados);
                }
                let args = s.casar_args(f, avaliados);
                s.chamar_direto(f, Some(this), args)
            }
            MembroSuper::Campo(v) => {
                let x = s.ler_campo_com_late_direto(this, v, span);
                s.chamar_valor_funcao(x, avaliados)
            }
            MembroSuper::DeMixin => s.nao_suportado("`super` dentro de mixin", span),
            MembroSuper::Sdk => match s.ctx.symbol_name(nome) {
                "toString" => s.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_to_string_handle".to_string(),
                        args: vec![(this, Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                "noSuchMethod" => s.lancar_nsm("noSuchMethod"),
                outro => {
                    let n = outro.to_string();
                    s.nao_suportado(&format!("`super.{n}` do SDK"), span)
                }
            },
        })
    }

    /// `super.nome` lido.
    pub fn ler_super(&mut self, nome: SymbolId, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        self.com_alvo_super(this, nome, false, span, &mut |s, alvo, this| match alvo {
            MembroSuper::Funcao(f) => {
                if s.ctx.program.functions[f].kind == FunctionKind::Getter {
                    s.chamar_direto(f, Some(this), Vec::new())
                } else {
                    // Tear-off de `super.m`: liga ao método da superclasse.
                    s.tearoff_de_metodo(this, f, span)
                }
            }
            MembroSuper::Campo(v) => s.ler_campo_com_late_direto(this, v, span),
            MembroSuper::DeMixin => s.nao_suportado("`super` dentro de mixin", span),
            MembroSuper::Sdk => {
                let n = s.ctx.symbol_name(nome).to_string();
                s.nao_suportado(&format!("`super.{n}` do SDK"), span)
            }
        })
    }

    /// `super.nome = v`.
    pub fn gravar_super(&mut self, nome: SymbolId, valor: Operand, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        let v2 = valor.clone();
        self.com_alvo_super(this, nome, true, span, &mut |s, alvo, this| match alvo {
            MembroSuper::Funcao(f) => {
                let args = s.casar_args(f, &[(None, v2.clone())]);
                s.chamar_direto(f, Some(this), args);
                v2.clone()
            }
            MembroSuper::Campo(v) => {
                s.gravar_campo(this, v, v2.clone(), span);
                v2.clone()
            }
            MembroSuper::DeMixin => s.nao_suportado("`super` dentro de mixin", span),
            MembroSuper::Sdk => {
                let n = s.ctx.symbol_name(nome).to_string();
                s.nao_suportado(&format!("`super.{n} =` do SDK"), span)
            }
        });
        valor
    }

    /// `super op e` (`super == o`, `super + x`).
    pub fn operador_super(&mut self, op_nome: &str, direita: Operand, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        let Some(sym) = self.ctx.interner.lookup(op_nome) else {
            return self.nao_suportado(&format!("`super {op_nome}`"), span);
        };
        let igualdade = op_nome == "==";
        let nome_op = op_nome.to_string();
        self.com_alvo_super(this, sym, false, span, &mut |s, alvo, this| match alvo {
            MembroSuper::Funcao(f) => {
                let args = s.casar_args(f, &[(None, direita.clone())]);
                s.chamar_direto(f, Some(this), args)
            }
            _ if igualdade => s.identicos(this, direita.clone()),
            _ => s.nao_suportado(&format!("`super {nome_op}`"), span),
        })
    }
}

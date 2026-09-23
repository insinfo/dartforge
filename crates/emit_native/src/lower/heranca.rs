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
pub enum MembroSuper {
    Funcao(usize),
    Campo(VariableId),
    /// A busca chegou ao SDK.
    Sdk,
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
        let mut atual = self.ctx.program.classes[cid.0 as usize].supertype_class;
        while let Some(c) = atual {
            let cl = &self.ctx.program.classes[c.0 as usize];
            if self.ctx.program.library(cl.library).is_sdk {
                return MembroSuper::Sdk;
            }
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
            atual = cl.supertype_class;
        }
        MembroSuper::Sdk
    }

    fn this_para_super(&mut self, span: Span) -> Option<Operand> {
        let t = self.this_param.clone();
        if t.is_none() {
            self.nao_suportado("`super` fora de membro de instância", span);
        }
        t
    }

    /// `super.nome(args)`.
    pub fn chamar_super_metodo(&mut self, nome: SymbolId, avaliados: &[Avaliado], span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        match self.membro_de_super(nome, false) {
            MembroSuper::Funcao(f) => {
                if self.ctx.program.functions[f].kind == FunctionKind::Getter {
                    let v = self.chamar_direto(f, Some(this), Vec::new());
                    return self.chamar_valor_funcao(v, avaliados);
                }
                let args = self.casar_args(f, avaliados);
                self.chamar_direto(f, Some(this), args)
            }
            MembroSuper::Campo(v) => {
                let x = self.ler_campo_com_late(this, v, span);
                self.chamar_valor_funcao(x, avaliados)
            }
            MembroSuper::Sdk => match self.ctx.symbol_name(nome) {
                "toString" => self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_to_string_handle".to_string(),
                        args: vec![(this, Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                "noSuchMethod" => self.lancar_nsm("noSuchMethod"),
                outro => {
                    let n = outro.to_string();
                    self.nao_suportado(&format!("`super.{n}` do SDK"), span)
                }
            },
        }
    }

    /// `super.nome` lido.
    pub fn ler_super(&mut self, nome: SymbolId, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        match self.membro_de_super(nome, false) {
            MembroSuper::Funcao(f) => {
                if self.ctx.program.functions[f].kind == FunctionKind::Getter {
                    self.chamar_direto(f, Some(this), Vec::new())
                } else {
                    // Tear-off de `super.m`: liga ao método da superclasse.
                    self.tearoff_de_metodo(this, f, span)
                }
            }
            MembroSuper::Campo(v) => self.ler_campo_com_late(this, v, span),
            MembroSuper::Sdk => {
                let n = self.ctx.symbol_name(nome).to_string();
                self.nao_suportado(&format!("`super.{n}` do SDK"), span)
            }
        }
    }

    /// `super.nome = v`.
    pub fn gravar_super(&mut self, nome: SymbolId, valor: Operand, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        match self.membro_de_super(nome, true) {
            MembroSuper::Funcao(f) => {
                let args = self.casar_args(f, &[(None, valor.clone())]);
                self.chamar_direto(f, Some(this), args);
                valor
            }
            MembroSuper::Campo(v) => {
                self.gravar_campo(this, v, valor.clone(), span);
                valor
            }
            MembroSuper::Sdk => {
                let n = self.ctx.symbol_name(nome).to_string();
                self.nao_suportado(&format!("`super.{n} =` do SDK"), span)
            }
        }
    }

    /// `super op e` (`super == o`, `super + x`).
    pub fn operador_super(&mut self, op_nome: &str, direita: Operand, span: Span) -> Operand {
        let Some(this) = self.this_para_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        let Some(sym) = self.ctx.interner.lookup(op_nome) else {
            return self.nao_suportado(&format!("`super {op_nome}`"), span);
        };
        match self.membro_de_super(sym, false) {
            MembroSuper::Funcao(f) => {
                let args = self.casar_args(f, &[(None, direita)]);
                self.chamar_direto(f, Some(this), args)
            }
            _ if op_nome == "==" => self.identicos(this, direita),
            _ => self.nao_suportado(&format!("`super {op_nome}`"), span),
        }
    }
}

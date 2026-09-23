//! Despacho pelo nome do membro quando o receptor não tem tipo estático
//! útil (`dynamic`, `Object`, `num`, o corpo de uma closure).
//!
//! A especificação (§17.21.1 "Ordinary Invocation", `dynamic`) manda procurar
//! o membro na classe **dinâmica** do receptor. O programa é conhecido
//! inteiro (mundo fechado): as classes do programa que têm um membro com esse
//! nome são enumeradas, e um `switch` sobre a classe do receptor escolhe a
//! implementação; o que não é objeto do programa (caixas, strings, coleções
//! do runtime) vai para o caminho `padrao`, que o chamador fornece — o
//! operador do runtime, ou o membro do SDK casado pelo nome.
//!
//! Os argumentos são avaliados **dentro** de cada ramo (só um ramo executa,
//! então cada argumento é avaliado uma vez, depois do receptor — a ordem da
//! especificação): o caminho do SDK por nome avalia os seus.

use super::fn_builder::FnBuilder;
use super::membros::{Avaliado, tem_corpo};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, VariableId};
use dartforge_frontend::ast::{BinaryOp, UnaryOp};

/// O texto do operador (o nome do membro, `operator +`) e o código dele em
/// `dartforge_dyn_op`.
pub fn operador(op: BinaryOp) -> Option<(&'static str, i64)> {
    Some(match op {
        BinaryOp::Add => ("+", 0),
        BinaryOp::Sub => ("-", 1),
        BinaryOp::Mul => ("*", 2),
        BinaryOp::Div => ("/", 3),
        BinaryOp::TruncDiv => ("~/", 4),
        BinaryOp::Rem => ("%", 5),
        BinaryOp::Lt => ("<", 6),
        BinaryOp::LtEq => ("<=", 7),
        BinaryOp::Gt => (">", 8),
        BinaryOp::GtEq => (">=", 9),
        BinaryOp::BitAnd => ("&", 10),
        BinaryOp::BitOr => ("|", 11),
        BinaryOp::BitXor => ("^", 12),
        BinaryOp::Shl => ("<<", 13),
        BinaryOp::Shr => (">>", 14),
        BinaryOp::UShr => (">>>", 15),
        _ => return None,
    })
}

/// O que um nome é numa classe concreta do programa.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Alvo {
    /// Método, operador, getter ou setter com corpo.
    Funcao(usize),
    /// Campo de instância (o getter/setter implícito).
    Campo(VariableId),
    /// Campo implícito de um enum (`index` = 0, `name` = 1).
    Slot(usize),
}

/// O que o despacho faz com o alvo escolhido.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Uso {
    /// Ler (`o.x`): getter chamado, campo lido; método vira tear-off.
    Ler,
    /// Chamar (`o.m(…)`): método chamado; campo/getter lido e o valor
    /// chamado como função.
    Chamar,
    /// Gravar (`o.x = v`): o valor é o único argumento; campo gravado,
    /// setter chamado. O resultado é o valor.
    Gravar,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Para cada classe concreta do programa, o membro de instância `nome`
    /// (subindo a cadeia de superclasses do programa), com o id de classe do
    /// runtime.
    pub fn alvos_por_nome(&self, nome: &str) -> Vec<(i64, Alvo)> {
        let Some(sym) = self.ctx.interner.lookup(nome) else {
            return Vec::new();
        };
        let mut saida = Vec::new();
        for (k, classe) in self.ctx.program.classes.iter().enumerate() {
            if self.ctx.program.library(classe.library).is_sdk
                || classe.modifiers.abstract_
                || super::membros::e_mixin(self.ctx, ClassId(k as u32))
            {
                continue;
            }
            let Some(id) = self.id_de_classe(ClassId(k as u32)) else {
                continue;
            };
            if super::enums::e_enum(self.ctx, ClassId(k as u32)) && (nome == "index" || nome == "name") {
                saida.push((id, Alvo::Slot(usize::from(nome == "name"))));
                continue;
            }
            for c in crate::lower::membros::linearizacao(self.ctx, ClassId(k as u32)) {
                let cl = &self.ctx.program.classes[c.0 as usize];
                if self.ctx.program.library(cl.library).is_sdk {
                    break;
                }
                if let Some(&f) = cl.instance_members.get(&sym) {
                    let f = f.0 as usize;
                    let func = &self.ctx.program.functions[f];
                    if let Some(v) = func.variable {
                        saida.push((id, Alvo::Campo(v)));
                        break;
                    }
                    if tem_corpo(self.ctx, f) {
                        saida.push((id, Alvo::Funcao(f)));
                        break;
                    }
                }
                if let Some(&v) = cl.fields.iter().find(|&&v| {
                    let var = &self.ctx.program.variables[v.0 as usize];
                    !var.static_ && var.name == sym
                }) {
                    saida.push((id, Alvo::Campo(v)));
                    break;
                }
            }
        }
        saida
    }

    /// Usa o alvo escolhido pela classe de `recv`; os demais receptores vão
    /// para `padrao`. `args` avalia os argumentos (dentro do ramo). O
    /// resultado é `Ref`.
    #[allow(clippy::too_many_arguments)]
    pub fn despachar(
        &mut self,
        recv: Operand,
        alvos: &[(i64, Alvo)],
        uso: Uso,
        args: &mut dyn FnMut(&mut Self) -> Vec<Avaliado>,
        padrao: &mut dyn FnMut(&mut Self) -> Operand,
        span: Span,
    ) -> Operand {
        let recv = self.coagir(recv, Type::Ref);
        if alvos.is_empty() {
            let r = padrao(self);
            return self.coagir_ou_nulo(r);
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let mut distintos: Vec<Alvo> = alvos.iter().map(|(_, a)| *a).collect();
        distintos.sort_unstable();
        distintos.dedup();
        let blocos: Vec<(Alvo, BlockId)> = distintos.iter().map(|&a| (a, self.new_block())).collect();
        let b_padrao = self.new_block();
        let juncao = self.new_block();
        let cases = alvos
            .iter()
            .map(|(id, a)| (*id, blocos.iter().find(|(g, _)| g == a).expect("bloco").1))
            .collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_padrao,
            cases,
        });
        let mut entradas = Vec::new();
        for (alvo, b) in blocos {
            self.set_block(b);
            let r = match (alvo, uso) {
                (Alvo::Campo(v), Uso::Gravar) => {
                    let av = args(self);
                    let x = av.first().map(|(_, x)| x.clone()).unwrap_or(Operand::Constant(Constant::Null));
                    self.gravar_campo(recv.clone(), v, x.clone(), span);
                    x
                }
                (Alvo::Funcao(f), Uso::Gravar) => {
                    let av = args(self);
                    let x = av.first().map(|(_, x)| x.clone()).unwrap_or(Operand::Constant(Constant::Null));
                    let a = self.casar_args(f, &av);
                    self.chamar_direto(f, Some(recv.clone()), a);
                    x
                }
                (Alvo::Slot(_), Uso::Gravar) => self.lancar_nsm("setter de enum"),
                (Alvo::Slot(i), _) => {
                    let ty = if i == 0 { Type::I64 } else { Type::Ref };
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_object_get".to_string(),
                            args: vec![
                                (recv.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(i as i64)), Type::I64),
                            ],
                            ret_ty: ty,
                        },
                        ty,
                    )
                }
                (Alvo::Campo(v), Uso::Ler) => self.ler_campo_com_late(recv.clone(), v, span),
                (Alvo::Campo(v), Uso::Chamar) => {
                    let f = self.ler_campo_com_late(recv.clone(), v, span);
                    let av = args(self);
                    self.chamar_valor_funcao(f, &av)
                }
                (Alvo::Funcao(f), Uso::Ler) => {
                    if self.ctx.program.functions[f].kind == FunctionKind::Getter {
                        self.chamar_direto(f, Some(recv.clone()), Vec::new())
                    } else {
                        self.tearoff_de_metodo(recv.clone(), f, span)
                    }
                }
                (Alvo::Funcao(f), Uso::Chamar) => {
                    if self.ctx.program.functions[f].kind == FunctionKind::Getter {
                        let v = self.chamar_direto(f, Some(recv.clone()), Vec::new());
                        let av = args(self);
                        self.chamar_valor_funcao(v, &av)
                    } else {
                        let av = args(self);
                        let a = self.casar_args(f, &av);
                        self.chamar_direto(f, Some(recv.clone()), a)
                    }
                }
            };
            let r = self.coagir_ou_nulo(r);
            if !self.is_terminated() {
                entradas.push((self.current_block, r));
                self.terminate(Terminator::Branch(juncao));
            }
        }
        self.set_block(b_padrao);
        let r = padrao(self);
        let r = self.coagir_ou_nulo(r);
        if !self.is_terminated() {
            entradas.push((self.current_block, r));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        if entradas.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// Para cada classe concreta do programa, o que grava `nome`: o setter
    /// `nome=` ou o campo `nome` (subindo a cadeia).
    pub fn alvos_de_escrita(&self, nome: &str) -> Vec<(i64, Alvo)> {
        let campo = self.ctx.interner.lookup(nome);
        let setter = self.ctx.interner.lookup(&format!("{nome}="));
        let mut saida = Vec::new();
        for (k, classe) in self.ctx.program.classes.iter().enumerate() {
            if self.ctx.program.library(classe.library).is_sdk
                || classe.modifiers.abstract_
                || super::membros::e_mixin(self.ctx, ClassId(k as u32))
            {
                continue;
            }
            let Some(id) = self.id_de_classe(ClassId(k as u32)) else {
                continue;
            };
            for c in crate::lower::membros::linearizacao(self.ctx, ClassId(k as u32)) {
                let cl = &self.ctx.program.classes[c.0 as usize];
                if self.ctx.program.library(cl.library).is_sdk {
                    break;
                }
                if let Some(&f) = setter.and_then(|s| cl.instance_members.get(&s)) {
                    let f = f.0 as usize;
                    if let Some(v) = self.ctx.program.functions[f].variable {
                        saida.push((id, Alvo::Campo(v)));
                        break;
                    }
                    if tem_corpo(self.ctx, f) {
                        saida.push((id, Alvo::Funcao(f)));
                        break;
                    }
                }
                if let Some(&v) = cl.fields.iter().find(|&&v| {
                    let var = &self.ctx.program.variables[v.0 as usize];
                    !var.static_ && Some(var.name) == campo
                }) {
                    saida.push((id, Alvo::Campo(v)));
                    break;
                }
            }
        }
        saida
    }

    /// `Ref` do valor; `void` (chamada sem valor) é null.
    fn coagir_ou_nulo(&mut self, r: Operand) -> Operand {
        if matches!(self.operand_type(&r), Type::Void | Type::Ptr) {
            return Operand::Constant(Constant::Null);
        }
        self.coagir(r, Type::Ref)
    }

    /// `NoSuchMethodError` em tempo de execução (o receptor não tem o
    /// membro): o caminho padrão do despacho quando nenhum membro do SDK
    /// casado pelo nome serve.
    pub fn lancar_nsm(&mut self, nome: &str) -> Operand {
        let n = self.emit(Instruction::Const(Constant::String(nome.to_string())), Type::Ref);
        let e = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_no_such_method_error_new".to_string(),
                args: vec![(n, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.emit_throw_op(e);
        Operand::Constant(Constant::Null)
    }

    /// `a op b` com algum lado sem tipo numérico estático: o `operator op`
    /// da classe do programa, ou o operador do runtime (`num`, `String`).
    pub fn operar_dinamico(&mut self, op: BinaryOp, a: Operand, b: Operand, span: Span) -> Operand {
        let Some((nome, codigo)) = operador(op) else {
            return self.nao_suportado("operador sobre num/dynamic/objeto", span);
        };
        let a = self.coagir(a, Type::Ref);
        let b = self.coagir(b, Type::Ref);
        let alvos: Vec<(i64, Alvo)> = self
            .alvos_por_nome(nome)
            .into_iter()
            .filter(|(_, x)| matches!(x, Alvo::Funcao(_)))
            .collect();
        let (a2, b2, b3) = (a.clone(), b.clone(), b.clone());
        let r = self.despachar(
            a,
            &alvos,
            Uso::Chamar,
            &mut |_s: &mut Self| vec![(None, b3.clone())],
            &mut |s: &mut Self| {
                s.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_dyn_op".to_string(),
                        args: vec![
                            (Operand::Constant(Constant::Int(codigo)), Type::I64),
                            (a2.clone(), Type::Ref),
                            (b2.clone(), Type::Ref),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            },
            span,
        );
        // Comparação: o contexto quer o `bool`.
        if matches!(op, BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq) {
            return self.coagir(r, Type::I1);
        }
        r
    }

    /// `-a`/`~a` sobre uma referência.
    pub fn unario_dinamico(&mut self, op: UnaryOp, a: Operand) -> Operand {
        let (nome, codigo) = if op == UnaryOp::Neg { ("unary-", 16) } else { ("~", 17) };
        let a = self.coagir(a, Type::Ref);
        let alvos: Vec<(i64, Alvo)> = self
            .alvos_por_nome(nome)
            .into_iter()
            .filter(|(_, x)| matches!(x, Alvo::Funcao(_)))
            .collect();
        let a2 = a.clone();
        self.despachar(
            a,
            &alvos,
            Uso::Chamar,
            &mut |_s: &mut Self| Vec::new(),
            &mut |s: &mut Self| {
                s.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_dyn_unario".to_string(),
                        args: vec![
                            (Operand::Constant(Constant::Int(codigo)), Type::I64),
                            (a2.clone(), Type::Ref),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            },
            Span { start: 0, end: 0 },
        )
    }

    /// O receptor não tem tipo estático útil para achar o membro: sem tipo
    /// (corpo de closure), `dynamic`, `Object`/`Object?`.
    pub fn receptor_dinamico(&self, e: dartforge_frontend::ast::ExprId) -> bool {
        let Some(t) = self.ctx.get_type(self.unit_id, e) else {
            return true;
        };
        t == self.ctx.core.dynamic_ || t == self.ctx.core.object || t == self.ctx.core.object_nullable
    }
}
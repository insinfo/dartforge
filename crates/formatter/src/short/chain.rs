//! Porte de `short/call_chain_visitor.dart`: achata uma cadeia de seletores
//! (`a.b(1)[2].c()!`) numa lista da esquerda para a direita e escreve as
//! quebras antes de cada `.`, com o tratamento das propriedades iniciais,
//! das chamadas com argumentos em bloco e da chamada pendurada final.

use super::arena::{RegraId, custo};
use super::arguments::ArgumentListVisitor;
use super::visitor::Visitante;
use crate::syntax::tree::{No, NoId};

/// Tipo de seletor (`_MethodSelector`, `_PrefixedSelector`,
/// `_PropertySelector`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoSeletor {
    Metodo,
    Prefixado,
    Propriedade,
}

/// `_Selector`: o nó do seletor e os pós-fixos (`[i]`, `!`, `()`) presos a ele.
#[derive(Debug, Clone)]
struct Seletor {
    tipo: TipoSeletor,
    no: NoId,
    posfixos: Vec<NoId>,
}

impl Seletor {
    fn e_propriedade(&self) -> bool {
        self.tipo != TipoSeletor::Metodo
    }
}

/// `CallChainVisitor`.
pub struct CallChainVisitor {
    alvo: NoId,
    propriedades: Vec<Seletor>,
    chamadas: Vec<Seletor>,
    chamadas_bloco: Option<Vec<Seletor>>,
    pendurada: Option<Seletor>,
    regra_ativa: bool,
    span_terminado: bool,
    regra_propriedades: Option<RegraId>,
}

impl CallChainVisitor {
    /// `CallChainVisitor(visitor, node)`.
    pub fn novo(v: &Visitante, no: NoId) -> CallChainVisitor {
        let mut chamadas: Vec<Seletor> = Vec::new();
        let alvo = unwrap_target(v, no, &mut chamadas);
        let mut propriedades = Vec::new();
        if matches!(v.no(unwrap_null_assertion(v, alvo)), No::SimpleIdentifier { .. }) {
            propriedades = chamadas.iter().take_while(|c| c.e_propriedade()).cloned().collect();
        }
        chamadas.drain(..propriedades.len());

        let mut chamadas_bloco: Option<Vec<Seletor>> = None;
        let mut pendurada: Option<Seletor> = None;
        let mut em_bloco = false;
        let n = chamadas.len();
        for (i, c) in chamadas.iter().enumerate() {
            if is_block_call(v, c) {
                em_bloco = true;
                chamadas_bloco.get_or_insert_with(Vec::new).push(c.clone());
            } else if em_bloco {
                if i == n - 1 {
                    pendurada = Some(c.clone());
                    break;
                }
                chamadas_bloco = None;
                break;
            }
        }
        if let Some(bs) = &chamadas_bloco {
            for b in bs {
                if let Some(p) = chamadas.iter().position(|c| c.no == b.no) {
                    chamadas.remove(p);
                }
            }
        }
        if let Some(p) = &pendurada
            && let Some(i) = chamadas.iter().position(|c| c.no == p.no)
        {
            chamadas.remove(i);
        }
        CallChainVisitor {
            alvo,
            propriedades,
            chamadas,
            chamadas_bloco,
            pendurada,
            regra_ativa: false,
            span_terminado: false,
            regra_propriedades: None,
        }
    }

    /// `visit()`.
    pub fn visit(mut self, v: &mut Visitante) {
        v.nest_expression(None, false);
        v.start_span(custo::NORMAL);
        let quebra_no_alvo = forces_split(v, self.alvo);
        if quebra_no_alvo {
            if self.propriedades.len() > 1 {
                let r = v.a.regra_posicional(None, self.propriedades.len(), 0, 0);
                self.regra_propriedades = Some(r);
                v.start_lazy_rule(Some(r));
            } else {
                self.enable_rule(v, true);
            }
        }
        v.visit(Some(self.alvo));

        if self.propriedades.len() == 1 {
            v.solo_zero_split();
            let p = self.propriedades[0].clone();
            self.escrever(v, &p);
        } else if self.propriedades.len() > 1 {
            if !quebra_no_alvo {
                let r = v.a.regra_posicional(None, self.propriedades.len(), 0, 0);
                self.regra_propriedades = Some(r);
                v.start_rule(Some(r));
            }
            for p in self.propriedades.clone() {
                let c = v.zero_split();
                v.a.antes_do_argumento(self.regra_propriedades.expect("regra"), Some(c));
                self.escrever(v, &p);
            }
            v.end_rule();
        }

        if self.chamadas.len() > 1 {
            v.start_block_argument_nesting();
        }
        for c in self.chamadas.clone() {
            self.enable_rule(v, false);
            v.zero_split();
            self.escrever(v, &c);
        }
        if self.chamadas.len() > 1 {
            v.end_block_argument_nesting();
        }

        if let Some(bs) = self.chamadas_bloco.clone() {
            self.enable_rule(v, false);
            v.zero_split();
            self.disable_rule(v);
            for b in &bs {
                self.escrever(v, b);
            }
            if let Some(p) = self.pendurada.clone() {
                self.escrever(v, &p);
            }
        }

        self.disable_rule(v);
        self.end_span(v);
        v.unnest();
    }

    /// `_Selector.write(visitor)`.
    fn escrever(&mut self, v: &mut Visitante, s: &Seletor) {
        self.escrever_seletor(v, s);
        v.nest_expression(None, false);
        for &p in &s.posfixos {
            match v.no(p) {
                No::FunctionExpressionInvocation {
                    type_arguments,
                    argument_list,
                    ..
                } => {
                    v.solo_zero_split();
                    v.visit(*type_arguments);
                    v.visit_argument_list(*argument_list, true);
                }
                No::IndexExpression { .. } => v.finish_index_expression(p),
                No::PostfixExpression { operator, .. } => v.token(*operator),
                _ => {}
            }
        }
        v.unnest();
    }

    fn escrever_seletor(&mut self, v: &mut Visitante, s: &Seletor) {
        match v.no(s.no) {
            No::MethodInvocation {
                operator,
                method_name,
                type_arguments,
                argument_list,
                ..
            } => {
                v.token_opt(*operator);
                let nome = match v.no(*method_name) {
                    No::SimpleIdentifier { token } => *token,
                    _ => unreachable!(),
                };
                v.token(nome);
                self.before_method_arguments(v, s);
                v.nest_expression(None, false);
                v.visit(*type_arguments);
                v.visit_argument_list(*argument_list, false);
                v.unnest();
            }
            No::PrefixedIdentifier { period, identifier, .. } => {
                v.token(*period);
                v.visit(Some(*identifier));
            }
            No::PropertyAccess {
                operator, property_name, ..
            } => {
                v.token(*operator);
                v.visit(Some(*property_name));
            }
            _ => unreachable!(),
        }
    }

    /// `_beforeMethodArguments(selector)`.
    fn before_method_arguments(&mut self, v: &mut Visitante, s: &Seletor) {
        if self.chamadas_bloco.is_none() && self.chamadas.last().is_some_and(|c| c.no == s.no) {
            self.disable_rule(v);
        }
        if self.propriedades.is_empty()
            && self.chamadas.len() == 1
            && self.chamadas_bloco.is_none()
            && matches!(v.no(self.alvo), No::SimpleIdentifier { .. })
        {
            self.end_span(v);
        }
    }

    fn disable_rule(&mut self, v: &mut Visitante) {
        if !self.regra_ativa {
            return;
        }
        v.end_rule();
        self.regra_ativa = false;
    }

    fn enable_rule(&mut self, v: &mut Visitante, preguicosa: bool) {
        if self.regra_ativa {
            return;
        }
        let r = v.a.regra(custo::NORMAL);
        if let Some(p) = self.regra_propriedades {
            v.a.restricoes_de_nomeados(p, r);
        }
        if preguicosa {
            v.start_lazy_rule(Some(r));
        } else {
            v.start_rule(Some(r));
        }
        self.regra_ativa = true;
    }

    fn end_span(&mut self, v: &mut Visitante) {
        if self.span_terminado {
            return;
        }
        v.end_span();
        self.span_terminado = true;
    }
}

/// `isBlockCall(visitor)`.
fn is_block_call(v: &Visitante, s: &Seletor) -> bool {
    if s.tipo != TipoSeletor::Metodo {
        return false;
    }
    let No::MethodInvocation { argument_list, .. } = v.no(s.no) else {
        return false;
    };
    ArgumentListVisitor::novo(v, *argument_list).has_block_arguments()
}

/// `_forcesSplit(expression)`.
fn forces_split(v: &Visitante, expr: NoId) -> bool {
    let mut e = expr;
    while let No::ParenthesizedExpression { expression, .. } = v.no(e) {
        e = *expression;
    }
    if v.is_collection_literal(e) {
        return false;
    }
    if let No::FunctionExpression { body, .. } = v.no(e) {
        let No::BlockFunctionBody { block, .. } = v.no(*body) else {
            return false;
        };
        return match v.no(*block) {
            No::Block { statements, .. } => statements.is_empty(),
            _ => false,
        };
    }
    let lista = match v.no(e) {
        No::MethodInvocation { argument_list, .. }
        | No::InstanceCreationExpression { argument_list, .. }
        | No::FunctionExpressionInvocation { argument_list, .. } => *argument_list,
        _ => return true,
    };
    let No::ArgumentList { arguments, .. } = v.no(lista) else {
        return true;
    };
    let Some(&ultimo) = arguments.last() else {
        return true;
    };
    if v.has_comma_after(ultimo) {
        return false;
    }
    let mut arg = ultimo;
    if let No::NamedExpression { expression, .. } = v.no(arg) {
        arg = *expression;
    }
    forces_split(v, arg)
}

/// `_unwrapNullAssertion(expression)`.
fn unwrap_null_assertion(v: &Visitante, e: NoId) -> NoId {
    if let No::PostfixExpression { operand, operator } = v.no(e)
        && v.tk.texto(v.fonte, *operator) == "!"
    {
        return *operand;
    }
    e
}

/// `_unwrapTarget(node, calls)`.
fn unwrap_target(v: &Visitante, no: NoId, chamadas: &mut Vec<Seletor>) -> NoId {
    if v.looks_like_static_call(no) {
        return no;
    }
    match v.no(no) {
        No::MethodInvocation { target: Some(t), .. } => unwrap_selector(v, *t, TipoSeletor::Metodo, no, chamadas),
        No::PropertyAccess { target: Some(t), .. } => unwrap_selector(v, *t, TipoSeletor::Propriedade, no, chamadas),
        No::PrefixedIdentifier { prefix, .. } => unwrap_selector(v, *prefix, TipoSeletor::Prefixado, no, chamadas),
        No::IndexExpression { target: Some(t), .. } => unwrap_postfix(v, no, *t, chamadas),
        No::FunctionExpressionInvocation { function, .. } => unwrap_postfix(v, no, *function, chamadas),
        No::PostfixExpression { operand, operator } if v.tk.texto(v.fonte, *operator) == "!" => unwrap_postfix(v, no, *operand, chamadas),
        _ => no,
    }
}

fn unwrap_postfix(v: &Visitante, no: NoId, alvo: NoId, chamadas: &mut Vec<Seletor>) -> NoId {
    let alvo = unwrap_target(v, alvo, chamadas);
    if chamadas.is_empty() {
        return no;
    }
    chamadas.last_mut().expect("seletor").posfixos.push(no);
    alvo
}

fn unwrap_selector(v: &Visitante, alvo: NoId, tipo: TipoSeletor, no: NoId, chamadas: &mut Vec<Seletor>) -> NoId {
    let alvo = unwrap_target(v, alvo, chamadas);
    chamadas.push(Seletor {
        tipo,
        no,
        posfixos: Vec::new(),
    });
    alvo
}

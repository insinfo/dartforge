//! Porte de `short/argument_list_visitor.dart`: `ArgumentListVisitor` e
//! `ArgumentSublist`, com o tratamento especial de argumentos em bloco
//! (funções com corpo `{}`, coleções e strings de várias linhas).

use super::arena::{ChunkId, RegraId, custo};
use super::visitante::Visitante;
use crate::sintaxe::arvore::{No, NoId};
use crate::sintaxe::token::Tk;

/// `ArgumentListVisitor`.
pub struct ArgumentListVisitor {
    esquerdo: Tk,
    direito: Tk,
    todos: Vec<NoId>,
    argumentos: ArgumentSublist,
    funcoes: Option<Vec<NoId>>,
    depois_das_funcoes: Option<ArgumentSublist>,
}

/// `ArgumentSublist`.
pub struct ArgumentSublist {
    todos: Vec<NoId>,
    posicionais: Vec<NoId>,
    nomeados: Vec<NoId>,
    blocos: Vec<(NoId, Tk)>,
    blocos_iniciais: usize,
    blocos_finais: usize,
    regra_bloco: Option<RegraId>,
    quebra_anterior: Option<ChunkId>,
}

impl ArgumentListVisitor {
    /// `ArgumentListVisitor(visitor, node)`.
    pub fn novo(v: &Visitante, lista: NoId) -> ArgumentListVisitor {
        let No::ArgumentList {
            left_parenthesis,
            arguments,
            right_parenthesis,
        } = v.no(lista)
        else {
            panic!("ArgumentList esperada");
        };
        Self::for_arguments(v, *left_parenthesis, *right_parenthesis, arguments)
    }

    /// `ArgumentListVisitor.forArguments`.
    pub fn for_arguments(v: &Visitante, esquerdo: Tk, direito: Tk, argumentos: &[NoId]) -> ArgumentListVisitor {
        match contiguous_functions(v, argumentos) {
            None => ArgumentListVisitor {
                esquerdo,
                direito,
                todos: argumentos.to_vec(),
                argumentos: ArgumentSublist::novo(v, argumentos, argumentos),
                funcoes: None,
                depois_das_funcoes: None,
            },
            Some((ini, fim)) => ArgumentListVisitor {
                esquerdo,
                direito,
                todos: argumentos.to_vec(),
                argumentos: ArgumentSublist::novo(v, argumentos, &argumentos[..ini]),
                funcoes: Some(argumentos[ini..fim].to_vec()),
                depois_das_funcoes: Some(ArgumentSublist::novo(v, argumentos, &argumentos[fim..])),
            },
        }
    }

    fn is_single(&self, v: &Visitante) -> bool {
        self.todos.len() == 1 && !matches!(v.no(self.todos[0]), No::NamedExpression { .. })
    }

    /// `hasBlockArguments`.
    pub fn has_block_arguments(&self) -> bool {
        !self.argumentos.blocos.is_empty() || self.funcoes.is_some()
    }

    /// `visit()`.
    pub fn visit(mut self, v: &mut Visitante) {
        let unico = self.is_single(v);
        if unico {
            v.start_span(custo::NORMAL);
        }
        v.start_span(custo::NORMAL);
        v.token(self.esquerdo);
        self.argumentos.visit(v);
        v.end_span();
        if let Some(funcoes) = &self.funcoes {
            if funcoes[0] == self.todos[0] {
                v.solo_zero_split();
            } else {
                v.solo_split(custo::NORMAL);
            }
            for (i, &arg) in funcoes.iter().enumerate() {
                if i > 0 {
                    v.space();
                }
                v.visit(Some(arg));
                if v.has_comma_after(arg) {
                    let virgula = v.arv.fim(arg) + 1;
                    v.token(virgula);
                }
            }
            v.start_span(custo::NORMAL);
            self.depois_das_funcoes.as_mut().expect("sublista").visit(v);
            v.end_span();
        }
        v.token(self.direito);
        if unico {
            v.end_span();
        }
    }
}

/// `_contiguousFunctions(arguments)`.
fn contiguous_functions(v: &Visitante, argumentos: &[NoId]) -> Option<(usize, usize)> {
    let mut inicio: Option<usize> = None;
    let mut fim: i64 = -1;
    for (i, &arg) in argumentos.iter().enumerate() {
        if is_block_function(v, arg) {
            inicio.get_or_insert(i);
            if fim != -1 && fim != i as i64 {
                return None;
            }
            fim = i as i64 + 1;
        }
    }
    let inicio = inicio?;
    let fim = fim as usize;
    let todos_nomeados = |xs: &[NoId]| xs.iter().all(|&a| matches!(v.no(a), No::NamedExpression { .. }));
    if todos_nomeados(argumentos) && (inicio > 0 || fim < argumentos.len()) {
        return None;
    }
    if todos_nomeados(&argumentos[inicio..fim]) {
        let seta_nomeada = |e: NoId| -> bool {
            let No::NamedExpression { expression, .. } = v.no(e) else {
                return false;
            };
            match v.no(*expression) {
                No::FunctionExpression { body, .. } => matches!(v.no(*body), No::ExpressionFunctionBody { .. }),
                _ => false,
            }
        };
        for &a in &argumentos[..inicio] {
            if seta_nomeada(a) {
                return None;
            }
        }
        for &a in &argumentos[fim..] {
            if seta_nomeada(a) {
                return None;
            }
        }
    }
    Some((inicio, fim))
}

/// `_isBlockFunction(expression)`.
fn is_block_function(v: &Visitante, expr: NoId) -> bool {
    let mut e = expr;
    if let No::NamedExpression { expression, .. } = v.no(e) {
        e = *expression;
    }
    match v.no(e) {
        No::MethodInvocation {
            target, argument_list, ..
        } => {
            if !is_valid_wrapping_target(v, *target) {
                return false;
            }
            let No::ArgumentList { arguments, .. } = v.no(*argument_list) else {
                return false;
            };
            if arguments.len() != 1 {
                return false;
            }
            return is_block_function(v, arguments[0]);
        }
        No::InstanceCreationExpression { argument_list, .. } => {
            let No::ArgumentList { arguments, .. } = v.no(*argument_list) else {
                return false;
            };
            if arguments.len() != 1 {
                return false;
            }
            return is_block_function(v, arguments[0]);
        }
        No::FunctionExpressionInvocation {
            function, argument_list, ..
        } => {
            let No::ArgumentList { arguments, .. } = v.no(*argument_list) else {
                return false;
            };
            if !arguments.is_empty() {
                return false;
            }
            e = *function;
        }
        _ => {}
    }
    while let No::ParenthesizedExpression { expression, .. } = v.no(e) {
        e = *expression;
    }
    let No::FunctionExpression { body, .. } = v.no(e) else {
        return false;
    };
    let No::BlockFunctionBody { block, .. } = v.no(*body) else {
        return false;
    };
    let No::Block {
        statements,
        right_bracket,
        ..
    } = v.no(*block)
    else {
        return false;
    };
    !statements.is_empty() || !v.tk.comentarios_de(*right_bracket).is_empty()
}

/// `_isValidWrappingTarget(expression)`.
fn is_valid_wrapping_target(v: &Visitante, alvo: Option<NoId>) -> bool {
    let Some(mut e) = alvo else {
        return true;
    };
    while let No::PropertyAccess { target: Some(t), .. } = v.no(e) {
        e = *t;
    }
    matches!(v.no(e), No::PrefixedIdentifier { .. } | No::SimpleIdentifier { .. })
}

impl ArgumentSublist {
    /// `ArgumentSublist(allArguments, arguments)`.
    fn novo(v: &Visitante, todos: &[NoId], argumentos: &[NoId]) -> ArgumentSublist {
        let (posicionais, nomeados) = split_argument_lists(v, argumentos);
        let mut blocos: Vec<(NoId, Tk)> = Vec::new();
        for &a in argumentos {
            if let Some(t) = block_token(v, a) {
                blocos.push((a, t));
            }
        }
        let e_bloco = |blocos: &Vec<(NoId, Tk)>, a: NoId| blocos.iter().any(|(b, _)| *b == a);
        let mut iniciais = 0;
        for &a in argumentos {
            if !e_bloco(&blocos, a) {
                break;
            }
            iniciais += 1;
        }
        let mut finais = 0;
        if iniciais != argumentos.len() {
            for &a in argumentos.iter().rev() {
                if !e_bloco(&blocos, a) {
                    break;
                }
                finais += 1;
            }
        }
        if iniciais != blocos.len() {
            iniciais = 0;
        }
        if finais != blocos.len() {
            finais = 0;
        }
        if iniciais == 0 && finais == 0 {
            blocos.clear();
        }
        ArgumentSublist {
            todos: todos.to_vec(),
            posicionais,
            nomeados,
            blocos,
            blocos_iniciais: iniciais,
            blocos_finais: finais,
            regra_bloco: None,
            quebra_anterior: None,
        }
    }

    fn visit(&mut self, v: &mut Visitante) {
        if !self.blocos.is_empty() {
            self.regra_bloco = Some(v.a.regra(custo::SPLIT_BLOCKS));
        }
        let regra = self.visit_positional(v);
        self.visit_named(v, regra);
    }

    fn visit_positional(&mut self, v: &mut Visitante) -> Option<RegraId> {
        if self.posicionais.is_empty() {
            return None;
        }
        let iniciais = self.blocos_iniciais.min(self.posicionais.len());
        let finais = self.blocos_finais.saturating_sub(self.nomeados.len());
        let regra = v.a.regra_posicional(self.regra_bloco, self.posicionais.len(), iniciais, finais);
        let posicionais = self.posicionais.clone();
        self.visit_arguments(v, &posicionais, regra, true);
        Some(regra)
    }

    fn visit_named(&mut self, v: &mut Visitante, posicional: Option<RegraId>) {
        if self.nomeados.is_empty() {
            return;
        }
        let iniciais = self.blocos_iniciais.saturating_sub(self.posicionais.len());
        let finais = self.blocos_finais.min(self.nomeados.len());
        let nomeada = v.a.regra_nomeada(self.regra_bloco, iniciais, finais);
        if let Some(p) = posicional {
            v.a.restricoes_de_nomeados(p, nomeada);
        }
        let nomeados = self.nomeados.clone();
        self.visit_arguments(v, &nomeados, nomeada, false);
    }

    fn visit_arguments(&mut self, v: &mut Visitante, argumentos: &[NoId], regra: RegraId, sao_posicionais: bool) {
        v.start_rule(Some(regra));
        let c = v.builder_split(true, argumentos[0] != self.todos[0]);
        self.quebra_anterior = Some(c);
        v.a.antes_do_argumento(regra, Some(c));
        if sao_posicionais {
            v.start_span(custo::POSITIONAL_ARGUMENTS);
        }
        for (i, &arg) in argumentos.iter().enumerate() {
            self.visit_argument(v, regra, arg);
            if i + 1 != argumentos.len() {
                let c = v.split();
                self.quebra_anterior = Some(c);
                v.a.antes_do_argumento(regra, Some(c));
            }
        }
        if sao_posicionais {
            v.end_span();
        }
        v.end_rule();
    }

    fn visit_argument(&mut self, v: &mut Visitante, regra: RegraId, arg: NoId) {
        let bloco = self.blocos.iter().find(|(b, _)| *b == arg).map(|(_, t)| *t);
        let nomeado = matches!(v.no(arg), No::NamedExpression { .. });
        let varios = self.todos.len() > 1 || matches!(v.no(self.todos[0]), No::RecordLiteral { .. });
        if let Some(t) = bloco {
            v.a.desligar_internas(regra);
            let rb = self.regra_bloco.expect("regra de bloco");
            v.before_block(t, rb, self.quebra_anterior);
        } else if varios {
            v.start_block_argument_nesting();
        } else if !nomeado {
            v.a.desligar_internas(regra);
        }

        if let No::NamedExpression { name, expression } = v.no(arg) {
            let No::Label { label, colon } = v.no(*name) else {
                panic!("Label esperado");
            };
            let No::SimpleIdentifier { token } = v.no(*label) else {
                panic!("SimpleIdentifier esperado");
            };
            v.visit_named_node(*token, *colon, *expression, Some(regra));
        } else {
            v.visit(Some(arg));
        }

        if bloco.is_some() {
            v.a.ligar_internas(regra);
        } else if varios {
            v.end_block_argument_nesting();
        } else if !nomeado {
            v.a.ligar_internas(regra);
        }

        if v.has_comma_after(arg) {
            let virgula = v.arv.fim(arg) + 1;
            v.token(virgula);
        }
    }
}

/// `_splitArgumentLists(arguments)`.
fn split_argument_lists(v: &Visitante, argumentos: &[NoId]) -> (Vec<NoId>, Vec<NoId>) {
    let mut posicionais = Vec::new();
    let mut nomeados = Vec::new();
    let mut em_nomeados = false;
    for &a in argumentos {
        if matches!(v.no(a), No::NamedExpression { .. }) {
            em_nomeados = true;
        } else if em_nomeados {
            return (Vec::new(), argumentos.to_vec());
        }
        if em_nomeados {
            nomeados.push(a);
        } else {
            posicionais.push(a);
        }
    }
    (posicionais, nomeados)
}

/// `_blockToken(expression)`.
fn block_token(v: &Visitante, expr: NoId) -> Option<Tk> {
    let mut e = expr;
    if let No::NamedExpression { expression, .. } = v.no(e) {
        e = *expression;
    }
    match v.no(e) {
        No::ListLiteral { left_bracket, .. } | No::SetOrMapLiteral { left_bracket, .. } => Some(*left_bracket),
        No::RecordLiteral { left_parenthesis, .. } => Some(*left_parenthesis),
        No::SimpleStringLiteral { .. } | No::StringInterpolation { .. } if v.is_multiline_string(e) => Some(v.arv.inicio(e)),
        _ => None,
    }
}

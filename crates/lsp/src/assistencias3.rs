//! Três assistências de condição do servidor do Dart 3.6.2, pelas regras de
//! docs/LSP-ESPECIFICACAO.md §13.10:
//!
//! | Assistência | Espécie |
//! | --- | --- |
//! | `Invert 'if' statement` | `refactor.invertIf` |
//! | `Invert conditional expression` | `refactor.invertConditional` |
//! | `Join 'if' statement with inner 'if' statement` | `refactor.joinWithInnerIf` |
//!
//! As três usam o `invertCondition` do `CorrectionUtils`
//! (`correction_utils.dart:345-410`), aqui [`inverter`]. Diferença
//! conhecida: no `Invert conditional expression` o cursor vale em qualquer
//! ponto da condicional mais interna que não esteja dentro dos argumentos
//! de uma chamada aninhada (o original sobe um número fixo de nós).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::acoes::AcaoDeCodigo;
use crate::projeto::Projeto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};

fn acao(uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo: titulo.into(),
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
        diagnostico: None,
        criar_arquivo: None,
    }
}

const PRECEDENCIA_DO_OU: u32 = 1;
const PRECEDENCIA_DO_E: u32 = 2;
const PRECEDENCIA_MAXIMA: u32 = 1 << 20;

/// A indentação da linha que contém `offset`.
fn indentacao(fonte: &str, offset: usize) -> &str {
    let ini = fonte[..offset.min(fonte.len())].rfind('\n').map_or(0, |i| i + 1);
    let resto = &fonte[ini..];
    &resto[..resto.len() - resto.trim_start_matches([' ', '\t']).len()]
}

impl Projeto {
    /// `invertCondition`: o texto da condição negada e a precedência dele.
    fn inverter(&self, unidade: UnitId, e: ExprId) -> (String, u32) {
        let u = self.programa().unit(unidade);
        let a = &u.ast;
        let fonte = u.source.as_str();
        let texto = |x: ExprId| {
            let s = a.expr(x).span;
            fonte.get(s.start..s.end).unwrap_or("").to_string()
        };
        match &a.expr(e).kind {
            ExprKind::Bool(v) => ((!*v).to_string(), PRECEDENCIA_MAXIMA),
            ExprKind::Binary { op, left, right } => {
                let comparacao = match op {
                    BinaryOp::Lt => Some(">="),
                    BinaryOp::Gt => Some("<="),
                    BinaryOp::LtEq => Some(">"),
                    BinaryOp::GtEq => Some("<"),
                    BinaryOp::Eq => Some("!="),
                    BinaryOp::NotEq => Some("=="),
                    _ => None,
                };
                if let Some(novo) = comparacao {
                    return (format!("{} {novo} {}", texto(*left), texto(*right)), PRECEDENCIA_MAXIMA);
                }
                // De Morgan; cada lado entre parênteses se liga mais fraco
                // que o operador novo.
                let (junta, precedencia) = match op {
                    BinaryOp::And => ("||", PRECEDENCIA_DO_OU),
                    BinaryOp::Or => ("&&", PRECEDENCIA_DO_E),
                    _ => return self.negar_outra(unidade, e),
                };
                let lado = |x: ExprId| {
                    let (t, p) = self.inverter(unidade, x);
                    if p < precedencia { format!("({t})") } else { t }
                };
                (format!("{} {junta} {}", lado(*left), lado(*right)), precedencia)
            }
            ExprKind::Is { value, ty, negated } => {
                let tipo = a.types[ty.0 as usize].span;
                let palavra = if *negated { "is" } else { "is!" };
                (format!("{} {palavra} {}", texto(*value), fonte.get(tipo.start..tipo.end).unwrap_or("")), PRECEDENCIA_MAXIMA)
            }
            // `!e`: o operando sem os parênteses de fora.
            ExprKind::Unary { op: UnaryOp::Not, operand } => {
                let mut dentro = *operand;
                while let ExprKind::Parenthesized(x) = &a.expr(dentro).kind {
                    dentro = *x;
                }
                (texto(dentro), PRECEDENCIA_MAXIMA)
            }
            ExprKind::Parenthesized(x) => self.inverter(unidade, *x),
            _ => self.negar_outra(unidade, e),
        }
    }

    /// As duas últimas linhas da tabela: `!texto` para uma expressão de tipo
    /// estático `bool`, o texto inalterado para as outras.
    fn negar_outra(&self, unidade: UnitId, e: ExprId) -> (String, u32) {
        let u = self.programa().unit(unidade);
        let s = u.ast.expr(e).span;
        let texto = u.source.get(s.start..s.end).unwrap_or("");
        let e_bool = self.consulta.corpos.units[unidade.0 as usize].get_type(e) == Some(self.consulta.core.bool_);
        (if e_bool { format!("!{texto}") } else { texto.to_string() }, PRECEDENCIA_MAXIMA)
    }

    /// As assistências desta lista com o cursor em `offset` de `unidade`.
    pub(crate) fn assistencias_de_condicao(&self, uri: &str, unidade: UnitId, offset: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        let u = self.programa().unit(unidade);
        let a = &u.ast;
        let fonte = u.source.as_str();
        let texto_do_comando = |s: StmtId| {
            let span = a.stmt(s).span;
            fonte.get(span.start..span.end).unwrap_or("").to_string()
        };
        let e_bloco = |s: StmtId| matches!(a.stmt(s).kind, StmtKind::Block(_));

        // O `if` mais interno que contém o cursor.
        let o_if = a
            .stmts
            .iter()
            .filter(|s| matches!(s.kind, StmtKind::If { .. }) && s.span.start <= offset && offset <= s.span.end)
            .min_by_key(|s| s.span.end - s.span.start);
        if let Some(s) = o_if
            && let StmtKind::If { condition, case_pattern, then, else_, .. } = &s.kind
        {
            let cond = a.expr(*condition).span;
            let entao = a.stmt(*then).span;
            // `Invert 'if' statement`: o cursor no próprio comando (no `if`,
            // nos parênteses ou no `else`), fora da condição e dos blocos.
            if let Some(senao) = else_
                && case_pattern.is_none()
                && e_bloco(*then)
                && e_bloco(*senao)
            {
                let no_comando = offset < cond.start || (offset > cond.end && offset <= entao.start) || (offset >= entao.end && offset <= a.stmt(*senao).span.start);
                if no_comando {
                    let (invertida, _) = self.inverter(unidade, *condition);
                    saida.push(acao(
                        uri,
                        "Invert 'if' statement",
                        "refactor.invertIf",
                        vec![(cond, invertida), (entao, texto_do_comando(*senao)), (a.stmt(*senao).span, texto_do_comando(*then))],
                    ));
                }
            }
            // `Join 'if' statement with inner 'if' statement`: o cursor no
            // comando ou na condição; sem `else`; o corpo é um `if` sem `else`.
            if else_.is_none() && offset <= entao.start {
                // `getSingleStatement`: o comando, ou o único de um bloco.
                let unico = match &a.stmt(*then).kind {
                    StmtKind::Block(l) if l.len() == 1 => Some(l[0]),
                    StmtKind::Block(_) => None,
                    _ => Some(*then),
                };
                if let Some(interno) = unico
                    && let StmtKind::If { condition: cond_interna, then: corpo, else_: None, .. } = &a.stmt(interno).kind
                {
                    // `shouldWrapParenthesisBeforeAnd`: binária mais fraca que `&&`.
                    let para_o_e = |x: ExprId| {
                        let sp = a.expr(x).span;
                        let t = fonte.get(sp.start..sp.end).unwrap_or("");
                        if matches!(a.expr(x).kind, ExprKind::Binary { op: BinaryOp::Or | BinaryOp::IfNull, .. }) { format!("({t})") } else { t.to_string() }
                    };
                    let prefixo = indentacao(fonte, s.span.start);
                    let eol = if fonte.contains("\r\n") { "\r\n" } else { "\n" };
                    // Os comandos do corpo interno, por linhas inteiras, com
                    // um nível de indentação a menos.
                    let comandos: Vec<StmtId> = match &a.stmt(*corpo).kind {
                        StmtKind::Block(l) => l.to_vec(),
                        _ => vec![*corpo],
                    };
                    let mut novo = String::new();
                    if let (Some(primeiro), Some(ultimo)) = (comandos.first(), comandos.last()) {
                        let de = fonte[..a.stmt(*primeiro).span.start].rfind('\n').map_or(0, |i| i + 1);
                        let fim_do_ultimo = a.stmt(*ultimo).span.end;
                        let ate = fonte[fim_do_ultimo..].find('\n').map_or(fonte.len(), |i| fim_do_ultimo + i + 1);
                        for linha in fonte[de..ate].split_inclusive('\n') {
                            novo.push_str(linha.strip_prefix("  ").unwrap_or(linha));
                        }
                    }
                    let juntas = format!("{} && {}", para_o_e(*condition), para_o_e(*cond_interna));
                    saida.push(acao(
                        uri,
                        "Join 'if' statement with inner 'if' statement",
                        "refactor.joinWithInnerIf",
                        vec![(s.span, format!("if ({juntas}) {{{eol}{novo}{prefixo}}}"))],
                    ));
                }
            }
        }

        // `Invert conditional expression`: a condicional do
        // `_getConditionalExpressionAncestor`, sobre a árvore do analyzer.
        let condicional = {
            let cx = crate::refatoracoes::Contexto::novo(self, unidade);
            cx.condicional_proxima(offset).and_then(|n| cx.expr_do_no(n))
        };
        if let Some(e) = condicional.map(|x| a.expr(x))
            && let ExprKind::Conditional { condition, then, else_ } = &e.kind
        {
            {
                let trecho = |x: ExprId| {
                    let s = a.expr(x).span;
                    (s, fonte.get(s.start..s.end).unwrap_or("").to_string())
                };
                let (invertida, _) = self.inverter(unidade, *condition);
                let ((span_entao, texto_entao), (span_senao, texto_senao)) = (trecho(*then), trecho(*else_));
                saida.push(acao(
                    uri,
                    "Invert conditional expression",
                    "refactor.invertConditional",
                    vec![(a.expr(*condition).span, invertida), (span_entao, texto_senao), (span_senao, texto_entao)],
                ));
            }
        }
        saida
    }
}

impl Projeto {
    /// Mais três assistências sintáticas (docs/LSP-ESPECIFICACAO.md §13.10):
    /// `Join 'if' statement with outer 'if' statement`
    /// (`refactor.joinWithOuterIf`), `Replace 'if-else' with conditional
    /// ('c ? x : y')` (`refactor.convert.ifElseToConditional`) e `Join
    /// variable declaration` (`refactor.joinVariableDeclaration`). Na
    /// última, "a mesma variável" é o mesmo nome no comando vizinho do mesmo
    /// bloco. Escrito sem compilar nem executar (2026-10-05).
    pub(crate) fn assistencias_de_juncao(&self, uri: &str, unidade: UnitId, offset: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        let u = self.programa().unit(unidade);
        let a = &u.ast;
        let fonte = u.source.as_str();
        let eol = if fonte.contains("\r\n") { "\r\n" } else { "\n" };
        let texto_da_expressao = |x: ExprId| {
            let s = a.expr(x).span;
            fonte.get(s.start..s.end).unwrap_or("")
        };
        // `getSingleStatement`: o comando, ou o único de um bloco.
        let unico = |s: StmtId| match &a.stmt(s).kind {
            StmtKind::Block(l) if l.len() == 1 => Some(l[0]),
            StmtKind::Block(_) => None,
            _ => Some(s),
        };

        // O `if` mais interno que contém o cursor.
        let o_if = a
            .stmts
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, StmtKind::If { .. }) && s.span.start <= offset && offset <= s.span.end)
            .min_by_key(|(_, s)| s.span.end - s.span.start);
        if let Some((indice, s)) = o_if
            && let StmtKind::If { condition, case_pattern, then, else_, .. } = &s.kind
        {
            let id = StmtId(indice as u32);
            let cond = a.expr(*condition).span;
            let entao = a.stmt(*then).span;
            // `Join … with outer`: sem `else`; o pai (pulando um bloco de um
            // comando só) é um `if` sem `else`.
            if else_.is_none() && offset <= entao.start {
                let externo = a.stmts.iter().find(|x| match &x.kind {
                    StmtKind::If { then: t, else_: None, .. } => *t == id || (*t != id && unico(*t) == Some(id)),
                    _ => false,
                });
                if let Some(externo) = externo
                    && let StmtKind::If { condition: cond_externa, .. } = &externo.kind
                {
                    let para_o_e = |x: ExprId| {
                        let t = texto_da_expressao(x);
                        if matches!(a.expr(x).kind, ExprKind::Binary { op: BinaryOp::Or | BinaryOp::IfNull, .. }) { format!("({t})") } else { t.to_string() }
                    };
                    let prefixo = indentacao(fonte, externo.span.start);
                    let comandos: Vec<StmtId> = match &a.stmt(*then).kind {
                        StmtKind::Block(l) => l.to_vec(),
                        _ => vec![*then],
                    };
                    let mut novo = String::new();
                    if let (Some(primeiro), Some(ultimo)) = (comandos.first(), comandos.last()) {
                        let de = fonte[..a.stmt(*primeiro).span.start].rfind('\n').map_or(0, |i| i + 1);
                        let fim_do_ultimo = a.stmt(*ultimo).span.end;
                        let ate = fonte[fim_do_ultimo..].find('\n').map_or(fonte.len(), |i| fim_do_ultimo + i + 1);
                        for linha in fonte[de..ate].split_inclusive('\n') {
                            novo.push_str(linha.strip_prefix("  ").unwrap_or(linha));
                        }
                    }
                    let juntas = format!("{} && {}", para_o_e(*cond_externa), para_o_e(*condition));
                    saida.push(acao(
                        uri,
                        "Join 'if' statement with outer 'if' statement",
                        "refactor.joinWithOuterIf",
                        vec![(externo.span, format!("if ({juntas}) {{{eol}{novo}{prefixo}}}"))],
                    ));
                }
            }
            // `Replace 'if-else' with conditional`: o cursor no próprio
            // comando; cada ramo é um comando só.
            if let Some(senao) = else_
                && case_pattern.is_none()
            {
                let no_comando = offset < cond.start || (offset > cond.end && offset <= entao.start) || (offset >= entao.end && offset <= a.stmt(*senao).span.start);
                if no_comando && let (Some(x), Some(y)) = (unico(*then), unico(*senao)) {
                    let c = texto_da_expressao(*condition);
                    let novo = match (&a.stmt(x).kind, &a.stmt(y).kind) {
                        (StmtKind::Return(Some(p)), StmtKind::Return(Some(q))) => {
                            Some(format!("return {c} ? {} : {};", texto_da_expressao(*p), texto_da_expressao(*q)))
                        }
                        (StmtKind::Expression(p), StmtKind::Expression(q)) => match (&a.expr(*p).kind, &a.expr(*q).kind) {
                            (
                                ExprKind::Assign { op: ast::AssignOp::Assign, target: t1, value: v1 },
                                ExprKind::Assign { op: ast::AssignOp::Assign, target: t2, value: v2 },
                            ) if texto_da_expressao(*t1) == texto_da_expressao(*t2) => {
                                Some(format!("{} = {c} ? {} : {};", texto_da_expressao(*t1), texto_da_expressao(*v1), texto_da_expressao(*v2)))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(novo) = novo {
                        saida.push(acao(uri, "Replace 'if-else' with conditional ('c ? x : y')", "refactor.convert.ifElseToConditional", vec![(s.span, novo)]));
                    }
                }
            }
        }

        // `Join variable declaration`: `T v;` seguido de `v = e;` no mesmo
        // bloco, com o cursor no nome atribuído ou na declaração.
        'blocos: for bloco in a.stmts.iter() {
            let StmtKind::Block(lista) = &bloco.kind else { continue };
            for par in lista.windows(2) {
                let (declaracao, atribuicao) = (a.stmt(par[0]), a.stmt(par[1]));
                let StmtKind::Variables(l) = &declaracao.kind else { continue };
                let [variavel] = &l.variables[..] else { continue };
                if variavel.initializer.is_some() {
                    continue;
                }
                let StmtKind::Expression(e) = &atribuicao.kind else { continue };
                let ExprKind::Assign { op: ast::AssignOp::Assign, target, value } = &a.expr(*e).kind else { continue };
                let ExprKind::Identifier(n) = &a.expr(*target).kind else { continue };
                if n.sym != variavel.name.sym {
                    continue;
                }
                let alvo = a.expr(*target).span;
                let no_nome_atribuido = alvo.start <= offset && offset <= alvo.end;
                let na_declaracao = declaracao.span.start <= offset && offset < declaracao.span.end;
                if !(no_nome_atribuido || na_declaracao) {
                    continue;
                }
                // O `=` fica entre o alvo e o valor.
                let Some(k) = fonte.get(alvo.end..a.expr(*value).span.start).and_then(|t| t.find('=')) else { continue };
                saida.push(acao(
                    uri,
                    "Join variable declaration",
                    "refactor.joinVariableDeclaration",
                    vec![(Span { start: variavel.name.span.end, end: alvo.end + k }, " ".to_string())],
                ));
                break 'blocos;
            }
        }
        saida
    }
}

impl crate::refatoracoes::Contexto<'_> {
    /// `_thisOrParentOfType`: o nó, ou o pai, se for de uma das espécies.
    fn este_ou_pai_de(&self, n: usize, especies: &[&str]) -> Option<usize> {
        if especies.contains(&self.especie(n)) {
            return Some(n);
        }
        self.pai(n).filter(|&p| especies.contains(&self.especie(p)))
    }

    /// `InvertConditionalExpression._getConditionalExpressionAncestor`
    /// (invert_conditional_expression.dart): a condicional "perto o
    /// bastante" do nó da seleção.
    pub(crate) fn condicional_proxima(&self, offset: usize) -> Option<usize> {
        let mut no = self.arvore.localizar(offset, offset)?;
        no = self.este_ou_pai_de(no, &["MethodInvocation"]).unwrap_or(no);
        no = self.este_ou_pai_de(no, &["AwaitExpression"]).unwrap_or(no);
        let booleanas = ["IsExpression", "BinaryExpression", "BooleanLiteral"];
        let booleana = if booleanas.contains(&self.especie(no)) {
            Some(no)
        } else if let Some(p) = self.pai(no).filter(|&p| booleanas.contains(&self.especie(p))) {
            Some(p)
        } else {
            self.este_ou_pai_de(no, &["PrefixExpression"])
        };
        no = booleana.unwrap_or(no);
        no = self.este_ou_pai_de(no, &["ParenthesizedExpression"]).unwrap_or(no);
        no = self.este_ou_pai_de(no, &["AwaitExpression"]).unwrap_or(no);
        self.este_ou_pai_de(no, &["ConditionalExpression"])
    }
}

//! Inline Method (`legacy/inline_method.dart`, docs/LSP-ESPECIFICACAO.md
//! §13.11.6): `_prepareMethod`, `_prepareMethodParts` (com o
//! `_VariablesVisitor`), o `_ReferenceProcessor` de cada referência, o
//! `_getMethodSourceForInvocation` (com os nomes em conflito do
//! `VisibleRangesComputer`) e a remoção da declaração.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::refatoracoes::{Contexto, Elem, PedidoDeRefatoracao, ResultadoDeRefatoracao};
use crate::refatoracoes_exec::{Estado, Mudanca, Severidade, Texto};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId};
use dartforge_types::{MemberRef, Resolved, Type};
use std::collections::HashMap;

type R = ResultadoDeRefatoracao;

/// `_ParameterOccurrence`.
#[derive(Debug, Clone)]
struct OcorrenciaDeParametro {
    faixa: (usize, usize),
    precedencia_do_pai: u8,
    em_interpolacao: bool,
}

/// `_SourcePart`.
#[derive(Debug, Clone, Default)]
struct Parte {
    base: usize,
    fonte: String,
    prefixo: String,
    /// Pelo nome do parâmetro, na ordem da primeira ocorrência.
    parametros: Vec<(String, Vec<OcorrenciaDeParametro>)>,
    /// Pela declaração do local: o nome e as faixas.
    variaveis: Vec<(usize, String, Vec<(usize, usize)>)>,
    this_explicitos: Vec<usize>,
    this_implicitos: Vec<usize>,
    classes_implicitas: Vec<(String, Vec<usize>)>,
}

/// O método embutido.
struct Metodo<'c, 'p> {
    /// A árvore da unidade da declaração.
    cxm: &'c Contexto<'p>,
    elemento: Elem,
    no: usize,
    parametros: Option<usize>,
    expressao: Option<usize>,
    parte_expressao: Option<Parte>,
    parte_comandos: Option<Parte>,
    /// Os parâmetros declarados: nome, posicional obrigatório, nomeado e o
    /// texto do valor padrão.
    declarados: Vec<(String, bool, bool, Option<String>)>,
    assincrono: bool,
}

/// `refactor.perform`/`refactor.validate` do `INLINE_METHOD`.
pub(crate) fn executar(cx: &Contexto<'_>, uri: &str, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    let _ = uri;
    let p = cx.p;
    let prog = p.programa();
    let fatal = "Method declaration or reference must be selected to activate this refactoring.";
    let falhar = |e: Estado| crate::refatoracoes_exec::concluir(pedido, e, Estado::default, Mudanca::default);
    // `_prepareMethod`.
    let Some((e, _, declaracao)) = cx.elemento_de_inline_method(pedido.offset) else { return falhar(Estado::fatal(fatal)) };
    if !cx.executavel(e) || cx.sintetico(e) {
        return falhar(Estado::fatal(fatal));
    }
    let (unidade_m, fid) = match e {
        Elem::FuncaoLocal(fid) => (cx.unidade, fid),
        Elem::Funcao(f) => match prog.function(f).node {
            FunctionRef::Function { unit, function } => (unit, function),
            _ => return falhar(Estado::fatal(fatal)),
        },
        _ => return falhar(Estado::fatal(fatal)),
    };
    let contexto_m;
    let cxm: &Contexto<'_> = if unidade_m == cx.unidade {
        cx
    } else {
        contexto_m = Contexto::novo(p, unidade_m);
        &contexto_m
    };
    let Some(no) = cxm
        .arvore
        .nos
        .iter()
        .position(|k| k.marca == Marca::Funcao(fid) && matches!(k.especie, "FunctionDeclaration" | "MethodDeclaration"))
    else {
        return falhar(Estado::fatal(fatal));
    };
    let excluir_fonte = declaracao;
    let embutir_tudo = excluir_fonte;
    if cx.operador(e) {
        return falhar(Estado::fatal("Cannot inline operator."));
    }
    if cx.gerador(e) {
        return falhar(Estado::fatal("Cannot inline a generator."));
    }
    // Os parâmetros e o corpo.
    let funcao_no = if cxm.especie(no) == "FunctionDeclaration" {
        cxm.filhos(no).iter().copied().find(|&c| cxm.especie(c) == "FunctionExpression").unwrap_or(no)
    } else {
        no
    };
    let parametros = cxm.filhos(funcao_no).iter().copied().find(|&c| cxm.especie(c) == "FormalParameterList");
    let corpo = cxm.filhos(funcao_no).iter().copied().find(|&c| matches!(cxm.especie(c), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"));
    let funcao = cxm.ast.function(fid);
    let declarados: Vec<(String, bool, bool, Option<String>)> = funcao
        .parameters
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|q| {
            let nome = q.name.map(|n| cxm.fonte[n.span.start..n.span.end].to_string()).unwrap_or_default();
            let padrao = q.default_value.map(|d| {
                let s = cxm.ast.expr(d).span;
                cxm.fonte[s.start..s.end].to_string()
            });
            (nome, q.kind == ast::ParameterKind::Required, q.kind == ast::ParameterKind::Named, padrao)
        })
        .collect();
    let mut m = Metodo {
        cxm,
        elemento: e,
        no,
        parametros,
        expressao: None,
        parte_expressao: None,
        parte_comandos: None,
        declarados,
        assincrono: matches!(funcao.modifier, AsyncModifier::Async | AsyncModifier::AsyncStar),
    };
    // `_prepareMethodParts`.
    let mut iniciais = Estado::default();
    match corpo.map(|c| cxm.especie(c)) {
        Some("ExpressionFunctionBody") => {
            let c = corpo.unwrap();
            if let Some(&x) = cxm.filhos(c).first() {
                m.expressao = Some(x);
                let (a, b) = faixa(cxm, x);
                match m.parte(cx, (a, b), fid) {
                    Ok(p) => m.parte_expressao = Some(p),
                    Err(e) => return R::ErroInterno(e),
                }
            }
        }
        Some("BlockFunctionBody") => {
            let c = corpo.unwrap();
            let Some(&bloco) = cxm.filhos(c).first() else { return falhar(Estado::fatal("Cannot inline method without body.")) };
            let mut comandos: Vec<usize> = cxm.filhos(bloco).to_vec();
            if let Some(&ultimo) = comandos.last()
                && cxm.especie(ultimo) == "ReturnStatement"
            {
                m.expressao = cxm.filhos(ultimo).first().copied();
                if let Some(x) = m.expressao {
                    match m.parte(cx, faixa(cxm, x), fid) {
                        Ok(p) => m.parte_expressao = Some(p),
                        Err(e) => return R::ErroInterno(e),
                    }
                }
                comandos.pop();
            }
            if !comandos.is_empty() {
                let tx = Texto::novo(cxm.fonte);
                let s = tx.faixa_de_linhas(faixa(cxm, comandos[0]).0, faixa(cxm, *comandos.last().unwrap()).1);
                match m.parte(cx, (s.start, s.end), fid) {
                    Ok(p) => m.parte_comandos = Some(p),
                    Err(e) => return R::ErroInterno(e),
                }
            }
            // `_ReturnsValidatorVisitor`: o segundo `return` (closures
            // incluídas).
            let mut quantos = 0;
            let mut pilha = vec![bloco];
            let mut ordem = Vec::new();
            while let Some(k) = pilha.pop() {
                ordem.push(k);
                pilha.extend(cxm.filhos(k).iter().rev().copied());
            }
            for k in ordem {
                if cxm.especie(k) == "ReturnStatement" {
                    quantos += 1;
                    if quantos == 2 {
                        iniciais.adicionar(Severidade::Erro, "Ambiguous return value.");
                    }
                }
            }
        }
        _ => return falhar(Estado::fatal("Cannot inline method without body.")),
    }
    // As referências.
    let referencias: Vec<(UnitId, Span)> = match e {
        Elem::FuncaoLocal(f) => {
            let d = cxm.ast.function(f).name.map(|n| n.span.start);
            let mut v: Vec<(UnitId, Span)> = cxm
                .ast
                .exprs
                .iter()
                .enumerate()
                .filter(|(i, x)| matches!(x.kind, ast::ExprKind::Identifier(_)) && cxm.corpos.declaracao_local(ExprId(*i as u32)) == d && d.is_some())
                .map(|(_, x)| (unidade_m, x.span))
                .collect();
            v.sort_by_key(|(_, s)| s.start);
            v
        }
        Elem::Funcao(f) => {
            let getter = prog.function(f).kind == FunctionKind::Getter;
            let mut v = crate::refatoracoes_exec::referencias_de_funcao(p, f, getter);
            v.sort_by_key(|(u, s)| (u.0, s.start));
            v
        }
        _ => Vec::new(),
    };
    if pedido.so_validar {
        return crate::refatoracoes_exec::concluir(pedido, iniciais, Estado::default, Mudanca::default);
    }
    if iniciais.tem_fatal() {
        return R::Erro(iniciais.mensagem().unwrap_or_default());
    }
    // `checkFinalConditions`: a mudança.
    let mut finais = Estado::default();
    let mut mudanca = Mudanca::default();
    let mut contextos: HashMap<UnitId, Contexto<'_>> = HashMap::new();
    let mut ja_assincronos: Vec<(UnitId, usize)> = Vec::new();
    for (u, s) in referencias {
        let cxr: &Contexto<'_> = if u == cx.unidade {
            cx
        } else if u == unidade_m {
            cxm
        } else {
            contextos.entry(u).or_insert_with(|| Contexto::novo(p, u));
            contextos.get(&u).unwrap()
        };
        let Some(uri_r) = p.uri_da_unidade(u) else { continue };
        let Some(n) = cxr.arvore.localizar(s.start, s.start) else { continue };
        if cxr.especie(n) != "SimpleIdentifier" {
            return R::ErroInterno(format!("type '{}Impl' is not a subtype of type 'SimpleIdentifier' in type cast", cxr.especie(n)));
        }
        if let Err(e) = m.processar(cxr, &uri_r, u, n, pedido.offset, embutir_tudo, &mut finais, &mut mudanca, &mut ja_assincronos) {
            return R::ErroInterno(e);
        }
    }
    if excluir_fonte && embutir_tudo
        && let Some(uri_m) = p.uri_da_unidade(unidade_m)
    {
        let (a, b) = faixa(cxm, no);
        let tx = Texto::novo(cxm.fonte);
        let inicio = pular_linhas_vazias_a_esquerda(cxm.fonte, tx.inicio_do_conteudo(a));
        let s = {
            let mut fim = b;
            if tx.inicio_da_linha(inicio) == inicio {
                fim = tx.fim_do_conteudo(b);
            }
            Span { start: inicio, end: fim }
        };
        mudanca.adicionar(&uri_m, s, "");
    }
    let mut todas = iniciais;
    todas.somar(finais);
    if todas.tem_erro() {
        return R::Erro(todas.mensagem().unwrap_or_default());
    }
    mudanca.resultado()
}

/// O intervalo de um nó.
fn faixa(cx: &Contexto<'_>, n: usize) -> (usize, usize) {
    (cx.arvore.nos[n].inicio, cx.arvore.nos[n].fim)
}

/// `_skipEmptyLinesLeft`.
fn pular_linhas_vazias_a_esquerda(t: &str, mut i: usize) -> usize {
    let b = t.as_bytes();
    let mut ultima = i;
    while i > 0 {
        let c = b[i - 1];
        if !matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
            return ultima;
        }
        if matches!(c, b'\r' | b'\n') {
            ultima = i;
        }
        i -= 1;
    }
    0
}

impl<'c, 'p> Metodo<'c, 'p> {
    /// `_createSourcePart(range)`: o texto, o prefixo da linha (sic: no
    /// texto da unidade do cursor) e o `_VariablesVisitor`.
    fn parte(&self, cx: &Contexto<'_>, (a, b): (usize, usize), fid: ast::FunctionId) -> Result<Parte, String> {
        let cxm = self.cxm;
        if a > cx.fonte.len() {
            return Err(format!("RangeError (offset): Invalid value: Not in inclusive range 0..{}: {a}", cx.fonte.len()));
        }
        let prefixo = Texto::novo(cx.fonte).prefixo_da_linha(a).to_string();
        let mut parte = Parte { base: a, fonte: cxm.fonte[a..b].to_string(), prefixo, ..Default::default() };
        // Os nomes dos parâmetros do método (pelo offset do nome).
        let nomes_dos_parametros: Vec<(usize, String)> = cxm
            .ast
            .function(fid)
            .parameters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|q| q.name.map(|n| (n.span.start, cxm.fonte[n.span.start..n.span.end].to_string())))
            .collect();
        let mut pilha = vec![0usize];
        while let Some(k) = pilha.pop() {
            let (ki, kf) = faixa(cxm, k);
            match cxm.especie(k) {
                "SimpleIdentifier" => {
                    if a <= ki && kf <= b {
                        self.qualificador_implicito(k, &mut parte);
                        self.parametro(k, &nomes_dos_parametros, &mut parte);
                        let e = cxm.elemento_do_identificador(k, true);
                        if e == Elem::VariavelLocal
                            && let Marca::Expr(x) = cxm.arvore.nos[k].marca
                            && let Some(d) = cxm.corpos.declaracao_local(x)
                        {
                            adicionar_variavel(&mut parte, d, cxm.texto_do_no(k).to_string(), (ki - a, kf - a));
                        }
                    }
                    continue;
                }
                "ThisExpression" => {
                    if a <= ki && ki <= b {
                        parte.this_explicitos.push(ki - a);
                    }
                    continue;
                }
                "VariableDeclaration" => {
                    let nome = crate::refatoracoes_exec::nome_no_inicio_pub(cxm.texto_do_no(k));
                    if a <= ki && ki + nome.len() <= b {
                        adicionar_variavel(&mut parte, ki, nome.clone(), (ki - a, ki + nome.len() - a));
                    }
                }
                _ => {}
            }
            // `visitNode`: só desce no que intersecta a faixa.
            if k != 0 && (kf <= a || ki >= b) {
                continue;
            }
            pilha.extend(cxm.filhos(k).iter().rev().copied());
        }
        Ok(parte)
    }

    /// `_addMemberQualifier`.
    fn qualificador_implicito(&self, n: usize, parte: &mut Parte) {
        let cxm = self.cxm;
        let prog = cxm.p.programa();
        // `getNodeQualifier`.
        if let Some(pai) = cxm.pai(n) {
            let tem = match cxm.especie(pai) {
                "MethodInvocation" => cxm.nome_do_metodo(pai) == Some(n) && cxm.filhos(pai).first() != Some(&n),
                "PropertyAccess" => cxm.filhos(pai).last() == Some(&n) && cxm.filhos(pai).len() > 1,
                "PrefixedIdentifier" => cxm.filhos(pai).get(1) == Some(&n),
                _ => false,
            };
            if tem {
                return;
            }
        }
        let Marca::Expr(x) = cxm.arvore.nos[n].marca else { return };
        let (classe, estatico): (Option<ClassId>, bool) = match cxm.corpos.get_resolved(x) {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                let fe = prog.function(*f);
                if fe.extension.is_some() || !matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator | FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                    return;
                }
                (fe.class, fe.static_)
            }
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                let ve = prog.variable(*v);
                if ve.extension.is_some() {
                    return;
                }
                (ve.class, ve.static_)
            }
            _ => return,
        };
        let Some(c) = classe else { return };
        let o = cxm.arvore.nos[n].inicio - parte.base;
        if estatico {
            let nome = cxm.p.nome(prog.class(c).name).to_string();
            match parte.classes_implicitas.iter_mut().find(|(k, _)| *k == nome) {
                Some((_, v)) => v.push(o),
                None => parte.classes_implicitas.push((nome, vec![o])),
            }
        } else {
            parte.this_implicitos.push(o);
        }
    }

    /// `_addParameter`.
    fn parametro(&self, n: usize, nomes: &[(usize, String)], parte: &mut Parte) {
        let cxm = self.cxm;
        if cxm.elemento_do_identificador(n, true) != Elem::Parametro {
            return;
        }
        let Marca::Expr(x) = cxm.arvore.nos[n].marca else { return };
        let Some(d) = cxm.corpos.declaracao_local(x) else { return };
        let Some((_, nome)) = nomes.iter().find(|(o, _)| *o == d) else { return };
        let (ki, kf) = faixa(cxm, n);
        let ocorrencia = OcorrenciaDeParametro {
            faixa: (ki - parte.base, kf - parte.base),
            precedencia_do_pai: cxm.precedencia_do_pai(n),
            em_interpolacao: cxm.pai(n).is_some_and(|p| cxm.especie(p) == "InterpolationExpression"),
        };
        match parte.parametros.iter_mut().find(|(k, _)| k == nome) {
            Some((_, v)) => v.push(ocorrencia),
            None => parte.parametros.push((nome.clone(), vec![ocorrencia])),
        }
    }

    /// `_ReferenceProcessor.init` e `_process`.
    #[allow(clippy::too_many_arguments)]
    fn processar(
        &self,
        cxr: &Contexto<'_>,
        uri_r: &str,
        u: UnitId,
        n: usize,
        offset: usize,
        embutir_tudo: bool,
        estado: &mut Estado,
        m: &mut Mudanca,
        ja_assincronos: &mut Vec<(UnitId, usize)>,
    ) -> Result<(), String> {
        let tx = Texto::novo(cxr.fonte);
        // `init`.
        let comando = cxr.com_pais(n).find(|&k| crate::refatoracoes::e_comando_especie(cxr.especie(k)));
        let (linhas_do_ref, prefixo_do_ref) = match comando {
            Some(c) => {
                let (a, b) = faixa(cxr, c);
                (Some(tx.faixa_de_linhas(a, b)), cxr.prefixo_do_no(c))
            }
            None => (None, tx.prefixo_da_linha(cxr.arvore.nos[n].inicio).to_string()),
        };
        // `_shouldProcess`.
        if !embutir_tudo {
            let (a, b) = faixa(cxr, n);
            if !(a <= offset && offset <= b) {
                return Ok(());
            }
        }
        // O método `async` num corpo síncrono.
        if self.assincrono
            && let Some(corpo) = cxr.com_pais(n).find(|&k| matches!(cxr.especie(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"))
        {
            let texto = cxr.texto_do_no(corpo);
            let sincrono = !texto.starts_with("async");
            if sincrono {
                if texto.starts_with("sync") {
                    estado.adicionar(Severidade::Fatal, "Cannot inline async into sync*.");
                    return Ok(());
                }
                let elemento_do_ref = executavel_envolvente(cxr, n);
                if let Some((_, futuro)) = elemento_do_ref
                    && !futuro
                {
                    estado.adicionar(Severidade::Fatal, "Cannot inline async into a function that does not return a Future.");
                    return Ok(());
                }
                let chave = (u, elemento_do_ref.map_or(usize::MAX, |(k, _)| k));
                if !ja_assincronos.contains(&chave) {
                    ja_assincronos.push(chave);
                    let o = cxr.arvore.nos[corpo].inicio;
                    m.adicionar(uri_r, Span { start: o, end: o }, "async ");
                }
            }
        }
        let pai = cxr.pai(n);
        if let Some(inv) = pai.filter(|&k| cxr.especie(k) == "MethodInvocation") {
            let nome = cxr.nome_do_metodo(inv);
            let alvo = cxr.filhos(inv).first().copied().filter(|&a| Some(a) != nome);
            let em_cascata = alvo.is_none() && cxr.pai(inv).is_some_and(|c| cxr.especie(c) == "CascadeExpression");
            let argumentos: Vec<usize> = cxr
                .filhos(inv)
                .iter()
                .copied()
                .find(|&k| cxr.especie(k) == "ArgumentList")
                .map(|l| cxr.filhos(l).to_vec())
                .unwrap_or_default();
            self.embutir_invocacao(cxr, uri_r, n, inv, em_cascata, alvo, &argumentos, linhas_do_ref, &prefixo_do_ref, estado, m);
            return Ok(());
        }
        let prog = cxr.p.programa();
        let e_acessor = match self.elemento {
            Elem::Funcao(f) => matches!(prog.function(f).kind, FunctionKind::Getter | FunctionKind::Setter),
            _ => false,
        };
        if cxr.e_metodo(self.elemento) {
            estado.adicionar(Severidade::Fatal, "Cannot inline class method reference.");
            return Ok(());
        }
        if e_acessor {
            let mut uso = n;
            let mut alvo: Option<usize> = None;
            let mut cascata = false;
            if let Some(pp) = pai {
                match cxr.especie(pp) {
                    "PrefixedIdentifier" => {
                        uso = pp;
                        alvo = cxr.filhos(pp).first().copied();
                    }
                    "PropertyAccess" => {
                        uso = pp;
                        let f = cxr.filhos(pp);
                        if f.len() >= 2 {
                            alvo = f.first().copied();
                        } else {
                            // `realTarget` de uma seção de cascata.
                            alvo = cxr.com_pais(pp).find(|&k| cxr.especie(k) == "CascadeExpression").and_then(|c| cxr.filhos(c).first().copied());
                            cascata = true;
                        }
                    }
                    _ => {}
                }
            }
            let mut argumentos = Vec::new();
            let escrita = match cxr.arvore.nos[n].marca {
                Marca::Expr(x) => cxr.escritas.contains(&x),
                _ => false,
            };
            if escrita
                && let Some(atrib) = cxr.com_pais(n).find(|&k| cxr.especie(k) == "AssignmentExpression")
                && let Some(&lado_direito) = cxr.filhos(atrib).get(1)
            {
                argumentos.push(lado_direito);
            }
            self.embutir_invocacao(cxr, uri_r, n, uso, cascata, alvo, &argumentos, linhas_do_ref, &prefixo_do_ref, estado, m);
            return Ok(());
        }
        // Referência a função (tear-off): a closure.
        let fonte = self.fonte_da_closure(cxr, &prefixo_do_ref);
        let fonte = fonte.strip_suffix(';').unwrap_or(&fonte).to_string();
        let (a, b) = faixa(cxr, n);
        m.adicionar(uri_r, Span { start: a, end: b }, fonte);
        Ok(())
    }

    /// O texto do `(` dos parâmetros ao fim da declaração, reindentado para
    /// o prefixo da referência e aparado.
    fn fonte_da_closure(&self, cxr: &Contexto<'_>, prefixo_do_ref: &str) -> String {
        let cxm = self.cxm;
        let inicio = self.parametros.map_or(faixa(cxm, self.no).1, |l| faixa(cxm, l).0);
        let fim = faixa(cxm, self.no).1;
        let fonte = &cxm.fonte[inicio.min(fim)..fim];
        let prefixo_do_metodo = Texto::novo(cxm.fonte).prefixo_da_linha(faixa(cxm, self.no).0).to_string();
        Texto::novo(cxr.fonte).trocar_recuo(fonte, &prefixo_do_metodo, prefixo_do_ref, false, false).trim().to_string()
    }

    /// `_canInlineBody(usage)`.
    fn pode_embutir_corpo(&self, cxr: &Contexto<'_>, uso: usize) -> bool {
        if self.parte_comandos.is_none() {
            return self.parte_expressao.is_some();
        }
        let pai = cxr.pai(uso);
        let avo = pai.and_then(|p| cxr.pai(p));
        let e = |k: Option<usize>, s: &str| k.is_some_and(|k| cxr.especie(k) == s);
        if let Some(pp) = pai {
            if crate::refatoracoes::e_comando_especie(cxr.especie(pp)) {
                return e(avo, "Block");
            }
            if cxr.especie(pp) == "AssignmentExpression" {
                if cxr.filhos(pp).first() == Some(&uso) {
                    return avo.is_some_and(|a| crate::refatoracoes::e_comando_especie(cxr.especie(a)) && e(cxr.pai(a), "Block"));
                }
                return self.parte_expressao.is_some();
            }
        }
        if self.parte_expressao.is_some() && e(pai, "VariableDeclaration") && e(avo, "VariableDeclarationList") {
            let p3 = avo.and_then(|a| cxr.pai(a));
            return e(p3, "VariableDeclarationStatement") && e(p3.and_then(|x| cxr.pai(x)), "Block");
        }
        false
    }

    /// `_inlineMethodInvocation`.
    #[allow(clippy::too_many_arguments)]
    fn embutir_invocacao(
        &self,
        cxr: &Contexto<'_>,
        uri_r: &str,
        no_ref: usize,
        uso: usize,
        cascata: bool,
        alvo: Option<usize>,
        argumentos: &[usize],
        linhas_do_ref: Option<Span>,
        prefixo_do_ref: &str,
        estado: &mut Estado,
        m: &mut Mudanca,
    ) {
        if cascata {
            estado.adicionar(Severidade::Erro, "Cannot inline cascade invocation.");
        }
        if self.pode_embutir_corpo(cxr, uso) {
            if let Some(parte) = &self.parte_comandos {
                let fonte = self.fonte_para_invocacao(cxr, parte, uso, alvo, argumentos, estado);
                let fonte = Texto::novo(cxr.fonte).trocar_recuo(&fonte, &parte.prefixo, prefixo_do_ref, true, true);
                if let Some(l) = linhas_do_ref {
                    m.adicionar(uri_r, Span { start: l.start, end: l.start }, fonte);
                }
            }
            if let Some(parte) = &self.parte_expressao {
                let mut fonte = self.fonte_para_invocacao(cxr, parte, uso, alvo, argumentos, estado);
                if let Some(x) = self.expressao
                    && self.cxm.precedencia(x) < cxr.precedencia_do_pai(uso)
                {
                    fonte = format!("({fonte})");
                }
                if cxr.pai(uso).is_some_and(|p| cxr.especie(p) == "AwaitExpression") && fonte.starts_with("await") {
                    fonte = fonte.get(6..).unwrap_or("").to_string();
                }
                let (a, b) = faixa(cxr, uso);
                m.adicionar(uri_r, Span { start: a, end: b }, fonte);
            } else if let Some(l) = linhas_do_ref {
                m.adicionar(uri_r, l, "");
            }
            return;
        }
        // Closure imediata: o identificador vira a closure.
        let fonte = self.fonte_da_closure(cxr, prefixo_do_ref);
        let (a, b) = faixa(cxr, no_ref);
        m.adicionar(uri_r, Span { start: a, end: b }, fonte);
    }

    /// `_getMethodSourceForInvocation`.
    fn fonte_para_invocacao(&self, cxr: &Contexto<'_>, parte: &Parte, contexto: usize, alvo: Option<usize>, argumentos: &[usize], estado: &mut Estado) -> String {
        let mut edicoes: Vec<(usize, usize, String)> = Vec::new();
        // O parâmetro de cada argumento (`staticParameterElement.name`).
        let posicionais: Vec<&(String, bool, bool, Option<String>)> = self.declarados.iter().filter(|d| !d.2).collect();
        let mut indice = 0usize;
        let mut por_nome: Vec<(String, usize)> = Vec::new();
        for &a in argumentos {
            if cxr.especie(a) == "NamedExpression" {
                let rotulo = cxr.filhos(a).first().and_then(|&l| cxr.filhos(l).first().copied());
                if let Some(r) = rotulo {
                    por_nome.push((cxr.texto_do_no(r).to_string(), a));
                }
            } else {
                if let Some(d) = posicionais.get(indice) {
                    por_nome.push((d.0.clone(), a));
                }
                indice += 1;
            }
        }
        for (nome, ocorrencias) in &parte.parametros {
            let mut argumento = por_nome.iter().find(|(n, _)| n == nome).map(|(_, a)| *a);
            if let Some(a) = argumento
                && cxr.especie(a) == "NamedExpression"
            {
                argumento = cxr.filhos(a).get(1).copied();
            }
            let (precedencia, fonte, e_identificador) = match argumento {
                Some(a) => (cxr.precedencia(a), cxr.texto_do_no(a).to_string(), cxr.especie(a) == "SimpleIdentifier"),
                None => {
                    let declarado = self.declarados.iter().find(|d| &d.0 == nome);
                    if declarado.is_some_and(|d| d.1) {
                        estado.adicionar(Severidade::Erro, format!("No argument for the parameter \"{nome}\"."));
                        continue;
                    }
                    (0, declarado.and_then(|d| d.3.clone()).unwrap_or_else(|| "null".to_string()), false)
                }
            };
            for o in ocorrencias {
                let texto = if o.em_interpolacao && !e_identificador {
                    format!("{{{fonte}}}")
                } else if precedencia < o.precedencia_do_pai {
                    format!("({fonte})")
                } else {
                    fonte.clone()
                };
                edicoes.push((o.faixa.0, o.faixa.1, texto));
            }
        }
        for (classe, offsets) in &parte.classes_implicitas {
            for &o in offsets {
                edicoes.push((o, o, format!("{classe}.")));
            }
        }
        if let Some(a) = alvo {
            let fonte_do_alvo = cxr.texto_do_no(a).to_string();
            for &o in &parte.this_explicitos {
                edicoes.push((o, o + 4, fonte_do_alvo.clone()));
            }
            for &o in &parte.this_implicitos {
                edicoes.push((o, o, format!("{fonte_do_alvo}.")));
            }
        }
        let conflitos = nomes_em_conflito(cxr, contexto);
        for (_, original, faixas) in &parte.variaveis {
            let mut unico = original.clone();
            let mut k = 2;
            while conflitos.contains(&unico) {
                unico = format!("{original}{k}");
                k += 1;
            }
            if &unico != original {
                for &(a, b) in faixas {
                    edicoes.push((a, b, unico.clone()));
                }
            }
        }
        edicoes.sort_by(|a, b| b.0.cmp(&a.0));
        let mut fonte = parte.fonte.clone();
        for (a, b, t) in edicoes {
            if a <= b && b <= fonte.len() {
                fonte.replace_range(a..b, &t);
            }
        }
        fonte
    }
}

/// `addVariable`.
fn adicionar_variavel(parte: &mut Parte, d: usize, nome: String, faixa: (usize, usize)) {
    match parte.variaveis.iter_mut().find(|(k, _, _)| *k == d) {
        Some((_, _, v)) => v.push(faixa),
        None => parte.variaveis.push((d, nome, vec![faixa])),
    }
}

/// O elemento envolvente da referência (`SearchMatch.element`): o membro de
/// unidade ou de classe executável que a contém, e se o retorno dele é
/// `Future`.
fn executavel_envolvente(cx: &Contexto<'_>, n: usize) -> Option<(usize, bool)> {
    let p = cx.p;
    for k in cx.com_pais(n) {
        let especie = cx.especie(k);
        let local = especie == "FunctionDeclaration" && cx.pai(k).is_some_and(|x| cx.especie(x) == "FunctionDeclarationStatement");
        if matches!(especie, "MethodDeclaration" | "FunctionDeclaration") && !local {
            let Marca::Funcao(fid) = cx.arvore.nos[k].marca else { return None };
            let f = p.funcao_do_no(cx.unidade, fid)?;
            let ret = p.consulta.outline.functions.get(f.0 as usize).map(|d| d.return_type);
            let futuro = ret.is_some_and(|t| matches!(p.consulta.tabela.get(t), Type::Interface { class, .. } if Some(*class) == p.consulta.core.future_class));
            return Some((k, futuro));
        }
        if especie == "ConstructorDeclaration" {
            return Some((k, false));
        }
        if matches!(especie, "FieldDeclaration" | "TopLevelVariableDeclaration") {
            return None;
        }
    }
    None
}

/// `_getNamesConflictingAt(node)`.
fn nomes_em_conflito(cx: &Contexto<'_>, n: usize) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    // `_getLocalsConflictingRange`.
    let (ini, fim) = match cx.com_pais(n).find(|&k| cx.especie(k) == "Block") {
        Some(b) => (cx.arvore.nos[n].inicio, cx.arvore.nos[b].fim),
        None => match cx.com_pais(n).find(|&k| matches!(cx.especie(k), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration")) {
            Some(x) => faixa(cx, x),
            None => (0, 0),
        },
    };
    if let Some(executavel) = cx.com_pais(n).find(|&k| matches!(cx.especie(k), "FunctionDeclaration" | "ConstructorDeclaration" | "MethodDeclaration")) {
        for (nome, (a, b)) in faixas_visiveis(cx, executavel) {
            if !(b <= ini) && !(a >= fim) && !v.contains(&nome) {
                v.push(nome);
            }
        }
    }
    // Os membros da classe envolvente e das superclasses.
    let p = cx.p;
    let prog = p.programa();
    if let Some(decl) = cx.com_pais(n).find(|&k| matches!(cx.especie(k), "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration"))
        && let Marca::Decl(d) = cx.arvore.nos[decl].marca
        && let Some(c) = cx.classe_da_declaracao(cx.unidade, d)
    {
        let mut classes: Vec<ClassId> = p.supertipos(c).into_iter().filter(|&x| x != c).collect();
        classes.sort_by_key(|x| x.0);
        classes.push(c);
        for k in classes {
            let ce = prog.class(k);
            let mut nomes: Vec<String> = Vec::new();
            for (s, _) in ce.instance_members.iter().chain(ce.static_members.iter()) {
                nomes.push(p.nome(*s).trim_end_matches('=').to_string());
            }
            for f in ce.fields.iter() {
                nomes.push(p.nome(prog.variable(*f).name).to_string());
            }
            for (s, _) in ce.constructors.iter() {
                nomes.push(p.nome(*s).to_string());
            }
            for tp in ce.type_params.iter() {
                nomes.push(p.nome(tp.name).to_string());
            }
            for nome in nomes {
                if !v.contains(&nome) {
                    v.push(nome);
                }
            }
        }
    }
    v
}

/// `VisibleRangesComputer.forNode(executavel)`: o nome de cada local e a
/// faixa onde é visível.
pub(crate) fn faixas_visiveis(cx: &Contexto<'_>, raiz: usize) -> Vec<(String, (usize, usize))> {
    let mut v: Vec<(String, (usize, usize))> = Vec::new();
    let nome_de = |t: &str| crate::refatoracoes_exec::nome_no_inicio_pub(t);
    let mut pilha = vec![raiz];
    while let Some(k) = pilha.pop() {
        match cx.especie(k) {
            "CatchClause" => {
                for &f in cx.filhos(k) {
                    if cx.especie(f) == "CatchClauseParameter" {
                        v.push((cx.texto_do_no(f).to_string(), faixa(cx, k)));
                    }
                }
                if let Some(&b) = cx.filhos(k).last() {
                    pilha.push(b);
                }
                continue;
            }
            "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter" => {
                // O corpo da função dona.
                let dona = cx.com_pais(k).find(|&x| matches!(cx.especie(x), "FunctionExpression" | "MethodDeclaration" | "ConstructorDeclaration"));
                let corpo = dona.and_then(|d| cx.filhos(d).iter().copied().find(|&c| matches!(cx.especie(c), "BlockFunctionBody" | "ExpressionFunctionBody")));
                if let Some(c) = corpo {
                    v.push((nome_do_parametro(cx, k), faixa(cx, c)));
                }
                continue;
            }
            "ForPartsWithDeclarations" => {
                if let Some(laco) = cx.pai(k)
                    && let Some(&lista) = cx.filhos(k).first()
                {
                    for &d in cx.filhos(lista) {
                        if cx.especie(d) == "VariableDeclaration" {
                            v.push((nome_de(cx.texto_do_no(d)), faixa(cx, laco)));
                            pilha.extend(cx.filhos(d).iter().copied());
                        }
                    }
                }
                continue;
            }
            "FunctionDeclaration" => {
                if let Some(s) = cx.pai(k)
                    && let Some(b) = cx.pai(s)
                    && cx.especie(b) == "Block"
                    && let Marca::Funcao(fid) = cx.arvore.nos[k].marca
                    && let Some(nome) = cx.ast.function(fid).name
                {
                    v.push((cx.fonte[nome.span.start..nome.span.end].to_string(), faixa(cx, b)));
                }
            }
            "VariableDeclarationStatement" => {
                if let Some(bloco) = cx.pai(k)
                    && let Some(&lista) = cx.filhos(k).iter().find(|&&c| cx.especie(c) == "VariableDeclarationList")
                {
                    for &d in cx.filhos(lista) {
                        if cx.especie(d) == "VariableDeclaration" {
                            v.push((nome_de(cx.texto_do_no(d)), faixa(cx, bloco)));
                            pilha.extend(cx.filhos(d).iter().copied());
                        }
                    }
                }
                continue;
            }
            _ => {}
        }
        pilha.extend(cx.filhos(k).iter().rev().copied());
    }
    v
}

/// O nome declarado de um parâmetro formal (o último identificador antes de
/// `=`, `:` ou `(`).
fn nome_do_parametro(cx: &Contexto<'_>, k: usize) -> String {
    let t = cx.texto_do_no(k);
    let corte = t.find(['=', ':']).unwrap_or(t.len());
    let antes = &t[..corte];
    let antes = match antes.find('(') {
        Some(i) if cx.especie(k) == "FunctionTypedFormalParameter" => &antes[..i],
        _ => antes,
    };
    antes
        .trim_end()
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .next()
        .unwrap_or("")
        .to_string()
}

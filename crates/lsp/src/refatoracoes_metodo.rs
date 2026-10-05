//! Extract Method (`legacy/extract_method.dart`, docs/LSP-ESPECIFICACAO.md
//! §13.11.3): as condições iniciais (`_checkSelection`,
//! `_initializeParameters`, `_initializeReturnType`, ocorrências, getter,
//! nomes), as finais (nome, parâmetros, `validateCreateFunction`/
//! `validateCreateMethod`) e a mudança com `extractAll = false`, com os
//! imports do `addLibraryImports`. O `ExitDetector` do analyzer
//! (`AN:src/dart/resolver/exit_detector.dart`) está portado aqui sobre a
//! árvore no formato do analyzer.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::refatoracoes::{Contexto, Elem, PedidoDeRefatoracao, ResultadoDeRefatoracao, SelecaoDeMetodo};
use crate::refatoracoes_exec::{Estado, Mudanca, Severidade, Texto, UM_RECUO, concluir, nome_das_opcoes, validar_nome};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionKind, LibraryId};
use dartforge_frontend::ast::{self, DeclKind, ExprId};
use dartforge_types::table::Exibicao;
use dartforge_types::{SubtypeEnv, Type, TypeId, TypeTable};
use std::collections::{BTreeSet, HashMap, HashSet};

type R = ResultadoDeRefatoracao;

/// Um parâmetro do método novo (`RefactoringMethodParameter`).
#[derive(Debug, Clone)]
struct Parametro {
    nome: String,
    tipo: String,
    /// Os parâmetros de um parâmetro de tipo função (`(int a)`).
    parametros: Option<String>,
}

/// Uma ocorrência da seleção (`_Occurrence`).
#[derive(Debug, Clone)]
struct Ocorrencia {
    faixa: (usize, usize),
    e_selecao: bool,
    /// Nome na seleção → nome nesta ocorrência.
    nomes: HashMap<String, String>,
}

/// O padrão de um trecho (`_SourcePattern`).
#[derive(Debug, Clone, Default)]
struct Padrao {
    tipos: Vec<Option<TypeId>>,
    normalizado: String,
    /// Nome original → `__refVar<i>`.
    nomes: Vec<(String, String)>,
}

impl Padrao {
    fn compativel(&self, outro: &Padrao, tabela: &TypeTable) -> bool {
        outro.normalizado == self.normalizado
            && outro.tipos.len() == self.tipos.len()
            && self.tipos.iter().zip(outro.tipos.iter()).all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => tabela.canonico(*a) == tabela.canonico(*b),
                (None, None) => true,
                _ => false,
            })
    }
}

/// O estado do `ExtractMethodRefactoringImpl`.
struct Extrair<'c, 'p> {
    cx: &'c Contexto<'p>,
    /// Uma cópia da tabela de tipos (o LUB e o `Future<T>` criam tipos).
    tabela: TypeTable,
    sel: SelecaoDeMetodo,
    /// `_selectionRange`.
    faixa: (usize, usize),
    /// `_parentMember`.
    membro: Option<usize>,
    parametros: Vec<Parametro>,
    /// Pelo id (o nome) do parâmetro.
    referencias: HashMap<String, Vec<(usize, usize)>>,
    nomes_locais: HashMap<String, Vec<(usize, usize)>>,
    nao_qualificados: HashSet<String>,
    tipo_retorno: Option<TypeId>,
    variavel_retorno: Option<String>,
    tem_await: bool,
    tipo_da_variavel: Option<String>,
    tipo_de_retorno: String,
    ocorrencias: Vec<Ocorrencia>,
    contexto_estatico: bool,
    criar_getter: bool,
    nomes: Vec<String>,
    importar: BTreeSet<LibraryId>,
    nome: String,
    /// Uma exceção do Dart (vira `UnhandledError`).
    excecao: Option<String>,
    /// `_getExpectedClosureReturnTypeCode`.
    retorno_da_closure: String,
}

/// `refactor.perform`/`refactor.validate` do `EXTRACT_METHOD`.
pub(crate) fn executar(cx: &Contexto<'_>, uri: &str, pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
    let preferido = match nome_das_opcoes(pedido) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let mut em = Extrair {
        cx,
        tabela: cx.p.consulta.tabela.clone(),
        sel: SelecaoDeMetodo { faixa: (pedido.offset, pedido.offset + pedido.comprimento), expressao: None, closure: None, comandos: Vec::new() },
        faixa: (pedido.offset, pedido.offset + pedido.comprimento),
        membro: None,
        parametros: Vec::new(),
        referencias: HashMap::new(),
        nomes_locais: HashMap::new(),
        nao_qualificados: HashSet::new(),
        tipo_retorno: None,
        variavel_retorno: None,
        tem_await: false,
        tipo_da_variavel: None,
        tipo_de_retorno: String::new(),
        ocorrencias: Vec::new(),
        contexto_estatico: false,
        criar_getter: false,
        nomes: Vec::new(),
        importar: BTreeSet::new(),
        nome: String::new(),
        excecao: None,
        retorno_da_closure: String::new(),
    };
    let iniciais = em.condicoes_iniciais(pedido.offset, pedido.comprimento);
    if let Some(e) = em.excecao.take() {
        return R::ErroInterno(e);
    }
    em.nome = preferido.or_else(|| em.nomes.first().cloned()).unwrap_or_else(|| "newMethod".to_string());
    let em = &em;
    concluir(pedido, iniciais, || em.condicoes_finais(), || em.mudanca(uri))
}

/// `getEnclosingClassOrUnitMember`: o nó logo abaixo da primeira classe,
/// enum, extensão, tipo de extensão, mixin ou unidade, subindo de `n`.
pub(crate) fn membro_envolvente(cx: &Contexto<'_>, n: usize) -> Option<usize> {
    let mut membro = n;
    for k in cx.com_pais(n) {
        if matches!(
            cx.especie(k),
            "ClassDeclaration" | "CompilationUnit" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration"
        ) {
            return Some(membro);
        }
        membro = k;
    }
    None
}

impl<'c, 'p> Extrair<'c, 'p> {
    fn no(&self, n: usize) -> (usize, usize) {
        let no = &self.cx.arvore.nos[n];
        (no.inicio, no.fim)
    }

    fn cobre(&self, n: usize) -> bool {
        let (i, f) = self.no(n);
        self.faixa.0 <= i && f <= self.faixa.1
    }

    /// Os nós em pré-ordem a partir de `raiz` (a ordem dos visitantes).
    fn pre_ordem(&self, raiz: usize) -> Vec<usize> {
        let mut v = Vec::new();
        let mut pilha = vec![raiz];
        while let Some(k) = pilha.pop() {
            v.push(k);
            pilha.extend(self.cx.filhos(k).iter().rev().copied());
        }
        v
    }

    /// O offset do nome declarado do elemento local de um identificador.
    fn declaracao_do_local(&self, n: usize, e: Elem) -> Option<usize> {
        match e {
            Elem::FuncaoLocal(fid) => self.cx.ast.function(fid).name.map(|x| x.span.start),
            Elem::VariavelLocal | Elem::Parametro => match self.cx.arvore.nos[n].marca {
                Marca::Expr(x) => self.cx.corpos.declaracao_local(x),
                _ => None,
            },
            _ => None,
        }
    }

    /// O identificador está em contexto de escrita (`inSetterContext`).
    fn em_escrita(&self, n: usize) -> bool {
        match self.cx.arvore.nos[n].marca {
            Marca::Expr(x) => self.cx.escritas.contains(&x),
            _ => false,
        }
    }

    /// `checkInitialConditions`.
    fn condicoes_iniciais(&mut self, o: usize, l: usize) -> Estado {
        let sel = match self.cx.selecao_de_metodo(o, l) {
            Err(m) => return Estado::fatal(m),
            Ok(s) => s,
        };
        self.faixa = sel.faixa;
        let base = sel.closure.or(sel.expressao).or(sel.comandos.first().copied());
        self.membro = base.and_then(|n| membro_envolvente(self.cx, n));
        self.sel = sel;
        let mut r = self.inicializar_parametros();
        self.tem_await = self.calcular_await();
        self.inicializar_tipo_de_retorno();
        self.inicializar_ocorrencias();
        self.criar_getter = self.pode_criar_getter() && self.sel.expressao.is_some_and(|e| self.expressao_de_getter(e));
        // Os nomes: os locais declarados no membro ficam de fora.
        let excluidos: HashSet<String> = match self.membro {
            Some(m) => self
                .pre_ordem(m)
                .into_iter()
                .filter(|&k| self.cx.especie(k) == "VariableDeclaration" && self.cx.arvore.nos[k].marca == Marca::VariavelLocal)
                .map(|k| nome_no_inicio(self.cx.texto_do_no(k)))
                .collect(),
            None => HashSet::new(),
        };
        if let Some(e) = self.sel.expressao {
            self.nomes = self.cx.nomes_para_expressao(self.cx.tipo_do_no(e), e, &excluidos, true);
        }
        if self.sel.closure.is_some() {
            self.retorno_da_closure = self.retorno_esperado_da_closure();
        }
        if self.sel.closure.is_some() && !self.parametros.is_empty() {
            let nomes: Vec<String> = self.parametros.iter().map(|p| p.nome.clone()).collect();
            let plural = if nomes.len() == 1 { "variable" } else { "variables" };
            return Estado::fatal(format!("Cannot extract the closure as a method,it references the external {plural} {}.", entre_aspas_com_and(&nomes)));
        }
        r
    }

    /// `_initializeParameters`.
    fn inicializar_parametros(&mut self) -> Estado {
        let mut r = Estado::default();
        let mut atribuidas: Vec<(usize, String)> = Vec::new();
        let ordem = self.pre_ordem(0);
        for &n in &ordem {
            match self.cx.especie(n) {
                "SimpleIdentifier" if self.cobre(n) => self.visitar_identificador(n, &mut atribuidas),
                "VariableDeclaration" if self.cobre(n) && self.cx.arvore.nos[n].marca == Marca::VariavelLocal => {
                    let d = self.cx.arvore.nos[n].inicio;
                    let nome = nome_no_inicio(self.cx.texto_do_no(n));
                    if self.usado_depois(d) && !atribuidas.iter().any(|(x, _)| *x == d) {
                        atribuidas.push((d, nome.clone()));
                    }
                    if let Some(faixa) = self.faixa_visivel(n) {
                        self.nomes_locais.entry(nome).or_default().push(faixa);
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = self.sel.expressao {
            self.tipo_retorno = self.cx.tipo_do_no(e);
        }
        let comandos = self.sel.comandos.clone();
        if !comandos.is_empty() {
            let tem_return = comandos.iter().any(|&c| self.pode_terminar_com_return(c));
            if tem_return {
                match Saida::novo(self.cx).sai(*comandos.last().unwrap()) {
                    Ok(true) => {}
                    Ok(false) => r.adicionar(
                        Severidade::Erro,
                        "Selected statements contain a return statement, but not all possible execution flows exit. Semantics may not be preserved.",
                    ),
                    Err(e) => {
                        self.excecao = Some(e);
                        return r;
                    }
                }
            }
            self.tipo_retorno = self.tipo_dos_returns(&comandos);
        }
        if atribuidas.len() == 1 {
            if self.tipo_retorno.is_some() {
                r.adicionar(Severidade::Fatal, "Ambiguous return value: Selected block contains assignment(s) to local variables and return statement.");
                return r;
            }
            let (d, nome) = atribuidas[0].clone();
            self.tipo_retorno = self.cx.corpos.tipo_local(d);
            self.variavel_retorno = Some(nome);
        }
        if atribuidas.len() > 1 {
            let lista: String = atribuidas.iter().map(|(_, n)| format!("{n}\n")).collect();
            r.adicionar(
                Severidade::Fatal,
                format!(
                    "Ambiguous return value: Selected block contains more than one assignment to local variables. Affected variables are:\n\n{}",
                    lista.trim()
                ),
            );
        }
        r
    }

    /// `_InitializeParametersVisitor.visitSimpleIdentifier`.
    fn visitar_identificador(&mut self, n: usize, atribuidas: &mut Vec<(usize, String)>) {
        let cx = self.cx;
        let nome = cx.texto_do_no(n).to_string();
        // O rótulo de um argumento nomeado: o elemento é o parâmetro (local
        // para `isLocalElement`), e o visitante sai.
        if let Some(p) = cx.pai(n)
            && cx.especie(p) == "Label"
            && cx.pai(p).is_some_and(|g| cx.especie(g) == "NamedExpression")
        {
            return;
        }
        let e = cx.elemento_do_identificador(n, false);
        let local = matches!(e, Elem::VariavelLocal | Elem::Parametro | Elem::FuncaoLocal(_));
        if local {
            let Some(d) = self.declaracao_do_local(n, e) else { return };
            if !(self.faixa.0 <= d && d <= self.faixa.1) {
                if !self.parametros.iter().any(|p| p.nome == nome) {
                    let tipo = if self.em_escrita(n) {
                        cx.corpos.tipo_local(d)
                    } else {
                        match cx.arvore.nos[n].marca {
                            Marca::Expr(x) => cx.corpos.get_type(x),
                            _ => None,
                        }
                    };
                    let mut buffer = String::new();
                    let codigo = match tipo {
                        Some(t) => {
                            let nomes = self.nomes_dos_parametros_do_local(e, d);
                            self.fonte_do_tipo(t, Some((&mut buffer, nomes)))
                        }
                        None => Some("dynamic".to_string()),
                    };
                    let Some(codigo) = codigo else { return };
                    self.parametros.push(Parametro { nome: nome.clone(), tipo: codigo, parametros: (!buffer.is_empty()).then_some(buffer) });
                }
                let faixa = self.no(n);
                self.referencias.entry(nome.clone()).or_default().push(faixa);
            }
            if self.em_escrita(n) && self.usado_depois(d) && matches!(e, Elem::VariavelLocal | Elem::Parametro) && !atribuidas.iter().any(|(x, _)| *x == d) {
                atribuidas.push((d, nome.clone()));
            }
        } else if !self.qualificado(n) {
            self.nao_qualificados.insert(nome);
        }
    }

    /// `SimpleIdentifier.isQualified`.
    fn qualificado(&self, n: usize) -> bool {
        let cx = self.cx;
        let Some(p) = cx.pai(n) else { return false };
        match cx.especie(p) {
            "PrefixedIdentifier" => cx.filhos(p).get(1) == Some(&n),
            "PropertyAccess" => cx.filhos(p).last() == Some(&n),
            "ConstructorName" => cx.filhos(p).get(1) == Some(&n),
            "MethodInvocation" => {
                cx.nome_do_metodo(p) == Some(n)
                    && (cx.filhos(p).first() != Some(&n) || cx.pai(p).is_some_and(|c| cx.especie(c) == "CascadeExpression" && cx.filhos(c).first() != Some(&p)))
            }
            _ => false,
        }
    }

    /// `_isUsedAfterSelection`: um identificador do membro com o mesmo
    /// elemento depois da seleção.
    fn usado_depois(&self, d: usize) -> bool {
        let Some(m) = self.membro else { return false };
        let (mi, mf) = self.no(m);
        self.cx.ast.exprs.iter().enumerate().any(|(i, e)| {
            matches!(e.kind, ast::ExprKind::Identifier(_))
                && e.span.start > self.faixa.1
                && mi <= e.span.start
                && e.span.end <= mf
                && self.cx.corpos.declaracao_local(ExprId(i as u32)) == Some(d)
        })
    }

    /// `VisibleRangesComputer`: o bloco que contém a declaração.
    fn faixa_visivel(&self, n: usize) -> Option<(usize, usize)> {
        let b = self.cx.com_pais(n).find(|&k| matches!(self.cx.especie(k), "Block" | "SwitchCase" | "SwitchPatternCase" | "SwitchDefault"))?;
        Some(self.no(b))
    }

    /// `_mayEndWithReturnStatement`: algum `return` fora de corpos de bloco
    /// aninhados.
    fn pode_terminar_com_return(&self, c: usize) -> bool {
        let mut pilha = vec![c];
        while let Some(k) = pilha.pop() {
            match self.cx.especie(k) {
                "ReturnStatement" => return true,
                "BlockFunctionBody" => {}
                _ => pilha.extend(self.cx.filhos(k).iter().copied()),
            }
        }
        false
    }

    /// `_ReturnTypeComputer`: o LUB dos tipos dos `return e;` (sem os de
    /// fundo), fora de corpos de bloco aninhados.
    fn tipo_dos_returns(&mut self, comandos: &[usize]) -> Option<TypeId> {
        let mut tipo: Option<TypeId> = None;
        for &c in comandos {
            let mut ordem = Vec::new();
            let mut pilha = vec![c];
            while let Some(k) = pilha.pop() {
                if self.cx.especie(k) == "BlockFunctionBody" {
                    continue;
                }
                ordem.push(k);
                pilha.extend(self.cx.filhos(k).iter().rev().copied());
            }
            for k in ordem {
                if self.cx.especie(k) != "ReturnStatement" {
                    continue;
                }
                let Some(&e) = self.cx.filhos(k).first() else { continue };
                let Some(t) = self.cx.tipo_do_no(e) else { continue };
                if self.e_fundo(t) {
                    continue;
                }
                tipo = Some(match tipo {
                    None => t,
                    Some(a) => {
                        let mut env = SubtypeEnv::new(&mut self.tabela, &self.cx.p.consulta.outline.hierarchy, &self.cx.p.consulta.core);
                        dartforge_types::lub(a, t, &mut env)
                    }
                });
            }
        }
        tipo
    }

    /// `DartType.isBottom`: `Never` ou parâmetro de tipo limitado por ele.
    fn e_fundo(&self, t: TypeId) -> bool {
        match self.tabela.get(t) {
            Type::Never => true,
            Type::Intersection { bound, .. } => self.e_fundo(*bound),
            _ => false,
        }
    }

    /// `_HasAwaitVisitor`.
    fn calcular_await(&self) -> bool {
        let raizes: Vec<usize> = match (self.sel.expressao, self.sel.comandos.is_empty()) {
            (Some(e), _) => vec![e],
            (None, false) => self.sel.comandos.clone(),
            _ => return false,
        };
        let mut pilha = raizes;
        while let Some(k) = pilha.pop() {
            match self.cx.especie(k) {
                "AwaitExpression" => return true,
                "FunctionExpression" => continue,
                "ForStatement" | "ForElement" if self.cx.texto_do_no(k).starts_with("await") => return true,
                _ => {}
            }
            pilha.extend(self.cx.filhos(k).iter().copied());
        }
        false
    }

    /// `_initializeReturnType`.
    fn inicializar_tipo_de_retorno(&mut self) {
        let core = &self.cx.p.consulta.core;
        if self.sel.closure.is_some() {
            self.tipo_da_variavel = Some(String::new());
            self.tipo_de_retorno = String::new();
            return;
        }
        let Some(t) = self.tipo_retorno else {
            self.tipo_da_variavel = None;
            self.tipo_de_retorno = if self.tem_await {
                let fv = self.futuro(core.void_);
                self.fonte_do_tipo(fv, None).unwrap_or_default()
            } else {
                "void".to_string()
            };
            return;
        };
        if matches!(self.tabela.get(t), Type::Dynamic) && !matches!(self.tabela.exibicao(t), Some(Exibicao::Invalido)) {
            self.tipo_da_variavel = Some(String::new());
            self.tipo_de_retorno = if self.tem_await {
                let fd = self.futuro(core.dynamic_);
                self.fonte_do_tipo(fd, None).unwrap_or_default()
            } else {
                String::new()
            };
            return;
        }
        let codigo = self.fonte_do_tipo(t, None).unwrap_or_default();
        self.tipo_da_variavel = Some(codigo.clone());
        if self.tem_await {
            let interface_nao_futuro = match self.tabela.get(t) {
                Type::Interface { class, .. } => Some(*class) != core.future_class,
                Type::FutureOr { .. } | Type::ExtensionType { .. } => true,
                _ => false,
            };
            if interface_nao_futuro {
                let f = self.futuro(t);
                self.tipo_de_retorno = self.fonte_do_tipo(f, None).unwrap_or_default();
            }
        } else {
            self.tipo_de_retorno = codigo;
        }
    }

    /// `Future<t>`.
    fn futuro(&mut self, t: TypeId) -> TypeId {
        match self.cx.p.consulta.core.future_class {
            Some(c) => self.tabela.intern(Type::Interface { class: c, args: vec![t].into_boxed_slice(), nullable: false }),
            None => t,
        }
    }

    /// `_computeCanCreateGetter`.
    fn pode_criar_getter(&self) -> bool {
        if self.sel.closure.is_some() || !self.parametros.is_empty() {
            return false;
        }
        if let Some(e) = self.sel.expressao
            && self.cx.especie(e) == "AssignmentExpression"
        {
            return false;
        }
        if !self.sel.comandos.is_empty() {
            return self.tipo_de_retorno != "void";
        }
        true
    }

    /// `_isExpressionForGetter`.
    fn expressao_de_getter(&self, e: usize) -> bool {
        let cx = self.cx;
        match cx.especie(e) {
            "BinaryExpression" => cx.filhos(e).iter().all(|&f| self.expressao_de_getter(f)),
            "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" => true,
            "PrefixExpression" => cx.filhos(e).first().is_some_and(|&f| self.expressao_de_getter(f)),
            "PrefixedIdentifier" | "PropertyAccess" => cx.filhos(e).first().is_some_and(|&f| self.expressao_de_getter(f)),
            "SimpleIdentifier" => true,
            _ => false,
        }
    }

    // -- Ocorrências -------------------------------------------------------------

    /// `_getSourcePattern(faixa)`.
    fn padrao(&self, (ini, fim): (usize, usize)) -> Padrao {
        let cx = self.cx;
        let mut p = Padrao::default();
        let mut trocas: Vec<(usize, usize, String)> = Vec::new();
        let mut adicionar = |p: &mut Padrao, ni: usize, nf: usize, nome: String, tipo: Option<TypeId>| {
            if ini <= ni && nf <= fim {
                let padrao = match p.nomes.iter().find(|(o, _)| *o == nome) {
                    Some((_, x)) => x.clone(),
                    None => {
                        p.tipos.push(tipo);
                        let x = format!("__refVar{}", p.nomes.len());
                        p.nomes.push((nome.clone(), x.clone()));
                        x
                    }
                };
                trocas.push((ni - ini, nf - ini, padrao));
            }
        };
        // `_GetSourcePatternVisitor` (o `NamedExpression` só visita a
        // expressão).
        let mut pilha = vec![0usize];
        while let Some(k) = pilha.pop() {
            let (ki, kf) = self.no(k);
            if kf < ini || ki > fim {
                continue;
            }
            match cx.especie(k) {
                "SimpleIdentifier" => {
                    let e = cx.elemento_do_identificador(k, true);
                    if matches!(e, Elem::VariavelLocal | Elem::Parametro | Elem::FuncaoLocal(_))
                        && let Some(d) = self.declaracao_do_local(k, e)
                    {
                        adicionar(&mut p, ki, kf, cx.texto_do_no(k).to_string(), self.tipo_do_elemento(e, d));
                    }
                }
                "VariableDeclaration" if cx.arvore.nos[k].marca == Marca::VariavelLocal => {
                    let nome = nome_no_inicio(cx.texto_do_no(k));
                    adicionar(&mut p, ki, ki + nome.len(), nome, cx.corpos.tipo_local(ki));
                }
                _ => {}
            }
            let filhos = cx.filhos(k);
            if cx.especie(k) == "NamedExpression" {
                if let Some(&x) = filhos.get(1) {
                    pilha.push(x);
                }
            } else {
                pilha.extend(filhos.iter().rev().copied());
            }
        }
        let mut texto = cx.fonte[ini..fim].to_string();
        trocas.sort_by(|a, b| b.0.cmp(&a.0));
        for (a, b, x) in trocas {
            texto.replace_range(a..b, &x);
        }
        p.normalizado = match dartforge_frontend::lexer::lex(&texto) {
            Ok(v) => v
                .iter()
                .filter(|t| t.kind != dartforge_frontend::token::Kind::Eof)
                .map(|t| texto[t.span.start..t.span.end].to_string())
                .collect::<Vec<_>>()
                .join("\u{FFFF}"),
            Err(_) => String::new(),
        };
        p
    }

    /// O tipo declarado de um elemento local (`VariableElement.type` ou
    /// `FunctionElement.type`).
    fn tipo_do_elemento(&self, e: Elem, d: usize) -> Option<TypeId> {
        match e {
            Elem::FuncaoLocal(fid) => self.cx.corpos.tipo_local(d).or_else(|| self.cx.corpos.tipo_de_execucao_de_funcao(fid)),
            _ => self.cx.corpos.tipo_local(d),
        }
    }

    /// `_initializeOccurrences` com o `_InitializeOccurrencesVisitor`.
    fn inicializar_ocorrencias(&mut self) {
        self.ocorrencias.clear();
        let Some(m) = self.membro else { return };
        let Some(pai) = self.cx.pai(m) else { return };
        let padrao_da_selecao = self.padrao(self.faixa);
        let mut estatico = false;
        self.visitar_ocorrencias(pai, &padrao_da_selecao, &mut estatico);
    }

    fn visitar_ocorrencias(&mut self, n: usize, padrao: &Padrao, forcar: &mut bool) {
        let cx = self.cx;
        let especie = cx.especie(n);
        let anterior = *forcar;
        match especie {
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => *forcar = true,
            "FieldDeclaration" | "MethodDeclaration" => *forcar = self.declaracao_estatica(n),
            _ => {}
        }
        if especie == "Block" && !self.sel.comandos.is_empty() {
            let cmds: Vec<usize> = cx.filhos(n).to_vec();
            self.janelas(&cmds, padrao, *forcar);
        }
        if matches!(especie, "SwitchCase" | "SwitchDefault" | "SwitchPatternCase") && !self.sel.comandos.is_empty() {
            let cmds: Vec<usize> = cx.filhos(n).iter().copied().filter(|&k| crate::refatoracoes::e_comando_especie(cx.especie(k))).collect();
            self.janelas(&cmds, padrao, *forcar);
        }
        if crate::refatoracoes::e_expressao_especie(especie)
            && (self.sel.closure.is_some() || self.sel.expressao.is_some_and(|e| cx.especie(e) == especie))
        {
            let faixa = self.no(n);
            self.tentar_ocorrencia(faixa, padrao, *forcar);
        }
        for f in cx.filhos(n).to_vec() {
            self.visitar_ocorrencias(f, padrao, forcar);
        }
        // O `finally` do visitante zera (não restaura) a marca.
        if matches!(
            especie,
            "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" | "FieldDeclaration" | "MethodDeclaration"
        ) {
            *forcar = false;
        } else {
            let _ = anterior;
        }
    }

    /// O membro é `static`.
    fn declaracao_estatica(&self, n: usize) -> bool {
        let t = self.cx.texto_do_no(n);
        // Depois do comentário e das anotações: o primeiro token é `static`?
        let (ini, _) = self.no(n);
        let inicio_real = self
            .cx
            .filhos(n)
            .iter()
            .filter(|&&f| matches!(self.cx.especie(f), "Comment" | "Annotation"))
            .map(|&f| self.no(f).1)
            .max()
            .unwrap_or(ini);
        let resto = self.cx.fonte[inicio_real..ini + t.len()].trim_start();
        resto.starts_with("static") && !resto[6..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
    }

    /// `_visitStatements`: janelas de `n` comandos.
    fn janelas(&mut self, comandos: &[usize], padrao: &Padrao, forcar: bool) {
        let n = self.sel.comandos.len();
        let mut i = 0;
        while i + n <= comandos.len() {
            let faixa = (self.no(comandos[i]).0, self.no(comandos[i + n - 1]).1);
            if self.tentar_ocorrencia(faixa, padrao, forcar) {
                i += n;
            } else {
                i += 1;
            }
        }
    }

    /// `_tryToFindOccurrence`.
    fn tentar_ocorrencia(&mut self, faixa: (usize, usize), padrao: &Padrao, forcar: bool) -> bool {
        if self.cx.analisar(faixa.0, faixa.1).fatal.is_some() {
            return false;
        }
        let p = self.padrao(faixa);
        if !padrao.compativel(&p, &self.tabela) {
            return false;
        }
        let e_selecao = !(faixa.1 <= self.faixa.0) && !(faixa.0 >= self.faixa.1);
        let mut nomes = HashMap::new();
        for (original, x) in &p.nomes {
            if let Some((da_selecao, _)) = padrao.nomes.iter().find(|(_, y)| y == x) {
                nomes.insert(da_selecao.clone(), original.clone());
            }
        }
        self.ocorrencias.push(Ocorrencia { faixa, e_selecao, nomes });
        if forcar {
            self.contexto_estatico = true;
        }
        true
    }

    // -- Tipos -------------------------------------------------------------------

    /// Os nomes dos parâmetros de um tipo de função de um local (os do
    /// `FunctionType` do analyzer): os da função local, os da closure que o
    /// inicializa, ou os do tipo escrito.
    fn nomes_dos_parametros_do_local(&self, e: Elem, d: usize) -> Vec<String> {
        let ast = self.cx.ast;
        let fonte = self.cx.fonte;
        let nomes_de = |ps: &[ast::Parameter]| ps.iter().map(|p| p.name.map(|n| fonte[n.span.start..n.span.end].to_string()).unwrap_or_default()).collect();
        if let Elem::FuncaoLocal(fid) = e {
            return ast.function(fid).parameters.as_deref().map(nomes_de).unwrap_or_default();
        }
        // A declaração: um parâmetro com tipo escrito, ou uma variável.
        if let Some((p, _)) = crate::projeto::parametro_em(ast, d) {
            if let Some(t) = p.ty
                && let ast::TypeKind::Function { parameters, .. } = &ast.ty(t).kind
            {
                return nomes_de(parameters);
            }
            if let Some(fs) = &p.function_parameters {
                return nomes_de(fs);
            }
            return Vec::new();
        }
        for s in ast.stmts.iter() {
            if let ast::StmtKind::Variables(l) = &s.kind
                && let Some(v) = l.variables.iter().find(|v| v.name.span.start == d)
            {
                if let Some(t) = l.ty
                    && let ast::TypeKind::Function { parameters, .. } = &ast.ty(t).kind
                {
                    return nomes_de(parameters);
                }
                if l.ty.is_none()
                    && let Some(i) = v.initializer
                    && let ast::ExprKind::FunctionExpression(f) = ast.expr(i).kind
                {
                    return ast.function(f).parameters.as_deref().map(nomes_de).unwrap_or_default();
                }
            }
        }
        Vec::new()
    }

    /// `LibraryElement.getTypeSource(type, librariesToImport,
    /// parametersBuffer)`.
    fn fonte_do_tipo(&mut self, t: TypeId, buffer: Option<(&mut String, Vec<String>)>) -> Option<String> {
        if let Some(Exibicao::Alias { typedef, args }) = self.tabela.exibicao(t).cloned() {
            let nulo = self.tabela.get(t).is_declared_nullable();
            return self.fonte_com_argumentos(Element::Typedef(typedef), nulo, &args);
        }
        if matches!(self.tabela.exibicao(t), Some(Exibicao::Invalido)) {
            return Some("dynamic".to_string());
        }
        if matches!(self.tabela.exibicao(t), Some(Exibicao::NeverAnulavel)) {
            return Some("Never".to_string());
        }
        match self.tabela.get(t).clone() {
            Type::Dynamic => Some("dynamic".to_string()),
            Type::Function { ret, positional, optional, named, .. } => {
                let Some((buf, nomes)) = buffer else { return Some("Function".to_string()) };
                buf.push('(');
                let mut i = 0usize;
                let mut escrever = |this: &mut Self, buf: &mut String, tipo: TypeId, nome: &str| {
                    let fonte = this.fonte_do_tipo(tipo, None);
                    if buf.len() != 1 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&fonte.unwrap_or_else(|| "null".to_string()));
                    buf.push(' ');
                    buf.push_str(nome);
                };
                for &p in positional.iter().chain(optional.iter()) {
                    let nome = nomes.get(i).cloned().unwrap_or_default();
                    escrever(self, buf, p, &nome);
                    i += 1;
                }
                let mut nomeados: Vec<(String, TypeId)> = named.iter().map(|(n, ty, _)| (self.cx.p.nome(*n).to_string(), *ty)).collect();
                nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                for (n, ty) in nomeados {
                    escrever(self, buf, ty, &n);
                }
                buf.push(')');
                self.fonte_do_tipo(ret, None)
            }
            Type::Interface { class, args, nullable } | Type::ExtensionType { decl: class, args, nullable } => {
                self.fonte_com_argumentos(Element::Class(class), nullable, &args)
            }
            Type::FutureOr { arg, nullable } => match self.classe_future_or() {
                Some(c) => self.fonte_com_argumentos(Element::Class(c), nullable, &[arg]),
                None => Some("FutureOr".to_string()),
            },
            Type::Null => {
                let core = &self.cx.p.consulta.core;
                match core.null_class {
                    Some(c) => self.fonte_com_argumentos(Element::Class(c), false, &[]),
                    None => Some("Null".to_string()),
                }
            }
            Type::Never => Some("Never".to_string()),
            Type::Record { positional, named, nullable } => {
                let mut s = String::from("(");
                let total = positional.len() + named.len();
                let mut i = 0usize;
                for &p in positional.iter() {
                    s.push_str(&self.fonte_do_tipo(p, None).unwrap_or_else(|| "null".to_string()));
                    if i + 1 < total {
                        s.push_str(", ");
                    }
                    i += 1;
                }
                if !named.is_empty() {
                    s.push('{');
                    let mut nomeados: Vec<(String, TypeId)> = named.iter().map(|(n, ty)| (self.cx.p.nome(*n).to_string(), *ty)).collect();
                    nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                    for (n, ty) in nomeados {
                        s.push_str(&self.fonte_do_tipo(ty, None).unwrap_or_else(|| "null".to_string()));
                        s.push(' ');
                        s.push_str(&n);
                        if i + 1 < total {
                            s.push_str(", ");
                        }
                        i += 1;
                    }
                    s.push('}');
                }
                s.push(')');
                if nullable {
                    s.push('?');
                }
                Some(s)
            }
            Type::TypeParameter { .. } | Type::Intersection { .. } => Some("dynamic".to_string()),
            Type::Void => Some("void".to_string()),
        }
    }

    /// A classe `FutureOr` de `dart:async`.
    fn classe_future_or(&self) -> Option<ClassId> {
        let p = self.cx.p;
        let prog = p.programa();
        let lib = p.consulta.core.async_library?;
        prog.library(lib).exported.iter().find(|(s, _)| p.nome(**s) == "FutureOr").and_then(|(_, b)| match b.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        })
    }

    /// `_getTypeCodeElementArguments`.
    fn fonte_com_argumentos(&mut self, el: Element, nulo: bool, args: &[TypeId]) -> Option<String> {
        let p = self.cx.p;
        let prog = p.programa();
        let atual = prog.unit(self.cx.unidade).library;
        let (biblioteca, nome) = match el {
            Element::Class(c) => (prog.class(c).library, p.nome(prog.class(c).name).to_string()),
            Element::Typedef(t) => (prog.typedef(t).library, p.nome(prog.typedef(t).name).to_string()),
            _ => return None,
        };
        let mut s = String::new();
        if biblioteca != atual {
            if nome.starts_with('_') {
                return None;
            }
            match self.import_do_elemento(atual, el, &nome) {
                Some(Some(prefixo)) => {
                    s.push_str(&prefixo);
                    s.push('.');
                }
                Some(None) => {}
                None => {
                    self.importar.insert(biblioteca);
                }
            }
        }
        s.push_str(&nome);
        if !args.is_empty() {
            s.push('<');
            for (i, &a) in args.iter().enumerate() {
                if i != 0 {
                    s.push_str(", ");
                }
                s.push_str(&self.fonte_do_tipo(a, None)?);
            }
            s.push('>');
        }
        if nulo {
            s.push('?');
        }
        Some(s)
    }

    /// `_getImportElement`: o primeiro import da unidade definidora cujo
    /// espaço de nomes tem o elemento: `Some(prefixo)`; `None` sem import.
    fn import_do_elemento(&self, atual: LibraryId, el: Element, nome: &str) -> Option<Option<String>> {
        let p = self.cx.p;
        let prog = p.programa();
        let lib = prog.library(atual);
        let definidora = lib.units.first().copied();
        let simbolo = prog.library(match el {
            Element::Class(c) => prog.class(c).library,
            Element::Typedef(t) => prog.typedef(t).library,
            _ => return None,
        });
        let _ = simbolo;
        let mut core_explicito = false;
        for imp in lib.imports.iter().filter(|i| Some(i.unit) == definidora) {
            let alvo = prog.library(imp.library);
            if alvo.uri == "dart:core" {
                core_explicito = true;
            }
            let Some(b) = alvo.exported.iter().find(|(s, _)| p.nome(**s) == nome).map(|(_, b)| *b) else { continue };
            if b.getter != Some(el) {
                continue;
            }
            // Os combinadores.
            let visivel = imp.combinators.iter().all(|c| match c {
                ast::Combinator::Show(ns) => ns.iter().any(|n| p.nome(n.sym) == nome),
                ast::Combinator::Hide(ns) => !ns.iter().any(|n| p.nome(n.sym) == nome),
            });
            if !visivel {
                continue;
            }
            return Some(imp.prefix.map(|x| p.nome(x).to_string()));
        }
        // O import implícito de `dart:core`.
        let biblioteca = match el {
            Element::Class(c) => prog.class(c).library,
            Element::Typedef(t) => prog.typedef(t).library,
            _ => return None,
        };
        if !core_explicito && prog.library(biblioteca).uri == "dart:core" {
            return Some(None);
        }
        None
    }

    // -- Condições finais ----------------------------------------------------------

    /// `checkFinalConditions`.
    fn condicoes_finais(&self) -> Estado {
        let mut r = validar_nome(&self.nome, "Method");
        r.somar(self.verificar_nomes_de_parametros());
        r.somar(self.conflitos_possiveis());
        r
    }

    /// `_checkParameterNames`.
    fn verificar_nomes_de_parametros(&self) -> Estado {
        let mut r = Estado::default();
        for (i, p) in self.parametros.iter().enumerate() {
            r.somar(validar_nome(&p.nome, "Parameter"));
            if self.parametros.iter().enumerate().any(|(j, o)| j != i && o.nome == p.nome) {
                r.adicionar(Severidade::Erro, format!("Parameter '{}' already exists", p.nome));
                return r;
            }
            let refs = self.referencias.get(&p.nome).map(|v| &v[..]).unwrap_or(&[]);
            let outros = self.nomes_locais.get(&p.nome).map(|v| &v[..]).unwrap_or(&[]);
            let cruza = refs.iter().any(|a| outros.iter().any(|b| !(a.1 <= b.0) && !(a.0 >= b.1)));
            if cruza || self.nao_qualificados.contains(&p.nome) {
                r.adicionar(Severidade::Erro, format!("'{}' is already used as a name in the selected code", p.nome));
                return r;
            }
        }
        r
    }

    /// `_checkPossibleConflicts`.
    fn conflitos_possiveis(&self) -> Estado {
        let cx = self.cx;
        let Some(m) = self.membro else { return Estado::default() };
        let Some(pai) = cx.pai(m) else { return Estado::default() };
        match cx.especie(pai) {
            "CompilationUnit" => self.validar_criar_funcao(),
            "ClassDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration" => {
                let Marca::Decl(d) = cx.arvore.nos[pai].marca else { return Estado::default() };
                match cx.classe_da_declaracao(cx.unidade, d) {
                    Some(c) => self.validar_criar_metodo(c, cx.especie(pai)),
                    None => Estado::default(),
                }
            }
            _ => Estado::default(),
        }
    }

    /// `validateCreateFunction`: `_validateWillConflict` e
    /// `_validateWillShadow(null)`.
    fn validar_criar_funcao(&self) -> Estado {
        let p = self.cx.p;
        let prog = p.programa();
        let lib = prog.unit(self.cx.unidade).library;
        let mut r = Estado::default();
        // Os elementos de topo de cada unidade, na ordem de `children`:
        // acessores, classes, enums, extensões, tipos de extensão, funções,
        // mixins, aliases e variáveis.
        for &u in prog.library(lib).units.iter() {
            for (especie, nome) in elementos_de_topo(self.cx, u) {
                if nome == self.nome {
                    r.adicionar(Severidade::Erro, format!("Library already declares {especie} with name '{}'.", self.nome));
                }
            }
        }
        r.somar(self.sombreamentos("function"));
        r
    }

    /// `_validateWillShadow(null)`: cada membro de classe com o nome, com
    /// uma referência não qualificada de outra classe.
    fn sombreamentos(&self, especie_criada: &str) -> Estado {
        let p = self.cx.p;
        let prog = p.programa();
        let mut r = Estado::default();
        for (i, c) in prog.classes.iter().enumerate() {
            let classe = ClassId(i as u32);
            for (&s, &f) in c.instance_members.iter().chain(c.static_members.iter()) {
                let Some((especie, qualificado, funcoes)) = self.declaracao_de_membro(c, s, f) else { continue };
                if self.referencia_nao_qualificada_de_outra_classe(&funcoes, classe) {
                    r.adicionar(Severidade::Erro, format!("Created {especie_criada} will shadow {especie} '{qualificado}'."));
                }
            }
        }
        r
    }

    /// O membro `s` → `f` da classe `c` é uma declaração com o nome
    /// procurado (`searchMemberDeclarations`: acessores explícitos, campos e
    /// métodos): a espécie, o nome qualificado e as funções cujas
    /// referências contam.
    fn declaracao_de_membro(
        &self,
        c: &dartforge_elements::model::ClassElement,
        s: dartforge_intern::SymbolId,
        f: dartforge_elements::model::FunctionElementId,
    ) -> Option<(&'static str, String, Vec<dartforge_elements::model::FunctionElementId>)> {
        let p = self.cx.p;
        let prog = p.programa();
        let chave = p.nome(s);
        if chave.trim_end_matches('=') != self.nome {
            return None;
        }
        let fe = prog.function(f);
        let qualificado = format!("{}.{}", p.nome(c.name), self.nome);
        match fe.kind {
            FunctionKind::Function | FunctionKind::Operator => Some(("method", qualificado, vec![f])),
            FunctionKind::ImplicitAccessor if !chave.ends_with('=') => {
                // O campo: as referências do getter e do setter.
                let mut fs = vec![f];
                for (&s2, &g) in c.instance_members.iter().chain(c.static_members.iter()) {
                    if p.nome(s2) == format!("{chave}=") {
                        fs.push(g);
                    }
                }
                Some(("field", qualificado, fs))
            }
            FunctionKind::Getter => Some(("getter", self.nome.clone(), vec![f])),
            FunctionKind::Setter => Some(("setter", self.nome.clone(), vec![f])),
            _ => None,
        }
    }

    /// Alguma referência sem qualificação a uma das funções, feita fora da
    /// classe dona.
    fn referencia_nao_qualificada_de_outra_classe(&self, funcoes: &[dartforge_elements::model::FunctionElementId], dona: ClassId) -> bool {
        let p = self.cx.p;
        let prog = p.programa();
        for &lib in p.bibliotecas.iter() {
            for &u in prog.library(lib).units.iter() {
                let unidade = prog.unit(u);
                let corpos = &p.consulta.corpos.units[u.0 as usize];
                for (i, e) in unidade.ast.exprs.iter().enumerate() {
                    let ast::ExprKind::Identifier(n) = &e.kind else { continue };
                    let alvo = match corpos.get_resolved(ExprId(i as u32)) {
                        Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(g), .. }) => *g,
                        _ => continue,
                    };
                    if !funcoes.contains(&alvo) {
                        continue;
                    }
                    // A classe que contém a referência.
                    let dentro = prog.classes.iter().enumerate().find(|(_, c)| {
                        c.decl.is_some_and(|d| d.unit == u && {
                            let s = unidade.ast.decl(d.decl).span;
                            s.start <= n.span.start && n.span.end <= s.end
                        })
                    });
                    if dentro.map(|(j, _)| ClassId(j as u32)) != Some(dona) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `validateCreateMethod`.
    fn validar_criar_metodo(&self, c: ClassId, especie_no: &str) -> Estado {
        let p = self.cx.p;
        let prog = p.programa();
        let classe = prog.class(c);
        let mut r = Estado::default();
        let especie_da_classe = match especie_no {
            "EnumDeclaration" => "enum",
            "MixinDeclaration" => "mixin",
            "ExtensionTypeDeclaration" => "extension type",
            _ => "class",
        };
        let nome_da_classe = p.nome(classe.name).to_string();
        // `_checkClassAlreadyDeclares`: acessores, campos, construtores,
        // métodos, parâmetros de tipo.
        let mut filhos: Vec<&'static str> = Vec::new();
        let mut membros: Vec<(&dartforge_intern::SymbolId, &dartforge_elements::model::FunctionElementId)> =
            classe.instance_members.iter().chain(classe.static_members.iter()).collect();
        membros.sort_by_key(|(_, f)| f.0);
        for (s, f) in &membros {
            let fe = prog.function(**f);
            if p.nome(**s).trim_end_matches('=') == self.nome && matches!(fe.kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                filhos.push(if fe.kind == FunctionKind::Setter || p.nome(**s).ends_with('=') { "setter" } else { "getter" });
            }
        }
        for v in classe.fields.iter() {
            if p.nome(prog.variable(*v).name) == self.nome {
                filhos.push("field");
            }
        }
        for (s, _) in classe.constructors.iter() {
            if p.nome(*s) == self.nome {
                filhos.push("constructor");
            }
        }
        for (s, f) in &membros {
            let fe = prog.function(**f);
            if p.nome(**s) == self.nome && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                filhos.push("method");
            }
        }
        for tp in classe.type_params.iter() {
            if p.nome(tp.name) == self.nome {
                filhos.push("type parameter");
            }
        }
        let capital = {
            let mut c = especie_da_classe.chars();
            c.next().map(|x| x.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        for f in filhos {
            r.adicionar(Severidade::Erro, format!("{capital} '{nome_da_classe}' already declares {f} with name '{}'.", self.nome));
        }
        if nome_da_classe == self.nome {
            r.adicionar(Severidade::Erro, format!("Created method has the same name as the declaring {especie_da_classe} '{}'.", self.nome));
        }
        // `_checkHierarchy(isRename: false)`: um membro de superclasse com o
        // nome.
        let supers = self.cx.p.supertipos(c);
        for (i, k) in prog.classes.iter().enumerate() {
            let id = ClassId(i as u32);
            if !supers.contains(&id) || id == c {
                continue;
            }
            for (&s, &f) in k.instance_members.iter().chain(k.static_members.iter()) {
                let Some((especie, qualificado, _)) = self.declaracao_de_membro(k, s, f) else { continue };
                r.adicionar(Severidade::Erro, format!("Created method will shadow {especie} '{qualificado}'."));
            }
        }
        r
    }

    // -- A mudança -------------------------------------------------------------------

    /// A assinatura (`signature`).
    fn assinatura(&self) -> String {
        if self.criar_getter {
            return format!("get {}", self.nome);
        }
        let mut s = format!("{}(", self.nome);
        for (i, p) in self.parametros.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            if p.tipo != "dynamic" && !p.tipo.is_empty() {
                s.push_str(&p.tipo);
                s.push(' ');
            }
            s.push_str(&p.nome);
            if let Some(ps) = &p.parametros {
                s.push_str(ps);
            }
        }
        s.push(')');
        s
    }

    /// `_getMethodBodySource`.
    fn corpo(&self) -> String {
        let cx = self.cx;
        let tx = Texto::novo(cx.fonte);
        let (ini, fim) = self.faixa;
        let mut fonte = cx.fonte[ini..fim].to_string();
        let mut trocas: Vec<(usize, usize, String)> = Vec::new();
        for p in &self.parametros {
            for &(a, b) in self.referencias.get(&p.nome).map(|v| &v[..]).unwrap_or(&[]) {
                trocas.push((a - ini, b - ini, p.nome.clone()));
            }
        }
        trocas.sort_by(|a, b| b.0.cmp(&a.0));
        for (a, b, t) in trocas {
            fonte.replace_range(a..b, &t);
        }
        if let Some(f) = self.sel.closure
            && let Some(base) = cx.com_pais(f).find(|&k| crate::refatoracoes::e_comando_especie(cx.especie(k)))
        {
            let recuo_base = cx.prefixo_do_no(base);
            let recuo_alvo = self.membro.map(|m| cx.prefixo_do_no(m)).unwrap_or_default();
            fonte = tx.trocar_recuo(&fonte, &recuo_base, &recuo_alvo, false, false).trim().to_string();
        }
        if let Some(&primeiro) = self.sel.comandos.first() {
            let recuo = cx.prefixo_do_no(primeiro);
            let alvo = format!("{}{UM_RECUO}", self.membro.map(|m| cx.prefixo_do_no(m)).unwrap_or_default());
            fonte = tx.trocar_recuo(&fonte, &recuo, &alvo, true, true);
        }
        fonte
    }

    /// `_getExpectedClosureReturnTypeCode`: o retorno do tipo do parâmetro
    /// que recebe a closure, se não `dynamic`.
    fn retorno_esperado_da_closure(&mut self) -> String {
        let cx = self.cx;
        let Some(f) = self.sel.closure else { return String::new() };
        let argumento = match cx.pai(f) {
            Some(p) if cx.especie(p) == "NamedExpression" => p,
            _ => f,
        };
        let Some(lista) = cx.pai(argumento).filter(|&l| cx.especie(l) == "ArgumentList") else { return String::new() };
        let Some(dono) = cx.pai(lista) else { return String::new() };
        let funcao = match cx.especie(dono) {
            "MethodInvocation" => match cx.nome_do_metodo(dono).map(|m| cx.elemento_do_identificador(m, true)) {
                Some(Elem::Funcao(f)) => f,
                _ => return String::new(),
            },
            "InstanceCreationExpression" => match cx.expr_do_no(dono).and_then(|x| cx.corpos.get_resolved(x)) {
                Some(dartforge_types::Resolved::Constructor(f)) => *f,
                _ => return String::new(),
            },
            _ => return String::new(),
        };
        let Some(dados) = cx.p.consulta.outline.functions.get(funcao.0 as usize) else { return String::new() };
        let parametro = if cx.especie(argumento) == "NamedExpression" {
            let rotulo = cx.filhos(argumento).first().and_then(|&l| cx.filhos(l).first().copied());
            let nome = rotulo.map(|r| cx.texto_do_no(r).to_string()).unwrap_or_default();
            dados.parameters.iter().find(|p| p.kind == ast::ParameterKind::Named && p.name.is_some_and(|n| cx.p.nome(n) == nome))
        } else {
            let indice = cx.filhos(lista).iter().filter(|&&k| cx.especie(k) != "NamedExpression").position(|&k| k == argumento);
            indice.and_then(|i| dados.parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named).nth(i))
        };
        let Some(parametro) = parametro else { return String::new() };
        let tabela = &cx.p.consulta.tabela;
        let Type::Function { ret, .. } = tabela.get(parametro.ty) else { return String::new() };
        let ret = *ret;
        match self.fonte_do_tipo(ret, None) {
            Some(t) if t != "dynamic" => format!("{t} "),
            _ => String::new(),
        }
    }

    /// `createChange`.
    fn mudanca(&self, uri: &str) -> Mudanca {
        let cx = self.cx;
        let tx = Texto::novo(cx.fonte);
        let mut m = Mudanca::default();
        for o in self.ocorrencias.iter().filter(|o| o.e_selecao) {
            let invocacao = if self.sel.closure.is_some() {
                self.nome.clone()
            } else {
                let mut s = String::new();
                if !self.sel.comandos.is_empty()
                    && let Some(tipo) = &self.tipo_da_variavel
                {
                    match &self.variavel_retorno {
                        Some(v) => {
                            let nome_na_ocorrencia = o.nomes.get(v).cloned().unwrap_or_else(|| "null".to_string());
                            if !self.parametros.iter().any(|p| &p.nome == v) {
                                if tipo.is_empty() {
                                    s.push_str("var ");
                                } else {
                                    s.push_str(tipo);
                                    s.push(' ');
                                }
                            }
                            s.push_str(&nome_na_ocorrencia);
                            s.push_str(" = ");
                        }
                        None => s.push_str("return "),
                    }
                }
                if self.tem_await {
                    s.push_str("await ");
                }
                s.push_str(&self.nome);
                if !self.criar_getter {
                    s.push('(');
                    for (i, p) in self.parametros.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(&o.nomes.get(&p.nome).cloned().unwrap_or_else(|| "null".to_string()));
                    }
                    s.push(')');
                }
                if !self.sel.comandos.is_empty() {
                    s.push(';');
                }
                s
            };
            m.adicionar(uri, Span { start: o.faixa.0, end: o.faixa.1 }, invocacao);
        }
        // A declaração.
        if let Some(membro) = self.membro {
            let prefixo = cx.prefixo_do_no(membro);
            let eol = tx.eol();
            let mut anotacoes = if self.contexto_estatico { "static ".to_string() } else { String::new() };
            let mut corpo = self.corpo();
            let mut declaracao: Option<String> = None;
            if let Some(f) = self.sel.closure {
                let tipo = &self.retorno_da_closure;
                let mut d = format!("{anotacoes}{tipo}{}{corpo}", self.nome);
                if cx.filhos(f).iter().any(|&c| cx.especie(c) == "ExpressionFunctionBody") {
                    d.push(';');
                }
                declaracao = Some(d);
            }
            let assincrono = if self.tem_await { " async" } else { "" };
            if self.sel.expressao.is_some() {
                let varias_linhas = corpo.contains(eol);
                if !varias_linhas {
                    if !self.tipo_de_retorno.is_empty() {
                        anotacoes.push_str(&format!("{} ", self.tipo_de_retorno));
                    }
                    declaracao = Some(format!("{anotacoes}{}{assincrono} => {corpo};", self.assinatura()));
                } else {
                    corpo = tx.recuar_a_esquerda(&format!("{};", corpo.trim())).trim().to_string();
                    if !self.tipo_de_retorno.is_empty() {
                        anotacoes.push_str(&format!("{} ", self.tipo_de_retorno));
                    }
                    let mut d = format!("{anotacoes}{}{assincrono} {{{eol}{prefixo}  ", self.assinatura());
                    if !self.tipo_de_retorno.is_empty() {
                        d.push_str("return ");
                    }
                    d.push_str(&format!("{corpo}{eol}{prefixo}}}"));
                    declaracao = Some(d);
                }
            }
            if !self.sel.comandos.is_empty() {
                if !self.tipo_de_retorno.is_empty() {
                    anotacoes.push_str(&format!("{} ", self.tipo_de_retorno));
                }
                let mut d = format!("{anotacoes}{}{assincrono} {{{eol}{corpo}", self.assinatura());
                if let Some(v) = &self.variavel_retorno {
                    d.push_str(&format!("{prefixo}  return {v};{eol}"));
                }
                d.push_str(&format!("{prefixo}}}"));
                declaracao = Some(d);
            }
            if let Some(d) = declaracao {
                let fim = cx.arvore.nos[membro].fim;
                m.adicionar(uri, Span { start: fim, end: fim }, format!("{eol}{eol}{prefixo}{d}"));
            }
        }
        adicionar_imports(cx, &mut m, &self.importar);
        m
    }
}

/// `addLibraryImports(change, targetLibrary, libraries)`: os imports
/// ordenados, entre os existentes, depois do `library` ou no topo.
pub(crate) fn adicionar_imports(cx: &Contexto<'_>, m: &mut Mudanca, bibliotecas: &BTreeSet<LibraryId>) {
    if bibliotecas.is_empty() {
        return;
    }
    let p = cx.p;
    let prog = p.programa();
    let lib = prog.unit(cx.unidade).library;
    let Some(&definidora) = prog.library(lib).units.first() else { return };
    let Some(uri_lib) = p.uri_da_unidade(definidora) else { return };
    let unidade = prog.unit(definidora);
    let fonte = unidade.source.as_str();
    let tx = Texto::novo(fonte);
    let eol = tx.eol();
    // As URIs a importar (relativas para `file:`), ordenadas.
    let pasta = unidade.path.as_deref().and_then(|c| c.parent()).map(|c| c.to_path_buf());
    let mut uris: Vec<String> = bibliotecas
        .iter()
        .map(|&b| {
            let u = &prog.library(b).uri;
            if let (Some(pasta), Ok(url)) = (&pasta, url::Url::parse(u))
                && url.scheme() == "file"
                && let Ok(caminho) = url.to_file_path()
            {
                return caminho_relativo(&caminho, pasta);
            }
            u.clone()
        })
        .collect();
    uris.sort();
    // As aspas preferidas.
    let aspas = if crate::refatoracoes_exec::regra_ligada(p, definidora, "prefer_single_quotes") {
        '\''
    } else if crate::refatoracoes_exec::regra_ligada(p, definidora, "prefer_double_quotes") {
        '"'
    } else {
        let (mut simples, mut duplas) = (0, 0);
        for d in unidade.unit.directives.iter() {
            let uri = match &d.kind {
                ast::DirectiveKind::Import { uri, .. } | ast::DirectiveKind::Export { uri, .. } => Some(uri.span),
                _ => None,
            };
            if let Some(s) = uri {
                if fonte[s.start..s.end].starts_with('"') {
                    duplas += 1;
                } else {
                    simples += 1;
                }
            }
        }
        if duplas > simples { '"' } else { '\'' }
    };
    let imports: Vec<(String, usize, usize)> = unidade
        .unit
        .directives
        .iter()
        .filter_map(|d| match &d.kind {
            ast::DirectiveKind::Import { uri, .. } => {
                let t = &fonte[uri.span.start..uri.span.end];
                let valor = t.trim_matches(|c| c == '\'' || c == '"').to_string();
                Some((valor, d.span.start, d.span.end))
            }
            _ => None,
        })
        .collect();
    if !imports.is_empty() {
        let mut primeiro_pacote = true;
        for u in &uris {
            let mut inserido = false;
            let pacote = u.starts_with("package:");
            let mut depois_de_dart = false;
            for (existente, ini, _) in &imports {
                if existente.starts_with("dart:") {
                    depois_de_dart = true;
                }
                if existente.starts_with("package:") {
                    primeiro_pacote = false;
                }
                if u.as_str() < existente.as_str() {
                    m.adicionar(&uri_lib, Span { start: *ini, end: *ini }, format!("import {aspas}{u}{aspas};{eol}"));
                    inserido = true;
                    break;
                }
            }
            if !inserido {
                let mut codigo = format!("{eol}import {aspas}{u}{aspas};");
                if pacote && primeiro_pacote && depois_de_dart {
                    codigo = format!("{eol}{codigo}");
                }
                let fim = imports.last().unwrap().2;
                m.adicionar(&uri_lib, Span { start: fim, end: fim }, codigo);
            }
            if pacote {
                primeiro_pacote = false;
            }
        }
        return;
    }
    if let Some(d) = unidade.unit.directives.iter().find(|d| matches!(d.kind, ast::DirectiveKind::Library { .. })) {
        let mut prefixo = format!("{eol}{eol}");
        for u in &uris {
            m.adicionar(&uri_lib, Span { start: d.span.end, end: d.span.end }, format!("{prefixo}import {aspas}{u}{aspas};"));
            prefixo = eol.to_string();
        }
        return;
    }
    // No topo: depois do `#!` e dos comentários `//` iniciais.
    let mut offset = 0usize;
    let mut linha_antes = false;
    let n = fonte.len();
    if n >= 2 && offset < n - 2 && &fonte[0..2] == "#!" {
        linha_antes = true;
        offset = tx.proxima_linha(offset);
        let mut vazio = offset;
        while n >= 2 && vazio < n - 2 {
            let proxima = tx.proxima_linha(vazio);
            let linha = &fonte[vazio..proxima];
            if linha.trim().is_empty() {
                vazio = proxima;
                continue;
            } else if linha.starts_with("//") {
                offset = vazio;
                break;
            } else {
                break;
            }
        }
    }
    while n >= 2 && offset < n - 2 && &fonte[offset..offset + 2] == "//" {
        linha_antes = true;
        offset = tx.proxima_linha(offset);
    }
    let proxima = tx.proxima_linha(offset);
    let linha_depois = !fonte[offset..proxima].trim().is_empty();
    for (i, u) in uris.iter().enumerate() {
        let mut codigo = format!("import {aspas}{u}{aspas};{eol}");
        if i == 0 && linha_antes {
            codigo = format!("{eol}{codigo}");
        }
        if i == uris.len() - 1 && linha_depois {
            codigo = format!("{codigo}{eol}");
        }
        m.adicionar(&uri_lib, Span { start: offset, end: offset }, codigo);
    }
}

/// `pathContext.relative(what, from: pasta)` com `/`.
pub(crate) fn caminho_relativo(alvo: &std::path::Path, pasta: &std::path::Path) -> String {
    let a: Vec<_> = alvo.components().collect();
    let b: Vec<_> = pasta.components().collect();
    let comum = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let mut partes: Vec<String> = Vec::new();
    for _ in comum..b.len() {
        partes.push("..".to_string());
    }
    for c in &a[comum..] {
        partes.push(c.as_os_str().to_string_lossy().into_owned());
    }
    partes.join("/")
}

/// Os elementos de topo de uma unidade com a espécie (`kind.displayName`),
/// na ordem de `CompilationUnitElementImpl.children`.
fn elementos_de_topo(cx: &Contexto<'_>, u: dartforge_elements::model::UnitId) -> Vec<(&'static str, String)> {
    let prog = cx.p.programa();
    let unidade = prog.unit(u);
    let fonte = unidade.source.as_str();
    let ast = &unidade.ast;
    let texto = |n: ast::Name| fonte[n.span.start..n.span.end].to_string();
    let decls: Vec<&ast::Decl> = unidade.unit.declarations.iter().map(|&d| ast.decl(d)).collect();
    let mut v: Vec<(&'static str, String)> = Vec::new();
    // Acessores: os explícitos e os sintéticos das variáveis, em ordem.
    for d in &decls {
        match &d.kind {
            DeclKind::Function(f) => {
                let fun = ast.function(*f);
                let Some(n) = fun.name else { continue };
                match fun.kind {
                    ast::FunctionKind::Getter => v.push(("getter", texto(n))),
                    ast::FunctionKind::Setter => v.push(("setter", texto(n))),
                    _ => {}
                }
            }
            DeclKind::Variables(l) => {
                for var in l.variables.iter() {
                    v.push(("getter", texto(var.name)));
                    if !(l.final_ || l.const_) || (l.late && var.initializer.is_none() && l.final_) {
                        v.push(("setter", texto(var.name)));
                    }
                }
            }
            _ => {}
        }
    }
    for (especie, f): (&'static str, fn(&ast::DeclKind) -> Option<ast::Name>) in [
        ("class", (|k: &ast::DeclKind| match k {
            DeclKind::Class(c) => Some(c.name),
            _ => None,
        }) as fn(&ast::DeclKind) -> Option<ast::Name>),
        ("enum", |k| match k {
            DeclKind::Enum(e) => Some(e.name),
            _ => None,
        }),
        ("extension", |k| match k {
            DeclKind::Extension(e) => e.name,
            _ => None,
        }),
        ("extension type", |k| match k {
            DeclKind::ExtensionType(e) => Some(e.name),
            _ => None,
        }),
    ] {
        for d in &decls {
            if let Some(n) = f(&d.kind) {
                v.push((especie, texto(n)));
            }
        }
    }
    for d in &decls {
        if let DeclKind::Function(f) = &d.kind {
            let fun = ast.function(*f);
            let Some(n) = fun.name else { continue };
            if matches!(fun.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                v.push(("function", texto(n)));
            }
        }
    }
    for d in &decls {
        if let DeclKind::Mixin(x) = &d.kind {
            v.push(("mixin", texto(x.name)));
        }
    }
    for d in &decls {
        if let DeclKind::Typedef(x) = &d.kind {
            v.push(("type alias", texto(x.name)));
        }
        if let DeclKind::Class(c) = &d.kind
            && c.mixin_application
        {
            // `class C = S with M;` é `ClassElement` (já contado em classes).
        }
    }
    for d in &decls {
        if let DeclKind::Variables(l) = &d.kind {
            for var in l.variables.iter() {
                v.push(("top level variable", texto(var.name)));
            }
        }
    }
    v
}

/// O nome no começo do texto de uma `VariableDeclaration`.
fn nome_no_inicio(t: &str) -> String {
    let fim = t.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(t.len());
    t[..fim].to_string()
}

/// `quotedAndCommaSeparatedWithAnd`.
fn entre_aspas_com_and(nomes: &[String]) -> String {
    let q: Vec<String> = nomes.iter().map(|n| format!("'{n}'")).collect();
    match q.len() {
        0 => String::new(),
        1 => q[0].clone(),
        2 => format!("{} and {}", q[0], q[1]),
        n => format!("{}, and {}", q[..n - 1].join(", "), q[n - 1]),
    }
}

// -- ExitDetector -----------------------------------------------------------------

/// O `ExitDetector` do analyzer sobre a árvore no formato do analyzer.
pub(crate) struct Saida<'c, 'p> {
    cx: &'c Contexto<'p>,
    /// `_enclosingBlockContainsBreak`.
    quebra: bool,
    /// `_enclosingBlockContainsContinue`.
    continua: bool,
    /// `_enclosingBlockBreaksLabel`: os comandos alvo de `break rótulo`.
    rotulos_quebrados: HashSet<usize>,
}

impl<'c, 'p> Saida<'c, 'p> {
    pub(crate) fn novo(cx: &'c Contexto<'p>) -> Saida<'c, 'p> {
        Saida { cx, quebra: false, continua: false, rotulos_quebrados: HashSet::new() }
    }

    fn texto(&self, n: usize) -> &str {
        self.cx.texto_do_no(n)
    }

    fn filhos(&self, n: usize) -> Vec<usize> {
        self.cx.filhos(n).to_vec()
    }

    /// `_nodeExits`.
    fn talvez(&mut self, n: Option<usize>) -> Result<bool, String> {
        match n {
            Some(n) => self.sai(n),
            None => Ok(false),
        }
    }

    fn literal_booleano(&self, n: usize) -> Option<bool> {
        (self.cx.especie(n) == "BooleanLiteral").then(|| self.texto(n) == "true")
    }

    /// `_visitExpressions`: de trás para a frente.
    fn expressoes(&mut self, ns: &[usize]) -> Result<bool, String> {
        for &n in ns.iter().rev() {
            if self.sai(n)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn comandos(&mut self, ns: &[usize]) -> Result<bool, String> {
        for &n in ns {
            if self.sai(n)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `node.accept(ExitDetector)`.
    pub(crate) fn sai(&mut self, n: usize) -> Result<bool, String> {
        let cx = self.cx;
        let f = self.filhos(n);
        match cx.especie(n) {
            "ArgumentList" => self.expressoes(&f),
            "AsExpression" | "AwaitExpression" | "ParenthesizedExpression" | "IsExpression" | "SpreadElement" | "ExpressionStatement" => {
                self.talvez(f.first().copied())
            }
            "AssertInitializer" | "AssertStatement" | "ConstructorReference" | "EmptyStatement" | "ExtensionOverride"
            | "FunctionDeclarationStatement" | "FunctionExpression" | "GenericFunctionType" | "SimpleIdentifier" | "PrefixedIdentifier"
            | "LibraryIdentifier" | "Label" | "NamedType" | "PostfixExpression" | "PrefixExpression" | "SuperExpression" | "ThisExpression"
            | "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation"
            | "AdjacentStrings" | "SymbolLiteral" | "RecordLiteral" => Ok(false),
            "AssignmentExpression" => {
                let (Some(&l), Some(&r)) = (f.first(), f.get(1)) else { return Ok(false) };
                if self.sai(l)? {
                    return Ok(true);
                }
                let entre = &cx.fonte[cx.arvore.nos[l].fim..cx.arvore.nos[r].inicio];
                let op = entre.trim();
                if op.starts_with("&&=") || op.starts_with("||=") || op.starts_with("??=") {
                    return Ok(false);
                }
                if cx.especie(l) == "PropertyAccess" && self.texto(l).contains("?.") {
                    let alvo = cx.filhos(l).first().copied();
                    let nome = cx.filhos(l).last().copied();
                    if let (Some(a), Some(b)) = (alvo, nome)
                        && cx.fonte[cx.arvore.nos[a].fim..cx.arvore.nos[b].inicio].trim() == "?."
                    {
                        return Ok(false);
                    }
                }
                self.sai(r)
            }
            "BinaryExpression" => {
                let (Some(&l), Some(&r)) = (f.first(), f.get(1)) else { return Ok(false) };
                let op = cx.fonte[cx.arvore.nos[l].fim..cx.arvore.nos[r].inicio].trim().to_string();
                if op == "||" {
                    if self.literal_booleano(l) == Some(false) {
                        return self.sai(r);
                    }
                    return self.sai(l);
                }
                if op == "&&" {
                    if self.literal_booleano(l) == Some(true) {
                        return self.sai(r);
                    }
                    return self.sai(l);
                }
                if op == "??" {
                    return self.sai(l);
                }
                Ok(self.sai(l)? || self.sai(r)?)
            }
            "Block" => self.comandos(&f),
            "BlockFunctionBody" => self.talvez(f.first().copied()),
            "BreakStatement" => {
                self.quebra = true;
                if let Some(&rotulo) = f.first() {
                    let nome = self.texto(rotulo).to_string();
                    if let Some(alvo) = self.alvo_do_rotulo(n, &nome) {
                        self.rotulos_quebrados.insert(alvo);
                    }
                }
                Ok(false)
            }
            "CascadeExpression" => {
                let Some((&alvo, secoes)) = f.split_first() else { return Ok(false) };
                Ok(self.sai(alvo)? || self.expressoes(secoes)?)
            }
            "ConditionalExpression" => {
                let (Some(&c), Some(&t), Some(&e)) = (f.first(), f.get(1), f.get(2)) else { return Ok(false) };
                if self.sai(c)? {
                    return Ok(true);
                }
                Ok(self.sai(t)? && self.sai(e)?)
            }
            "ContinueStatement" => {
                self.continua = true;
                Ok(false)
            }
            "DoStatement" => {
                let (fora_q, fora_c) = (self.quebra, self.continua);
                self.quebra = false;
                self.continua = false;
                let r = (|| -> Result<bool, String> {
                    let corpo = self.talvez(f.first().copied())?;
                    let quebra_ou_continua = self.quebra || self.continua;
                    if corpo && !quebra_ou_continua {
                        return Ok(true);
                    }
                    let Some(&c) = f.get(1) else { return Ok(false) };
                    if self.sai(c)? {
                        return Ok(true);
                    }
                    if self.literal_booleano(c) == Some(true) && !self.quebra {
                        return Ok(true);
                    }
                    Ok(false)
                })();
                self.quebra = fora_q;
                self.continua = fora_c;
                r
            }
            "ForElement" | "ForStatement" => {
                let fora = self.quebra;
                self.quebra = false;
                let r = self.for_sai(n, &f);
                self.quebra = fora;
                r
            }
            "FunctionExpressionInvocation" => {
                if self.talvez(f.first().copied())? {
                    return Ok(true);
                }
                let lista = f.iter().copied().find(|&k| cx.especie(k) == "ArgumentList");
                self.talvez(lista)
            }
            "FunctionReference" => self.talvez(f.first().copied()),
            "IfElement" | "IfStatement" => {
                let Some(&c) = f.first() else { return Ok(false) };
                let entao = f.get(1).copied();
                let senao = f.get(2).copied();
                if self.sai(c)? {
                    return Ok(true);
                }
                match self.literal_booleano(c) {
                    Some(true) => return self.talvez(entao),
                    Some(false) if senao.is_some() => return self.talvez(senao),
                    _ => {}
                }
                let a = self.talvez(entao)?;
                let b = self.talvez(senao)?;
                if senao.is_none() {
                    return Ok(false);
                }
                Ok(a && b)
            }
            "IndexExpression" => {
                let alvo = if f.len() >= 2 { Some(f[0]) } else { self.alvo_da_cascata(n) };
                if self.talvez(alvo)? {
                    return Ok(true);
                }
                self.talvez(f.last().copied())
            }
            "InstanceCreationExpression" => self.talvez(f.iter().copied().find(|&k| cx.especie(k) == "ArgumentList")),
            "LabeledStatement" => {
                let corpo = f.last().copied();
                let r = self.talvez(corpo);
                let quebrado = corpo.is_some_and(|c| self.rotulos_quebrados.contains(&c));
                if let Some(c) = corpo {
                    self.rotulos_quebrados.remove(&c);
                }
                Ok(r? && !quebrado)
            }
            "ListLiteral" | "SetOrMapLiteral" => {
                for k in f.iter().copied().filter(|&k| cx.especie(k) != "TypeArgumentList") {
                    if self.sai(k)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "MapLiteralEntry" => Ok(self.talvez(f.first().copied())? || self.talvez(f.get(1).copied())?),
            "MethodInvocation" => {
                let nome = cx.nome_do_metodo(n);
                let explicito = f.first().copied().filter(|&a| Some(a) != nome);
                let alvo = explicito.or_else(|| self.alvo_da_cascata(n));
                if let Some(a) = alvo {
                    if self.sai(a)? {
                        return Ok(true);
                    }
                    let nulo = match (explicito, nome) {
                        (Some(x), Some(m)) => cx.fonte[cx.arvore.nos[x].fim..cx.arvore.nos[m].inicio].trim().starts_with('?'),
                        (None, Some(m)) => cx.fonte[cx.arvore.nos[n].inicio..cx.arvore.nos[m].inicio].trim().starts_with('?'),
                        _ => false,
                    };
                    if nulo {
                        return Ok(false);
                    }
                }
                if let Some(m) = nome
                    && self.elemento_sai(m)
                {
                    return Ok(true);
                }
                self.talvez(f.iter().copied().find(|&k| cx.especie(k) == "ArgumentList"))
            }
            "NamedExpression" => self.talvez(f.get(1).copied()),
            "PropertyAccess" => {
                let alvo = if f.len() >= 2 { Some(f[0]) } else { self.alvo_da_cascata(n) };
                self.talvez(alvo)
            }
            "RethrowExpression" | "ReturnStatement" | "ThrowExpression" => Ok(true),
            "SwitchCase" | "SwitchDefault" | "SwitchPatternCase" => {
                let cmds: Vec<usize> = f.iter().copied().filter(|&k| crate::refatoracoes::e_comando_especie(cx.especie(k))).collect();
                self.comandos(&cmds)
            }
            "SwitchExpression" => {
                for k in f.iter().copied().filter(|&k| cx.especie(k) == "SwitchExpressionCase") {
                    if !self.sai(k)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            "SwitchExpressionCase" => {
                // A guarda (`when`) e a expressão.
                let guarda = f.first().and_then(|&g| cx.filhos(g).iter().copied().find(|&w| cx.especie(w) == "WhenClause"));
                let condicao = guarda.and_then(|w| cx.filhos(w).first().copied());
                Ok(self.talvez(condicao)? || self.talvez(f.last().copied())?)
            }
            "SwitchStatement" => {
                let fora = self.quebra;
                self.quebra = false;
                let r = (|| -> Result<bool, String> {
                    let membros: Vec<usize> = f.iter().copied().filter(|&k| matches!(cx.especie(k), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
                    let mut tem_default = false;
                    let mut caso_que_nao_sai = false;
                    for (i, &m) in membros.iter().enumerate() {
                        let vazio = !cx.filhos(m).iter().any(|&k| crate::refatoracoes::e_comando_especie(cx.especie(k)));
                        if cx.especie(m) == "SwitchDefault" {
                            tem_default = true;
                            if vazio && i + 1 == membros.len() {
                                caso_que_nao_sai = true;
                                continue;
                            }
                        }
                        if !vazio && !self.sai(m)? {
                            caso_que_nao_sai = true;
                        }
                    }
                    if caso_que_nao_sai {
                        return Ok(false);
                    }
                    Ok(tem_default)
                })();
                self.quebra = fora;
                r
            }
            "TryStatement" => {
                let corpo = f.first().copied();
                let finalmente = f.last().copied().filter(|&k| f.len() > 1 && cx.especie(k) == "Block");
                if self.talvez(finalmente)? {
                    return Ok(true);
                }
                if !self.talvez(corpo)? {
                    return Ok(false);
                }
                for c in f.iter().copied().filter(|&k| cx.especie(k) == "CatchClause") {
                    let b = cx.filhos(c).last().copied();
                    if !self.talvez(b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            "VariableDeclaration" => self.talvez(f.first().copied()),
            "VariableDeclarationList" => {
                let vs: Vec<usize> = f.iter().copied().filter(|&k| cx.especie(k) == "VariableDeclaration").collect();
                for &v in vs.iter().rev() {
                    if self.sai(v)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "VariableDeclarationStatement" => {
                let Some(lista) = f.iter().copied().find(|&k| cx.especie(k) == "VariableDeclarationList") else { return Ok(false) };
                for v in cx.filhos(lista).iter().copied().filter(|&k| cx.especie(k) == "VariableDeclaration") {
                    if self.sai(v)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "WhileStatement" => {
                let fora = self.quebra;
                self.quebra = false;
                let r = (|| -> Result<bool, String> {
                    let Some(&c) = f.first() else { return Ok(false) };
                    if self.sai(c)? {
                        return Ok(true);
                    }
                    let _ = self.talvez(f.get(1).copied())?;
                    if self.literal_booleano(c) == Some(true) && !self.quebra {
                        return Ok(true);
                    }
                    Ok(false)
                })();
                self.quebra = fora;
                r
            }
            "YieldStatement" => self.talvez(f.first().copied()),
            outro => Err(format!("Bad state: Missing a visit method for a node of type {outro}Impl")),
        }
    }

    /// As partes de um `for` (`visitForStatement`/`visitForElement`).
    fn for_sai(&mut self, n: usize, f: &[usize]) -> Result<bool, String> {
        let cx = self.cx;
        let Some(&partes) = f.first() else { return Ok(false) };
        let corpo = f.get(1).copied();
        match cx.especie(partes) {
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" | "ForEachPartsWithPattern" => {
                let iteravel = cx.filhos(partes).last().copied();
                let r = self.talvez(iteravel)?;
                let _ = self.talvez(corpo)?;
                Ok(r)
            }
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                if cx.especie(n) == "ForStatement" && cx.especie(partes) == "ForPartsWithPattern" {
                    return Err("UnimplementedError".to_string());
                }
                let Some(p) = cx.arvore.partes_de_for.get(&partes).cloned() else { return Ok(false) };
                if let Some(i) = p.inicio {
                    match cx.especie(partes) {
                        "ForPartsWithDeclarations" => {
                            if self.sai(i)? {
                                return Ok(true);
                            }
                        }
                        "ForPartsWithExpression" => {
                            if self.sai(i)? {
                                return Ok(true);
                            }
                        }
                        _ => {}
                    }
                }
                if self.talvez(p.condicao)? {
                    return Ok(true);
                }
                if self.expressoes(&p.atualizacoes)? {
                    return Ok(true);
                }
                let volta = self.talvez(corpo)?;
                let verdadeiro = match p.condicao {
                    None => true,
                    Some(c) => self.literal_booleano(c) == Some(true),
                };
                if verdadeiro && (volta || !self.quebra) {
                    return Ok(true);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// `realTarget` de uma seção de cascata sem alvo escrito: o alvo da
    /// `CascadeExpression`.
    fn alvo_da_cascata(&self, n: usize) -> Option<usize> {
        let cx = self.cx;
        let mut k = n;
        while let Some(p) = cx.pai(k) {
            if cx.especie(p) == "CascadeExpression" {
                return cx.filhos(p).first().copied().filter(|&a| a != k);
            }
            if !matches!(cx.especie(p), "MethodInvocation" | "PropertyAccess" | "IndexExpression" | "AssignmentExpression") {
                return None;
            }
            k = p;
        }
        None
    }

    /// O comando rotulado com `nome` que contém o `break`.
    fn alvo_do_rotulo(&self, n: usize, nome: &str) -> Option<usize> {
        let cx = self.cx;
        for k in cx.com_pais(n) {
            if cx.especie(k) == "LabeledStatement" {
                let rotulos: Vec<String> = cx
                    .filhos(k)
                    .iter()
                    .filter(|&&l| cx.especie(l) == "Label")
                    .filter_map(|&l| cx.filhos(l).first().map(|&i| cx.texto_do_no(i).to_string()))
                    .collect();
                if rotulos.iter().any(|r| r == nome) {
                    return cx.filhos(k).last().copied();
                }
            }
            if cx.especie(k) == "SwitchStatement" {
                // Um `case` rotulado.
                for m in cx.filhos(k).iter().copied() {
                    let tem = cx
                        .filhos(m)
                        .iter()
                        .filter(|&&l| cx.especie(l) == "Label")
                        .any(|&l| cx.filhos(l).first().is_some_and(|&i| cx.texto_do_no(i) == nome));
                    if tem {
                        return Some(m);
                    }
                }
            }
        }
        None
    }

    /// `_elementExits`: o executável tem `@alwaysThrows` ou retorna `Never`.
    fn elemento_sai(&self, m: usize) -> bool {
        let cx = self.cx;
        match cx.elemento_do_identificador(m, true) {
            Elem::Funcao(f) => {
                let p = cx.p;
                let declarado = p.programa().function(f).declaracao_publica.unwrap_or(f);
                let nunca = p
                    .consulta
                    .outline
                    .functions
                    .get(declarado.0 as usize)
                    .is_some_and(|d| matches!(p.consulta.tabela.get(d.return_type), Type::Never));
                nunca || tem_always_throws(cx, declarado)
            }
            Elem::FuncaoLocal(fid) => {
                let t = cx.ast.function(fid).name.and_then(|n| cx.corpos.tipo_local(n.span.start));
                matches!(t.map(|t| cx.p.consulta.tabela.get(t)), Some(Type::Function { ret, .. }) if matches!(cx.p.consulta.tabela.get(*ret), Type::Never))
            }
            _ => false,
        }
    }
}

/// A declaração tem a anotação `@alwaysThrows` (de `package:meta`).
fn tem_always_throws(cx: &Contexto<'_>, f: dartforge_elements::model::FunctionElementId) -> bool {
    let prog = cx.p.programa();
    let dartforge_elements::model::FunctionRef::Function { unit, function } = prog.function(f).node else { return false };
    let unidade = prog.unit(unit);
    let alvo = unidade.ast.function(function).span;
    let decls = unidade.ast.decls.iter().map(|d| (&d.metadata, d.span)).chain(unidade.ast.members.iter().map(|m| (&m.metadata, m.span)));
    for (meta, span) in decls {
        if span.start <= alvo.start && alvo.end <= span.end {
            return meta.iter().any(|a| {
                let t = &unidade.source[a.span.start..a.span.end];
                t == "@alwaysThrows" || t.ends_with(".alwaysThrows")
            });
        }
    }
    false
}

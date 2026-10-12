//! O `KeywordHelper` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/keyword_helper.dart`, lido inteiro) e a
//! `KeywordSuggestion` (`candidate_suggestion.dart:452-512`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Isp, Recurso};
use super::*;
use dartforge_frontend::token::Op;
use std::sync::{Mutex, OnceLock};

/// O texto de uma sugestão de palavra-chave como `&'static str` (o
/// `Rel.palavra`): os textos possíveis são finitos (a palavra mais um texto
/// anotado fixo do passe), guardados uma vez cada.
fn estatico(s: &str) -> &'static str {
    static TEXTOS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut m = TEXTOS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&x) = m.get(s) {
        return x;
    }
    let x: &'static str = Box::leak(s.to_string().into_boxed_str());
    m.insert(x);
    x
}

/// `annotatedText.withoutCaret`.
fn sem_circunflexo(t: &str) -> (String, Option<usize>) {
    match t.find('^') {
        None => (t.to_string(), None),
        Some(i) => (format!("{}{}", &t[..i], &t[i + 1..]), Some(i)),
    }
}

/// `CollectionElement` que não é `Expression`.
fn elemento_nao_expressao(e: &str) -> bool {
    matches!(e, "IfElement" | "ForElement" | "SpreadElement" | "MapLiteralEntry" | "NullAwareElement")
}

impl<'a, 'c> Isp<'a, 'c> {
    /// Empurra a `KeywordSuggestion` (`completion`, `selectionOffset`).
    pub(super) fn sugestao_de_palavra(&mut self, completion: String, selecao: usize, score: f64) {
        let antes = self.coletor.itens.len();
        let fim = completion.encode_utf16().count();
        let item = self.coletor.empurrar(grupo::PALAVRA, especie::PALAVRA_CHAVE, completion.clone(), completion.clone(), None);
        item.rel.palavra = Some(estatico(&completion));
        item.do_passe = true;
        if selecao != fim {
            item.selecao = Some((selecao, 0));
        }
        self.registrar(antes, score);
    }

    /// `addKeyword`: o casamento é pelo lexema.
    pub(super) fn kw(&mut self, palavra: &str) {
        let s = self.score(palavra);
        if s != -1.0 {
            self.sugestao_de_palavra(palavra.to_string(), palavra.encode_utf16().count(), s);
        }
    }

    /// `addKeywordAndText`: o casamento é pelo lexema; o texto vai depois.
    pub(super) fn kw_e_texto(&mut self, palavra: &str, anotado: &str) {
        let s = self.score(palavra);
        if s != -1.0 {
            let (bruto, c) = sem_circunflexo(anotado);
            let sel = palavra.encode_utf16().count() + c.map_or(bruto.encode_utf16().count(), |i| bruto[..i].encode_utf16().count());
            self.sugestao_de_palavra(format!("{palavra}{bruto}"), sel, s);
        }
    }

    /// `addText`: o casamento é pelo texto sem o `^`.
    pub(super) fn texto_anotado(&mut self, anotado: &str) {
        let (bruto, c) = sem_circunflexo(anotado);
        let s = self.score(&bruto);
        if s != -1.0 {
            let sel = c.map_or(bruto.encode_utf16().count(), |i| bruto[..i].encode_utf16().count());
            self.sugestao_de_palavra(bruto, sel, s);
        }
    }

    /// `_isAbsentOrIn`.
    pub(super) fn ausente_ou_em(&self, t: Option<usize>) -> bool {
        t.is_none_or(|i| {
            let s = self.span_tok(i);
            s.start <= self.offset && self.offset <= s.end
        })
    }

    /// A palavra que abre a cláusula filha `especie` de `n`
    /// (`extendsKeyword`, `withKeyword`…).
    pub(super) fn palavra_da_clausula(&self, n: usize, especie: &str) -> Option<usize> {
        let c = self.filho(n, especie)?;
        let i = self.tok_depois(self.ini(c))?;
        (self.span_tok(i).start < self.fim(c).max(self.ini(c) + 1) && self.palavra(i)).then_some(i)
    }

    /// Um modificador escrito antes da palavra `ate` (`class`, `mixin`) da
    /// declaração.
    pub(super) fn modificador_antes(&self, n: usize, palavra: &str, ate: &str) -> Option<usize> {
        let ini = self.primeiro_depois_das_anotacoes(n).map_or(self.ini(n), |i| self.span_tok(i).start);
        let fim = self.palavra_em(ate, ini, self.fim(n)).map_or(self.fim(n), |i| self.span_tok(i).start);
        self.palavra_em(palavra, ini, fim)
    }

    /// `addClassDeclarationKeywords`.
    pub(super) fn kw_declaracao_de_classe(&mut self, n: usize) {
        if self.ausente_ou_em(self.palavra_da_clausula(n, "ExtendsClause")) {
            self.kw("extends");
        }
        if self.ausente_ou_em(self.palavra_da_clausula(n, "WithClause")) {
            self.kw("with");
        }
        if self.ausente_ou_em(self.palavra_da_clausula(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addClassMemberKeywords`.
    pub(super) fn kw_membro_de_classe(&mut self) {
        for p in ["const", "covariant", "dynamic", "factory", "final", "get", "operator", "set", "static", "var", "void", "late"] {
            self.kw(p);
        }
    }

    /// `addClassModifiers`.
    pub(super) fn kw_modificadores_de_classe(&mut self, n: usize) {
        let abstrato = self.modificador_antes(n, "abstract", "class");
        let selado = self.modificador_antes(n, "sealed", "class");
        let base = self.modificador_antes(n, "base", "class");
        let final_ = self.modificador_antes(n, "final", "class");
        let interface = self.modificador_antes(n, "interface", "class");
        let mixin = self.modificador_antes(n, "mixin", "class");
        if self.ausente_ou_em(abstrato) && selado.is_none() {
            self.kw("abstract");
        }
        if self.tem_recurso(Recurso::ClassModifiers) && self.tem_recurso(Recurso::SealedClass) {
            if base.is_none() && final_.is_none() && interface.is_none() && mixin.is_none() && selado.is_none() {
                if abstrato.is_none() {
                    self.kw("sealed");
                } else {
                    self.kw("base");
                    self.kw("final");
                    self.kw("interface");
                    self.kw("mixin");
                }
            }
            if base.is_some() && self.ausente_ou_em(mixin) {
                self.kw("mixin");
            }
            if mixin.is_some() && self.ausente_ou_em(base) {
                self.kw("base");
            }
        }
    }

    /// O `else` de um `IfElement` e o elemento depois dele.
    pub(super) fn senao_do_if(&self, n: usize) -> (Option<usize>, Option<usize>) {
        let abre = self.op_em(Op::LParen, self.ini(n), self.fim(n));
        let depois = abre.and_then(|a| self.par(a)).map_or(self.ini(n), |f| self.span_tok(f).end);
        let Some(e) = self.palavra_em("else", depois, self.fim(n)) else { return (None, None) };
        let fim_e = self.span_tok(e).end;
        (Some(e), self.filhos(n).iter().copied().find(|&k| self.ini(k) >= fim_e))
    }

    /// `couldHaveTrailingElse`.
    pub(super) fn pode_ter_else(&self, el: usize) -> bool {
        let mut f = el;
        loop {
            match self.especie(f) {
                "IfElement" => match self.senao_do_if(f) {
                    (Some(_), Some(s)) => f = s,
                    _ => break,
                },
                "ForElement" => match self.filhos(f).last() {
                    Some(&b) => f = b,
                    None => break,
                },
                _ => break,
            }
        }
        if self.especie(f) != "IfElement" || self.senao_do_if(f).0.is_some() {
            return false;
        }
        // `thenElement` não sintético: há um elemento depois do `)`.
        let abre = self.op_em(Op::LParen, self.ini(f), self.fim(f));
        let Some(fecha) = abre.and_then(|a| self.par(a)) else { return false };
        let fim_f = self.span_tok(fecha).end;
        self.filhos(f).iter().any(|&k| self.ini(k) >= fim_f && !self.todo_sintetico(k))
    }

    /// `addCollectionElementKeywords`.
    pub(super) fn kw_elemento_de_colecao(&mut self, literal: usize, elementos: &[usize], constante: bool, estatico: bool) {
        self.kw("for");
        self.kw("if");
        if let Some(anterior) = self.elemento_antes(elementos) {
            let prox = self.tok_depois(self.fim(anterior));
            if prox.is_none_or(|i| self.offset <= self.span_tok(i).start) {
                if self.pode_ter_else(anterior) {
                    self.kw("else");
                } else if let Some(k) = elementos.iter().position(|&e| e == anterior)
                    && k > 0
                    && self.pode_ter_else(elementos[k - 1])
                {
                    self.kw("else");
                }
            }
        }
        self.kw_expressao(Some(literal), constante, estatico);
    }

    /// `addCompilationUnitDeclarationKeywords`.
    pub(super) fn kw_declaracao_de_unidade(&mut self) {
        for p in ["abstract", "class", "const", "covariant", "dynamic", "enum", "external", "final", "mixin", "typedef", "var", "void"] {
            self.kw(p);
        }
        if self.tem_recurso(Recurso::ExtensionMethods) {
            self.kw("extension");
        }
        self.kw("late");
        if self.tem_recurso(Recurso::ClassModifiers) {
            self.kw("base");
            self.kw("interface");
        }
        if self.tem_recurso(Recurso::SealedClass) {
            self.kw("sealed");
        }
    }

    /// `addConstantExpressionKeywords`.
    pub(super) fn kw_expressao_constante(&mut self, em_contexto_constante: bool) {
        self.kw("false");
        self.kw("null");
        self.kw("true");
        if !em_contexto_constante {
            self.kw("const");
        }
    }

    /// `addConstructorInitializerKeywords`. A lista vazia (o fasta sempre
    /// cria ao menos um inicializador, sintético) conta como um
    /// `ConstructorFieldInitializer` sintético.
    pub(super) fn kw_inicializador_de_construtor(&mut self, ctor: usize, init: Option<usize>) {
        self.kw("assert");
        let inits = self.filhos_de(ctor, &["ConstructorFieldInitializer", "SuperConstructorInvocation", "RedirectingConstructorInvocation", "AssertInitializer"]);
        let ultimo = inits.last().copied();
        if init.is_none() || ultimo == init {
            let nao_sintetico = match ultimo {
                Some(u) if self.todo_sintetico(u) && inits.len() > 1 => Some(inits[inits.len() - 2]),
                u => u,
            };
            let ok = (nao_sintetico.is_some() && nao_sintetico == init)
                || nao_sintetico.is_none_or(|u| !matches!(self.especie(u), "SuperConstructorInvocation" | "RedirectingConstructorInvocation"));
            if ok {
                if self.pai(ctor).is_none_or(|p| self.especie(p) != "ExtensionTypeDeclaration") {
                    self.kw("super");
                }
                self.kw("this");
            }
        } else if let Some(i) = init
            && self.especie(i) == "ConstructorFieldInitializer"
        {
            let (fim_igual, prox) = match self.op_em(Op::Assign, self.ini(i), self.fim(i).max(self.ini(i) + 1)) {
                Some(eq) => (self.span_tok(eq).end, self.toks.get(eq + 1).map_or(self.fonte.len(), |t| t.span.start)),
                None => {
                    let p = self.sintetico(self.fim(i)).start;
                    (p, p)
                }
            };
            if fim_igual <= self.offset && self.offset <= prox {
                self.kw("this");
            }
        }
    }

    /// `addDirectiveKeywords`.
    pub(super) fn kw_diretiva(&mut self, antes: Option<usize>) {
        let tem_library = self.filhos(0).iter().any(|&k| self.especie(k) == "LibraryDirective");
        if antes.is_none() && !tem_library {
            self.kw("library");
        }
        self.kw_e_texto("import", " '^';");
        self.kw_e_texto("export", " '^';");
        self.kw_e_texto("part", " '^';");
        let sem_diretivas = !self.filhos(0).iter().any(|&k| self.e_diretiva(k));
        if sem_diretivas {
            self.texto_anotado("part of '^';");
        }
    }

    /// `addEnumDeclarationKeywords`.
    pub(super) fn kw_declaracao_de_enum(&mut self, n: usize) {
        if self.ausente_ou_em(self.palavra_da_clausula(n, "WithClause")) {
            self.kw("with");
        }
        if self.ausente_ou_em(self.palavra_da_clausula(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addEnumMemberKeywords`.
    pub(super) fn kw_membro_de_enum(&mut self) {
        for p in ["const", "dynamic", "final", "get", "late", "operator", "set", "static", "var", "void"] {
            self.kw(p);
        }
    }

    /// `constIsValid` de `addExpressionKeywords`.
    pub(super) fn const_valido(&self, n: usize) -> bool {
        let mut n = n;
        if elemento_nao_expressao(self.especie(n)) {
            match self.pai(n) {
                Some(p) => n = p,
                None => return false,
            }
        }
        let e = self.especie(n);
        if super::isp::ESPECIES_DE_EXPRESSAO.contains(&e) {
            return !self.contexto_constante(n);
        }
        match e {
            "Block" | "EmptyStatement" | "ExpressionStatement" | "IfStatement" | "PatternVariableDeclaration" | "SwitchPatternCase"
            | "SwitchStatement" | "WhenClause" => true,
            "RecordPattern" => self.filhos_de(n, &["PatternField"]).is_empty(),
            "VariableDeclaration" => !self.pai(n).and_then(|l| self.lista_real(l)).is_some_and(|l| l.const_),
            "VariableDeclarationStatement" => !self.filho(n, "VariableDeclarationList").and_then(|l| self.lista_real(l)).is_some_and(|l| l.const_),
            _ => false,
        }
    }

    /// O `:` de um `SwitchPatternCase` (o offset; sintético: o do próximo
    /// token depois do padrão).
    pub(super) fn dois_pontos_do_caso(&self, n: usize) -> usize {
        let depois = self.filho(n, "GuardedPattern").map_or_else(|| self.tok_depois(self.ini(n)).map_or(self.ini(n), |i| self.span_tok(i).end), |g| self.fim(g));
        match self.op_em(Op::Colon, depois, self.fim(n).max(depois + 1)) {
            Some(i) => self.span_tok(i).start,
            None => self.sintetico(depois).start,
        }
    }

    /// `switchIsValid` de `addExpressionKeywords`.
    pub(super) fn switch_valido(&self, n: usize) -> bool {
        let mut n = n;
        if elemento_nao_expressao(self.especie(n)) {
            match self.pai(n) {
                Some(p) => n = p,
                None => return true,
            }
        }
        !(self.especie(n) == "SwitchPatternCase" && self.offset <= self.dois_pontos_do_caso(n))
    }

    /// `addExpressionKeywords`.
    pub(super) fn kw_expressao(&mut self, no: Option<usize>, constante: bool, estatico: bool) {
        self.kw("false");
        self.kw("null");
        self.kw("true");
        match no {
            Some(n) => {
                if self.const_valido(n) {
                    self.kw("const");
                }
                if !constante && !estatico {
                    self.kw("super");
                    self.kw("this");
                }
                if self.em_async(n) || self.em_gerador(n) {
                    self.kw("await");
                }
                if self.switch_valido(n) && self.tem_recurso(Recurso::Patterns) {
                    self.kw("switch");
                }
            }
            None => {
                if self.tem_recurso(Recurso::Patterns) {
                    self.kw("switch");
                }
            }
        }
    }

    /// `addExtensionDeclarationKeywords`.
    pub(super) fn kw_declaracao_de_extensao(&mut self, n: usize) {
        let on = self.filho(n, "ExtensionOnClause").and_then(|c| self.palavra_em("on", self.ini(c), self.fim(c).max(self.ini(c) + 1)));
        if on.is_none() {
            self.kw("on");
            let sem_nome = matches!(self.decl_real(n).map(|d| &d.kind), Some(ast::DeclKind::Extension(x)) if x.name.is_none());
            if sem_nome && self.tem_recurso(Recurso::InlineClass) {
                self.texto_anotado("type");
            }
        }
    }

    /// `addExtensionMemberKeywords`.
    pub(super) fn kw_membro_de_extensao(&mut self, estatico: bool) {
        self.kw("const");
        self.kw("dynamic");
        self.kw("final");
        self.kw("get");
        if !estatico {
            self.kw("operator");
        }
        self.kw("set");
        if !estatico {
            self.kw("static");
        }
        self.kw("var");
        self.kw("void");
    }

    /// `addExtensionTypeMemberKeywords`.
    pub(super) fn kw_membro_de_tipo_de_extensao(&mut self, estatico: bool) {
        self.kw_membro_de_extensao(estatico);
    }

    /// `addFieldDeclarationKeywords`: `palavra` é a palavra-chave que a
    /// recuperação usou como nome da única variável.
    pub(super) fn kw_declaracao_de_campo(&mut self, n: usize, palavra: Option<&str>) {
        let Some(lista_no) = self.filho(n, "VariableDeclarationList") else { return };
        let Some(l) = self.lista_real(lista_no) else { return };
        let primeira = l.variables.first();
        let ate = primeira.map_or(self.fim(n), |v| v.name.span.start);
        let ini = self.primeiro_depois_das_anotacoes(n).map_or(self.ini(n), |i| self.span_tok(i).start);
        let external = self.palavra_em("external", ini, ate);
        let estatico = self.palavra_em("static", ini, ate);
        let abstrato = self.palavra_em("abstract", ini, ate);
        let covariante = self.palavra_em("covariant", ini, ate);
        let late = self.palavra_em("late", ini, ate);
        if self.ausente_ou_em(external) && palavra != Some("external") {
            self.kw("external");
        }
        if l.ty.is_none() {
            self.kw("dynamic");
            self.kw("void");
        }
        if estatico.is_none() && palavra != Some("static") {
            if self.ausente_ou_em(abstrato) && palavra != Some("abstract") {
                self.kw("abstract");
            }
            if self.ausente_ou_em(covariante) && palavra != Some("covariant") {
                self.kw("covariant");
            }
            if self.ausente_ou_em(late) && palavra != Some("late") {
                self.kw("late");
            }
            self.kw("static");
        }
        if primeira.is_some() && !l.const_ && !l.final_ && !matches!(palavra, Some("const" | "final" | "var")) {
            self.kw("const");
            self.kw("final");
            if l.ty.is_none() {
                self.kw("var");
            }
        }
    }

    /// `FormalParameterList.inNamedGroup`.
    pub(super) fn em_grupo_nomeado(&self, lista: usize) -> bool {
        let de = self.ini(lista) + 1;
        let fim = self.fim(lista);
        let chave = self.op_em(Op::LBrace, de, fim);
        let colchete = self.op_em(Op::LBracket, de, fim);
        let esq = match (chave, colchete) {
            (Some(a), Some(b)) if b < a => return false,
            (Some(a), _) => a,
            _ => return false,
        };
        let left = self.span_tok(esq).end;
        let right = self
            .par(esq)
            .filter(|&j| self.span_tok(j).start < fim)
            .map(|j| self.span_tok(j).start)
            .or_else(|| self.tok_antes(fim).filter(|&j| self.e_op(j, Op::RParen)).map(|j| self.span_tok(j).start))
            .unwrap_or(fim);
        left <= self.offset && self.offset <= right
    }

    /// `addFormalParameterKeywords`.
    pub(super) fn kw_parametro_formal(&mut self, lista: usize) {
        self.kw("covariant");
        self.kw("final");
        if self.em_grupo_nomeado(lista) {
            self.kw("required");
        }
        if self.pai(lista).is_some_and(|p| self.especie(p) == "ConstructorDeclaration") {
            if self.tem_recurso(Recurso::SuperParameters) {
                self.kw("super");
            }
            self.kw("this");
        }
    }

    /// `addFunctionBodyModifiers`.
    pub(super) fn kw_modificadores_de_corpo(&mut self, corpo: Option<usize>) {
        let palavra = corpo.and_then(|c| {
            let i = self.tok_depois(self.ini(c))?;
            (matches!(self.texto_tok(i), "async" | "sync") && self.span_tok(i).start < self.fim(c).max(self.ini(c) + 1)).then_some(i)
        });
        if self.ausente_ou_em(palavra) {
            self.kw("async");
            if corpo.is_none_or(|c| self.especie(c) != "ExpressionFunctionBody") {
                self.kw_e_texto("async", "*");
                self.kw_e_texto("sync", "*");
            }
        }
    }

    /// `addImportDirectiveKeywords`.
    pub(super) fn kw_diretiva_import(&mut self, n: usize) {
        let (ini, fim) = (self.ini(n), self.fim(n));
        let deferred = self.palavra_em("deferred", ini, fim);
        let como = self.palavra_em("as", ini, fim);
        let primeiro = self.filhos_de(n, &["ShowCombinator", "HideCombinator"]).first().copied();
        if primeiro.is_none_or(|c| self.offset < self.ini(c)) {
            match deferred {
                None => match como {
                    None => {
                        self.kw_e_texto("deferred", " as");
                        self.kw("as");
                        self.kw("hide");
                        self.kw("show");
                    }
                    Some(a) if self.offset < self.span_tok(a).start => self.kw("deferred"),
                    Some(a) => {
                        // `prefix`: o identificador depois do `as` (sintético:
                        // no próximo token).
                        let fim_a = self.span_tok(a).end;
                        let fim_prefixo = match self.tok_depois(fim_a) {
                            Some(i) if self.e_identificador(i) => self.span_tok(i).end,
                            _ => self.sintetico(fim_a).start,
                        };
                        if self.offset > fim_prefixo {
                            self.kw("hide");
                            self.kw("show");
                        }
                    }
                },
                Some(d) if self.offset > self.span_tok(d).end && como.is_none() => self.kw("as"),
                Some(_) => {
                    self.kw("hide");
                    self.kw("show");
                }
            }
        } else {
            self.kw("hide");
            self.kw("show");
        }
    }

    /// `addMixinDeclarationKeywords`.
    pub(super) fn kw_declaracao_de_mixin(&mut self, n: usize) {
        if self.ausente_ou_em(self.palavra_da_clausula(n, "MixinOnClause")) {
            self.kw("on");
        }
        if self.ausente_ou_em(self.palavra_da_clausula(n, "ImplementsClause")) {
            self.kw("implements");
        }
    }

    /// `addMixinMemberKeywords`.
    pub(super) fn kw_membro_de_mixin(&mut self) {
        for p in ["const", "covariant", "dynamic", "final", "get", "operator", "set", "static", "var", "void", "late"] {
            self.kw(p);
        }
    }

    /// `addMixinModifiers`.
    pub(super) fn kw_modificadores_de_mixin(&mut self, n: usize) {
        if self.ausente_ou_em(self.modificador_antes(n, "base", "mixin")) {
            self.kw("base");
        }
    }

    /// `addPatternKeywords`.
    pub(super) fn kw_padrao(&mut self) {
        self.kw_expressao_constante(false);
        self.kw_padrao_de_variavel();
    }

    /// `addStatementKeywords`.
    pub(super) fn kw_comando(&mut self, n: usize) {
        if self.em_async(n) {
            self.kw("await");
        } else if self.em_gerador(n) {
            self.kw("await");
            self.kw("yield");
            self.kw_e_texto("yield", "*");
        }
        if self.em_laco(n) {
            self.kw("break");
            self.kw("continue");
        }
        if self.em_switch(n) {
            self.kw("break");
        }
        self.kw("assert");
        self.kw("do");
        self.kw("dynamic");
        self.kw("final");
        self.kw("for");
        self.kw("if");
        if self.em_catch(n) {
            self.kw("rethrow");
        }
        self.kw("return");
        if !self.tem_recurso(Recurso::Patterns) {
            self.kw("switch");
        }
        self.kw("throw");
        self.kw("try");
        self.kw("var");
        self.kw("void");
        self.kw("while");
        if self.em_gerador(n) {
            self.kw("yield");
            self.kw_e_texto("yield", "*");
        }
        self.kw("late");
    }

    /// `addTryClauseKeywords`.
    pub(super) fn kw_clausulas_de_try(&mut self, pode_ter_finally: bool) {
        self.kw("catch");
        if pode_ter_finally {
            self.kw("finally");
        }
        self.kw("on");
    }

    /// `addVariablePatternKeywords`.
    pub(super) fn kw_padrao_de_variavel(&mut self) {
        self.kw("final");
        self.kw("var");
    }
}

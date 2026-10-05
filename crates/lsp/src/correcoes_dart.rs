//! Produtores de correção portados do servidor do Dart 3.6.2 sobre a árvore
//! do analyzer ([`crate::arvore_analyzer`]): `RemoveUnusedLocalVariable`,
//! `RemoveUnusedElement` e `RemoveUnusedField` (`COR:remove_unused*.dart`),
//! com os utilitários de intervalo do `RangeFactory`
//! (docs/LSP-ESPECIFICACAO.md §13.7.4.0 e §13.7.4.5).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::projeto::{Alvo, Concreto, palavra};
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::Texto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, TypedefId, VariableId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{MemberRef, Resolved};

/// O elemento declarado por um nó (o `declaredElement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Declarado {
    Classe(ClassId),
    Typedef(TypedefId),
    Funcao(FunctionElementId),
    /// Função local, pelo offset do nome.
    FuncaoLocal(usize),
    Variavel(VariableId),
}

impl Contexto<'_> {
    // -- Tokens e intervalos ----------------------------------------------------

    /// O primeiro token que começa em `pos` ou depois (sem o fim de arquivo).
    pub(crate) fn token_seguinte(&self, pos: usize) -> Option<Span> {
        let i = self.tokens.partition_point(|t| t.span.start < pos);
        self.tokens.get(i).filter(|t| t.kind != Kind::Eof).map(|t| t.span)
    }

    /// O último token que termina em `pos` ou antes.
    pub(crate) fn token_anterior(&self, pos: usize) -> Option<Span> {
        let i = self.tokens.partition_point(|t| t.span.end <= pos);
        i.checked_sub(1).map(|k| self.tokens[k].span)
    }

    fn lexema(&self, s: Span) -> &str {
        &self.fonte[s.start..s.end]
    }

    /// Os filhos de `n` de uma espécie.
    pub(crate) fn filhos_da_especie(&self, n: usize, especie: &str) -> Vec<usize> {
        self.filhos(n).iter().copied().filter(|&f| self.especie(f) == especie).collect()
    }

    /// `thisOrAncestorMatching`.
    fn este_ou_ancestral(&self, n: usize, pred: impl Fn(&str) -> bool) -> Option<usize> {
        let mut atual = Some(n);
        while let Some(k) = atual {
            if pred(self.especie(k)) {
                return Some(k);
            }
            atual = self.pai(k);
        }
        None
    }

    /// `range.nodeInList(list, item)`.
    pub(crate) fn no_em_lista(&self, lista: &[usize], item: usize) -> Span {
        let s = self.arvore.span(item);
        if lista.len() == 1 {
            if let Some(t) = self.token_seguinte(s.end)
                && self.lexema(t) == ","
            {
                return Span { start: s.start, end: t.end };
            }
            // O dono é a `ConstructorDeclaration` (os inicializadores):
            // do token antes do `:` ao corpo.
            if let Some(dono) = self.pai(item)
                && self.especie(dono) == "ConstructorDeclaration"
                && let Some(sep) = self.token_anterior(self.arvore.nos[lista[0]].inicio)
                && self.lexema(sep) == ":"
                && let Some(antes) = self.token_anterior(sep.start)
                && let Some(corpo) = self.filhos(dono).iter().copied().find(|&f| self.especie(f).ends_with("FunctionBody"))
            {
                return Span { start: antes.end, end: self.arvore.nos[corpo].inicio };
            }
            return s;
        }
        let i = lista.iter().position(|&x| x == item).unwrap_or(0);
        if i == 0 {
            Span { start: s.start, end: self.arvore.nos[lista[1]].inicio }
        } else {
            Span { start: self.arvore.nos[lista[i - 1]].fim, end: s.end }
        }
    }

    /// `utils.getLinesRange(range.node(n))`.
    fn linhas_do_no(&self, n: usize) -> Span {
        let s = self.arvore.span(n);
        Texto::novo(self.fonte).faixa_de_linhas(s.start, s.end)
    }

    /// O nome (o último identificador) de um `DeclaredVariablePattern`.
    fn nome_do_padrao(&self, n: usize) -> Option<Span> {
        let fim = self.arvore.nos[n].fim;
        palavra(self.fonte, fim.checked_sub(1)?).filter(|s| s.end == fim)
    }

    /// O `DartPattern.patternContext`.
    fn contexto_do_padrao(&self, padrao: usize) -> Option<usize> {
        let mut atual = padrao;
        loop {
            let mut pai = self.pai(atual)?;
            if matches!(self.especie(pai), "MapPatternEntry" | "PatternField" | "RestPatternElement") {
                pai = self.pai(pai)?;
            }
            match self.especie(pai) {
                "ForEachPartsWithPattern" | "PatternVariableDeclaration" | "PatternAssignment" | "GuardedPattern" => return Some(pai),
                e if e.ends_with("Pattern") => atual = pai,
                _ => return None,
            }
        }
    }

    /// `AssignedVariablePattern`: o `x` solto de um padrão de atribuição.
    fn padrao_atribuido(&self, n: usize) -> bool {
        self.especie(n) == "DeclaredVariablePattern"
            && self.filhos(n).is_empty()
            && self.contexto_do_padrao(n).is_some_and(|c| self.especie(c) == "PatternAssignment")
            && !self.texto_do_no(n).contains(char::is_whitespace)
    }

    /// O local visível em `pos` com o nome `nome` é o declarado em `decl`
    /// (a faixa mais interna do `VisibleRangesComputer` que cobre `pos`
    /// cobre a declaração).
    fn local_em(&self, corpo: usize, pos: usize, nome: &str, decl: usize) -> bool {
        crate::refatoracoes_embutir::faixas_visiveis(self, corpo)
            .into_iter()
            .filter(|(n, (a, b))| n == nome && *a <= pos && pos <= *b)
            .min_by_key(|(_, (a, b))| b - a)
            .is_some_and(|(_, (a, b))| a <= decl && decl <= b)
    }

    // -- RemoveUnusedLocalVariable ---------------------------------------------

    /// `RemoveUnusedLocalVariable.compute` (`remove_unused_local_variable.dart`):
    /// as edições (intervalo, texto), ou `None` quando o Dart não oferece.
    pub(crate) fn remover_variavel_local(&self, erro: Span) -> Option<Vec<(Span, String)>> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let mut comandos: Vec<(Span, String)> = Vec::new();
        // `_deleteDeclaration`.
        let declarado = match self.especie(node) {
            "VariableDeclaration" => {
                let lista = self.pai(node).filter(|&l| self.especie(l) == "VariableDeclarationList")?;
                let comando = self.pai(lista).filter(|&c| self.especie(c) == "VariableDeclarationStatement")?;
                let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
                if variaveis.len() == 1 {
                    let inicializador = self.filhos(variaveis[0]).first().copied();
                    match inicializador {
                        Some(i) if self.especie(i) == "MethodInvocation" => {
                            comandos.push((Span { start: self.arvore.nos[comando].inicio, end: self.arvore.nos[i].inicio }, String::new()));
                        }
                        _ => comandos.push((self.linhas_do_no(comando), String::new())),
                    }
                } else {
                    comandos.push((self.no_em_lista(&variaveis, node), String::new()));
                }
                true
            }
            "DeclaredVariablePattern" => self.excluir_padrao_declarado(node, &mut comandos),
            _ => false,
        };
        if !declarado {
            return None;
        }
        // `_deleteReferences`: o elemento (na `VariableDeclaration`, só com o
        // erro no nome).
        let (decl, nome) = match self.especie(node) {
            "VariableDeclaration" => {
                let s = palavra(self.fonte, self.arvore.nos[node].inicio)?;
                if erro.start < s.start || erro.start > s.end {
                    return None;
                }
                (s.start, self.lexema(s).to_string())
            }
            _ => {
                let s = self.nome_do_padrao(node)?;
                (s.start, self.lexema(s).to_string())
            }
        };
        let corpo = self.este_ou_ancestral(node, |e| e.ends_with("FunctionBody"))?;
        let mut apagados: Vec<Span> = Vec::new();
        for referencia in self.referencias_locais(corpo, decl, &nome) {
            let faixa = self.faixa_da_referencia(referencia)?;
            let mut coberta = false;
            for outra in &apagados {
                if outra.start <= faixa.start && faixa.end <= outra.end {
                    coberta = true;
                    break;
                } else if faixa.start < outra.end && outra.start < faixa.end {
                    return None;
                }
            }
            if coberta {
                continue;
            }
            comandos.push((faixa, String::new()));
            apagados.push(faixa);
        }
        Some(comandos)
    }

    /// A declaração de um `DeclaredVariablePattern` pelo pai (a lista,
    /// o `&&`, o campo de objeto ou de record).
    fn excluir_padrao_declarado(&self, node: usize, comandos: &mut Vec<(Span, String)>) -> bool {
        let Some(pai) = self.pai(node) else { return false };
        let tipo = self.filhos(node).first().copied();
        let s = self.arvore.span(node);
        match self.especie(pai) {
            "ListPattern" | "MapPatternEntry" => {
                let novo = match tipo {
                    Some(t) => format!("{} _", self.texto_do_no(t)),
                    None => "_".to_string(),
                };
                comandos.push((s, novo));
                true
            }
            "LogicalAndPattern" => {
                let filhos = self.filhos(pai);
                let (Some(&esq), Some(&dir)) = (filhos.first(), filhos.get(1)) else { return false };
                if let Some(t) = tipo {
                    comandos.push((s, format!("{} _", self.texto_do_no(t))));
                } else if esq == node {
                    comandos.push((Span { start: s.start, end: self.arvore.nos[dir].inicio }, String::new()));
                } else {
                    comandos.push((Span { start: self.arvore.nos[esq].fim, end: s.end }, String::new()));
                }
                true
            }
            "PatternField" => {
                let campo = pai;
                let Some(dono) = self.pai(campo) else { return false };
                let nome_do_campo = self.filhos_da_especie(campo, "PatternFieldName").first().copied();
                let Some(nome) = self.nome_do_padrao(node) else { return false };
                // `:x` → `x: ` antes do padrão.
                let explicito = |comandos: &mut Vec<(Span, String)>, n: usize| {
                    let dois_pontos = self.arvore.nos[n].fim - 1;
                    comandos.push((Span { start: dois_pontos, end: s.start }, format!("{}: ", self.lexema(nome))));
                };
                let sem_nome = |n: usize| self.texto_do_no(n).trim() == ":";
                match self.especie(dono) {
                    "ObjectPattern" => {
                        let Some(n) = nome_do_campo else { return false };
                        let campos = self.filhos_da_especie(dono, "PatternField");
                        if campos.len() == 1
                            && let Some(decl) = self.pai(dono).filter(|&d| self.especie(d) == "PatternVariableDeclaration")
                            && let Some(cmd) = self.pai(decl).filter(|&c| self.especie(c) == "PatternVariableDeclarationStatement")
                        {
                            comandos.push((self.linhas_do_no(cmd), String::new()));
                            return true;
                        }
                        if tipo.is_some() && self.contexto_do_padrao(dono).is_some_and(|c| self.especie(c) == "GuardedPattern") {
                            if sem_nome(n) {
                                explicito(comandos, n);
                            }
                            comandos.push((nome, "_".to_string()));
                            return true;
                        }
                        comandos.push((self.no_em_lista(&campos, campo), String::new()));
                        true
                    }
                    "RecordPattern" => {
                        if let Some(n) = nome_do_campo
                            && sem_nome(n)
                        {
                            explicito(comandos, n);
                        }
                        comandos.push((nome, "_".to_string()));
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `findLocalElementReferences3(corpo, elemento)`: os `SimpleIdentifier`
    /// do local e os `AssignedVariablePattern` dele que são elementos diretos
    /// de uma `ListPattern` ou o padrão (sem parênteses) de um campo de
    /// `RecordPattern` (o visitante não desce nesses dois padrões).
    fn referencias_locais(&self, corpo: usize, decl: usize, nome: &str) -> Vec<usize> {
        let mut v = Vec::new();
        let mut pilha = vec![corpo];
        let mut ordem: Vec<usize> = Vec::new();
        while let Some(k) = pilha.pop() {
            ordem.push(k);
            match self.especie(k) {
                "ListPattern" => {
                    for &f in self.filhos(k) {
                        if self.padrao_atribuido(f) && self.texto_do_no(f) == nome && self.local_em(corpo, self.arvore.nos[f].inicio, nome, decl) {
                            v.push(f);
                        }
                    }
                    continue;
                }
                "RecordPattern" => {
                    for &campo in self.filhos(k) {
                        let Some(&padrao) = self.filhos(campo).iter().rev().find(|&&f| self.especie(f) != "PatternFieldName") else { continue };
                        let mut interno = padrao;
                        while self.especie(interno) == "ParenthesizedPattern" {
                            let Some(&i) = self.filhos(interno).first() else { break };
                            interno = i;
                        }
                        if self.padrao_atribuido(interno) && self.texto_do_no(interno) == nome && self.local_em(corpo, self.arvore.nos[interno].inicio, nome, decl) {
                            v.push(padrao);
                        }
                    }
                    continue;
                }
                "SimpleIdentifier" => {
                    if let Marca::Expr(x) = self.arvore.nos[k].marca
                        && self.corpos.declaracao_local(x) == Some(decl)
                    {
                        v.push(k);
                    }
                }
                _ => {}
            }
            for &f in self.filhos(k).iter().rev() {
                pilha.push(f);
            }
        }
        v.sort_by_key(|&k| self.arvore.nos[k].inicio);
        v
    }

    /// `_referenceRangeToDelete`: só o lado esquerdo de uma atribuição.
    fn faixa_da_referencia(&self, referencia: usize) -> Option<Span> {
        let pai = self.pai(referencia)?;
        if self.especie(pai) != "AssignmentExpression" || self.filhos(pai).first() != Some(&referencia) {
            return None;
        }
        let avo = self.pai(pai)?;
        if self.especie(avo) == "ArgumentList" {
            let operador = self.token_seguinte(self.arvore.nos[referencia].fim)?;
            let depois = self.token_seguinte(operador.end)?;
            return Some(Span { start: self.arvore.nos[pai].inicio, end: depois.start });
        }
        Some(self.linhas_do_no(avo))
    }

    // -- RemoveUnusedElement / RemoveUnusedField --------------------------------

    /// O `declaredElement` dos nós que o `RemoveUnusedElement` aceita.
    fn declarado(&self, n: usize) -> Option<Declarado> {
        let p = self.p.programa();
        match (self.especie(n), self.arvore.nos[n].marca) {
            ("ClassDeclaration" | "EnumDeclaration", Marca::Decl(d)) => self.classe_da_declaracao(self.unidade, d).map(Declarado::Classe),
            ("FunctionTypeAlias", Marca::Decl(d)) => (0..p.typedefs.len())
                .map(|i| TypedefId(i as u32))
                .find(|&t| p.typedef(t).decl.unit == self.unidade && p.typedef(t).decl.decl == d)
                .map(Declarado::Typedef),
            ("FunctionDeclaration", Marca::Funcao(fid)) => {
                if self.pai(n).is_some_and(|x| self.especie(x) == "FunctionDeclarationStatement") {
                    self.ast.function(fid).name.map(|nm| Declarado::FuncaoLocal(nm.span.start))
                } else {
                    self.p.funcao_do_no(self.unidade, fid).map(Declarado::Funcao)
                }
            }
            ("MethodDeclaration", Marca::Funcao(fid)) => self.p.funcao_do_no(self.unidade, fid).map(Declarado::Funcao),
            ("VariableDeclaration", _) => {
                let s = palavra(self.fonte, self.arvore.nos[n].inicio)?;
                crate::destaques::variavel_declarada_em(self.p, self.unidade, s.start).map(Declarado::Variavel)
            }
            _ => None,
        }
    }

    /// A resolução casa com o elemento (o `writeOrReadElement`, o acessor
    /// da variável).
    fn resolve_para(&self, r: &Resolved, alvo: Declarado) -> bool {
        let p = self.p.programa();
        let funcao = |f: FunctionElementId| match alvo {
            Declarado::Funcao(g) => f == g,
            Declarado::Variavel(v) => p.function(f).variable == Some(v),
            _ => false,
        };
        match r {
            Resolved::Element(Element::Class(c)) => alvo == Declarado::Classe(*c),
            Resolved::Element(Element::Typedef(t)) => alvo == Declarado::Typedef(*t),
            Resolved::Element(Element::Function(f)) => funcao(*f),
            Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => alvo == Declarado::Variavel(*v),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => funcao(*f),
            _ => false,
        }
    }

    /// `_ElementReferenceCollector` sobre a unidade: os intervalos dos
    /// `SimpleIdentifier` que referem o elemento (expressões, nomes de
    /// campo em inicializadores, o `returnType` dos construtores da
    /// classe, anotações, referências de documentação), dos `NamedType` e
    /// dos `this.x` do campo.
    fn referencias_do_declarado(&self, alvo: Declarado) -> Vec<Span> {
        let p = self.p.programa();
        let u = p.unit(self.unidade);
        let ast = self.ast;
        let mut v: Vec<Span> = Vec::new();
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            let nome = match &e.kind {
                ExprKind::Identifier(n) => n.span,
                ExprKind::Property { name, .. } => name.span,
                _ => continue,
            };
            let por_local = matches!(alvo, Declarado::FuncaoLocal(d) if self.corpos.declaracao_local(id) == Some(d));
            let resolvido = self.corpos.get_resolved(id).is_some_and(|r| self.resolve_para(r, alvo));
            // A chamada `o.m()`/`m()` guarda a resolução na chamada.
            let pela_chamada = ast.exprs.iter().enumerate().any(|(j, c)| match &c.kind {
                ExprKind::Call { target, .. } if *target == id => self.corpos.get_resolved(ast::ExprId(j as u32)).is_some_and(|r| match r {
                    Resolved::Constructor(f) => matches!(alvo, Declarado::Classe(c) if p.function(*f).class == Some(c)) && matches!(e.kind, ExprKind::Identifier(_)),
                    outro => self.resolve_para(outro, alvo),
                }),
                _ => false,
            });
            if por_local || resolvido || pela_chamada {
                v.push(nome);
            }
        }
        let tipo = match alvo {
            Declarado::Classe(c) => Some(Element::Class(c)),
            Declarado::Typedef(t) => Some(Element::Typedef(t)),
            _ => None,
        };
        if let Some(el) = tipo {
            for t in &ast.types {
                let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
                let (b, n) = match &name[..] {
                    [n] if crate::projeto::declaracao_de_parametro_de_tipo(ast, n.span.start, n.sym).is_none() => (p.lookup(u.library, n.sym), *n),
                    [pr, n] => (p.lookup_prefixed(u.library, pr.sym, n.sym), *n),
                    _ => continue,
                };
                if b.and_then(|b| b.getter) == Some(el) {
                    v.push(n.span);
                }
            }
        }
        if let Declarado::Classe(c) = alvo {
            for (mi, m) in ast.members.iter().enumerate() {
                if let MemberKind::Constructor(k) = &m.kind
                    && self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32)).and_then(|f| p.function(f).class) == Some(c)
                {
                    v.push(k.class_name.span);
                }
            }
        }
        if let Declarado::Variavel(campo) = alvo
            && let Some(classe) = p.variable(campo).class
        {
            let nome = p.variable(campo).name;
            for (mi, m) in ast.members.iter().enumerate() {
                let MemberKind::Constructor(k) = &m.kind else { continue };
                if self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32)).and_then(|f| p.function(f).class) != Some(classe) {
                    continue;
                }
                for q in k.parameters.iter() {
                    if q.this_ && q.name.is_some_and(|n| n.sym == nome) {
                        v.push(q.span);
                    }
                }
                for ini in k.initializers.iter() {
                    if let ast::Initializer::Field { name, .. } = ini
                        && name.sym == nome
                    {
                        v.push(name.span);
                    }
                }
            }
        }
        // Anotações: o nome resolve para a classe ou para o acessor.
        for a in crate::projeto::metadados(ast, &u.unit) {
            let b = match &a.name[..] {
                [n, ..] if p.lookup(u.library, n.sym).is_some() => p.lookup(u.library, n.sym).map(|b| (b, n.span)),
                [pr, n, ..] => p.lookup_prefixed(u.library, pr.sym, n.sym).map(|b| (b, n.span)),
                _ => None,
            };
            if let Some((b, s)) = b
                && let Some(g) = b.getter
                && self.resolve_para(&Resolved::Element(g), alvo)
            {
                v.push(s);
            }
        }
        // `[x]` em documentação.
        for c in crate::dartdoc::comentarios(self.fonte) {
            for r in &c.referencias {
                for i in 0..r.len() {
                    let Some((a, concreto)) = self.p.resolver_referencia_doc(self.unidade, c.span, &r[..=i]) else { continue };
                    let casa = match (alvo, &a, concreto) {
                        (_, Alvo::Topo(el), _) => self.resolve_para(&Resolved::Element(*el), alvo),
                        (_, _, Some(Concreto::Funcao(f))) => self.resolve_para(&Resolved::Element(Element::Function(f)), alvo),
                        (Declarado::Variavel(v0), _, Some(Concreto::Variavel(v1))) => v0 == v1,
                        (Declarado::FuncaoLocal(d), Alvo::Local { unidade, declaracao }, _) => *unidade == self.unidade && *declaracao == d,
                        _ => false,
                    };
                    if casa {
                        v.push(r[i].0);
                    }
                }
            }
        }
        v.sort_by_key(|s| (s.start, s.end));
        v.dedup();
        v
    }

    /// `RemoveUnusedElement.compute`: as deleções, ou `None` sem fix.
    pub(crate) fn remover_elemento(&self, erro: Span) -> Option<Vec<Span>> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let especie = self.especie(node);
        if especie == "ConstructorDeclaration" {
            let dono = self.pai(node)?;
            if !matches!(self.especie(dono), "ClassDeclaration" | "EnumDeclaration") {
                return None;
            }
            let membros: Vec<usize> = self
                .filhos(dono)
                .iter()
                .copied()
                .filter(|&f| matches!(self.especie(f), "ConstructorDeclaration" | "MethodDeclaration" | "FieldDeclaration"))
                .collect();
            return Some(vec![self.no_em_lista(&membros, node)]);
        }
        if !matches!(especie, "ClassDeclaration" | "EnumDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "MethodDeclaration" | "VariableDeclaration") {
            return None;
        }
        let alvo = self.declarado(node)?;
        if !self.referencias_do_declarado(alvo).is_empty() {
            return None;
        }
        let pai = self.pai(node);
        let avo = pai.and_then(|x| self.pai(x));
        let faixa = match (especie, pai, avo) {
            ("VariableDeclaration", Some(lista), Some(avo)) if self.especie(lista) == "VariableDeclarationList" => {
                let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
                if variaveis.len() == 1 { self.linhas_do_no(avo) } else { self.no_em_lista(&variaveis, node) }
            }
            _ => self.linhas_do_no(node),
        };
        Some(vec![faixa])
    }

    /// `RemoveUnusedField.compute`: as deleções, ou `None` sem fix.
    pub(crate) fn remover_campo(&self, erro: Span) -> Option<Vec<Span>> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if self.especie(node) != "VariableDeclaration" {
            return None;
        }
        let Declarado::Variavel(campo) = self.declarado(node)? else { return None };
        if self.p.programa().variable(campo).class.is_none() {
            return None;
        }
        let mut referencias = vec![node];
        for s in self.referencias_do_declarado(Declarado::Variavel(campo)) {
            referencias.push(self.arvore.localizar(s.start, s.end)?);
        }
        let mut faixas: Vec<Span> = Vec::new();
        for r in referencias {
            let no = self.este_ou_ancestral(r, |e| matches!(e, "VariableDeclaration" | "ExpressionStatement" | "ConstructorFieldInitializer" | "FieldFormalParameter"))?;
            let pai = self.pai(no);
            let avo = pai.and_then(|x| self.pai(x));
            let faixa = match self.especie(no) {
                "VariableDeclaration" if pai.is_some_and(|l| self.especie(l) == "VariableDeclarationList") && avo.is_some() => {
                    let variaveis = self.filhos_da_especie(pai?, "VariableDeclaration");
                    if variaveis.len() == 1 { self.linhas_do_no(avo?) } else { self.no_em_lista(&variaveis, no) }
                }
                "ConstructorFieldInitializer" => {
                    let construtor = pai?;
                    let inicializadores: Vec<usize> = self
                        .filhos(construtor)
                        .iter()
                        .copied()
                        .filter(|&f| matches!(self.especie(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                        .collect();
                    if inicializadores.len() == 1 {
                        let parametros = self.filhos_da_especie(construtor, "FormalParameterList").first().copied()?;
                        Span { start: self.arvore.nos[parametros].fim, end: self.arvore.nos[no].fim }
                    } else {
                        self.no_em_lista(&inicializadores, no)
                    }
                }
                "FieldFormalParameter" => self.faixa_de_parametro_de_campo(no)?,
                _ => self.linhas_do_no(no),
            };
            faixas.push(faixa);
        }
        // `_uniqueSourceRanges`: sai a faixa coberta por outra (outra
        // entrada da lista, mesmo que igual).
        let unicas: Vec<Span> = faixas
            .iter()
            .enumerate()
            .filter(|(i, c)| !faixas.iter().enumerate().any(|(j, o)| j != *i && o.start <= c.start && c.end <= o.end))
            .map(|(_, c)| *c)
            .collect();
        // Sobreposição parcial: `ConflictingEditException`.
        for (i, a) in unicas.iter().enumerate() {
            for b in &unicas[i + 1..] {
                if a.start < b.end && b.start < a.end {
                    return None;
                }
            }
        }
        Some(unicas)
    }

    // -- AddAsync ------------------------------------------------------------------

    /// `AddAsync` (`add_async.dart`, fora do `missingReturn`) com o
    /// `convertFunctionFromSyncToAsync` do `DartFileEditBuilder`: o
    /// `FunctionBody` ancestral sem palavra-chave ganha `async ` (com um
    /// espaço antes se está colado no token anterior; o corpo vazio não); o
    /// retorno escrito da função ou método envolvente (a closure não tem)
    /// vira `Future<flatten(T)>`, salvo `dynamic` e `Future`. As edições e as
    /// bibliotecas a importar.
    pub(crate) fn adicionar_async(&self, erro: Span) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_types::Type;
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let corpo = self.este_ou_ancestral(node, |e| e.ends_with("FunctionBody"))?;
        let texto = self.texto_do_no(corpo);
        let palavra_chave = |k: &str| texto.strip_prefix(k).is_some_and(|r| !r.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$'));
        if palavra_chave("async") || palavra_chave("sync") {
            return None;
        }
        let mut edicoes = Vec::new();
        let ini = self.arvore.nos[corpo].inicio;
        if self.especie(corpo) != "EmptyFunctionBody" {
            let colado = self.token_anterior(ini).is_some_and(|t| t.end == ini);
            edicoes.push((Span { start: ini, end: ini }, format!("{}async ", if colado { " " } else { "" })));
        }
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, ini);
        // `_replaceReturnTypeWithFuture`.
        let mut atual = self.pai(corpo);
        while let Some(k) = atual {
            match self.especie(k) {
                "FunctionDeclaration" | "MethodDeclaration" => {
                    let anotacao = self
                        .filhos(k)
                        .iter()
                        .copied()
                        .find(|&f| matches!(self.especie(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"));
                    let Some(anotacao) = anotacao else { break };
                    let elemento = self.elemento_declarado(k);
                    let tabela = &self.p.consulta.tabela;
                    let tipo = match elemento {
                        crate::refatoracoes::Elem::Funcao(f) => self.p.consulta.outline.functions.get(f.0 as usize).map(|d| d.return_type),
                        crate::refatoracoes::Elem::FuncaoLocal(fid) => self.ast.function(fid).name.and_then(|n| match self.corpos.tipo_local(n.span.start).map(|t| tabela.get(t)) {
                            Some(Type::Function { ret, .. }) => Some(*ret),
                            _ => None,
                        }),
                        _ => None,
                    };
                    let Some(tipo) = tipo else { break };
                    let futuro = self.p.consulta.core.future_class;
                    let (achatado, anulavel) = match tabela.get(tipo) {
                        Type::Dynamic => break,
                        Type::Interface { class, .. } if Some(*class) == futuro => break,
                        Type::FutureOr { arg, nullable } => (*arg, *nullable),
                        _ => (tipo, false),
                    };
                    let texto_novo = escritor.escrever_future(achatado, anulavel).unwrap_or_else(|| "void".to_string());
                    edicoes.push((self.arvore.span(anotacao), texto_novo));
                    break;
                }
                "FunctionExpression" if self.pai(k).is_none_or(|x| self.especie(x) != "FunctionDeclaration") => break,
                _ => {}
            }
            atual = self.pai(k);
        }
        Some((edicoes, escritor.importar))
    }

    // -- ReplaceWithNotNullAware, ChangeToStaticAccess, RemoveAssertion ----------

    /// `ReplaceWithNotNullAware.compute` no `coveringNode`: o intervalo, o
    /// texto novo e o `{0}` do título.
    pub(crate) fn trocar_operador_null_aware(&self, n: usize) -> Option<(Span, String, &'static str)> {
        let s = self.arvore.span(n);
        // O operador: o primeiro token da seção de cascata, ou o token
        // depois do alvo.
        let operador_depois_do_alvo = |n: usize| -> Option<Span> {
            let primeiro = self.token_seguinte(s.start)?;
            if matches!(self.lexema(primeiro), "?.." | ".." | "?." | ".") && primeiro.start == s.start {
                return Some(primeiro);
            }
            let alvo = *self.filhos(n).first()?;
            self.token_seguinte(self.arvore.nos[alvo].fim)
        };
        match self.especie(n) {
            "MethodInvocation" | "PropertyAccess" => {
                let op = operador_depois_do_alvo(n)?;
                if !matches!(self.lexema(op), "?." | "?.." | "." | "..") {
                    return None;
                }
                let novo = if self.lexema(op) == "?." { "." } else { ".." };
                Some((op, novo.to_string(), novo))
            }
            "IndexExpression" => {
                let primeiro = self.token_seguinte(s.start)?;
                if primeiro.start == s.start && matches!(self.lexema(primeiro), "?.." | "..") {
                    return Some((primeiro, "..".to_string(), ".."));
                }
                let alvo = *self.filhos(n).first()?;
                let q = self.token_seguinte(self.arvore.nos[alvo].fim)?;
                if self.lexema(q) == "?" {
                    return Some((q, String::new(), "["));
                }
                None
            }
            "SpreadElement" => {
                let op = self.token_seguinte(s.start)?;
                Some((op, "...".to_string(), "..."))
            }
            _ => None,
        }
    }

    /// `ChangeToStaticAccess.compute`: o nome da classe ou extensão (`{0}`),
    /// o intervalo do alvo, o `writeReference` dela e as bibliotecas a
    /// importar.
    pub(crate) fn acesso_estatico(&self, erro: Span) -> Option<(String, Span, String, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_elements::model::Element;
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if self.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let pai = self.pai(node)?;
        let alvo = match self.especie(pai) {
            "MethodInvocation" if self.nome_do_metodo(pai) == Some(node) => {
                let f = self.filhos(pai);
                let primeiro = *f.first()?;
                (primeiro != node).then_some(primeiro)?
            }
            "PrefixedIdentifier" if self.filhos(pai).get(1) == Some(&node) => *self.filhos(pai).first()?,
            _ => return None,
        };
        let f = match self.elemento_do_identificador(node, true) {
            crate::refatoracoes::Elem::Funcao(f) => f,
            crate::refatoracoes::Elem::Variavel => {
                let Marca::Expr(x) = self.arvore.nos[node].marca else { return None };
                match self.corpos.get_resolved(x)? {
                    Resolved::Member { member: MemberRef::Variable(v), .. } => self.p.programa().variable(*v).getter?,
                    Resolved::Member { member: MemberRef::Function(f), .. } => *f,
                    _ => return None,
                }
            }
            _ => return None,
        };
        let prog = self.p.programa();
        let fe = prog.function(f);
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[alvo].inicio);
        let (nome, referencia) = if let Some(c) = fe.class {
            let nome = self.p.nome(prog.class(c).name).to_string();
            (nome.clone(), escritor.referencia_de(Element::Class(c), &nome))
        } else if let Some(x) = fe.extension {
            let nome = self.p.nome(prog.extension(x).name?).to_string();
            (nome.clone(), escritor.referencia_de(Element::Extension(x), &nome))
        } else {
            return None;
        };
        Some((nome, self.arvore.span(alvo), referencia, escritor.importar))
    }

    /// `RemoveAssertion`: o `AssertInitializer` filho de construtor sai
    /// por `nodeInList`.
    pub(crate) fn remover_assercao(&self, erro: Span) -> Option<Span> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let pai = self.pai(node)?;
        if self.especie(pai) != "ConstructorDeclaration" {
            return None;
        }
        let inicializadores: Vec<usize> = self
            .filhos(pai)
            .iter()
            .copied()
            .filter(|&f| matches!(self.especie(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        if !inicializadores.contains(&node) {
            return None;
        }
        Some(self.no_em_lista(&inicializadores, node))
    }

    /// `MakeFinal` (`make_final.dart:84-97`) na lista do `node`
    /// (`_getVariableDeclarationList`) com uma variável: `var` → `final`;
    /// `late` → ` final` depois dele; sem palavra-chave → `final ` no começo
    /// da lista.
    pub(crate) fn tornar_final(&self, erro: Span) -> Option<(Span, String)> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let pai = self.pai(node);
        let lista = match self.especie(node) {
            "VariableDeclarationList" => node,
            "VariableDeclaration" | "NamedType" if pai.is_some_and(|x| self.especie(x) == "VariableDeclarationList") => pai?,
            _ => {
                let avo = pai.and_then(|x| self.pai(x))?;
                if pai.is_some_and(|x| self.especie(x) == "NamedType") && self.especie(avo) == "VariableDeclarationList" {
                    avo
                } else {
                    return None;
                }
            }
        };
        if self.filhos_da_especie(lista, "VariableDeclaration").len() != 1 {
            return None;
        }
        let s = self.arvore.span(lista);
        // As palavras-chave do começo da lista (`late`, `var`/`final`/`const`).
        let mut late = None;
        let mut palavra = None;
        let mut pos = s.start;
        while let Some(t) = self.token_seguinte(pos) {
            if t.start >= s.end {
                break;
            }
            match self.lexema(t) {
                "late" => late = Some(t),
                "var" | "final" | "const" => palavra = Some(t),
                _ => break,
            }
            pos = t.end;
        }
        match (palavra, late) {
            (Some(k), _) if self.lexema(k) == "var" => Some((k, "final".to_string())),
            (_, Some(l)) => Some((Span { start: l.end, end: l.end }, " final".to_string())),
            (None, None) => Some((Span { start: s.start, end: s.start }, "final ".to_string())),
            _ => None,
        }
    }

    // -- ConvertIntoBlockBody.missingBody -----------------------------------------

    /// `ConvertIntoBlockBody._computeMissingBody` com `node` no erro: o
    /// corpo envolvente (`getEnclosingFunctionBody`) vazio ou de expressão
    /// vira bloco (`// TODO: implement x` e `throw UnimplementedError();`, ou
    /// `[return ]e;`).
    pub(crate) fn converter_em_corpo_de_bloco(&self, erro: Span) -> Option<(Span, String)> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        // A closure, a função, o construtor ou o método envolvente.
        let dono = ["FunctionExpression", "FunctionDeclaration", "ConstructorDeclaration", "MethodDeclaration"]
            .iter()
            .find_map(|e| self.este_ou_ancestral(node, |x| x == *e))?;
        let corpo_de = |n: usize| self.filhos(n).iter().copied().rev().find(|&f| self.especie(f).ends_with("FunctionBody"));
        let corpo = match self.especie(dono) {
            "FunctionDeclaration" => {
                let expr = self.filhos(dono).iter().copied().find(|&f| self.especie(f) == "FunctionExpression")?;
                corpo_de(expr)?
            }
            _ => corpo_de(dono)?,
        };
        let texto_do_corpo = self.texto_do_no(corpo);
        if texto_do_corpo.starts_with("sync*") || texto_do_corpo.starts_with("async*") {
            return None;
        }
        // `_getFunctionElement(body.parent)`: método, construtor ou
        // expressão de função.
        let pai = self.pai(corpo)?;
        let elemento = match self.especie(pai) {
            "MethodDeclaration" | "FunctionExpression" | "ConstructorDeclaration" => self.elemento_do_corpo(pai)?,
            _ => return None,
        };
        let linhas: Vec<String> = match self.especie(corpo) {
            "EmptyFunctionBody" => {
                let mut l = vec![format!("// TODO: implement {}", self.nome_exibido(pai)?)];
                if !self.retorna_void(elemento) {
                    l.push("throw UnimplementedError();".to_string());
                }
                l
            }
            "ExpressionFunctionBody" => {
                let expr = *self.filhos(corpo).first()?;
                if erro.start >= self.arvore.nos[expr].inicio {
                    return None;
                }
                let tipo = self.tipo_do_no(expr).map(|t| self.p.consulta.tabela.get(t).clone());
                let void_ou_fundo = matches!(tipo, Some(dartforge_types::Type::Void) | Some(dartforge_types::Type::Never));
                let retorno = if !void_ou_fundo && !self.retorna_void(elemento) { "return " } else { "" };
                vec![format!("{retorno}{};", self.texto_do_no(expr))]
            }
            _ => return None,
        };
        let tx = Texto::novo(self.fonte);
        let prefixo = self.prefixo_do_no(pai);
        let eol = tx.eol();
        let anterior = self.token_anterior(self.arvore.nos[corpo].inicio)?;
        let mut s = String::from(" ");
        if texto_do_corpo.starts_with("async") {
            s.push_str("async ");
        }
        s.push('{');
        for l in &linhas {
            s.push_str(eol);
            s.push_str(&prefixo);
            s.push_str(crate::refatoracoes_exec::UM_RECUO);
            s.push_str(l);
        }
        s.push_str(eol);
        s.push_str(&prefixo);
        s.push('}');
        Some((Span { start: anterior.end, end: self.arvore.nos[corpo].fim }, s))
    }

    /// O elemento executável de um método, construtor ou expressão de função.
    fn elemento_do_corpo(&self, n: usize) -> Option<crate::refatoracoes::Elem> {
        use crate::refatoracoes::Elem;
        match (self.especie(n), self.arvore.nos[n].marca) {
            ("MethodDeclaration", _) => Some(self.elemento_declarado(n)).filter(|e| *e != Elem::Nenhum),
            ("FunctionExpression", Marca::Funcao(fid)) => Some(match self.p.funcao_do_no(self.unidade, fid) {
                Some(f) => Elem::Funcao(f),
                None => Elem::FuncaoLocal(fid),
            }),
            ("FunctionExpression", _) => {
                // A expressão de uma `FunctionDeclaration`: a marca está no pai.
                let pai = self.pai(n)?;
                Some(self.elemento_declarado(pai)).filter(|e| *e != Elem::Nenhum)
            }
            ("ConstructorDeclaration", _) => {
                let s = self.arvore.span(n);
                let (mi, _) = self.ast.members.iter().enumerate().find(|(_, m)| matches!(m.kind, MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)?;
                self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32)).map(Elem::Funcao)
            }
            _ => None,
        }
    }

    /// O `displayName` do executável (o nome sem `=`; o construtor é o da
    /// classe com `.nome`).
    fn nome_exibido(&self, n: usize) -> Option<String> {
        use crate::refatoracoes::Elem;
        match self.elemento_do_corpo(n)? {
            Elem::Funcao(f) => {
                let p = self.p.programa();
                let fe = p.function(f);
                let nome = crate::projeto::nome_base(self.p.nome(fe.name)).to_string();
                if fe.kind == dartforge_elements::model::FunctionKind::Constructor {
                    let classe = fe.class.map(|c| self.p.nome(p.class(c).name).to_string()).unwrap_or_default();
                    return Some(if nome.is_empty() { classe } else { format!("{classe}.{nome}") });
                }
                Some(nome)
            }
            Elem::FuncaoLocal(fid) => Some(self.ast.function(fid).name.map(|x| self.lexema(x.span).to_string()).unwrap_or_default()),
            _ => None,
        }
    }

    /// `_forFieldFormalParameter`.
    fn faixa_de_parametro_de_campo(&self, no: usize) -> Option<Span> {
        let parametro = match self.pai(no) {
            Some(p) if self.especie(p) == "DefaultFormalParameter" => p,
            _ => no,
        };
        let lista = self.pai(parametro)?;
        let s = self.arvore.span(parametro);
        let parametros: Vec<usize> = self.filhos(lista).to_vec();
        if parametros.len() == 1 {
            let ls = self.arvore.span(lista);
            return Some(Span { start: ls.start + 1, end: ls.end - 1 });
        }
        let mut anterior = self.token_anterior(s.start)?;
        let mut seguinte = self.token_seguinte(s.end)?;
        if self.lexema(seguinte) == "," {
            seguinte = self.token_seguinte(seguinte.end)?;
            return Some(Span { start: s.start, end: seguinte.start });
        }
        let primeiro_opcional = matches!(self.lexema(anterior), "{" | "[");
        if primeiro_opcional {
            anterior = self.token_anterior(anterior.start)?;
            if matches!(self.lexema(seguinte), "}" | "]") {
                seguinte = self.token_seguinte(seguinte.end)?;
            }
        }
        let antes = self.token_anterior(anterior.start)?;
        Some(Span { start: antes.end, end: seguinte.start })
    }
}

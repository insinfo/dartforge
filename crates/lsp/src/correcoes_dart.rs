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

    // -- Comentários e intervalos com comentários ------------------------------

    /// A linha (0-based) do offset.
    fn linha(&self, o: usize) -> usize {
        let b = self.fonte.as_bytes();
        let fim = o.min(b.len());
        let mut n = 0;
        let mut i = 0;
        while i < fim {
            match b[i] {
                b'\r' if b.get(i + 1) == Some(&b'\n') => {
                    n += 1;
                    i += 2;
                    continue;
                }
                b'\r' | b'\n' => n += 1,
                _ => {}
            }
            i += 1;
        }
        n
    }

    /// Os comentários entre o token anterior e o token `t`
    /// (`precedingComments`, em ordem).
    fn comentarios_antes(&self, t: Span) -> Vec<Span> {
        let a = self.token_anterior(t.start).map_or(0, |p| p.end);
        self.comentarios.iter().copied().filter(|c| c.start >= a && c.end <= t.start).collect()
    }

    /// O primeiro e o último token do nó.
    fn tokens_do_no(&self, n: usize) -> Option<(Span, Span)> {
        let s = self.arvore.span(n);
        Some((self.token_seguinte(s.start)?, self.token_anterior(s.end)?))
    }

    /// `range.deletionRange(node, overrideEnd)`.
    pub(crate) fn faixa_de_exclusao(&self, n: usize, fim_forcado: Option<Span>) -> Option<Span> {
        let (primeiro, ultimo) = self.tokens_do_no(n)?;
        let comeco = self.comentarios_antes(primeiro).first().copied().unwrap_or(primeiro);
        let fim_inicial = fim_forcado.unwrap_or(ultimo);
        let seguinte = self.token_seguinte(fim_inicial.end);
        let depois: Option<Span> = match seguinte {
            Some(t) => Some(self.comentarios_antes(t).first().copied().unwrap_or(t)),
            None => self.comentarios.iter().copied().find(|c| c.start >= fim_inicial.end),
        };
        match depois {
            Some(e) => Some(Span { start: comeco.start, end: e.start }),
            None => {
                // `end.isEof`.
                let e_comentario = comeco != primeiro;
                let anotado = matches!(self.especie(n), "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "MethodDeclaration" | "FieldDeclaration" | "ConstructorDeclaration" | "TopLevelVariableDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias");
                let inicio = if anotado && e_comentario {
                    let primeiro_depois = self.primeiro_token_apos_comentario_e_metadados(n)?;
                    self.token_anterior(primeiro_depois.start).map_or(primeiro_depois.start, |p| p.end)
                } else {
                    match self.token_anterior(comeco.start) {
                        None => comeco.start,
                        Some(p) => p.end,
                    }
                };
                Some(Span { start: inicio, end: fim_inicial.end })
            }
        }
    }

    /// `firstTokenAfterCommentAndMetadata`.
    pub(crate) fn primeiro_token_apos_comentario_e_metadados(&self, n: usize) -> Option<Span> {
        let mut pos = self.arvore.nos[n].inicio;
        for &f in self.filhos(n) {
            if matches!(self.especie(f), "Comment" | "Annotation") {
                pos = pos.max(self.arvore.nos[f].fim);
            }
        }
        self.token_seguinte(pos)
    }

    /// `_leadingComment(token)`.
    fn comentario_inicial(&self, t: Span) -> Span {
        let antes = self.comentarios_antes(t);
        let Some(anterior) = self.token_anterior(t.start) else {
            return antes.first().copied().unwrap_or(t);
        };
        let linha_t = self.linha(t.start);
        let linha_anterior = self.linha(anterior.start);
        let mut i = 0;
        if linha_t != linha_anterior {
            while i < antes.len() && self.linha(antes[i].start) == linha_anterior {
                i += 1;
            }
        }
        antes.get(i).copied().unwrap_or(t)
    }

    /// `_shouldIncludeCommentsAfterComma`.
    fn incluir_comentarios_apos_virgula(&self, virgula: Span) -> bool {
        let Some(depois) = self.token_seguinte(virgula.end) else { return false };
        if matches!(self.lexema(depois), "}" | ")" | "]") {
            return true;
        }
        self.linha(virgula.start) != self.linha(depois.start)
    }

    /// `trailingComment(token, returnComma)`: o token (ou comentário) e se
    /// inclui a vírgula.
    fn comentario_final(&self, t: Span, devolver_virgula: bool) -> (Span, bool) {
        let mut ultimo = t;
        let mut seguinte = self.token_seguinte(ultimo.end);
        let inclui_virgula = seguinte.is_some_and(|s| self.lexema(s) == "," && self.incluir_comentarios_apos_virgula(s));
        if inclui_virgula {
            ultimo = seguinte.unwrap_or(t);
            seguinte = self.token_seguinte(ultimo.end);
        }
        let precedentes = |s: Option<Span>, depois_de: usize| -> Vec<Span> {
            match s {
                Some(s) => self.comentarios_antes(s),
                None => self.comentarios.iter().copied().filter(|c| c.start >= depois_de).collect(),
            }
        };
        let mut cadeia = precedentes(seguinte, ultimo.end);
        if cadeia.is_empty() && inclui_virgula && self.linha(t.start) != self.linha(ultimo.start) {
            cadeia = self.comentarios_antes(ultimo);
            ultimo = t;
        }
        if let Some(&primeiro) = cadeia.first() {
            let linha = self.linha(ultimo.start);
            if self.linha(primeiro.start) == linha {
                let mut c = primeiro;
                for &prox in &cadeia[1..] {
                    if self.linha(prox.start) != linha {
                        break;
                    }
                    c = prox;
                }
                return (c, inclui_virgula);
            }
        }
        (if devolver_virgula { ultimo } else { t }, false)
    }

    /// `range.nodeInListWithComments(lineInfo, list, node)`.
    fn no_em_lista_com_comentarios(&self, lista: &[usize], node: usize) -> Option<Span> {
        let (ini, fim) = self.tokens_do_no(node)?;
        if lista.len() == 1 {
            let inicial = self.comentario_inicial(ini);
            let (final_, _) = self.comentario_final(fim, true);
            return Some(Span { start: inicial.start, end: final_.end });
        }
        let i = lista.iter().position(|&x| x == node)?;
        if i == 0 {
            let este = self.comentario_inicial(ini);
            let (ini_prox, _) = self.tokens_do_no(lista[1])?;
            let proximo = self.comentario_inicial(ini_prox);
            return Some(Span { start: este.start, end: proximo.start });
        }
        let (_, fim_anterior) = self.tokens_do_no(lista[i - 1])?;
        let (anterior, virgula_anterior) = self.comentario_final(fim_anterior, false);
        let (este, virgula_este) = self.comentario_final(fim, virgula_anterior);
        let mut token_anterior = anterior;
        if !virgula_anterior && virgula_este {
            token_anterior = self.token_seguinte(token_anterior.end)?;
        }
        Some(Span { start: token_anterior.end, end: este.end })
    }

    /// `range.nodeWithComments(lineInfo, node)`.
    fn no_com_comentarios(&self, node: usize) -> Option<Span> {
        let (ini, fim) = self.tokens_do_no(node)?;
        let primeiro_da_unidade = self.token_seguinte(0) == Some(ini);
        let inicial = if primeiro_da_unidade { ini } else { self.comentario_inicial(ini) };
        let (final_, _) = self.comentario_final(fim, false);
        Some(Span { start: inicial.start, end: final_.end })
    }

    // -- Produtores dos códigos publicados sem fix ---------------------------------

    /// `RemoveNameFromDeclarationClause`: o título e a deleção.
    pub(crate) fn remover_nome_da_clausula(&self, erro: Span) -> Option<(String, Span)> {
        let tipo = self.arvore.localizar(erro.start, erro.end)?;
        let clausula = self.pai(tipo)?;
        let nome = match self.especie(clausula) {
            "ExtendsClause" => return Some(("Remove 'extends' clause".to_string(), self.faixa_de_exclusao(clausula, None)?)),
            "ImplementsClause" => "implements",
            "MixinOnClause" => "on",
            "WithClause" => "with",
            _ => return None,
        };
        let nomes = self.filhos_da_especie(clausula, "NamedType");
        if nomes.len() == 1 {
            return Some((format!("Remove '{nome}' clause"), self.faixa_de_exclusao(clausula, None)?));
        }
        if !nomes.contains(&tipo) {
            return None;
        }
        Some((format!("Remove name from '{nome}' clause"), self.no_em_lista(&nomes, tipo)))
    }

    /// `AddClassModifier`: o ponto de inserção do modificador.
    pub(crate) fn ponto_do_modificador(&self, erro: Span) -> Option<usize> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if !matches!(
            self.especie(node),
            "ClassDeclaration" | "ClassTypeAlias" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias"
        ) || self.pai(node) != Some(0)
        {
            return None;
        }
        Some(self.primeiro_token_apos_comentario_e_metadados(node)?.start)
    }

    /// `UseEqEqNull`/`UseNotEqNull`: `[expression.end, is.end)`.
    pub(crate) fn faixa_do_is_null(&self, erro: Span) -> Option<Span> {
        let n = self.arvore.localizar2(erro.start, erro.end.saturating_sub(1))?;
        if self.especie(n) != "IsExpression" {
            return None;
        }
        let e = *self.filhos(n).first()?;
        Some(Span { start: self.arvore.nos[e].fim, end: self.arvore.nos[n].fim })
    }

    /// `RemoveExtendsClause`: `[extends, {)`.
    pub(crate) fn remover_extends(&self, erro: Span) -> Option<Span> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let classe = self.este_ou_ancestral(node, |e| e == "ClassDeclaration")?;
        let extends = self.filhos_da_especie(classe, "ExtendsClause").first().copied()?;
        let mut pos = self.arvore.nos[extends].fim;
        loop {
            let t = self.token_seguinte(pos)?;
            if self.lexema(t) == "{" {
                return Some(Span { start: self.arvore.nos[extends].inicio, end: t.start });
            }
            pos = t.end;
        }
    }

    /// `ExtendClassForMixin`: o nome (o último trecho entre aspas da
    /// mensagem) e o ponto depois dos parâmetros de tipo ou do nome.
    pub(crate) fn estender_para_mixin(&self, erro: Span, mensagem: &str) -> Option<(String, usize)> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let classe = self.este_ou_ancestral(node, |e| e == "ClassDeclaration")?;
        if !self.filhos_da_especie(classe, "ExtendsClause").is_empty() {
            return None;
        }
        let fim = mensagem.rfind('\'')?;
        let ini = mensagem[..fim].rfind('\'').map_or(0, |i| i + 1);
        let nome = mensagem[ini..fim].to_string();
        let ponto = match self.filhos_da_especie(classe, "TypeParameterList").first() {
            Some(&tp) => self.arvore.nos[tp].fim,
            None => {
                let Marca::Decl(d) = self.arvore.nos[classe].marca else { return None };
                match &self.ast.decl(d).kind {
                    ast::DeclKind::Class(c) => c.name.span.end,
                    _ => return None,
                }
            }
        };
        Some((nome, ponto))
    }

    /// `ReplaceWithExtensionName`: o nome (com o prefixo) e o alvo.
    pub(crate) fn nome_da_extensao(&self, erro: Span) -> Option<(String, Span)> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if self.especie(node) != "SimpleIdentifier" {
            return None;
        }
        let pai = self.pai(node)?;
        let alvo = match self.especie(pai) {
            "MethodInvocation" if self.nome_do_metodo(pai) == Some(node) => *self.filhos(pai).first()?,
            "PropertyAccess" if self.filhos(pai).get(1) == Some(&node) => *self.filhos(pai).first()?,
            _ => return None,
        };
        if alvo == node {
            return None;
        }
        // O `ExtensionOverride`: `E(x)`, `E<T>(x)` ou `p.E(x)`, com o nome
        // resolvido para a extensão.
        let ids: Vec<usize> = self.filhos(alvo).iter().copied().filter(|&f| self.especie(f) == "SimpleIdentifier").collect();
        let extensao = ids.iter().copied().find(|&i| match self.arvore.nos[i].marca {
            Marca::Expr(x) => matches!(self.corpos.get_resolved(x), Some(Resolved::Element(dartforge_elements::model::Element::Extension(_)))),
            _ => false,
        })?;
        let prefixo = ids.iter().copied().find(|&i| i != extensao && self.arvore.nos[i].fim <= self.arvore.nos[extensao].inicio);
        let nome = match prefixo {
            Some(p) => format!("{}.{}", self.texto_do_no(p), self.texto_do_no(extensao)),
            None => self.texto_do_no(extensao).to_string(),
        };
        Some((nome, self.arvore.span(alvo)))
    }

    /// `RemoveParenthesesInGetterInvocation`: a lista de argumentos da
    /// `FunctionExpressionInvocation` pai do `coveringNode`.
    pub(crate) fn parenteses_do_getter(&self, erro: Span) -> Option<Span> {
        let n = self.arvore.localizar2(erro.start, erro.end.saturating_sub(1))?;
        let pai = self.pai(n)?;
        if self.especie(pai) != "FunctionExpressionInvocation" {
            return None;
        }
        let args = self.filhos_da_especie(pai, "ArgumentList").first().copied()?;
        Some(self.arvore.span(args))
    }

    /// `MakeSuperInvocationLast`: a deleção e a inserção `, <texto>`.
    pub(crate) fn super_por_ultimo(&self, erro: Span) -> Option<Vec<(Span, String)>> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if !matches!(self.especie(node), "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "ConstructorFieldInitializer" | "AssertInitializer") {
            return None;
        }
        let construtor = self.pai(node).filter(|&c| self.especie(c) == "ConstructorDeclaration")?;
        let inicializadores: Vec<usize> = self
            .filhos(construtor)
            .iter()
            .copied()
            .filter(|&f| matches!(self.especie(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let exclusao = self.no_em_lista_com_comentarios(&inicializadores, node)?;
        let faixa = self.no_com_comentarios(node)?;
        let texto = self.fonte[faixa.start..faixa.end].to_string();
        let (_, fim_ultimo) = self.tokens_do_no(*inicializadores.last()?)?;
        let (t, _) = self.comentario_final(fim_ultimo, false);
        Some(vec![(exclusao, String::new()), (Span { start: t.end, end: t.end }, format!(", {texto}"))])
    }

    /// `RemoveDeadCode` num `SwitchMember` (o `coveringNode` dos
    /// `unreachable_switch_*`): `deletionRange(member)`, até o `:` quando o
    /// membro anterior não tem comandos.
    pub(crate) fn remover_membro_morto(&self, erro: Span) -> Option<Span> {
        let n = self.arvore.localizar2(erro.start, erro.end.saturating_sub(1))?;
        if !matches!(self.especie(n), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase") {
            return None;
        }
        let switch = self.pai(n)?;
        if self.especie(switch) != "SwitchStatement" {
            return None;
        }
        let membros: Vec<usize> = self.filhos(switch).iter().copied().filter(|&f| matches!(self.especie(f), "SwitchCase" | "SwitchDefault" | "SwitchPatternCase")).collect();
        let i = membros.iter().position(|&m| m == n)?;
        let sem_comandos = |m: usize| !self.filhos(m).iter().any(|&f| self.e_comando(f));
        let mut fim_forcado = None;
        if i > 0 && sem_comandos(membros[i - 1]) {
            // O `:` do membro: depois do padrão, da guarda ou do `default`.
            let depois = self
                .filhos(n)
                .iter()
                .copied()
                .filter(|&f| !self.e_comando(f) && self.especie(f) != "Label")
                .map(|f| self.arvore.nos[f].fim)
                .max()
                .unwrap_or(self.arvore.nos[n].inicio);
            let mut pos = depois;
            loop {
                let t = self.token_seguinte(pos)?;
                if self.lexema(t) == ":" {
                    fim_forcado = Some(t);
                    break;
                }
                pos = t.end;
            }
        }
        self.faixa_de_exclusao(n, fim_forcado)
    }

    /// `RemoveComparison` (`remove_comparison.dart`): `verdadeira` para os
    /// `…_TRUE`, falsa para os `…_FALSE`.
    pub(crate) fn remover_comparacao(&self, erro: Span, verdadeira: bool) -> Option<Vec<(Span, String)>> {
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let pai = self.pai(node)?;
        let tx = Texto::novo(self.fonte);
        let tirar_recuo = |texto: &str| -> String {
            let eol = tx.eol();
            texto
                .split(eol)
                .map(|l| l.strip_prefix(crate::refatoracoes_exec::UM_RECUO).unwrap_or(l))
                .collect::<Vec<_>>()
                .join(eol)
        };
        let com_comentarios = |n: usize| -> Option<String> {
            let (ini, fim) = self.tokens_do_no(n)?;
            let primeiro = self.comentarios_antes(ini).first().copied().unwrap_or(ini);
            Some(self.fonte[primeiro.start..fim.end].to_string())
        };
        match self.especie(pai) {
            "AssertInitializer" if verdadeira => {
                let construtor = self.pai(pai)?;
                let lista: Vec<usize> = self
                    .filhos(construtor)
                    .iter()
                    .copied()
                    .filter(|&f| matches!(self.especie(f), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                    .collect();
                if lista.len() == 1 {
                    let parametros = self.filhos_da_especie(construtor, "FormalParameterList").first().copied()?;
                    return Some(vec![(Span { start: self.arvore.nos[parametros].fim, end: self.arvore.nos[pai].fim }, String::new())]);
                }
                Some(vec![(self.no_em_lista(&lista, pai), String::new())])
            }
            "AssertStatement" if verdadeira => Some(vec![(self.linhas_do_no(pai), String::new())]),
            "BinaryExpression" => {
                let f = self.filhos(pai);
                let (esq, dir) = (*f.first()?, *f.get(1)?);
                let operador = self.token_seguinte(self.arvore.nos[esq].fim)?;
                let e_e = self.lexema(operador) == "&&";
                let ou = self.lexema(operador) == "||";
                if !((e_e && verdadeira) || (ou && !verdadeira)) {
                    return None;
                }
                let s = if esq == node {
                    Span { start: self.arvore.nos[node].inicio, end: self.arvore.nos[dir].inicio }
                } else {
                    Span { start: self.arvore.nos[esq].fim, end: self.arvore.nos[node].fim }
                };
                Some(vec![(s, String::new())])
            }
            "IfElement" => {
                let ramos: Vec<usize> = self.filhos(pai).iter().copied().filter(|&f| f != node).collect();
                let (entao, senao) = (ramos.first().copied(), ramos.get(1).copied());
                if verdadeira {
                    let t = com_comentarios(entao?)?;
                    return Some(vec![(self.arvore.span(pai), tirar_recuo(&t))]);
                }
                match senao {
                    Some(e) => {
                        let t = com_comentarios(e)?;
                        Some(vec![(self.arvore.span(pai), tirar_recuo(&t))])
                    }
                    None => {
                        let colecao = self.pai(pai)?;
                        if !matches!(self.especie(colecao), "ListLiteral" | "SetOrMapLiteral") {
                            return None;
                        }
                        let elementos: Vec<usize> = self.filhos(colecao).iter().copied().filter(|&f| self.especie(f) != "TypeArgumentList").collect();
                        Some(vec![(self.no_em_lista(&elementos, pai), String::new())])
                    }
                }
            }
            "IfStatement" => {
                let ramos: Vec<usize> = self.filhos(pai).iter().copied().filter(|&f| f != node).collect();
                let (entao, senao) = (ramos.first().copied(), ramos.get(1).copied());
                let substituir = |r: usize| -> Option<Vec<(Span, String)>> {
                    if self.especie(r) == "Block" {
                        let s = self.arvore.span(r);
                        let linhas = tx.faixa_de_linhas(s.start + 1, s.end - 1);
                        let texto = tirar_recuo(&self.fonte[linhas.start..linhas.end]);
                        return Some(vec![(self.linhas_do_no(pai), texto)]);
                    }
                    let t = com_comentarios(r)?;
                    Some(vec![(self.arvore.span(pai), tirar_recuo(&t))])
                };
                if verdadeira {
                    return substituir(entao?);
                }
                match senao {
                    Some(e) => substituir(e),
                    None => {
                        let bloco = self.pai(pai).filter(|&b| self.especie(b) == "Block")?;
                        let comandos: Vec<usize> = self.filhos(bloco).to_vec();
                        Some(vec![(self.no_em_lista(&comandos, pai), String::new())])
                    }
                }
            }
            _ => None,
        }
    }

    /// `MakeReturnTypeNullable`: o fim da anotação de retorno (ou do
    /// argumento de tipo, num corpo assíncrono ou gerador).
    pub(crate) fn retorno_anulavel(&self, erro: Span, conversoes_estritas: bool) -> Option<usize> {
        use dartforge_types::Type;
        let node = self.arvore.localizar(erro.start, erro.end)?;
        if !self.e_expressao(node) {
            return None;
        }
        let tipo = self.tipo_do_no(node)?;
        let corpo = self.este_ou_ancestral(node, |e| e.ends_with("FunctionBody"))?;
        let mut funcao = self.pai(corpo)?;
        if self.especie(funcao) == "FunctionExpression" {
            funcao = self.pai(funcao)?;
        }
        if !matches!(self.especie(funcao), "MethodDeclaration" | "FunctionDeclaration") {
            return None;
        }
        let anotacao = self
            .filhos(funcao)
            .iter()
            .copied()
            .find(|&f| matches!(self.especie(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"))?;
        let elemento = self.elemento_declarado(funcao);
        let tabela = &self.p.consulta.tabela;
        let mut declarado = match elemento {
            crate::refatoracoes::Elem::Funcao(f) => self.p.consulta.outline.functions.get(f.0 as usize).map(|d| d.return_type)?,
            crate::refatoracoes::Elem::FuncaoLocal(fid) => match self.ast.function(fid).name.and_then(|n| self.corpos.tipo_local(n.span.start)).map(|t| tabela.get(t)) {
                Some(Type::Function { ret, .. }) => *ret,
                _ => return None,
            },
            _ => return None,
        };
        let mut alvo = anotacao;
        let texto_do_corpo = self.texto_do_no(corpo);
        if texto_do_corpo.starts_with("async") || texto_do_corpo.starts_with("sync") {
            if self.especie(anotacao) != "NamedType" {
                return None;
            }
            let args = self.filhos_da_especie(anotacao, "TypeArgumentList").first().copied()?;
            let lista = self.filhos(args);
            if lista.len() != 1 {
                return None;
            }
            alvo = lista[0];
            declarado = match tabela.get(declarado) {
                Type::Interface { args, .. } if args.len() == 1 => args[0],
                Type::FutureOr { arg, .. } => *arg,
                _ => return None,
            };
        }
        if self.especie(node) != "NullLiteral" {
            let mut t = tabela.clone();
            let nao_nulo = dartforge_types::non_nullable(tipo, &mut t);
            let atribuivel = if !conversoes_estritas && matches!(t.get(declarado), Type::Dynamic) {
                true
            } else {
                let mut env = dartforge_types::SubtypeEnv::new(&mut t, &self.p.consulta.outline.hierarchy, &self.p.consulta.core);
                dartforge_types::is_subtype(declarado, nao_nulo, &mut env)
            };
            if !atribuivel {
                return None;
            }
        }
        Some(self.arvore.nos[alvo].fim)
    }

    // -- RemoveUnusedParameter e AddConst ------------------------------------------

    /// O tipo (`isRequiredPositional`) de um nó de parâmetro, pelo
    /// parâmetro do parser no intervalo dele.
    fn posicional_obrigatorio(&self, n: usize) -> bool {
        let s = self.arvore.span(n);
        let dentro = |q: &&ast::Parameter| q.span.start >= s.start && q.span.end <= s.end;
        let q = self
            .ast
            .functions
            .iter()
            .flat_map(|f| f.parameters.iter().flatten())
            .find(dentro)
            .or_else(|| {
                self.ast.members.iter().find_map(|m| match &m.kind {
                    MemberKind::Constructor(k) => k.parameters.iter().find(dentro),
                    _ => None,
                })
            });
        q.is_some_and(|q| q.kind == ast::ParameterKind::Required)
    }

    /// `RemoveUnusedParameter.compute`: a deleção.
    pub(crate) fn remover_parametro(&self, erro: Span) -> Option<Span> {
        let mut talvez = self.arvore.localizar(erro.start, erro.end)?;
        if self.especie(talvez) == "SimpleIdentifier"
            && let Some(p) = self.pai(talvez)
        {
            talvez = p;
        }
        if !matches!(self.especie(talvez), "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter") {
            return None;
        }
        let mut parametro = talvez;
        if let Some(p) = self.pai(parametro)
            && self.especie(p) == "DefaultFormalParameter"
        {
            parametro = p;
        }
        let lista = self.pai(parametro).filter(|&l| self.especie(l) == "FormalParameterList")?;
        let parametros: Vec<usize> = self.filhos(lista).to_vec();
        let i = parametros.iter().position(|&x| x == parametro)?;
        let s = |n: usize| self.arvore.span(n);
        let ls = s(lista);
        if i == 0 {
            if parametros.len() == 1 {
                return Some(Span { start: ls.start + 1, end: ls.end - 1 });
            }
            let seguinte = parametros[1];
            if self.posicional_obrigatorio(parametro) && !self.posicional_obrigatorio(seguinte) {
                // O delimitador `[`/`{`: o token antes do primeiro opcional.
                let delimitador = self.token_anterior(s(seguinte).start).filter(|t| matches!(&self.fonte[t.start..t.end], "[" | "{"));
                return Some(match delimitador {
                    Some(d) => Span { start: s(parametro).start, end: d.start },
                    None => Span { start: s(parametro).start, end: s(seguinte).start },
                });
            }
            return Some(Span { start: s(parametro).start, end: s(seguinte).start });
        }
        let anterior = parametros[i - 1];
        if self.posicional_obrigatorio(anterior) && !self.posicional_obrigatorio(parametro) {
            if i == parametros.len() - 1 {
                // `)` ou a vírgula final antes dele.
                let fecha = Span { start: ls.end - 1, end: ls.end };
                let antes = self.token_anterior(fecha.start);
                let alvo = match antes {
                    Some(a) if &self.fonte[a.start..a.end] == "," => a,
                    _ => fecha,
                };
                return Some(Span { start: s(anterior).end, end: alvo.start });
            }
            let seguinte = parametros[i + 1];
            return Some(Span { start: s(parametro).start, end: s(seguinte).start });
        }
        Some(Span { start: s(anterior).end, end: s(parametro).end })
    }

    /// `AddConst.compute` nos caminhos que o `non_constant_map_pattern_key`
    /// alcança (a chave do `MapPatternEntry`): binário ou prefixo →
    /// `const (…)` na entrada; lista, conjunto/mapa ou criação sem palavra
    /// → `const ` e os `const` internos apagados.
    pub(crate) fn adicionar_const(&self, erro: Span) -> Option<Vec<(Span, String)>> {
        let mut alvo = self.arvore.localizar(erro.start, erro.end)?;
        if self.especie(alvo) == "SimpleIdentifier" {
            alvo = self.pai(alvo)?;
        }
        if self.especie(alvo) == "ConstructorDeclaration" {
            let t = self.primeiro_token_apos_comentario_e_metadados(alvo)?;
            return Some(vec![(Span { start: t.start, end: t.start }, "const ".to_string())]);
        }
        let parenteses_e_const = |n: usize| {
            let s = self.arvore.span(n);
            vec![(Span { start: s.end, end: s.end }, ")".to_string()), (Span { start: s.start, end: s.start }, "const (".to_string())]
        };
        if self.especie(alvo) == "TypeArgumentList" {
            alvo = self.este_ou_ancestral(alvo, |e| e == "ConstantPattern")?;
        }
        if self.especie(alvo) == "ConstantPattern" {
            // `canBeConst`/literal de tipo: fora dos códigos publicados.
            return None;
        }
        if matches!(self.especie(alvo), "BinaryExpression" | "PrefixExpression") {
            let pai = self.pai(alvo)?;
            if let Some(avo) = self.pai(pai)
                && self.especie(avo) == "ParenthesizedPattern"
            {
                let o = self.arvore.nos[avo].inicio;
                return Some(vec![(Span { start: o, end: o }, "const ".to_string())]);
            }
            return Some(parenteses_e_const(pai));
        }
        let inserir = |n: usize| -> Vec<(Span, String)> {
            let o = self.arvore.nos[n].inicio;
            let mut v = vec![(Span { start: o, end: o }, "const ".to_string())];
            // `_ConstRangeFinder`: os `const` de criações e literais
            // internos (sem entrar em closures).
            let mut pilha = vec![n];
            while let Some(k) = pilha.pop() {
                if self.especie(k) == "FunctionExpression" {
                    continue;
                }
                if matches!(self.especie(k), "InstanceCreationExpression" | "ListLiteral" | "SetOrMapLiteral")
                    && let Some(t) = self.token_seguinte(self.arvore.nos[k].inicio)
                    && &self.fonte[t.start..t.end] == "const"
                    && let Some(prox) = self.token_seguinte(t.end)
                {
                    v.push((Span { start: t.start, end: prox.start }, String::new()));
                }
                for &f in self.filhos(k).iter().rev() {
                    pilha.push(f);
                }
            }
            v
        };
        if matches!(self.especie(alvo), "ListLiteral" | "SetOrMapLiteral") {
            return Some(inserir(alvo));
        }
        if self.especie(alvo) == "NamedType" {
            alvo = self.pai(alvo)?;
        }
        if self.especie(alvo) == "ConstructorName" {
            alvo = self.pai(alvo)?;
        }
        if self.especie(alvo) == "InstanceCreationExpression" {
            let sem_palavra = self.token_seguinte(self.arvore.nos[alvo].inicio).is_some_and(|t| !matches!(&self.fonte[t.start..t.end], "new" | "const"));
            if sem_palavra {
                return Some(inserir(alvo));
            }
        }
        None
    }

    // -- AddMissingSwitchCases --------------------------------------------------------

    /// `AddMissingSwitchCases.compute` com `insertCaseClauseAtEnd`: o
    /// `node` é o `switch`; as testemunhas com partes vêm do verificador de
    /// constantes (o `diagnostic.data`). A edição e as bibliotecas a
    /// importar.
    pub(crate) fn casos_ausentes(&self, erro: Span) -> Option<(Span, String, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        use dartforge_types::constantes::ParteDeTestemunha as P;
        let node = self.arvore.localizar(erro.start, erro.end)?;
        let expressao = match self.especie(node) {
            "SwitchStatement" => false,
            "SwitchExpression" => true,
            _ => return None,
        };
        let prog = self.p.programa();
        let lib = prog.unit(self.unidade).library;
        let consulta = &self.p.consulta;
        let mut tabela = consulta.tabela.clone();
        let mapa = dartforge_types::constantes::testemunhas_de_switch(
            prog,
            &consulta.nomes,
            &mut tabela,
            &consulta.core,
            &consulta.outline,
            &consulta.corpos,
            &self.p.bibliotecas,
            lib,
        );
        let lista = mapa.get(&(self.unidade, self.arvore.nos[node].inicio))?;
        if lista.is_empty() {
            return None;
        }
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let recuo_da_linha = tx.prefixo_da_linha(self.arvore.nos[node].inicio).to_string();
        let um = crate::refatoracoes_exec::UM_RECUO;
        // Os tokens `)`, `{` e `}` do `switch`.
        let s = self.arvore.span(node);
        let fecha = Span { start: s.end - 1, end: s.end };
        let escrutinio = *self.filhos(node).first()?;
        let parentese = self.token_seguinte(self.arvore.nos[escrutinio].fim)?;
        let abre = self.token_seguinte(parentese.end)?;
        if &self.fonte[abre.start..abre.end] != "{" || &self.fonte[fecha.start..fecha.end] != "}" {
            return None;
        }
        let uma_linha = self.linha(abre.start) == self.linha(fecha.start);
        let offset = if uma_linha { abre.end } else { tx.inicio_da_linha(fecha.start) };
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, offset);
        // Valor de enum privado de outra biblioteca: a testemunha sai e
        // entra o `default`.
        let inacessivel = |partes: &[P]| {
            partes.iter().any(|q| match q {
                P::ValorDeEnum { enumeracao, valor } => {
                    let c = prog.class(*enumeracao);
                    (self.p.nome(c.name).starts_with('_') || self.p.nome(prog.variable(*valor).name).starts_with('_')) && c.library != lib
                }
                _ => false,
            })
        };
        let mut texto = String::new();
        if uma_linha {
            texto.push_str(eol);
        }
        let mut precisa_default = false;
        for partes in lista {
            if inacessivel(partes) {
                precisa_default = true;
                continue;
            }
            let mut padrao = String::new();
            for q in partes {
                match q {
                    P::Texto(t) => padrao.push_str(t),
                    P::ValorDeEnum { enumeracao, valor } => {
                        let nome = self.p.nome(prog.class(*enumeracao).name).to_string();
                        padrao.push_str(&escritor.referencia_de(dartforge_elements::model::Element::Class(*enumeracao), &nome));
                        padrao.push('.');
                        padrao.push_str(self.p.nome(prog.variable(*valor).name));
                    }
                    P::Tipo(t) => padrao.push_str(&escritor.escrever_tipo(Some(*t), false).unwrap_or_default()),
                }
            }
            if expressao {
                texto.push_str(&format!("{recuo_da_linha}{um}// TODO: Handle this case.{eol}{recuo_da_linha}{um}{padrao} => throw UnimplementedError(),{eol}"));
            } else {
                texto.push_str(&format!(
                    "{recuo_da_linha}{um}case {padrao}:{eol}{recuo_da_linha}{um}{um}// TODO: Handle this case.{eol}{recuo_da_linha}{um}{um}throw UnimplementedError();{eol}"
                ));
            }
        }
        if precisa_default {
            if expressao {
                texto.push_str(&format!("{recuo_da_linha}{um}// TODO: Handle this case.{eol}{recuo_da_linha}{um}_ => throw UnimplementedError(),{eol}"));
            } else {
                texto.push_str(&format!(
                    "{recuo_da_linha}{um}default:{eol}{recuo_da_linha}{um}{um}// TODO: Handle this case.{eol}{recuo_da_linha}{um}{um}throw UnimplementedError();{eol}"
                ));
            }
        }
        if uma_linha {
            // `linePrefix(switchKeyword.offset)`.
            texto.push_str(&recuo_da_linha);
        }
        Some((Span { start: offset, end: offset }, texto, escritor.importar))
    }

    // -- AddMissingRequiredArgument -------------------------------------------------

    /// O tipo é `Widget` do Flutter ou subtipo (`isWidgetType`).
    fn e_widget(&self, t: dartforge_types::TypeId) -> bool {
        let prog = self.p.programa();
        let dartforge_types::Type::Interface { class, .. } = self.p.consulta.tabela.get(t) else { return false };
        let widget = |c: dartforge_elements::model::ClassId| {
            let k = prog.class(c);
            self.p.nome(k.name) == "Widget" && prog.library(k.library).uri == "package:flutter/src/widgets/framework.dart"
        };
        widget(*class) || self.p.supertipos(*class).into_iter().any(widget)
    }

    /// `getDefaultStringParameterValue2(parameter, quote)`: o texto e a
    /// posição do cursor.
    fn valor_padrao_de_argumento(&self, tipo: dartforge_types::TypeId, aspas: &str, anotacao: Option<(&ast::Ast, ast::TypeId)>) -> Option<String> {
        use dartforge_types::Type;
        let core = &self.p.consulta.core;
        let tabela = &self.p.consulta.tabela;
        match tabela.get(tipo) {
            Type::Interface { class, .. } if Some(*class) == core.list_class => Some("[]".to_string()),
            Type::Interface { class, .. } if Some(*class) == core.map_class => Some("{}".to_string()),
            Type::Interface { class, .. } if Some(*class) == core.string_class => Some(format!("{aspas}{aspas}")),
            Type::Function { positional, optional, named, .. } => {
                // Os nomes dos parâmetros vêm da anotação de tipo de função.
                let nomes: Vec<String> = match anotacao.map(|(a, t)| &a.ty(t).kind) {
                    Some(ast::TypeKind::Function { parameters, .. }) => parameters.iter().map(|q| q.name.map(|n| self.p.nome(n.sym).to_string()).unwrap_or_default()).collect(),
                    _ => Vec::new(),
                };
                let mut partes = Vec::new();
                let mut i = 0usize;
                for &q in positional.iter().chain(optional.iter()) {
                    let tipo = if matches!(tabela.get(q), Type::Dynamic) { String::new() } else { format!("{} ", self.p.consulta.formatar(q)) };
                    partes.push(format!("{tipo}{}", nomes.get(i).cloned().unwrap_or_default()));
                    i += 1;
                }
                for (n, q, _) in named.iter() {
                    let tipo = if matches!(tabela.get(*q), Type::Dynamic) { String::new() } else { format!("{} ", self.p.consulta.formatar(*q)) };
                    partes.push(format!("{tipo}{}", self.p.nome(*n)));
                }
                Some(format!("({}) {{  }}", partes.join(", ")))
            }
            _ => None,
        }
    }

    /// `AddMissingRequiredArgument` para cada parâmetro nomeado obrigatório
    /// sem argumento nas chamadas cujo nome está nas linhas `ini..fim` (o
    /// `missing_required_argument` não é publicado: as chamadas são
    /// examinadas no pedido). O nome do parâmetro e a inserção.
    pub(crate) fn argumentos_requeridos(&self, ini: usize, fim: usize) -> Vec<(String, Span, String)> {
        use dartforge_types::Resolved;
        let prog = self.p.programa();
        let aspas = if crate::refatoracoes_exec::regra_ligada(self.p, self.unidade, "prefer_double_quotes") { "\"" } else { "'" };
        let mut saida = Vec::new();
        for (n, no) in self.arvore.nos.iter().enumerate() {
            let (alvo_do_erro, funcao, lista, criacao) = match no.especie {
                "MethodInvocation" => {
                    let Some(m) = self.nome_do_metodo(n) else { continue };
                    let crate::refatoracoes::Elem::Funcao(f) = self.elemento_do_identificador(m, true) else { continue };
                    let Some(l) = self.filhos_da_especie(n, "ArgumentList").first().copied() else { continue };
                    (m, f, l, None)
                }
                "InstanceCreationExpression" => {
                    let Some(cn) = self.filhos_da_especie(n, "ConstructorName").first().copied() else { continue };
                    let Some(x) = self.expr_do_no(n) else { continue };
                    let f = match self.corpos.get_resolved(x) {
                        Some(Resolved::Constructor(f)) => *f,
                        _ => match self.construtores_por_alvo.iter().find(|(alvo, _)| self.ast.expr(**alvo).span.start == no.inicio) {
                            Some((_, f)) => *f,
                            None => continue,
                        },
                    };
                    let Some(l) = self.filhos_da_especie(n, "ArgumentList").first().copied() else { continue };
                    (cn, f, l, Some(n))
                }
                _ => continue,
            };
            let s = self.arvore.span(alvo_do_erro);
            if !(s.start <= fim && ini <= s.end) {
                continue;
            }
            let Some(dados) = self.p.consulta.outline.functions.get(funcao.0 as usize) else { continue };
            let argumentos: Vec<usize> = self.filhos(lista).to_vec();
            let dados_nomeados: Vec<String> = argumentos
                .iter()
                .filter(|&&a| self.especie(a) == "NamedExpression")
                .filter_map(|&a| self.filhos(a).first().and_then(|&r| self.filhos(r).first()).map(|&id| self.texto_do_no(id).to_string()))
                .collect();
            // As anotações dos parâmetros declarados (para os nomes de um
            // tipo de função).
            let (ast_decl, declarados): (Option<&ast::Ast>, Vec<&ast::Parameter>) = match prog.function(funcao).node {
                dartforge_elements::model::FunctionRef::Function { unit, function } => {
                    (Some(&prog.unit(unit).ast), prog.unit(unit).ast.function(function).parameters.iter().flatten().collect())
                }
                dartforge_elements::model::FunctionRef::Constructor { unit, member } => match &prog.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (Some(&prog.unit(unit).ast), k.parameters.iter().collect()),
                    _ => (None, Vec::new()),
                },
                _ => (None, Vec::new()),
            };
            let widget = criacao.and_then(|c| self.tipo_do_no(c)).is_some_and(|t| self.e_widget(t));
            for q in dados.parameters.iter() {
                if !(q.required && q.kind == ast::ParameterKind::Named) {
                    continue;
                }
                let Some(nome) = q.externo.or(q.name).map(|s| self.p.nome(s).to_string()) else { continue };
                if dados_nomeados.contains(&nome) {
                    continue;
                }
                let lista_s = self.arvore.span(lista);
                let (mut offset, mut virgula_final, mut entre) = (lista_s.start + 1, false, false);
                if let Some(&ultimo) = argumentos.last() {
                    offset = self.arvore.nos[ultimo].fim;
                    virgula_final = self.token_seguinte(offset).is_some_and(|t| &self.fonte[t.start..t.end] == ",");
                    if self.especie(ultimo) == "NamedExpression" && widget {
                        let rotulo = self.filhos(ultimo).first().and_then(|&r| self.filhos(r).first()).map(|&id| self.texto_do_no(id).to_string());
                        if matches!(rotulo.as_deref(), Some("child") | Some("children")) {
                            offset = self.arvore.nos[ultimo].inicio;
                            virgula_final = true;
                            entre = true;
                        }
                    }
                }
                let anotacao = declarados.iter().find(|p| p.name.is_some_and(|n| self.p.nome(n.sym) == nome)).and_then(|p| p.ty).and_then(|t| ast_decl.map(|a| (a, t)));
                let valor = self.valor_padrao_de_argumento(q.ty, aspas, anotacao).unwrap_or_else(|| "null".to_string());
                let mut texto = String::new();
                if !argumentos.is_empty() && !entre {
                    texto.push_str(", ");
                }
                texto.push_str(&format!("{nome}: {valor}"));
                if widget {
                    if !virgula_final {
                        texto.push(',');
                    } else if entre {
                        let eol = Texto::novo(self.fonte).eol();
                        texto.push(',');
                        texto.push_str(eol);
                        texto.push_str(Texto::novo(self.fonte).prefixo_da_linha(offset));
                    }
                }
                saida.push((nome, Span { start: offset, end: offset }, texto));
            }
        }
        saida
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

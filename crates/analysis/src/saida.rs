//! O `ExitDetector` do analyzer 3.6.2 (`analyzer/lib/src/dart/resolver/exit_detector.dart`)
//! sobre a árvore no formato do analyzer ([`crate::arvore_analyzer`]): usado
//! pela extração de método do LSP e pelo `use_build_context_synchronously`
//! (`terminatesControl`). O `_elementExits` vem de fora, pelo nó do nome.
//! Movido do LSP em 2026-10-05 (escrito sem compilar nem executar).

use crate::arvore_analyzer::Arvore;
use std::collections::HashSet;

fn e_comando_especie(e: &str) -> bool {
    crate::arvore_analyzer::e_comando(e)
}

/// A árvore e o texto dela.
#[derive(Clone, Copy)]
struct Cx<'a> {
    arvore: &'a Arvore,
    fonte: &'a str,
}

impl<'a> Cx<'a> {
    fn especie(&self, n: usize) -> &'static str {
        self.arvore.nos[n].especie
    }

    fn pai(&self, n: usize) -> Option<usize> {
        self.arvore.nos[n].pai
    }

    fn filhos(&self, n: usize) -> &'a [usize] {
        &self.arvore.nos[n].filhos
    }

    fn com_pais(&self, n: usize) -> impl Iterator<Item = usize> + 'a {
        let arvore = self.arvore;
        std::iter::successors(Some(n), move |&k| arvore.nos[k].pai)
    }

    fn texto_do_no(&self, n: usize) -> &'a str {
        let no = &self.arvore.nos[n];
        &self.fonte[no.inicio..no.fim]
    }

    /// O `methodName` de uma `MethodInvocation`: o último `SimpleIdentifier`
    /// filho.
    fn nome_do_metodo(&self, n: usize) -> Option<usize> {
        self.filhos(n).iter().rev().copied().find(|&f| self.especie(f) == "SimpleIdentifier")
    }
}

/// O `ExitDetector` do analyzer sobre a árvore no formato do analyzer.
pub struct Saida<'a> {
    cx: Cx<'a>,
    /// `_elementExits` do identificador (o `methodName`) de índice dado.
    elemento: &'a dyn Fn(usize) -> bool,
    /// `_enclosingBlockContainsBreak`.
    quebra: bool,
    /// `_enclosingBlockContainsContinue`.
    continua: bool,
    /// `_enclosingBlockBreaksLabel`: os comandos alvo de `break rótulo`.
    rotulos_quebrados: HashSet<usize>,
}

impl<'a> Saida<'a> {
    /// `elemento(n)`: o `_elementExits` do elemento do identificador `n`
    /// (o executável com `@alwaysThrows` ou de retorno `Never`).
    pub fn novo(arvore: &'a Arvore, fonte: &'a str, elemento: &'a dyn Fn(usize) -> bool) -> Saida<'a> {
        Saida { cx: Cx { arvore, fonte }, elemento, quebra: false, continua: false, rotulos_quebrados: HashSet::new() }
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
    pub fn sai(&mut self, n: usize) -> Result<bool, String> {
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
                    && (self.elemento)(m)
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
                let cmds: Vec<usize> = f.iter().copied().filter(|&k| e_comando_especie(cx.especie(k))).collect();
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
                        let vazio = !cx.filhos(m).iter().any(|&k| e_comando_especie(cx.especie(k)));
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
}

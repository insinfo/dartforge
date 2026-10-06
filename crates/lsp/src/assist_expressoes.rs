//! Assistências sobre expressões, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to 'isNotEmpty'` | `refactor.convert.isNotEmpty` | `ConvertIntoIsNotEmpty` |
//! | `Convert to an int literal` | `refactor.convert.toIntLiteral` | `ConvertToIntLiteral` |
//! | `Convert to a spread`, `Inline invocation of 'addAll'` | `refactor.convert.toSpread`, `refactor.inline` | `ConvertAddAllToSpread` |
//! | `Inline invocation of 'add'` | `refactor.inline` | `InlineInvocation` |
//! | `Convert to an 'if' element` | `refactor.convert.toIfElement` | `ConvertConditionalExpressionToIfElement` |
//! | `Convert to map literal` | `refactor.convert.toMapLiteral` | `ConvertToMapLiteral` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::FunctionElementId;
use dartforge_types::{MemberRef, Resolved};

impl Contexto<'_> {
    fn acao_simples(&self, uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
        AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
            diagnostico: None,
            criar_arquivo: None,
        }
    }

    /// `ConvertIntoIsNotEmpty` (convert_into_is_not_empty.dart): `!x.isEmpty`
    /// vira `x.isNotEmpty` quando quem declara o `isEmpty` resolvido declara
    /// também um `isNotEmpty` (`getChildren`).
    pub(crate) fn converter_em_is_not_empty(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "SimpleIdentifier" {
            return None;
        }
        let acesso = self.pai(no)?;
        if !matches!(self.especie(acesso), "PropertyAccess" | "PrefixedIdentifier") {
            return None;
        }
        let identificador = *self.filhos(acesso).last()?;
        // O elemento do `isEmpty` (o getter ou o campo).
        let x = self.expr_do_no(acesso)?;
        let prog = self.p.programa();
        let nome_e = |f: FunctionElementId| self.p.nome(prog.function(f).name).trim_end_matches('=').to_string();
        let (nome, dono_classe, dono_extensao) = match self.corpos.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                let fe = prog.function(*f);
                (nome_e(*f), fe.class, fe.extension)
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } => {
                let ve = prog.variable(*v);
                (self.p.nome(ve.name).to_string(), ve.class, ve.extension)
            }
            _ => return None,
        };
        if nome != "isEmpty" {
            return None;
        }
        let tem_is_not_empty = |instancia: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                estaticos: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                campos: &[dartforge_elements::model::VariableId]| {
            instancia.keys().chain(estaticos.keys()).any(|s| self.p.nome(*s).trim_end_matches('=') == "isNotEmpty")
                || campos.iter().any(|v| self.p.nome(prog.variable(*v).name) == "isNotEmpty")
        };
        let tem = match (dono_classe, dono_extensao) {
            (Some(c), _) => {
                let ce = prog.class(c);
                tem_is_not_empty(&ce.instance_members, &ce.static_members, &ce.fields)
            }
            (None, Some(e)) => {
                let ee = prog.extension(e);
                tem_is_not_empty(&ee.instance_members, &ee.static_members, &ee.fields)
            }
            _ => false,
        };
        if !tem {
            return None;
        }
        let prefixo = self.pai(acesso)?;
        if self.especie(prefixo) != "PrefixExpression" {
            return None;
        }
        let operador = self.token_seguinte(self.arvore.nos[prefixo].inicio)?;
        if &self.fonte[operador.start..operador.end] != "!" {
            return None;
        }
        Some(self.acao_simples(
            uri,
            "Convert to 'isNotEmpty'",
            "refactor.convert.isNotEmpty",
            vec![
                // `range.startStart(prefixExpression, prefixExpression.operand)`.
                (Span { start: self.arvore.nos[prefixo].inicio, end: self.arvore.nos[acesso].inicio }, String::new()),
                (self.arvore.span(identificador), "isNotEmpty".into()),
            ],
        ))
    }

    /// `ConvertToIntLiteral` (convert_to_int_literal.dart): um literal
    /// `double` de valor inteiro vira `int` (cortado no `.`, que preserva os
    /// separadores de dígitos, quando não há expoente).
    pub(crate) fn converter_em_literal_int(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "DoubleLiteral" {
            return None;
        }
        let span = self.arvore.span(no);
        let lexema = &self.fonte[span.start..span.end];
        let valor: f64 = lexema.replace('_', "").parse().ok()?;
        // `value.truncate()` lança em infinito e NaN; fora da faixa do `int`
        // o resultado não é igual ao valor.
        if !valor.is_finite() || valor.trunc() != valor || valor.abs() >= 9.223_372_036_854_775_808e18 {
            return None;
        }
        let inteiro = valor as i64;
        let edicao = match lexema.find('.') {
            Some(ponto) if ponto > 0 && !lexema.to_lowercase().contains('e') => (Span { start: span.start + ponto, end: span.end }, String::new()),
            _ => (span, inteiro.to_string()),
        };
        Some(self.acao_simples(uri, "Convert to an int literal", "refactor.convert.toIntLiteral", vec![edicao]))
    }

    /// Os argumentos de uma `ArgumentList`.
    fn argumentos_da_lista(&self, invocacao: usize) -> Vec<usize> {
        self.filhos(invocacao)
            .iter()
            .copied()
            .find(|&k| self.especie(k) == "ArgumentList")
            .map(|l| self.filhos(l).to_vec())
            .unwrap_or_default()
    }

    /// Os elementos de um `ListLiteral` (sem os argumentos de tipo).
    fn elementos_da_lista(&self, lista: usize) -> Vec<usize> {
        self.filhos(lista).iter().copied().filter(|&k| self.especie(k) != "TypeArgumentList").collect()
    }

    /// `ConvertAddAllToSpread` (convert_add_all_to_spread.dart): no nome do
    /// primeiro `..addAll(x)` em cascata sobre um literal de lista, `x` entra
    /// na lista como `...x` (ou `...?a` de `a ?? []`, `if (c) ...a` de
    /// `c ? a : []`); com um literal de lista, os elementos dele
    /// (`Inline invocation of 'addAll'`).
    pub(crate) fn converter_add_all_em_espalhamento(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let nome = self.arvore.localizar(inicio, fim)?;
        if self.especie(nome) != "SimpleIdentifier" {
            return None;
        }
        let invocacao = self.pai(nome)?;
        if self.especie(invocacao) != "MethodInvocation" || self.nome_do_metodo(invocacao) != Some(nome) || self.texto_do_no(nome) != "addAll" {
            return None;
        }
        // `isCascaded`: o operador do nome é `..` ou `?..`.
        let operador = self.token_anterior(self.arvore.nos[nome].inicio)?;
        if !matches!(&self.fonte[operador.start..operador.end], ".." | "?..") {
            return None;
        }
        let argumentos = self.argumentos_da_lista(invocacao);
        if argumentos.len() != 1 {
            return None;
        }
        let argumento = argumentos[0];
        let em_linha = self.especie(argumento) == "ListLiteral";
        let cascata = self.com_pais(invocacao).find(|&k| self.especie(k) == "CascadeExpression")?;
        let fc = self.filhos(cascata);
        let (alvo, primeira) = (*fc.first()?, *fc.get(1)?);
        if self.especie(alvo) != "ListLiteral" || primeira != invocacao {
            return None;
        }
        let lista_vazia = |k: usize| self.especie(k) == "ListLiteral" && self.elementos_da_lista(k).is_empty();
        let mut texto: Option<String> = None;
        match self.especie(argumento) {
            "BinaryExpression" if self.operador_binario(argumento) == Some("??") => {
                let f = self.filhos(argumento);
                if lista_vazia(f[1]) {
                    texto = Some(format!("...?{}", self.texto_do_no(f[0])));
                }
            }
            "ConditionalExpression" => {
                let f = self.filhos(argumento);
                if lista_vazia(f[2]) {
                    texto = Some(format!("if ({}) ...{}", self.texto_do_no(f[0]), self.texto_do_no(f[1])));
                }
            }
            "ListLiteral" => {
                let elementos = self.elementos_da_lista(argumento);
                let (&primeiro, &ultimo) = (elementos.first()?, elementos.last()?);
                texto = Some(self.fonte[self.arvore.nos[primeiro].inicio..self.arvore.nos[ultimo].fim].to_string());
            }
            _ => {}
        }
        let texto = texto.unwrap_or_else(|| format!("...{}", self.texto_do_no(argumento)));
        let elementos_do_alvo = self.elementos_da_lista(alvo);
        let insercao = match elementos_do_alvo.last() {
            Some(&u) => (Span { start: self.arvore.nos[u].fim, end: self.arvore.nos[u].fim }, format!(", {texto}")),
            None => {
                let abre = self.tokens.iter().find(|t| t.span.start >= self.arvore.nos[alvo].inicio && &self.fonte[t.span.start..t.span.end] == "[")?.span;
                (Span { start: abre.end, end: abre.end }, texto)
            }
        };
        let (titulo, especie) = if em_linha {
            ("Inline invocation of 'addAll'", "refactor.inline")
        } else {
            ("Convert to a spread", "refactor.convert.toSpread")
        };
        Some(self.acao_simples(uri, titulo, especie, vec![insercao, (self.arvore.span(invocacao), String::new())]))
    }

    /// `InlineInvocation` (inline_invocation.dart): no nome do primeiro
    /// `..add(e)` de uma cascata sobre um literal de lista, `e` entra na lista.
    pub(crate) fn embutir_invocacao_add(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let nome = self.arvore.localizar(inicio, fim)?;
        if self.especie(nome) != "SimpleIdentifier" || self.texto_do_no(nome) != "add" {
            return None;
        }
        let invocacao = self.pai(nome)?;
        if self.especie(invocacao) != "MethodInvocation" || self.nome_do_metodo(invocacao) != Some(nome) {
            return None;
        }
        let operador = self.token_anterior(self.arvore.nos[nome].inicio)?;
        if !matches!(&self.fonte[operador.start..operador.end], ".." | "?..") {
            return None;
        }
        let argumentos = self.argumentos_da_lista(invocacao);
        if argumentos.len() != 1 {
            return None;
        }
        let cascata = self.pai(invocacao)?;
        if self.especie(cascata) != "CascadeExpression" {
            return None;
        }
        let fc = self.filhos(cascata);
        let (alvo, primeira) = (*fc.first()?, *fc.get(1)?);
        if self.especie(alvo) != "ListLiteral" || primeira != invocacao {
            return None;
        }
        let texto = self.texto_do_no(argumentos[0]).to_string();
        let insercao = match self.elementos_da_lista(alvo).last() {
            Some(&u) => (Span { start: self.arvore.nos[u].fim, end: self.arvore.nos[u].fim }, format!(", {texto}")),
            None => {
                let abre = self.tokens.iter().find(|t| t.span.start >= self.arvore.nos[alvo].inicio && &self.fonte[t.span.start..t.span.end] == "[")?.span;
                (Span { start: abre.end, end: abre.end }, texto)
            }
        };
        Some(self.acao_simples(uri, "Inline invocation of 'add'", "refactor.inline", vec![insercao, (self.arvore.span(invocacao), String::new())]))
    }

    /// `ConvertConditionalExpressionToIfElement`: a condicional elemento de um
    /// literal de lista ou de conjunto (através de parênteses) vira
    /// `if (c) a else b`.
    pub(crate) fn condicional_em_elemento_if(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let condicional = self.com_pais(no).find(|&k| self.especie(k) == "ConditionalExpression")?;
        let mut trocar = condicional;
        let mut pai = self.pai(condicional)?;
        while self.especie(pai) == "ParenthesizedExpression" {
            trocar = pai;
            pai = self.pai(pai)?;
        }
        let e_conjunto = || {
            // `SetOrMapLiteral.isSet`: o tipo estático é um `Set`.
            let Some(x) = self.expr_do_no(pai) else { return false };
            let Some(t) = self.corpos.get_type(x) else { return false };
            matches!(self.p.consulta.tabela.get(t), dartforge_types::Type::Interface { class, .. } if Some(*class) == self.p.consulta.core.set_class)
        };
        if !(self.especie(pai) == "ListLiteral" || (self.especie(pai) == "SetOrMapLiteral" && e_conjunto())) {
            return None;
        }
        let f = self.filhos(condicional);
        let (c, a, b) = (self.sem_parenteses(*f.first()?), self.sem_parenteses(*f.get(1)?), self.sem_parenteses(*f.get(2)?));
        let texto = format!("if ({}) {} else {}", self.texto_do_no(c), self.texto_do_no(a), self.texto_do_no(b));
        Some(self.acao_simples(uri, "Convert to an 'if' element", "refactor.convert.toIfElement", vec![(self.arvore.span(trocar), texto)]))
    }

    /// Os tokens de um `Comment` de documentação: cada linha `///` ou o
    /// `/** … */` inteiro, com o tipo (`true`: de uma linha).
    fn tokens_do_comentario(&self, comentario: usize) -> Vec<(Span, bool)> {
        let span = self.arvore.span(comentario);
        let texto = &self.fonte[span.start..span.end];
        let mut v = Vec::new();
        let mut i = 0;
        while i < texto.len() {
            let resto = &texto[i..];
            if resto.starts_with("/*") {
                let fim = resto.find("*/").map_or(resto.len(), |k| k + 2);
                v.push((Span { start: span.start + i, end: span.start + i + fim }, false));
                i += fim;
            } else if resto.starts_with("//") {
                let fim = resto.find(['\r', '\n']).unwrap_or(resto.len());
                v.push((Span { start: span.start + i, end: span.start + i + fim }, true));
                i += fim;
            } else {
                i += resto.chars().next().map_or(1, char::len_utf8);
            }
        }
        v
    }

    /// `ConvertDocumentationIntoBlock`: o comentário de documentação feito
    /// só de linhas `///` vira `/** … */`.
    pub(crate) fn documentacao_em_bloco(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let comentario = self.com_pais(no).find(|&k| self.especie(k) == "Comment")?;
        let tokens = self.tokens_do_comentario(comentario);
        if tokens.is_empty() || tokens.iter().any(|&(s, linha)| !linha || !self.fonte[s.start..s.end].starts_with("///")) {
            return None;
        }
        let prefixo = self.prefixo_do_no(comentario);
        let eol = crate::refatoracoes_exec::Texto::novo(self.fonte).eol();
        let mut s = format!("/**{eol}");
        for (t, _) in &tokens {
            s.push_str(&format!("{prefixo} *{}{eol}", &self.fonte[t.start + 3..t.end]));
        }
        s.push_str(&format!("{prefixo} */"));
        Some(self.acao_simples(uri, "Convert to block documentation comment", "refactor.convert.blockComment", vec![(self.arvore.span(comentario), s)]))
    }

    /// `ConvertDocumentationIntoLine`: o `/** … */` vira linhas `///`.
    pub(crate) fn documentacao_em_linhas(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let comentario = self.com_pais(no).find(|&k| self.especie(k) == "Comment")?;
        let tokens = self.tokens_do_comentario(comentario);
        let [(token, false)] = tokens[..] else { return None };
        let eol = crate::refatoracoes_exec::Texto::novo(self.fonte).eol();
        let prefixo = self.prefixo_do_no(comentario);
        let mut novas: Vec<String> = Vec::new();
        let mut primeira = true;
        let mut prefixo_da_linha = String::new();
        for linha in self.fonte[token.start..token.end].split(eol) {
            if primeira {
                primeira = false;
                let mut l = linha.strip_prefix("/**")?.trim();
                if let Some(x) = l.strip_suffix("*/") {
                    l = x.trim();
                }
                if !l.is_empty() {
                    novas.push(format!("/// {l}"));
                    prefixo_da_linha = format!("{eol}{prefixo}");
                }
            } else {
                let l = linha.trim_start();
                if l.starts_with("*/") {
                    break;
                }
                let mut l = l.strip_prefix('*')?;
                if let Some(x) = l.strip_suffix("*/") {
                    l = x.trim_end();
                }
                novas.push(format!("{prefixo_da_linha}///{l}"));
                prefixo_da_linha = format!("{eol}{prefixo}");
            }
        }
        Some(self.acao_simples(uri, "Convert to line documentation comment", "refactor.convert.lineComment", vec![(self.arvore.span(comentario), novas.concat())]))
    }

    /// `ConvertToMapLiteral` (convert_to_map_literal.dart): `Map()` ou
    /// `LinkedHashMap()` sem argumentos vira `{}`, com os argumentos de tipo
    /// escritos ou, fora de declaração tipada, os do tipo estático.
    pub(crate) fn converter_em_literal_de_mapa(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let criacao = self.com_pais(no).find(|&k| self.especie(k) == "InstanceCreationExpression")?;
        let argumentos = self.filhos(criacao).iter().copied().find(|&k| self.especie(k) == "ArgumentList")?;
        let nome_do_construtor = self.filhos(criacao).iter().copied().find(|&k| self.especie(k) == "ConstructorName")?;
        if self.arvore.nos[no].inicio > self.arvore.nos[argumentos].inicio || self.filhos(nome_do_construtor).len() > 1 || !self.filhos(argumentos).is_empty() {
            return None;
        }
        let tipo = self.expr_do_no(criacao).and_then(|x| self.corpos.get_type(x))?;
        let consulta = &self.p.consulta;
        let prog = self.p.programa();
        let dartforge_types::Type::Interface { class, args, .. } = consulta.tabela.get(tipo) else { return None };
        let e_mapa = Some(*class) == consulta.core.map_class
            || (self.p.nome(prog.class(*class).name) == "LinkedHashMap" && prog.library(prog.class(*class).library).uri == "dart:collection");
        if !e_mapa {
            return None;
        }
        let tipo_do_construtor = *self.filhos(nome_do_construtor).first()?;
        let argumentos_escritos = self.filhos(tipo_do_construtor).iter().copied().find(|&k| self.especie(k) == "TypeArgumentList");
        let mut texto = String::new();
        let mut importar = std::collections::BTreeSet::new();
        match argumentos_escritos {
            Some(a) => texto.push_str(self.texto_do_no(a)),
            None => {
                let lista = self.com_pais(criacao).find(|&k| self.especie(k) == "VariableDeclarationList");
                let lista_tipada = lista.is_some_and(|l| {
                    self.filhos(l).iter().any(|&f| matches!(self.especie(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"))
                });
                let todos_dynamic = args.first().is_some_and(|&a| matches!(consulta.tabela.get(a), dartforge_types::Type::Dynamic))
                    && args.last().is_some_and(|&a| matches!(consulta.tabela.get(a), dartforge_types::Type::Dynamic));
                if !lista_tipada && !args.is_empty() && !todos_dynamic {
                    // `writeTypes`: cada um pelo `writeType`, com ", ".
                    let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[criacao].inicio);
                    let escritos: Vec<String> = args.iter().map(|&a| escritor.escrever_tipo(Some(a), false).unwrap_or_default()).collect();
                    importar = escritor.importar;
                    texto.push('<');
                    texto.push_str(&escritos.join(", "));
                    texto.push('>');
                }
            }
        }
        texto.push_str("{}");
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        m.adicionar(uri, self.arvore.span(criacao), texto);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to map literal".into(),
            especie: "refactor.convert.toMapLiteral".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}

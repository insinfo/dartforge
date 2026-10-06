//! Os ajudantes menores do `InScopeCompletionPass` do analysis server 3.6.2:
//! o `IdentifierHelper` (`identifier_helper.dart`), o `LabelHelper`
//! (`label_helper.dart`), o `OverrideHelper` (`override_helper.dart`, com o
//! `writeOverride` de `change_builder_dart.dart:515-642`), o `UriHelper`
//! (`uri_helper.dart`) e as sugestões de closure, de argumento nomeado e de
//! campo nomeado de record (`_addClosureSuggestion`, `NamedArgumentSuggestion`,
//! `_suggestRecordLiteralNamedFields`; `candidate_suggestion.dart`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::Isp;
use super::isp_visitas::ParametroInvocado;
use super::*;
use dartforge_frontend::token::Op;

/// As bibliotecas `dart:` que o `_addDartSuggestions` oferece: as de
/// `sdk_library_metadata/lib/libraries.dart` do SDK 3.6.2 com categorias
/// (não internas), sem `implementation: true` e sem `dart:_`, na ordem do
/// arquivo.
const BIBLIOTECAS_DART: &[&str] = &[
    "dart:async",
    "dart:collection",
    "dart:concurrent",
    "dart:convert",
    "dart:core",
    "dart:developer",
    "dart:ffi",
    "dart:html",
    "dart:indexed_db",
    "dart:io",
    "dart:isolate",
    "dart:js",
    "dart:js_interop",
    "dart:js_interop_unsafe",
    "dart:js_util",
    "dart:math",
    "dart:mirrors",
    "dart:typed_data",
    "dart:cli",
    "dart:svg",
    "dart:web_audio",
    "dart:web_gl",
];

/// `getCamelWords` (`analyzer_plugin/src/utilities/string_utilities.dart`).
pub(super) fn palavras_camel(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let b: Vec<u16> = s.encode_utf16().collect();
    let minuscula = |c: u16| (b'a' as u16..=b'z' as u16).contains(&c);
    let maiuscula = |c: u16| (b'A' as u16..=b'Z' as u16).contains(&c);
    let mut partes = Vec::new();
    let (mut era_min, mut era_mai) = (false, false);
    let mut inicio = 0usize;
    for i in 0..b.len() {
        let c = b[i];
        let (nova_min, nova_mai) = (minuscula(c), maiuscula(c));
        if era_min && nova_mai {
            partes.push(String::from_utf16_lossy(&b[inicio..i]));
            inicio = i;
        }
        if era_mai && nova_mai && i + 1 < b.len() && minuscula(b[i + 1]) {
            partes.push(String::from_utf16_lossy(&b[inicio..i]));
            inicio = i;
        }
        era_min = nova_min;
        era_mai = nova_mai;
    }
    partes.push(String::from_utf16_lossy(&b[inicio..]));
    partes
}

/// `getCamelWordCombinations`.
pub(super) fn combinacoes_camel(nome: &str) -> Vec<String> {
    let partes = palavras_camel(nome);
    (0..partes.len()).map(|i| format!("{}{}", partes[i].to_lowercase(), partes[i + 1..].concat())).collect()
}

/// `String.toUpperCamelCase` (`utilities/extensions/string.dart:59-77`).
fn camel_maiusculo(s: &str) -> Option<String> {
    let capitalizar = |w: &str| {
        let mut cs = w.chars();
        match cs.next() {
            None => String::new(),
            Some(c) if w.chars().count() <= 1 => c.to_uppercase().collect(),
            Some(c) => format!("{}{}", c.to_uppercase(), cs.as_str().to_lowercase()),
        }
    };
    let palavras: Vec<&str> = s.split('_').collect();
    if palavras.len() < 2 {
        let primeira = palavras.first()?;
        if primeira.is_empty() {
            return None;
        }
        return Some(capitalizar(primeira));
    }
    let mut r = String::new();
    for w in palavras {
        if w.is_empty() {
            return None;
        }
        r.push_str(&capitalizar(w));
    }
    Some(r)
}

/// Uma regra de lint ligada no `analysis_options.yaml` que vale para o
/// arquivo (`codeStyleOptions`).
fn regra_ligada(caminho: Option<&std::path::Path>, nome: &str) -> bool {
    caminho.is_some_and(|c| crate::refatoracoes_exec::regra_ligada_em(c, nome))
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- IdentifierHelper ------------------------------------------------------------

    /// `identifierHelper(includePrivateIdentifiers: …)`: a primeira chamada
    /// fixa a opção.
    pub(super) fn ih(&mut self, privados: bool) {
        if self.privados.is_none() {
            self.privados = Some(privados);
        }
    }

    /// `_createNameSuggestion` / `IdentifierSuggestion` (`suggestName`, 500).
    pub(super) fn item_nome(&mut self, nome: &str, corpo: bool) {
        if nome.is_empty() {
            return;
        }
        let Some(s) = self.pontuar(nome) else { return };
        let completion = if corpo { format!("{nome} {{}}") } else { nome.to_string() };
        let selecao = nome.encode_utf16().count() + if corpo { 2 } else { 0 };
        let antes = self.coletor.itens.len();
        let fim = completion.encode_utf16().count();
        let item = self.coletor.empurrar(grupo::LOCAL, especie::VARIAVEL, completion.clone(), completion, None);
        item.rel.fixa = Some(500);
        if selecao != fim {
            item.selecao = Some((selecao, 0));
        }
        self.marcar(antes, s, false);
    }

    /// `addSuggestionsFromTypeName`.
    pub(super) fn ih_do_nome_de_tipo(&mut self, nome_do_tipo: &str) {
        let mut nomes = combinacoes_camel(nome_do_tipo);
        if let Some(i) = nomes.iter().position(|n| n == nome_do_tipo) {
            nomes.remove(i);
        }
        let privados = self.privados.unwrap_or(false);
        for n in nomes {
            self.item_nome(&n, false);
            if privados {
                self.item_nome(&format!("_{n}"), false);
            }
        }
    }

    /// `addVariable(type)`: só um `NamedType`.
    pub(super) fn ih_variavel(&mut self, tipo: Option<usize>) {
        if let Some(t) = tipo
            && self.especie(t) == "NamedType"
        {
            let nome = self.nome_do_tipo_nomeado(t);
            self.ih_do_nome_de_tipo(&nome);
        }
    }

    /// `addTopLevelName`: o nome do arquivo em `UpperCamelCase`, se a
    /// biblioteca ainda não declara esse nome.
    pub(super) fn ih_nome_de_topo(&mut self, corpo: bool) {
        let Some(caminho) = self.caminho.clone() else { return };
        let Some(base) = caminho.file_stem().and_then(|s| s.to_str()) else { return };
        let Some(candidato) = camel_maiusculo(base) else { return };
        let lib = self.consulta.programa.library(self.biblioteca());
        if lib.declared.keys().any(|s| self.consulta.nome(*s) == candidato) {
            return;
        }
        self.item_nome(&candidato, corpo);
    }

    // -- LabelHelper ----------------------------------------------------------------

    /// `addLabels`: os rótulos visíveis no `break`/`continue`.
    pub(super) fn rotulos(&mut self, comando: usize) {
        let com_casos = self.especie(comando) == "ContinueStatement";
        let mut atual = Some(comando);
        while let Some(k) = atual {
            match self.especie(k) {
                "SwitchStatement" => {
                    if com_casos {
                        for m in self.filhos_de(k, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]) {
                            let rotulos = self.filhos_de(m, &["Label"]);
                            self.visitar_rotulos(&rotulos);
                        }
                    }
                }
                "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody" => return,
                "LabeledStatement" => {
                    let rotulos = self.filhos_de(k, &["Label"]);
                    self.visitar_rotulos(&rotulos);
                }
                _ => {}
            }
            atual = self.pai(k);
        }
    }

    /// `_visitLabels` / `suggestLabel` (`Relevance.label`, 1000).
    pub(super) fn visitar_rotulos(&mut self, rotulos: &[usize]) {
        for &r in rotulos {
            let nome = self.fonte[self.ini(r)..self.fim(r)].trim_end_matches(':').trim().to_string();
            let Some(s) = self.pontuar(&nome) else { continue };
            if nome.is_empty() || nome == "_" {
                continue;
            }
            let antes = self.coletor.itens.len();
            // `ElementKind.LABEL` → `CompletionItemKind.Text`.
            let item = self.coletor.empurrar(grupo::LOCAL, 1, nome.clone(), nome, None);
            item.rel.fixa = Some(1000);
            self.marcar(antes, s, false);
        }
    }

    // -- OverrideHelper -------------------------------------------------------------

    /// `interface.isSuperImplemented(nome)`: há implementação concreta na
    /// cadeia de superclasses (com os mixins).
    pub(super) fn super_implementado(&self, c: ClassId, nome: SymbolId) -> bool {
        let p = &self.consulta.programa;
        let mut atual = p.class(p.dono_da_classe(c)).supertype_class;
        let mut vistas = HashSet::new();
        while let Some(x) = atual {
            if !vistas.insert(x) {
                break;
            }
            let cl = p.class(x);
            let concreto = |k: ClassId| p.class(k).instance_members.get(&nome).is_some_and(|&f| !p.function(f).abstract_);
            if concreto(x) || cl.mixin_classes.iter().any(|&m| concreto(m)) {
                return true;
            }
            atual = cl.supertype_class;
        }
        false
    }

    /// `containingNode.hasOverride`: o membro que contém o ponto já tem
    /// `@override`.
    pub(super) fn membro_tem_override(&self) -> bool {
        let Some(c) = self.no_coberto() else { return false };
        let Some(m) = self.ancestral(c, &["MethodDeclaration", "FieldDeclaration"]) else { return false };
        self.filhos_de(m, &["Annotation"]).iter().any(|&a| self.fonte[self.ini(a)..self.fim(a)].trim() == "@override")
    }

    /// `computeOverridesFor(interfaceElement, SourceRange(offset, 0), skipAt)`.
    pub(super) fn sobrescritas(&mut self, c: ClassId, sem_arroba: bool) {
        let p = &self.consulta.programa;
        let dona = p.dono_da_classe(c);
        let lib_c = p.class(dona).library;
        let declarados: HashSet<SymbolId> = p.class(dona).instance_members.keys().copied().collect();
        let membros = self.membros_da_interface(c, false, false);
        let ja_tem_override = self.membro_tem_override();
        for f in membros {
            let fe = self.consulta.programa.function(f);
            let simbolo = fe.name;
            if declarados.contains(&simbolo) {
                continue;
            }
            let nome_cru = self.consulta.nome(simbolo).to_string();
            if nome_cru.starts_with('_') && fe.library != lib_c {
                continue;
            }
            if self.anotacoes_do_membro(f).iter().any(|a| a == "nonVirtual") {
                continue;
            }
            let exibicao = nome_cru.trim_end_matches('=').to_string();
            let s = self.score("override").max(self.score("operator")).max(self.score(&exibicao));
            if s == -1.0 {
                continue;
            }
            let invocar_super = self.super_implementado(c, simbolo);
            self.item_sobrescrita(c, f, invocar_super, sem_arroba, ja_tem_override, s);
        }
    }

    /// `suggestOverride`: o texto do `writeOverride`, a seleção e o rótulo.
    pub(super) fn item_sobrescrita(&mut self, c: ClassId, f: FunctionElementId, invocar_super: bool, sem_arroba: bool, ja_tem_override: bool, score: f64) {
        let fe = self.consulta.programa.function(f).clone();
        let nome_cru = self.consulta.nome(fe.name).to_string();
        let nome = nome_cru.trim_end_matches('=').to_string();
        let setter = fe.kind == FunctionKind::Setter || (fe.kind == FunctionKind::ImplicitAccessor && nome_cru.ends_with('='));
        let getter = !setter && matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor);
        let operador = fe.kind == FunctionKind::Operator;
        // O membro como a classe o vê (os parâmetros de tipo dos supertipos
        // substituídos).
        let this = self.tipo_this(c);
        let lib = self.biblioteca();
        let busca = if setter { self.consulta.nomes.lookup(&nome) } else { Some(fe.name) };
        let tipo = busca
            .and_then(|s| self.consulta.resolvedor().lookup_member(this, s, setter, lib))
            .map(|(_, t)| t)
            .unwrap_or_else(|| super::tipo_declarado(self.consulta, f));
        let (retorno, posicionais, opcionais, nomeados, parametros_de_tipo) = match self.consulta.tabela.get(tipo).clone() {
            Type::Function { ret, positional, optional, named, type_params, .. } if !getter && !setter => (ret, positional.to_vec(), optional.to_vec(), named.to_vec(), type_params.to_vec()),
            _ if setter => (self.consulta.core.void_, vec![tipo], Vec::new(), Vec::new(), Vec::new()),
            _ => (tipo, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        let parametros_ast = self.parametros_do_membro(f);
        let mut escritor = crate::escrever_tipo::Escritor::da_consulta(&*self.consulta, lib, Some(c), None);
        escritor.copiado = Some(f);
        let mut s = String::from("@override\n  ");
        let mut exib = String::new();
        let selecao: (usize, usize);
        if getter {
            s.push_str(&format!("// TODO: implement {nome}\n  "));
        }
        if !setter && let Some(t) = escritor.escrever_tipo(Some(retorno), false) {
            s.push_str(&t);
            s.push(' ');
        }
        if getter {
            s.push_str("get ");
        } else if setter {
            s.push_str("set ");
        } else if operador {
            s.push_str("operator ");
        }
        s.push_str(&nome);
        exib.push_str(&nome);
        if getter {
            s.push_str(" => ");
            let corpo = if invocar_super { format!("super.{nome}") } else { "throw UnimplementedError()".to_string() };
            selecao = (s.len(), corpo.len());
            s.push_str(&corpo);
            s.push_str(if invocar_super { ";\n" } else { ";" });
            exib.push_str(" => …");
        } else {
            // `writeTypeParameters` + `writeParameters`.
            let mut cabeca = String::new();
            if !parametros_de_tipo.is_empty() {
                cabeca.push('<');
                for (i, tp) in parametros_de_tipo.iter().enumerate() {
                    if i > 0 {
                        cabeca.push_str(", ");
                    }
                    let dados = self.consulta.tabela.param(*tp);
                    cabeca.push_str(self.consulta.nome(dados.name));
                    if dados.explicito
                        && let Some(b) = escritor.escrever(dados.bound, false)
                    {
                        cabeca.push_str(" extends ");
                        cabeca.push_str(&b);
                    }
                }
                cabeca.push('>');
            }
            let tipos_dos_parametros: Vec<TypeId> = posicionais.iter().chain(opcionais.iter()).copied().collect();
            let mut nomes_usados: HashSet<String> = parametros_ast.iter().filter_map(|x| x.nome.clone()).collect();
            cabeca.push('(');
            let (mut viu_nomeado, mut viu_opcional) = (false, false);
            let mut nomes_da_chamada: Vec<(bool, String)> = Vec::new();
            for (i, prm) in parametros_ast.iter().enumerate() {
                if i > 0 {
                    cabeca.push_str(", ");
                }
                if prm.nomeado && !viu_nomeado {
                    cabeca.push('{');
                    viu_nomeado = true;
                } else if prm.opcional && !viu_opcional {
                    cabeca.push('[');
                    viu_opcional = true;
                }
                let nome_p = match &prm.nome {
                    Some(n) if !n.is_empty() => n.clone(),
                    _ => {
                        let mut k = 1;
                        while nomes_usados.contains(&format!("p{k}")) {
                            k += 1;
                        }
                        let n = format!("p{k}");
                        nomes_usados.insert(n.clone());
                        n
                    }
                };
                let tipo_p = if prm.nomeado {
                    nomeados.iter().find(|(s, _, _)| self.consulta.nome(*s) == nome_p).map(|(_, t, _)| *t)
                } else {
                    tipos_dos_parametros.get(prm.posicao).copied()
                };
                if prm.covariante {
                    cabeca.push_str("covariant ");
                }
                if prm.requerido_nomeado {
                    cabeca.push_str("required ");
                }
                match escritor.escrever_tipo(tipo_p, false) {
                    Some(t) => {
                        cabeca.push_str(&t);
                        cabeca.push(' ');
                        cabeca.push_str(&nome_p);
                    }
                    None => cabeca.push_str(&nome_p),
                }
                if let Some(d) = &prm.padrao {
                    cabeca.push_str(" = ");
                    cabeca.push_str(d);
                }
                nomes_da_chamada.push((prm.nomeado, nome_p));
            }
            if viu_nomeado {
                cabeca.push('}');
            }
            if viu_opcional {
                cabeca.push(']');
            }
            cabeca.push(')');
            s.push_str(&cabeca);
            exib.push_str(&cabeca);
            s.push_str(" {\n    ");
            s.push_str(&format!("// TODO: implement {nome}"));
            let invocacao = {
                let mut x = String::from(if operador { " " } else { "." });
                x.push_str(&nome);
                x.push_str(if operador { " " } else { "(" });
                for (i, (nomeado, n)) in nomes_da_chamada.iter().enumerate() {
                    if i > 0 {
                        x.push_str(", ");
                    }
                    if *nomeado {
                        x.push_str(n);
                        x.push_str(": ");
                    }
                    x.push_str(n);
                }
                x.push_str(if operador { ";" } else { ");" });
                x
            };
            let e_void = matches!(self.consulta.tabela.get(retorno), Type::Void);
            if setter {
                if invocar_super {
                    s.push_str("\n    ");
                    let primeiro = nomes_da_chamada.first().map(|(_, n)| n.clone()).unwrap_or_default();
                    let corpo = format!("super.{nome} = {primeiro};");
                    selecao = (s.len(), corpo.len());
                    s.push_str(&corpo);
                } else {
                    selecao = (s.len(), 0);
                }
            } else if e_void {
                if invocar_super {
                    s.push_str("\n    ");
                    let corpo = format!("super{invocacao}");
                    selecao = (s.len(), corpo.len());
                    s.push_str(&corpo);
                } else {
                    selecao = (s.len(), 0);
                }
            } else {
                s.push_str("\n    ");
                let corpo = if invocar_super { format!("return super{invocacao}") } else { "throw UnimplementedError();".to_string() };
                selecao = (s.len(), corpo.len());
                s.push_str(&corpo);
            }
            s.push_str("\n  }");
            exib.push_str(" { … }");
        }
        let importar: Vec<LibraryId> = escritor.importar.iter().copied().collect();
        drop(escritor);
        // `completion = replacement.trim()`, sem o `@override` que o membro
        // já tem, ou sem o `@` já digitado.
        let mut completion = s.trim().to_string();
        let mut inicio = s.find(&completion).unwrap_or(0);
        if ja_tem_override && completion.starts_with("@override") {
            let resto = completion["@override".len()..].trim().to_string();
            inicio = s.find(&resto).unwrap_or(inicio);
            completion = resto;
        }
        if sem_arroba && completion.starts_with("@override") {
            completion = completion[1..].to_string();
            inicio += 1;
        }
        if completion.is_empty() || exib.is_empty() {
            return;
        }
        if sem_arroba {
            exib = format!("override {exib}");
        }
        let utf16 = |x: &str| x.encode_utf16().count();
        let sel_ini = selecao.0.saturating_sub(inicio);
        let selecao_utf16 = (utf16(&s[inicio..inicio + sel_ini.min(s.len() - inicio)]), utf16(&s[selecao.0..(selecao.0 + selecao.1).min(s.len())]));
        let especie_lsp = if getter || setter { especie::PROPRIEDADE } else { especie::METODO };
        let antes = self.coletor.itens.len();
        let origem = self.consulta.origem(self.consulta.inicio_da_funcao(f));
        let item = self.coletor.empurrar(grupo::MEMBRO, especie_lsp, completion.clone(), completion, None);
        item.exibicao = Some(exib);
        item.selecao = Some(selecao_utf16);
        item.rel.fixa = Some(750);
        item.origem = origem;
        item.casar = Some(if operador { "override_operator".to_string() } else { format!("override_{nome}") });
        if let Some(&l) = importar.first() {
            let uri = self.consulta.programa.library(l).uri.clone();
            if uri.starts_with("dart:") || uri.starts_with("package:") {
                let (span, novo) = crate::acoes::inserir_import(self.fonte, self.cu, &uri);
                item.importar = Some(ImportAutomatico { uri, span, texto: novo });
            }
        }
        self.marcar(antes, score, false);
    }

    /// Os parâmetros declarados de um membro, na ordem (nome, espécie,
    /// `required`, `covariant`, o texto do valor padrão).
    pub(super) fn parametros_do_membro(&self, f: FunctionElementId) -> Vec<ParametroEscrito> {
        let p = &self.consulta.programa;
        let fe = p.function(f);
        match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let u = p.unit(unit);
                let func = u.ast.function(function);
                let mut posicao = 0usize;
                func.parameters
                    .iter()
                    .flatten()
                    .map(|x| {
                        let nomeado = x.kind == ast::ParameterKind::Named;
                        let pos = posicao;
                        if !nomeado {
                            posicao += 1;
                        }
                        ParametroEscrito {
                            nome: x.nome_externo().map(|n| u.source[n.span.start..n.span.end].to_string()),
                            nomeado,
                            opcional: x.kind == ast::ParameterKind::Optional,
                            requerido_nomeado: nomeado && x.required,
                            covariante: x.covariant,
                            padrao: x.default_value.map(|e| {
                                let s = u.ast.expr(e).span;
                                u.source[s.start..s.end].to_string()
                            }),
                            posicao: pos,
                        }
                    })
                    .collect()
            }
            _ => {
                // O setter implícito de um campo: `_x`.
                if fe.kind == FunctionKind::ImplicitAccessor && self.consulta.nome(fe.name).ends_with('=') {
                    let base = self.consulta.nome(fe.name).trim_end_matches('=').to_string();
                    let covariante = fe.variable.is_some_and(|v| match p.variable(v).node {
                        dartforge_elements::model::VariableRef::Field { unit, member, .. } => {
                            p.unit(unit).ast.member(member).span.start < p.unit(unit).source.len()
                                && p.unit(unit).source[p.unit(unit).ast.member(member).span.start..p.unit(unit).ast.member(member).span.end].contains("covariant ")
                        }
                        _ => false,
                    });
                    vec![ParametroEscrito { nome: Some(format!("_{base}")), nomeado: false, opcional: false, requerido_nomeado: false, covariante, padrao: None, posicao: 0 }]
                } else {
                    Vec::new()
                }
            }
        }
    }

    // -- UriHelper ------------------------------------------------------------------

    /// `addSuggestions(uri)`: o nó é o `SimpleStringLiteral` da diretiva.
    pub(super) fn uris(&mut self, n: usize) {
        let (inicio, fim) = (self.ini(n), self.fim(n));
        let tamanho = self.fonte.len();
        let ultimo_e_aspa = || {
            let c = self.fonte[..fim].chars().last();
            matches!(c, Some('"') | Some('\''))
        };
        let ok = if self.offset > inicio {
            if self.offset < fim {
                true
            } else if self.offset == fim {
                fim == inicio + 1 || (fim == tamanho && !ultimo_e_aspa())
            } else {
                false
            }
        } else {
            self.offset == inicio && self.offset == fim && fim == tamanho && ultimo_e_aspa()
        };
        if ok {
            self.literal_de_uri(n);
        }
    }

    /// `_simpleStringLiteral`.
    pub(super) fn literal_de_uri(&mut self, n: usize) {
        let Some(pai) = self.pai(n) else { return };
        let Some(parcial) = self.uri_parcial(n) else { return };
        match self.especie(pai) {
            "Configuration" | "ImportDirective" | "ExportDirective" => {
                self.uris_dart();
                self.uris_de_pacote(&parcial);
                self.uris_de_arquivo(&parcial);
            }
            "PartDirective" | "PartOfDirective" => self.uris_de_arquivo(&parcial),
            _ => {}
        }
    }

    /// `contentsOffset` do literal (depois de `r` e das aspas).
    pub(super) fn inicio_do_conteudo(&self, n: usize) -> usize {
        let t = &self.fonte[self.ini(n)..self.fim(n)];
        let mut k = 0usize;
        if t.starts_with('r') {
            k += 1;
        }
        let resto = &t[k..];
        if resto.starts_with("'''") || resto.starts_with("\"\"\"") {
            k += 3;
        } else if resto.starts_with('\'') || resto.starts_with('"') {
            k += 1;
        }
        self.ini(n) + k
    }

    /// `_extractPartialUri`.
    pub(super) fn uri_parcial(&self, n: usize) -> Option<String> {
        let c = self.inicio_do_conteudo(n);
        if self.offset < c {
            return None;
        }
        Some(self.fonte[c..self.offset.min(self.fonte.len())].to_string())
    }

    /// `_suggestUri` (`Relevance.import` 900; `dart:core` 100).
    pub(super) fn item_uri(&mut self, uri: &str) {
        let Some(s) = self.pontuar(uri) else { return };
        let especie_lsp = if uri.starts_with("dart:") {
            especie::MODULO
        } else if uri.ends_with(".dart") {
            17
        } else {
            19
        };
        let antes = self.coletor.itens.len();
        let item = self.coletor.empurrar(grupo::BIBLIOTECA, especie_lsp, uri.to_string(), uri.to_string(), None);
        item.rel.fixa = Some(if uri == "dart:core" { 100 } else { 900 });
        self.marcar(antes, s, false);
    }

    /// `_addDartSuggestions`.
    pub(super) fn uris_dart(&mut self) {
        self.item_uri("dart:");
        for u in BIBLIOTECAS_DART {
            self.item_uri(u);
        }
    }

    /// `_addPackageSuggestions`.
    pub(super) fn uris_de_pacote(&mut self, parcial: &str) {
        let Some(arquivo) = self.caminho.clone() else { return };
        let Some(config) = dartforge_elements::config::PackageConfig::discover(&arquivo).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()) else {
            return;
        };
        self.item_uri("package:");
        for (nome, pasta) in config.package_dirs.iter() {
            let prefixo = format!("package:{nome}/");
            self.item_uri(&prefixo);
            if pasta.is_dir() {
                self.pasta_de_pacote(parcial, &prefixo, pasta);
            }
        }
    }

    /// `_addPackageFolderSuggestions`.
    pub(super) fn pasta_de_pacote(&mut self, parcial: &str, prefixo: &str, pasta: &std::path::Path) {
        let Ok(entradas) = std::fs::read_dir(pasta) else { return };
        let mut filhos: Vec<std::path::PathBuf> = entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
        filhos.sort();
        for f in filhos {
            let Some(nome) = f.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            if f.is_dir() {
                let filho = format!("{prefixo}{nome}/");
                self.item_uri(&filho);
                if parcial.starts_with(&filho) {
                    self.pasta_de_pacote(parcial, &filho, &f);
                }
            } else if nome.ends_with(".dart") {
                self.item_uri(&format!("{prefixo}{nome}"));
            }
        }
    }

    /// `_addFileSuggestions`.
    pub(super) fn uris_de_arquivo(&mut self, parcial: &str) {
        let Some(arquivo) = self.caminho.clone() else { return };
        let pai_uri = if parcial.ends_with('/') {
            parcial.to_string()
        } else {
            let d = match parcial.rfind('/') {
                Some(0) => "/".to_string(),
                Some(i) => parcial[..i].to_string(),
                None => ".".to_string(),
            };
            if d != "." && !d.ends_with('/') { format!("{d}/") } else { d }
        };
        let prefixo_uri = if pai_uri == "." { String::new() } else { pai_uri.clone() };
        let esquema = pai_uri.split_once(':').map(|(e, _)| e).filter(|e| !e.contains('/') && e.len() > 1);
        if !pai_uri.starts_with("file://") && esquema.is_some() {
            return;
        }
        let caminho_dir = if let Some(r) = pai_uri.strip_prefix("file://") { std::path::PathBuf::from(r) } else { std::path::PathBuf::from(&pai_uri) };
        let fonte_dir = arquivo.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        let mut dir = caminho_dir.clone();
        if dir.is_relative() {
            dir = fonte_dir.join(&caminho_dir);
            let normalizar = |p: &std::path::Path| {
                let mut v: Vec<std::path::Component> = Vec::new();
                for c in p.components() {
                    match c {
                        std::path::Component::CurDir => {}
                        std::path::Component::ParentDir => {
                            v.pop();
                        }
                        x => v.push(x),
                    }
                }
                v.iter().collect::<std::path::PathBuf>()
            };
            dir = normalizar(&dir);
            let em_lib = |p: &std::path::Path| p.components().any(|c| c.as_os_str() == "lib");
            if em_lib(&fonte_dir) && !em_lib(&dir) {
                return;
            }
        }
        let Ok(entradas) = std::fs::read_dir(&dir) else { return };
        let mut filhos: Vec<std::path::PathBuf> = entradas.filter_map(|e| e.ok().map(|e| e.path())).collect();
        filhos.sort();
        let proprio = arquivo.file_name().and_then(|s| s.to_str()).map(str::to_string);
        for f in filhos {
            let Some(nome) = f.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            let completion = if f.is_dir() {
                (!nome.starts_with('.')).then(|| format!("{prefixo_uri}{nome}/"))
            } else if nome.ends_with(".dart") {
                Some(format!("{prefixo_uri}{nome}"))
            } else {
                None
            };
            if let Some(c) = completion
                && Some(&c) != proprio.as_ref()
            {
                self.item_uri(&c);
            }
        }
    }

    // -- Closures, argumentos nomeados e campos de record ------------------------------

    /// `_addClosureSuggestion` sem os nomes dos posicionais (gerados `p0`…).
    pub(super) fn sugerir_closure(&mut self, tipo: TypeId, virgula: bool) {
        self.sugerir_closure_com_nomes(tipo, &[], virgula);
    }

    /// `_addClosureSuggestion`: a de bloco e a de seta, ambas com
    /// `matcherScore` 0 (`Relevance.closure`, 900). `nomes` são os dos
    /// posicionais do tipo de função escrito (o tipo do DartForge não os
    /// guarda).
    pub(super) fn sugerir_closure_com_nomes(&mut self, tipo: TypeId, nomes: &[String], virgula: bool) {
        let Type::Function { positional, optional, named, .. } = self.consulta.tabela.get(tipo).clone() else { return };
        let incluir_tipos = regra_ligada(self.caminho.as_deref(), "always_specify_types");
        let recuo = self.recuo_da_linha();
        let mut parametros: Vec<(String, TypeId, u8, bool)> = Vec::new();
        for (i, &t) in positional.iter().enumerate() {
            parametros.push((nomes.get(i).cloned().unwrap_or_default(), t, 0, false));
        }
        for (i, &t) in optional.iter().enumerate() {
            parametros.push((nomes.get(positional.len() + i).cloned().unwrap_or_default(), t, 1, false));
        }
        // `FunctionType.parameters`: os nomeados em ordem lexical.
        let mut nomeados: Vec<(String, TypeId, bool)> = named.iter().map(|(n, t, r)| (self.consulta.nome(*n).to_string(), *t, *r)).collect();
        nomeados.sort_by(|a, b| a.0.cmp(&b.0));
        for (n, t, r) in nomeados {
            parametros.push((n, t, 2, r));
        }
        let montar = |isp: &Self, tipos: bool, palavras: bool| -> String {
            let existentes: HashSet<String> = parametros.iter().map(|p| p.0.clone()).collect();
            let mut b = String::from("(");
            let (mut nomeado, mut opcional) = (false, false);
            for (i, (nome, t, k, req)) in parametros.iter().enumerate() {
                if i != 0 {
                    b.push_str(", ");
                }
                if *k == 2 && !nomeado {
                    nomeado = true;
                    b.push('{');
                } else if *k == 1 && !opcional {
                    opcional = true;
                    b.push('[');
                }
                if tipos {
                    b.push_str(&isp.consulta.formatar(*t));
                    b.push(' ');
                }
                let mut n = nome.clone();
                if n.is_empty() {
                    n = format!("p{i}");
                    let mut indice = 1;
                    while existentes.contains(&n) {
                        n = format!("p{i}_{indice}");
                        indice += 1;
                    }
                }
                if palavras && *req {
                    b.push_str("required ");
                }
                b.push_str(&n);
            }
            if nomeado {
                b.push('}');
            } else if opcional {
                b.push(']');
            }
            b.push(')');
            b
        };
        let completo = montar(self, incluir_tipos, true);
        let curto = montar(self, false, false);
        for bloco in [true, false] {
            let mut texto = completo.clone();
            let exib;
            let selecao;
            if bloco {
                exib = format!("{curto} {{}}");
                texto.push_str(" {\n");
                texto.push_str(&recuo);
                texto.push_str("  ");
                selecao = texto.encode_utf16().count();
                texto.push('\n');
                texto.push_str(&recuo);
                texto.push('}');
            } else {
                exib = format!("{curto} =>");
                texto.push_str(" => ");
                selecao = texto.encode_utf16().count();
            }
            if virgula {
                texto.push(',');
            }
            let antes = self.coletor.itens.len();
            let item = self.coletor.empurrar(grupo::NOMEADO, especie::METODO, exib.clone(), texto, None);
            item.exibicao = Some(exib);
            item.selecao = Some((selecao, 0));
            item.rel.fixa = Some(900);
            self.marcar(antes, 0.0, false);
        }
    }

    /// `NamedArgumentSuggestion` (`Relevance.requiredNamedArgument` 950 /
    /// `namedArgument` 900): `nome: ` (+ `[]` num parâmetro lista de widget,
    /// + `,`); `substituir == Some(0)` é o `replacementLength: 0`.
    pub(super) fn sugerir_argumento_nomeado(&mut self, p: &ParametroInvocado, dois_pontos: bool, virgula: bool, substituir: Option<usize>) {
        let Some(s) = self.pontuar(&p.nome) else { return };
        let mut completion = p.nome.clone();
        if dois_pontos {
            completion.push_str(": ");
        }
        let mut selecao = completion.encode_utf16().count();
        if p.widget && dois_pontos && self.e_lista(p.tipo) {
            let n = completion.encode_utf16().count();
            completion.push_str("[]");
            selecao = n + 1;
        }
        if virgula {
            completion.push(',');
        }
        let detalhe = self.consulta.formatar(p.tipo);
        let antes = self.coletor.itens.len();
        let fim = completion.encode_utf16().count();
        let item = self.coletor.empurrar(grupo::NOMEADO, especie::VARIAVEL, completion.clone(), completion.clone(), Some(detalhe));
        item.exibicao = Some(completion);
        if selecao != fim {
            item.selecao = Some((selecao, 0));
        }
        item.rel.fixa = Some(if p.obrigatorio || p.anotado_required { 950 } else { 900 });
        item.substituir_vazio = substituir == Some(0);
        self.marcar(antes, s, false);
    }

    /// `type.isDartCoreList`.
    pub(super) fn e_lista(&self, t: TypeId) -> bool {
        matches!(self.consulta.tabela.get(t), Type::Interface { class, .. } if Some(*class) == self.consulta.core.list_class)
    }

    /// `_suggestRecordLiteralNamedFields`: `container` é o nó visitado,
    /// `literal` o `RecordLiteral` (ou nenhum), `novo` se é campo novo
    /// (`nome: `, com a vírgula quando o token deslocado não é `,` nem `)`).
    pub(super) fn sugerir_campos_nomeados_de_record(&mut self, container: usize, literal: Option<usize>, novo: bool) {
        let alvo = literal.unwrap_or(container);
        let contexto = self.pai(alvo).and_then(|p| self.tipo_de_contexto_em(p, self.ini(alvo)));
        let Some(t) = contexto else { return };
        let Type::Record { named, .. } = self.consulta.tabela.get(t).clone() else { return };
        // `_computeDisplacedToken`: o primeiro token do nó em `offset` ou
        // depois.
        let deslocado = self.tok_depois(self.offset).filter(|&i| self.span_tok(i).start < self.fim(container).max(self.offset + 1));
        let Some(d) = deslocado else { return };
        let incluidos: HashSet<String> = match literal {
            Some(l) => self
                .filhos_de(l, &["NamedExpression"])
                .iter()
                .filter_map(|&e| self.filho(e, "Label"))
                .map(|r| self.fonte[self.ini(r)..self.fim(r)].trim_end_matches(':').trim().to_string())
                .collect(),
            None => HashSet::new(),
        };
        let virgula = !self.e_op(d, Op::Comma) && !self.e_op(d, Op::RParen);
        for (n, tipo) in named.iter() {
            let nome = self.consulta.nome(*n).to_string();
            if incluidos.contains(&nome) {
                continue;
            }
            let Some(s) = self.pontuar(&nome) else { continue };
            let mut completion = nome.clone();
            if novo {
                completion.push_str(": ");
            }
            let selecao = completion.encode_utf16().count();
            if novo && virgula {
                completion.push(',');
            }
            let detalhe = self.consulta.formatar(*tipo);
            let antes = self.coletor.itens.len();
            let fim = completion.encode_utf16().count();
            let item = self.coletor.empurrar(grupo::NOMEADO, especie::VARIAVEL, completion.clone(), completion.clone(), Some(detalhe));
            item.exibicao = Some(completion);
            if selecao != fim {
                item.selecao = Some((selecao, 0));
            }
            item.rel.fixa = Some(950);
            self.marcar(antes, s, false);
        }
    }
}

/// Um parâmetro declarado, para o `writeParameters` da sobrescrita.
#[derive(Debug, Clone)]
pub(super) struct ParametroEscrito {
    pub nome: Option<String>,
    pub nomeado: bool,
    pub opcional: bool,
    pub requerido_nomeado: bool,
    pub covariante: bool,
    pub padrao: Option<String>,
    /// A posição entre os posicionais (para o tipo substituído).
    pub posicao: usize,
}

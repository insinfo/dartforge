//! O `DeclarationHelper` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/declaration_helper.dart`, lido inteiro),
//! o `VisibilityTracker` e a tradução dos candidatos dele em itens
//! (`candidate_suggestion.dart`, `suggestion_builder.dart`).
//!
//! A ordem das sugestões é a do Dart: os locais de dentro para fora (os
//! comandos de um bloco do último para o primeiro), os membros da
//! declaração envolvente, as declarações de topo de cada unidade da
//! biblioteca por categoria (classes, enums, extension types, mixins,
//! typedefs, acessores, extensões, funções, variáveis), os prefixos, os
//! imports na ordem das diretivas da unidade definidora (o `dart:core`
//! implícito por último) e os membros herdados. Os espaços de nomes do
//! Dart são `HashMap` de `String` (a ordem de iteração é a do hash da VM);
//! aqui a ordem dentro de um espaço de nomes é a dos nomes, o que só muda a
//! ordem entre candidatos de mesmo `matcherScore` (o cliente reordena pelo
//! `sortText`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Flags, Isp, Operacao, Recurso};
use super::*;
use dartforge_elements::model::{ExtensionId, TypedefId, UnitId, VariableId, VariableRef};
use dartforge_frontend::token::Op;

/// O `importData` de um candidato: o prefixo pelo qual o elemento é
/// visível e se a biblioteca ainda não está importada.
#[derive(Debug, Clone)]
pub(super) struct Importe {
    pub prefixo: Option<String>,
    pub nao_importado: bool,
}

/// Os elementos de uma unidade por categoria, na ordem de declaração
/// (`CompilationUnitElement.classes`, `.enums`…).
#[derive(Default)]
pub(super) struct DaUnidade {
    pub(super) classes: Vec<ClassId>,
    pub(super) enums: Vec<ClassId>,
    pub(super) tipos_de_extensao: Vec<ClassId>,
    pub(super) mixins: Vec<ClassId>,
    pub(super) typedefs: Vec<TypedefId>,
    pub(super) acessores: Vec<FunctionElementId>,
    pub(super) extensoes: Vec<ExtensionId>,
    pub(super) funcoes: Vec<FunctionElementId>,
    pub(super) variaveis: Vec<VariableId>,
}

/// O nome de um item sem a chamada.
fn texto_sem_parenteses(s: &str) -> &str {
    s.split('(').next().unwrap_or(s)
}

/// O texto de um elemento sem o `=` de setter (`displayName`).
fn sem_igual(s: &str) -> String {
    s.trim_end_matches('=').to_string()
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- O estado ------------------------------------------------------------------

    /// `declarationHelper(...)`: a primeira chamada fixa as opções; o tipo
    /// de contexto função com retorno `void` desliga `mustBeNonVoid` e liga
    /// `preferNonInvocation`.
    pub(super) fn dh(&mut self, mut f: Flags) {
        if let Some(t) = self.tipo_de_contexto
            && let Type::Function { ret, .. } = self.consulta.tabela.get(t)
            && matches!(self.consulta.tabela.get(*ret), Type::Void)
        {
            f.nao_void = false;
            f.preferir_sem_invocacao = true;
        }
        if self.flags.is_none() {
            self.flags = Some(f);
        }
    }

    /// As opções vigentes.
    pub(super) fn fl(&self) -> Flags {
        self.flags.clone().unwrap_or_default()
    }

    /// `state.matcher.score(texto)`, `None` quando não casa.
    pub(super) fn pontuar(&mut self, texto: &str) -> Option<f64> {
        let s = self.score(texto);
        (s != -1.0).then_some(s)
    }

    /// `VisibilityTracker.isVisible`.
    pub(super) fn visivel(&mut self, nome: &str, imp: Option<&Importe>) -> bool {
        let q = match imp.and_then(|i| i.prefixo.as_deref()) {
            Some(p) => format!("{p}.{nome}"),
            None => nome.to_string(),
        };
        if imp.is_some_and(|i| i.nao_importado) {
            return !self.visiveis.contains(&q);
        }
        self.visiveis.insert(q)
    }

    /// `_recordOperation`.
    pub(super) fn registrar_operacao(&mut self, o: Operacao) {
        self.operacoes.push(o);
    }

    /// Marca os itens do coletor desde `antes` como do passe, com o score;
    /// `identificador` é o `CompletionSuggestionKind.IDENTIFIER` de um
    /// executável (o rótulo mostra os parâmetros; a inserção não).
    pub(super) fn marcar(&mut self, antes: usize, score: f64, identificador: bool) {
        let metodo = self.metodo_do_super.clone();
        for i in &mut self.coletor.itens[antes..] {
            i.do_passe = true;
            if identificador {
                i.identificador = true;
            }
            // `superMatchesFeature` e `isNoSuchMethodFeature` (o nome do
            // método que contém o `super.▮` não é penalizado).
            if let Some(m) = &metodo
                && i.rel.especie.is_some_and(|e| matches!(e, crate::relevancia::Especie::Metodo | crate::relevancia::Especie::Campo))
                && texto_sem_parenteses(&i.inserir) == m
            {
                i.rel.super_corresponde = true;
                i.rel.no_such_method = false;
            }
        }
        self.registrar(antes, score);
    }

    /// `completionPrefix` (`prefixo.`) nos itens desde `antes`.
    pub(super) fn prefixar(&mut self, antes: usize, imp: Option<&Importe>) {
        if let Some(p) = imp.and_then(|i| i.prefixo.clone()) {
            for i in &mut self.coletor.itens[antes..] {
                i.inserir = format!("{p}.{}", i.inserir);
                i.rotulo = format!("{p}.{}", i.rotulo);
            }
        }
    }

    /// O grupo antigo do item pela biblioteca dona (só para a ordem do
    /// caminho sem o passe).
    pub(super) fn grupo_da(&self, dona: LibraryId) -> u8 {
        if dona == self.biblioteca() { grupo::BIBLIOTECA } else { grupo::IMPORTADO }
    }

    /// `_executableSuggestionKind == IDENTIFIER`.
    pub(super) fn identificador_executavel(&self) -> bool {
        self.fl().preferir_sem_invocacao
    }

    /// `request.target.isFunctionalArgument()`: o alvo é um argumento cujo
    /// parâmetro tem tipo de função (o tipo de contexto da posição).
    pub(super) fn argumento_funcional(&self) -> bool {
        let Some(c) = self.no_coberto() else { return false };
        let mut n = c;
        if let Some(p) = self.pai(n)
            && self.especie(p) == "NamedExpression"
        {
            n = p;
        }
        self.pai(n).is_some_and(|p| self.especie(p) == "ArgumentList")
            && self.tipo_de_contexto.is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Function { .. }))
    }

    /// `type is VoidType` de um retorno.
    pub(super) fn e_void(&self, t: TypeId) -> bool {
        matches!(self.consulta.tabela.get(t), Type::Void)
    }

    /// O retorno de uma função (`returnType`).
    pub(super) fn retorno_de(&self, f: FunctionElementId) -> TypeId {
        self.consulta.outline.functions[f.0 as usize].return_type
    }

    /// `isVisibleIn`: da biblioteca do pedido, ou de nome público.
    pub(super) fn visivel_em(&self, dona: LibraryId, nome: &str) -> bool {
        dona == self.biblioteca() || !nome.starts_with('_')
    }

    /// O `thisType` de uma classe (os parâmetros dela como argumentos).
    pub(super) fn tipo_this(&mut self, c: ClassId) -> TypeId {
        let params: Vec<_> = self.consulta.outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let args: Box<[TypeId]> = params.iter().map(|&p| self.consulta.tabela.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let tipo = if self.consulta.programa.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
            Type::ExtensionType { decl: c, args, nullable: false }
        } else {
            Type::Interface { class: c, args, nullable: false }
        };
        self.consulta.tabela.intern(tipo)
    }

    /// A versão de linguagem de uma biblioteca habilita o recurso.
    pub(super) fn recurso_da_biblioteca(&self, l: LibraryId, r: Recurso) -> bool {
        let v = self.consulta.programa.library(l).features.versao();
        let minimo = match r {
            Recurso::ExtensionMethods => (2, 6),
            Recurso::EnhancedEnums | Recurso::SuperParameters => (2, 17),
            Recurso::Patterns | Recurso::ClassModifiers | Recurso::SealedClass => (3, 0),
            Recurso::InlineClass => (3, 3),
        };
        (v.major, v.minor) >= minimo
    }

    /// `isWildcardVariable`: `_` com o recurso de curingas.
    pub(super) fn curinga(&self, nome: &str) -> bool {
        nome == "_" && self.features.tem(dartforge_frontend::Feature::WildcardVariables)
    }

    // -- Os itens ------------------------------------------------------------------

    /// Um local (variável ou parâmetro): `LocalVariableSuggestion`,
    /// `FormalParameterSuggestion`.
    pub(super) fn item_local(&mut self, nome: &str, tipo: Option<TypeId>, parametro: bool, score: f64) {
        let distancia = self.distancia;
        self.distancia += 1;
        let antes = self.coletor.itens.len();
        let detalhe = tipo.map(|t| self.consulta.formatar(t));
        let item = self.coletor.empurrar(grupo::LOCAL, especie::VARIAVEL, nome.to_string(), nome.to_string(), detalhe);
        item.rel.especie = Some(if parametro { crate::relevancia::Especie::Parametro } else { crate::relevancia::Especie::Local });
        item.rel.tipo = tipo;
        item.rel.local = true;
        item.rel.distancia = Some(distancia);
        self.marcar(antes, score, false);
    }

    /// O detalhe `(T a, …) → R` de um tipo de função com os nomes dos
    /// posicionais dados.
    pub(super) fn detalhe_com_nomes(&self, tipo: TypeId, nomes: &[Option<String>]) -> String {
        let Type::Function { ret, positional, optional, named, .. } = self.consulta.tabela.get(tipo).clone() else {
            return self.consulta.formatar(tipo);
        };
        let p = |i: usize, t: TypeId| match nomes.get(i).cloned().flatten() {
            Some(n) => format!("{} {n}", self.consulta.formatar(t)),
            None => self.consulta.formatar(t),
        };
        let mut partes: Vec<String> = positional.iter().enumerate().map(|(i, &t)| p(i, t)).collect();
        if !optional.is_empty() {
            let o: Vec<String> = optional.iter().enumerate().map(|(i, &t)| p(positional.len() + i, t)).collect();
            partes.push(format!("[{}]", o.join(", ")));
        }
        if !named.is_empty() {
            let n: Vec<String> = named
                .iter()
                .map(|(s, t, r)| format!("{}{} {}", if *r { "required " } else { "" }, self.consulta.formatar(*t), self.consulta.nome(*s)))
                .collect();
            partes.push(format!("{{{}}}", n.join(", ")));
        }
        format!("({}) → {}", partes.join(", "), self.consulta.formatar(ret))
    }

    /// Uma função local (`LocalFunctionSuggestion`): o nó é o
    /// `FunctionDeclaration` dela.
    pub(super) fn item_funcao_local(&mut self, no: usize, nome: &str, tipo: Option<TypeId>, score: f64) {
        let f = self.funcao_real(no);
        let nomes: Vec<Option<String>> = f
            .and_then(|f| f.parameters.as_ref())
            .map(|ps| ps.iter().filter(|p| p.kind != ast::ParameterKind::Named).map(|p| p.name.map(|n| self.nome_real(n).to_string())).collect())
            .unwrap_or_default();
        let obrigatorios: Vec<String> = f
            .and_then(|f| f.parameters.as_ref())
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| {
                        let n = self.nome_real(p.nome_externo()?).to_string();
                        match p.kind {
                            ast::ParameterKind::Required => Some(n),
                            ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let detalhe = tipo.map(|t| self.detalhe_com_nomes(t, &nomes));
        let antes = self.coletor.itens.len();
        let item = self.coletor.empurrar(grupo::LOCAL, especie::FUNCAO, nome.to_string(), nome.to_string(), detalhe);
        item.chamada = Some(Chamada::Parametros(obrigatorios));
        item.rel.especie = Some(crate::relevancia::Especie::Funcao);
        item.rel.tipo = tipo.and_then(|t| retorno(self.consulta, t));
        let ident = self.identificador_executavel();
        self.marcar(antes, score, ident);
    }

    /// Um parâmetro de tipo (`TypeParameterSuggestion`).
    pub(super) fn item_parametro_de_tipo(&mut self, nome: &str, score: f64) {
        let antes = self.coletor.itens.len();
        self.coletor.empurrar(grupo::LOCAL, especie::PARAMETRO_DE_TIPO, nome.to_string(), nome.to_string(), None).rel.especie =
            Some(crate::relevancia::Especie::ParametroDeTipo);
        self.marcar(antes, score, false);
    }

    /// Um membro (método, acessor) com o tipo visto pelo receptor; a
    /// distância de herança é a partir de `referencia`.
    pub(super) fn item_membro(&mut self, f: FunctionElementId, tipo: TypeId, referencia: Option<ClassId>, score: f64, identificador: bool) {
        let antes = self.coletor.itens.len();
        let de_object = self.consulta.core.object_class.is_some() && self.consulta.programa.function(f).class == self.consulta.core.object_class;
        let g = if de_object { grupo::MEMBRO_DE_OBJECT } else { grupo::MEMBRO };
        self.coletor.receptor = referencia;
        self.coletor.funcao(&*self.consulta, g, f, tipo);
        self.coletor.receptor = None;
        self.marcar(antes, score, identificador);
    }

    /// Um campo (`FieldSuggestion`): constante de enum vira `Enum.valor`.
    pub(super) fn item_campo(&mut self, v: VariableId, tipo: Option<TypeId>, referencia: Option<ClassId>, score: f64) {
        let p = &self.consulta.programa;
        let ve = p.variable(v);
        let constante_de_enum = ve.class.is_some_and(|c| p.class(c).enum_constants.contains(&v));
        let tipo = tipo.or_else(|| self.consulta.tipo_da_variavel(v)).unwrap_or(self.consulta.core.dynamic_);
        let antes = self.coletor.itens.len();
        match ve.getter {
            Some(g) => {
                self.coletor.receptor = referencia;
                self.coletor.funcao(&*self.consulta, grupo::MEMBRO, g, tipo);
                self.coletor.receptor = None;
            }
            None => {
                let nome = self.consulta.nome(ve.name).to_string();
                let detalhe = self.consulta.formatar(tipo);
                let origem = self.consulta.origem(self.consulta.inicio_da_variavel(v));
                let item = self.coletor.empurrar(grupo::MEMBRO, especie::CAMPO, nome.clone(), nome, Some(detalhe));
                item.origem = origem;
                item.rel.especie = Some(crate::relevancia::Especie::Campo);
                item.rel.tipo = Some(tipo);
            }
        }
        if constante_de_enum && let Some(c) = ve.class {
            let e = self.consulta.nome(p.class(c).name).to_string();
            for i in &mut self.coletor.itens[antes..] {
                i.inserir = format!("{e}.{}", i.inserir);
                i.rotulo = i.inserir.clone();
                i.rel.especie = Some(crate::relevancia::Especie::SemTabela);
                i.rel.distancia = None;
            }
        }
        self.marcar(antes, score, false);
    }

    /// Um elemento de topo (classe, enum, mixin, extensão, typedef, função,
    /// acessor, variável) com o prefixo do import.
    pub(super) fn item_elemento(&mut self, el: Element, imp: Option<&Importe>, score: f64, identificador: bool) {
        let antes = self.coletor.itens.len();
        let g = self.grupo_da(super::biblioteca_do_elemento(self.consulta, el));
        self.coletor.elemento(&*self.consulta, g, el);
        self.prefixar(antes, imp);
        self.marcar(antes, score, identificador);
    }

    /// Um construtor (`ConstructorSuggestion` pelo `suggestConstructor`): o
    /// texto já calculado, `INVOCATION` ou `IDENTIFIER`.
    pub(super) fn item_construtor(&mut self, f: FunctionElementId, completion: String, invocacao: bool, imp: Option<&Importe>, score: f64) {
        let consulta = &*self.consulta;
        let classe = consulta.programa.function(f).class;
        let assinatura = consulta.outline.functions[f.0 as usize].signature;
        let detalhe = consulta.detalhe_de_funcao(f, assinatura);
        let parametros: Vec<String> = consulta.outline.functions[f.0 as usize]
            .parameters
            .iter()
            .filter_map(|p| {
                let n = consulta.nome(p.externo.or(p.name)?).to_string();
                match p.kind {
                    ast::ParameterKind::Required => Some(n),
                    ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                    _ => None,
                }
            })
            .collect();
        let origem = consulta.origem(consulta.inicio_da_funcao(f));
        let dona = consulta.programa.function(f).library;
        let g = self.grupo_da(dona);
        let antes = self.coletor.itens.len();
        let item = self.coletor.empurrar(g, especie::CONSTRUTOR, completion.clone(), completion, Some(detalhe));
        item.chamada = Some(Chamada::Parametros(parametros));
        item.origem = origem;
        item.rel.especie = Some(crate::relevancia::Especie::Construtor);
        // `ctx(inst(classe envolvente))`: a classe com argumentos `Never`.
        item.classe = classe;
        self.prefixar(antes, imp);
        self.marcar(antes, score, !invocacao);
    }

    // -- addLexicalDeclarations ----------------------------------------------------

    /// `addLexicalDeclarations`.
    pub(super) fn dh_lexicas(&mut self, n: usize) {
        let tipo = self.fl().tipo;
        let membro = if tipo { self.tipos_locais(n) } else { self.declaracoes_locais(n) };
        let Some(m) = membro else { return };
        let mut pai = Some(self.pai(m).unwrap_or(m));
        if let Some(p) = pai {
            if matches!(self.especie(p), "ConstructorDeclaration" | "FieldDeclaration" | "MethodDeclaration") {
                pai = self.pai(p);
            } else if p == 0 {
                pai = Some(m);
            }
        }
        let mut topo = None;
        if let Some(p) = pai
            && p != 0
            && self.pai(p) == Some(0)
        {
            topo = Some(p);
            self.membros_do_no_envolvente(p);
            pai = self.pai(p);
        }
        if pai == Some(0) {
            self.declaracoes_de_topo();
            self.dh_prefixos_de_import();
            self.declaracoes_importadas();
            self.registrar_operacao(Operacao::MembrosEstaticos);
        }
        let f = self.fl();
        if let Some(t) = topo
            && !f.estatico
            && !f.tipo
        {
            self.membros_herdados(t);
        }
    }

    /// `_addLocalDeclarations`: devolve o membro que contém as declarações.
    pub(super) fn declaracoes_locais(&mut self, n: usize) -> Option<usize> {
        let mut anterior: Option<usize> = None;
        let mut atual = Some(n);
        while let Some(k) = atual {
            match self.especie(k) {
                "Block" => {
                    let comandos: Vec<usize> = self.filhos(k).to_vec();
                    self.visitar_comandos(&comandos, anterior);
                }
                "CatchClause" => self.visitar_catch(k),
                "CommentReference" => return self.visitar_referencia_de_comentario(k),
                "ConstructorDeclaration" => {
                    self.visitar_lista_de_parametros(self.filho(k, "FormalParameterList"));
                    return Some(k);
                }
                "DeclaredVariablePattern" => self.visitar_padrao_de_variavel(k),
                "FieldDeclaration" => return Some(k),
                "ForElement" | "ForStatement" => {
                    let partes = self.filhos(k).first().copied();
                    if let Some(p) = partes
                        && Some(p) != anterior
                    {
                        self.visitar_partes_de_for(p);
                    }
                }
                "ForPartsWithDeclarations" => {
                    let variaveis = self.filho(k, "VariableDeclarationList");
                    if variaveis != anterior {
                        self.visitar_partes_de_for(k);
                    }
                }
                "FunctionDeclaration" => {
                    if self.pai(k).is_none_or(|p| self.especie(p) != "FunctionDeclarationStatement") {
                        return Some(k);
                    }
                }
                "FunctionDeclarationStatement" => {
                    if let Some(d) = self.filho(k, "FunctionDeclaration") {
                        self.sugerir_funcao_local(d);
                    }
                }
                "FunctionExpression" => {
                    self.visitar_lista_de_parametros(self.filho(k, "FormalParameterList"));
                    self.visitar_parametros_de_tipo(self.filho(k, "TypeParameterList"));
                }
                "IfElement" | "IfStatement" => self.visitar_if(k),
                "MethodDeclaration" => {
                    self.visitar_lista_de_parametros(self.filho(k, "FormalParameterList"));
                    self.visitar_parametros_de_tipo(self.filho(k, "TypeParameterList"));
                    return Some(k);
                }
                "SwitchCase" | "SwitchDefault" => {
                    let comandos: Vec<usize> = self.filhos(k).iter().copied().filter(|&c| super::isp::e_comando(self.especie(c))).collect();
                    self.visitar_comandos(&comandos, anterior);
                }
                "SwitchExpressionCase" => self.visitar_caso_de_switch_expressao(k),
                "SwitchPatternCase" => self.visitar_caso_de_padrao(k, anterior),
                "VariableDeclarationList" => self.visitar_lista_de_variaveis(k, anterior),
                "CompilationUnit" => return Some(k),
                e if self.pai(k) == Some(0) && e != "ScriptTag" && !self.e_diretiva(k) => return Some(k),
                _ => {}
            }
            anterior = Some(k);
            atual = self.pai(k);
        }
        None
    }

    /// `_addLocalTypes`.
    pub(super) fn tipos_locais(&mut self, n: usize) -> Option<usize> {
        let mut atual = Some(n);
        while let Some(k) = atual {
            match self.especie(k) {
                "CommentReference" => return Some(k),
                "ConstructorDeclaration" => {
                    self.visitar_lista_de_parametros(self.filho(k, "FormalParameterList"));
                    return Some(k);
                }
                "FieldDeclaration" => return Some(k),
                "FunctionDeclaration" => {
                    if self.pai(k).is_none_or(|p| self.especie(p) != "FunctionDeclarationStatement") {
                        return Some(k);
                    }
                }
                "FunctionExpression" | "GenericFunctionType" => self.visitar_parametros_de_tipo(self.filho(k, "TypeParameterList")),
                "MethodDeclaration" => {
                    self.visitar_parametros_de_tipo(self.filho(k, "TypeParameterList"));
                    return Some(k);
                }
                "CompilationUnit" => return Some(k),
                e if self.pai(k) == Some(0) && e != "ScriptTag" && !self.e_diretiva(k) => return Some(k),
                _ => {}
            }
            atual = self.pai(k);
        }
        None
    }

    /// `_suggestVariable` de um local declarado com o nome em `nome_off`
    /// (`constante`: a declaração é `const`).
    pub(super) fn sugerir_variavel(&mut self, nome: &str, nome_off: usize, constante: bool) {
        if self.curinga(nome) || !self.visivel(nome, None) {
            return;
        }
        if self.fl().constante && !constante {
            return;
        }
        if let Some(s) = self.pontuar(nome) {
            let tipo = self.tipo_do_local(nome_off);
            self.item_local(nome, tipo, false, s);
        }
    }

    /// `_suggestParameter` (o nó é o do parâmetro).
    pub(super) fn sugerir_parametro(&mut self, no: usize) {
        let Some(p) = self.parametro_real(no) else { return };
        let Some(nome) = p.name else { return };
        let texto = self.nome_real(nome).to_string();
        if !self.visivel(&texto, None) {
            return;
        }
        if self.fl().constante || texto == "_" {
            return;
        }
        if let Some(s) = self.pontuar(&texto) {
            let tipo = self.tipo_do_local(nome.span.start).or_else(|| self.tipo_do_parametro_declarado(no, &texto));
            self.item_local(&texto, tipo, true, s);
        }
    }

    /// O tipo de um parâmetro pela assinatura da função ou do construtor
    /// declarado (quando o corpo não registrou o local).
    pub(super) fn tipo_do_parametro_declarado(&self, no: usize, nome: &str) -> Option<TypeId> {
        let lista = self.lista_de_parametros_de(no).or_else(|| self.pai(no))?;
        let dono = self.pai(lista)?;
        let f = match self.especie(dono) {
            "ConstructorDeclaration" => self.construtor_declarado(dono),
            "MethodDeclaration" => self.funcao_real(dono).and_then(|x| x.name).and_then(|n| self.funcao_declarada(n.span.start)),
            "FunctionExpression" => {
                let d = self.pai(dono).filter(|&d| self.especie(d) == "FunctionDeclaration")?;
                self.funcao_real(d).and_then(|x| x.name).and_then(|n| self.funcao_declarada(n.span.start))
            }
            _ => None,
        }?;
        self.consulta.outline.functions[f.0 as usize].parameters.iter().find(|p| p.name.is_some_and(|s| self.consulta.nome(s) == nome)).map(|p| p.ty)
    }

    /// `_suggestLocalFunction` (o nó é o `FunctionDeclaration`).
    pub(super) fn sugerir_funcao_local(&mut self, d: usize) {
        let Some(f) = self.funcao_real(d) else { return };
        let Some(nome) = f.name else { return };
        let texto = self.nome_real(nome).to_string();
        if !self.visivel(&texto, None) {
            return;
        }
        let tipo = self.tipo_do_local(nome.span.start);
        let fl = self.fl();
        let ret_void = tipo.is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Function { ret, .. } if self.e_void(*ret)));
        if fl.atribuivel || fl.constante || (fl.nao_void && ret_void) {
            return;
        }
        if texto == "_" {
            return;
        }
        if let Some(s) = self.pontuar(&texto) {
            self.item_funcao_local(d, &texto, tipo, s);
        }
    }

    /// `_visitCatchClause`.
    pub(super) fn visitar_catch(&mut self, k: usize) {
        for p in self.filhos_de(k, &["CatchClauseParameter"]) {
            let nome = self.fonte[self.ini(p)..self.fim(p)].to_string();
            let off = self.ini(p);
            self.sugerir_variavel(&nome, off, false);
        }
    }

    /// `_visitCommentReference`.
    pub(super) fn visitar_referencia_de_comentario(&mut self, k: usize) -> Option<usize> {
        let comentario = self.pai(k)?;
        if let Some(membro) = self.pai(comentario) {
            match self.especie(membro) {
                "ConstructorDeclaration" => self.visitar_lista_de_parametros(self.filho(membro, "FormalParameterList")),
                "FunctionDeclaration" => {
                    if let Some(fe) = self.filho(membro, "FunctionExpression") {
                        self.visitar_lista_de_parametros(self.filho(fe, "FormalParameterList"));
                        self.visitar_parametros_de_tipo(self.filho(fe, "TypeParameterList"));
                    }
                }
                "FunctionExpression" | "MethodDeclaration" => {
                    self.visitar_lista_de_parametros(self.filho(membro, "FormalParameterList"));
                    self.visitar_parametros_de_tipo(self.filho(membro, "TypeParameterList"));
                }
                _ => {}
            }
        }
        Some(comentario)
    }

    /// `_visitDeclaredVariablePattern`.
    pub(super) fn visitar_padrao_de_variavel(&mut self, k: usize) {
        if let Some(i) = self.nome_do_padrao_de_variavel(k) {
            let nome = self.texto_tok(i).to_string();
            let off = self.span_tok(i).start;
            self.sugerir_variavel(&nome, off, false);
        }
    }

    /// `_visitForLoopParts`.
    pub(super) fn visitar_partes_de_for(&mut self, k: usize) {
        match self.especie(k) {
            "ForEachPartsWithDeclaration" => {
                if let Some(d) = self.filho(k, "DeclaredIdentifier")
                    && let Some(i) = self.tok_antes(self.fim(d))
                {
                    let nome = self.texto_tok(i).to_string();
                    let off = self.span_tok(i).start;
                    self.sugerir_variavel(&nome, off, false);
                }
            }
            "ForEachPartsWithPattern" => {
                if let Some(p) = self.filhos(k).iter().copied().find(|&c| super::isp::e_padrao(self.especie(c))) {
                    self.visitar_padrao(p);
                }
            }
            "ForPartsWithDeclarations" => {
                if let Some(l) = self.filho(k, "VariableDeclarationList") {
                    let constante = self.lista_real(l).is_some_and(|x| x.const_);
                    for v in self.filhos_de(l, &["VariableDeclaration"]) {
                        if let Some(var) = self.variavel_real(v) {
                            let nome = self.nome_real(var.name).to_string();
                            self.sugerir_variavel(&nome, var.name.span.start, constante);
                        }
                    }
                }
            }
            "ForPartsWithPattern" => {
                if let Some(d) = self.filho(k, "PatternVariableDeclaration")
                    && let Some(p) = self.filhos(d).iter().copied().find(|&c| super::isp::e_padrao(self.especie(c)))
                {
                    self.visitar_padrao(p);
                }
            }
            _ => {}
        }
    }

    /// `_visitIfElement` / `_visitIfStatement`.
    pub(super) fn visitar_if(&mut self, k: usize) {
        let abre = self.op_em(Op::LParen, self.ini(k), self.fim(k));
        let depois = abre.and_then(|a| self.par(a)).map_or(self.ini(k), |f| self.span_tok(f).end);
        let senao = self.palavra_em("else", depois, self.fim(k));
        if senao.is_none_or(|e| self.offset < self.span_tok(e).start)
            && let Some(c) = self.filho(k, "CaseClause")
            && let Some(g) = self.filho(c, "GuardedPattern")
            && let Some(&p) = self.filhos(g).first()
        {
            self.visitar_padrao(p);
        }
    }

    /// `_visitParameterList`.
    pub(super) fn visitar_lista_de_parametros(&mut self, lista: Option<usize>) {
        let Some(l) = lista else { return };
        for p in self.filhos(l).to_vec() {
            if super::isp::e_parametro(self.especie(p)) {
                self.sugerir_parametro(p);
            }
        }
    }

    /// `_visitPattern`.
    pub(super) fn visitar_padrao(&mut self, p: usize) {
        match self.especie(p) {
            "DeclaredVariablePattern" => self.visitar_padrao_de_variavel(p),
            "CastPattern" | "ListPattern" | "LogicalAndPattern" | "LogicalOrPattern" | "MapPattern" | "NullAssertPattern" | "NullCheckPattern"
            | "ObjectPattern" | "ParenthesizedPattern" | "RecordPattern" => {
                for c in self.filhos(p).to_vec() {
                    match self.especie(c) {
                        e if super::isp::e_padrao(e) => {
                            if !matches!(self.especie(p), "ObjectPattern" | "RecordPattern" | "MapPattern") {
                                self.visitar_padrao(c);
                            }
                        }
                        "RestPatternElement" => {
                            if matches!(self.especie(p), "ListPattern" | "MapPattern")
                                && let Some(q) = self.filhos(c).iter().copied().find(|&x| super::isp::e_padrao(self.especie(x)))
                            {
                                self.visitar_padrao(q);
                            }
                        }
                        "PatternField" => {
                            if let Some(q) = self.filhos(c).iter().copied().find(|&x| super::isp::e_padrao(self.especie(x))) {
                                self.visitar_padrao(q);
                            }
                        }
                        "MapPatternEntry" => {
                            if let Some(&q) = self.filhos(c).last()
                                && super::isp::e_padrao(self.especie(q))
                            {
                                self.visitar_padrao(q);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// `_visitStatements`: do último para o primeiro, sem o filho de onde se
    /// veio.
    pub(super) fn visitar_comandos(&mut self, comandos: &[usize], filho: Option<usize>) {
        for &c in comandos.iter().rev() {
            if Some(c) == filho {
                continue;
            }
            if self.ini(c) >= self.offset {
                continue;
            }
            match self.especie(c) {
                "VariableDeclarationStatement" => {
                    let Some(l) = self.filho(c, "VariableDeclarationList") else { continue };
                    let constante = self.lista_real(l).is_some_and(|x| x.const_);
                    for v in self.filhos_de(l, &["VariableDeclaration"]) {
                        if self.fim(v) < self.offset
                            && let Some(var) = self.variavel_real(v)
                        {
                            let nome = self.nome_real(var.name).to_string();
                            self.sugerir_variavel(&nome, var.name.span.start, constante);
                        }
                    }
                }
                "FunctionDeclarationStatement" => {
                    if let Some(d) = self.filho(c, "FunctionDeclaration")
                        && self.ini(d) < self.offset
                        && self.funcao_real(d).and_then(|f| f.name).is_some_and(|n| !self.nome_real(n).is_empty())
                    {
                        self.sugerir_funcao_local(d);
                    }
                }
                "PatternVariableDeclarationStatement" => {
                    if let Some(d) = self.filho(c, "PatternVariableDeclaration")
                        && self.fim(d) < self.offset
                        && let Some(p) = self.filhos(d).iter().copied().find(|&x| super::isp::e_padrao(self.especie(x)))
                    {
                        self.visitar_padrao(p);
                    }
                }
                _ => {}
            }
        }
    }

    /// `_visitSwitchExpressionCase`.
    pub(super) fn visitar_caso_de_switch_expressao(&mut self, k: usize) {
        let gp = self.filho(k, "GuardedPattern");
        let seta = self.op_em(Op::Arrow, gp.map_or(self.ini(k), |g| self.fim(g)), self.fim(k).max(self.ini(k) + 1));
        let fim_seta = seta.map_or_else(|| self.sintetico(gp.map_or(self.ini(k), |g| self.fim(g))).end, |s| self.span_tok(s).end);
        if self.offset >= fim_seta
            && let Some(g) = gp
            && let Some(&p) = self.filhos(g).first()
        {
            self.visitar_padrao(p);
        }
    }

    /// `_visitSwitchPatternCase`.
    pub(super) fn visitar_caso_de_padrao(&mut self, k: usize, filho: Option<usize>) {
        let dois_pontos = self.dois_pontos_do_caso(k);
        let fim_dp = match self.tok_em(dois_pontos) {
            Some(i) if self.e_op(i, Op::Colon) => self.span_tok(i).end,
            _ => dois_pontos,
        };
        if self.offset < fim_dp {
            return;
        }
        let comandos: Vec<usize> = self.filhos(k).iter().copied().filter(|&c| super::isp::e_comando(self.especie(c))).collect();
        self.visitar_comandos(&comandos, filho);
        if let Some(g) = self.filho(k, "GuardedPattern")
            && let Some(&p) = self.filhos(g).first()
        {
            self.visitar_padrao(p);
        }
        if let Some(sw) = self.pai(k)
            && self.especie(sw) == "SwitchStatement"
        {
            let membros = self.filhos_de(sw, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]);
            if let Some(i) = membros.iter().position(|&m| m == k) {
                for &m in membros[..i].iter().rev() {
                    let vazio = !self.filhos(m).iter().any(|&c| super::isp::e_comando(self.especie(c)));
                    if self.especie(m) == "SwitchPatternCase" && vazio {
                        if let Some(g) = self.filho(m, "GuardedPattern")
                            && let Some(&p) = self.filhos(g).first()
                        {
                            self.visitar_padrao(p);
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    /// `_visitTypeParameterList`.
    pub(super) fn visitar_parametros_de_tipo(&mut self, lista: Option<usize>) {
        let Some(l) = lista else { return };
        if self.fl().excluidos.contains(&l) {
            return;
        }
        for tp in self.filhos_de(l, &["TypeParameter"]) {
            let nome = self.primeiro_depois_das_anotacoes(tp).map(|i| self.texto_tok(i).to_string());
            if let Some(n) = nome
                && n != "_"
            {
                self.sugerir_parametro_de_tipo(&n);
            }
        }
    }

    /// `_suggestTypeParameter`.
    pub(super) fn sugerir_parametro_de_tipo(&mut self, nome: &str) {
        if self.visivel(nome, None)
            && let Some(s) = self.pontuar(nome)
        {
            self.item_parametro_de_tipo(nome, s);
        }
    }

    /// `_visitVariableDeclarationList`: as variáveis antes do filho.
    pub(super) fn visitar_lista_de_variaveis(&mut self, k: usize, filho: Option<usize>) {
        let Some(f) = filho.filter(|&f| self.especie(f) == "VariableDeclaration") else { return };
        // Só listas locais declaram `LocalVariableElement`.
        let local = self.pai(k).is_some_and(|p| matches!(self.especie(p), "VariableDeclarationStatement" | "ForPartsWithDeclarations"));
        if !local {
            return;
        }
        let constante = self.lista_real(k).is_some_and(|x| x.const_);
        let vs = self.filhos_de(k, &["VariableDeclaration"]);
        let Some(i) = vs.iter().position(|&v| v == f) else { return };
        for &v in vs[..i].iter().rev() {
            if let Some(var) = self.variavel_real(v) {
                let nome = self.nome_real(var.name).to_string();
                self.sugerir_variavel(&nome, var.name.span.start, constante);
            }
        }
    }

    // -- A declaração envolvente -----------------------------------------------------

    /// `_addMembersOfEnclosingNode`.
    pub(super) fn membros_do_no_envolvente(&mut self, d: usize) {
        let tipo = self.fl().tipo;
        match self.especie(d) {
            "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" => {
                let Some(c) = self.classe_do_no(d) else { return };
                if !tipo {
                    self.membros_da_instancia_envolvente(Some(c), None);
                    if self.especie(d) == "ExtensionTypeDeclaration"
                        && let Some(r) = self.consulta.programa.class(c).representation
                    {
                        self.sugerir_campo(r, None, None);
                    }
                }
                let nomes: Vec<String> = self.consulta.programa.class(c).type_params.iter().map(|t| self.consulta.nome(t.name).to_string()).collect();
                self.sugerir_parametros_de_tipo(&nomes);
            }
            "ExtensionDeclaration" => {
                let nome = self.decl_real(d).and_then(|x| match &x.kind {
                    ast::DeclKind::Extension(e) => e.name.map(|n| n.span.start),
                    _ => None,
                });
                let Some(x) = self.extensao_em(self.ini(d), nome) else { return };
                if !tipo {
                    self.membros_da_instancia_envolvente(None, Some(x));
                }
                let nomes: Vec<String> = self.consulta.programa.extension(x).type_params.iter().map(|t| self.consulta.nome(t.name).to_string()).collect();
                self.sugerir_parametros_de_tipo(&nomes);
            }
            "ClassTypeAlias" => {
                if let Some(c) = self.classe_do_no(d) {
                    let nomes: Vec<String> = self.consulta.programa.class(c).type_params.iter().map(|t| self.consulta.nome(t.name).to_string()).collect();
                    self.sugerir_parametros_de_tipo(&nomes);
                }
            }
            "FunctionTypeAlias" | "GenericTypeAlias" => {
                if let Some(t) = self.typedef_do_no(d) {
                    let nomes: Vec<String> = self.consulta.programa.typedef(t).type_params.iter().map(|t| self.consulta.nome(t.name).to_string()).collect();
                    self.sugerir_parametros_de_tipo(&nomes);
                }
            }
            _ => {}
        }
    }

    /// O typedef declarado pelo nó.
    pub(super) fn typedef_do_no(&self, d: usize) -> Option<TypedefId> {
        let alvo = self.mapear(self.decl_real(d)?.span.start);
        let p = &self.consulta.programa;
        (0..p.typedefs.len())
            .map(|i| TypedefId(i as u32))
            .find(|&t| p.typedef(t).decl.unit == self.unidade && p.unit(self.unidade).ast.decl(p.typedef(t).decl.decl).span.start == alvo)
    }

    /// `_suggestTypeParameters`.
    pub(super) fn sugerir_parametros_de_tipo(&mut self, nomes: &[String]) {
        for n in nomes {
            if n != "_" {
                self.sugerir_parametro_de_tipo(n);
            }
        }
    }

    /// Os membros declarados de uma classe (ou extensão): acessores
    /// declarados, campos, métodos, cada grupo na ordem de declaração.
    pub(super) fn membros_declarados(&self, c: Option<ClassId>, x: Option<ExtensionId>) -> (Vec<FunctionElementId>, Vec<VariableId>, Vec<FunctionElementId>) {
        let p = &self.consulta.programa;
        let (inst, est, campos, constantes): (Vec<FunctionElementId>, Vec<FunctionElementId>, Vec<VariableId>, Vec<VariableId>) = match (c, x) {
            (Some(c), _) => {
                let cl = p.class(c);
                (cl.instance_members.values().copied().collect(), cl.static_members.values().copied().collect(), cl.fields.clone(), cl.enum_constants.clone())
            }
            (None, Some(x)) => {
                let e = p.extension(x);
                (e.instance_members.values().copied().collect(), e.static_members.values().copied().collect(), e.fields.clone(), Vec::new())
            }
            _ => return (Vec::new(), Vec::new(), Vec::new()),
        };
        let mut todos: Vec<FunctionElementId> = inst.into_iter().chain(est).collect();
        todos.sort();
        todos.dedup();
        let acessores = todos.iter().copied().filter(|&f| matches!(p.function(f).kind, FunctionKind::Getter | FunctionKind::Setter)).collect();
        let metodos = todos.iter().copied().filter(|&f| matches!(p.function(f).kind, FunctionKind::Function | FunctionKind::Operator)).collect();
        let mut vs = constantes;
        for v in campos {
            if !vs.contains(&v) {
                vs.push(v);
            }
        }
        (acessores, vs, metodos)
    }

    /// `_addMembersOfEnclosingInstance`.
    pub(super) fn membros_da_instancia_envolvente(&mut self, c: Option<ClassId>, x: Option<ExtensionId>) {
        let estatico = self.fl().estatico;
        let referencia = match (c, x) {
            (Some(c), _) => Some(c),
            (None, Some(x)) => {
                let on = self.consulta.outline.extensions[x.0 as usize].on;
                match self.consulta.tabela.get(on) {
                    Type::Interface { class, .. } => Some(*class),
                    Type::ExtensionType { decl, .. } => Some(*decl),
                    _ => None,
                }
            }
            _ => None,
        };
        let (acessores, campos, metodos) = self.membros_declarados(c, x);
        for f in acessores {
            if !estatico || self.consulta.programa.function(f).static_ {
                self.sugerir_propriedade(f, None, false, None, referencia);
            }
        }
        for v in campos {
            let ve = self.consulta.programa.variable(v);
            if !estatico || ve.static_ {
                self.sugerir_campo(v, None, referencia);
            }
        }
        for f in metodos {
            if !estatico || self.consulta.programa.function(f).static_ {
                self.sugerir_metodo(f, None, false, None, referencia);
            }
        }
        let this = match (c, x) {
            (Some(c), _) => Some(self.tipo_this(c)),
            (None, Some(x)) => Some(self.consulta.outline.extensions[x.0 as usize].on),
            _ => None,
        };
        if let Some(t) = this
            && matches!(self.consulta.tabela.get(t), Type::Interface { .. } | Type::ExtensionType { .. })
        {
            self.membros_de_extensoes(t, &HashSet::new(), true, true);
        }
    }

    // -- As sugestões de membros -----------------------------------------------------

    /// `_suggestField`.
    pub(super) fn sugerir_campo(&mut self, v: VariableId, tipo: Option<TypeId>, referencia: Option<ClassId>) {
        let ve = self.consulta.programa.variable(v).clone();
        let nome = self.consulta.nome(ve.name).to_string();
        if !self.visivel(&nome, None) {
            return;
        }
        let f = self.fl();
        if (f.atribuivel && ve.setter.is_none()) || (f.constante && !ve.const_) {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_campo(v, tipo, referencia, s);
        }
    }

    /// `_suggestMethod`.
    pub(super) fn sugerir_metodo(&mut self, m: FunctionElementId, tipo: Option<TypeId>, ignorar_visibilidade: bool, imp: Option<&Importe>, referencia: Option<ClassId>) {
        let nome = sem_igual(self.consulta.nome(self.consulta.programa.function(m).name));
        if !ignorar_visibilidade && !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        let tipo = tipo.unwrap_or_else(|| super::tipo_declarado(self.consulta, m));
        let ret = match self.consulta.tabela.get(tipo) {
            Type::Function { ret, .. } => *ret,
            _ => self.retorno_de(m),
        };
        if f.atribuivel || f.constante || (f.nao_void && self.e_void(ret)) {
            return;
        }
        let Some(s) = self.pontuar(&nome) else { return };
        // Operadores: o `_getCompletionString` é nulo.
        if self.consulta.programa.function(m).kind == FunctionKind::Operator {
            return;
        }
        if nome == "setState" && self.e_state_exato(m) {
            self.item_set_state(m, referencia, s);
            return;
        }
        let ident = f.preferir_sem_invocacao || self.argumento_funcional();
        let antes = self.coletor.itens.len();
        self.item_membro(m, tipo, referencia, s, ident);
        self.prefixar(antes, imp);
    }

    /// `ClassElement.isExactState`: o `State` do Flutter.
    pub(super) fn e_state_exato(&self, m: FunctionElementId) -> bool {
        let p = &self.consulta.programa;
        let Some(c) = p.function(m).class else { return false };
        self.consulta.nome(p.class(c).name) == "State" && p.library(p.class(c).library).uri == "package:flutter/src/widgets/framework.dart"
    }

    /// `SetStateMethodSuggestion`: `setState(() {\n$indent  \n$indent});`.
    pub(super) fn item_set_state(&mut self, m: FunctionElementId, referencia: Option<ClassId>, score: f64) {
        let recuo = self.recuo_da_linha();
        let completion = format!("setState(() {{\n{recuo}  \n{recuo}}});");
        let selecao = format!("setState(() {{\n{recuo}  ").encode_utf16().count();
        let tipo = super::tipo_declarado(self.consulta, m);
        let antes = self.coletor.itens.len();
        self.item_membro(m, tipo, referencia, score, true);
        for i in &mut self.coletor.itens[antes..] {
            i.inserir = completion.clone();
            i.exibicao = Some("setState(() {});".to_string());
            i.selecao = Some((selecao, 0));
            i.chamada = None;
        }
    }

    /// `state.indent`: o recuo da linha do cursor.
    pub(super) fn recuo_da_linha(&self) -> String {
        // `getRequestLineIndent`: o branco do começo da linha até o primeiro
        // caractere que não é branco antes do cursor.
        let o = self.offset.min(self.fonte.len());
        let ini = self.fonte[..o].rfind('\n').map_or(0, |i| i + 1);
        self.fonte[ini..o].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
    }

    /// `_suggestProperty`: `f` é o getter ou o setter declarado (ou o
    /// acessor implícito de um campo).
    pub(super) fn sugerir_propriedade(&mut self, f: FunctionElementId, tipo: Option<TypeId>, ignorar_visibilidade: bool, imp: Option<&Importe>, referencia: Option<ClassId>) {
        let fe = self.consulta.programa.function(f).clone();
        let nome = sem_igual(self.consulta.nome(fe.name));
        if !ignorar_visibilidade && !self.visivel(&nome, imp) {
            return;
        }
        let fl = self.fl();
        let getter = matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) && !self.consulta.nome(fe.name).ends_with('=');
        let sem_setter = getter && self.setter_correspondente(f).is_none();
        let ret = if getter { tipo.unwrap_or_else(|| self.retorno_de(f)) } else { self.consulta.core.void_ };
        if (fl.atribuivel && sem_setter) || fl.constante || (fl.nao_void && self.e_void(ret)) {
            return;
        }
        let Some(s) = self.pontuar(&nome) else { return };
        if fe.kind == FunctionKind::ImplicitAccessor {
            // Sintético: só o getter, como `FieldSuggestion` da variável.
            if getter && let Some(v) = fe.variable {
                let antes = self.coletor.itens.len();
                self.item_campo(v, tipo, referencia, s);
                self.prefixar(antes, imp);
            }
            return;
        }
        let tipo = tipo.unwrap_or_else(|| super::tipo_declarado(self.consulta, f));
        let antes = self.coletor.itens.len();
        self.item_membro(f, tipo, referencia, s, false);
        self.prefixar(antes, imp);
    }

    /// `correspondingSetter` de um getter (de membro ou de topo).
    pub(super) fn setter_correspondente(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let p = &self.consulta.programa;
        let fe = p.function(f);
        if let Some(v) = fe.variable {
            return p.variable(v).setter;
        }
        let nome = format!("{}=", self.consulta.nome(fe.name));
        let s = self.consulta.nomes.lookup(&nome)?;
        if let Some(c) = fe.class {
            let cl = p.class(c);
            return if fe.static_ { cl.static_members.get(&s).copied() } else { cl.instance_members.get(&s).copied() };
        }
        if let Some(x) = fe.extension {
            let e = p.extension(x);
            return if fe.static_ { e.static_members.get(&s).copied() } else { e.instance_members.get(&s).copied() };
        }
        let b = p.library(fe.library).declared.get(&self.consulta.nomes.lookup(self.consulta.nome(fe.name))?)?;
        match b.setter {
            Some(Element::Function(g)) => Some(g),
            _ => None,
        }
    }

    // -- Os membros de instância ------------------------------------------------------

    /// Os membros de instância visíveis numa classe, na ordem dos mapas do
    /// `InheritanceManager3` (`inheritance_manager3.dart`): o
    /// `getInterface(c).map`; com `so_super`, o `getInheritedConcreteMap2`
    /// (`superImplemented.last`); com `herdados`, o `getInheritedMap2` (os
    /// `overridden`, na ordem dos candidatos).
    pub(super) fn membros_da_interface(&self, c: ClassId, so_super: bool, herdados: bool) -> Vec<FunctionElementId> {
        let mut cache = HashMap::new();
        let mut processando = HashSet::new();
        let i = interface_de(self.consulta, self.consulta.core.object_class, self.consulta.programa.dono_da_classe(c), &mut cache, &mut processando);
        let mapa = if so_super {
            &i.super_implementado
        } else if herdados {
            &i.sobrescritos
        } else {
            &i.mapa
        };
        mapa.ordem.iter().map(|k| mapa.valores[k]).collect()
    }

    /// `_canAccessInstanceMember`: não estático, acessível (nome privado só
    /// na biblioteca dele), e as anotações `@internal`, `@protected` e
    /// `@visibleForTesting` do `package:meta`.
    pub(super) fn pode_acessar_membro(&self, f: FunctionElementId) -> bool {
        let p = &self.consulta.programa;
        let fe = p.function(f);
        if fe.static_ {
            return false;
        }
        let lib = self.biblioteca();
        if self.consulta.nome(fe.name).starts_with('_') && fe.library != lib {
            return false;
        }
        let anotacoes = self.anotacoes_do_membro(f);
        let mesmo_pacote = || {
            let a = p.library(fe.library).uri.split('/').next().map(str::to_string);
            let b = p.library(lib).uri.split('/').next().map(str::to_string);
            fe.library == lib || (a.is_some() && a == b && p.library(fe.library).uri.starts_with("package:")) || !p.library(fe.library).uri.starts_with("package:")
        };
        if anotacoes.iter().any(|a| a == "internal") && !mesmo_pacote() {
            return false;
        }
        if anotacoes.iter().any(|a| a == "protected")
            && let Some(dona) = fe.class
            && p.class(dona).library != lib
        {
            let Some(contexto) = self.classe_envolvente_do_alvo() else { return false };
            if distancia_de_heranca(self.consulta, contexto, dona).is_none() {
                return false;
            }
        }
        if anotacoes.iter().any(|a| a == "visibleForTesting") && fe.library != lib {
            if !mesmo_pacote() {
                return false;
            }
            let em_test = p.unit(self.unidade).path.as_ref().is_some_and(|c| c.components().any(|x| x.as_os_str() == "test"));
            if !em_test {
                return false;
            }
        }
        true
    }

    /// Os nomes das anotações de um membro (o último segmento).
    pub(super) fn anotacoes_do_membro(&self, f: FunctionElementId) -> Vec<String> {
        let p = &self.consulta.programa;
        let meta: &[ast::Annotation] = match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let a = &p.unit(unit).ast;
                match a.members.iter().find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x == function)) {
                    Some(m) => &m.metadata,
                    None => &[],
                }
            }
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => &p.unit(unit).ast.member(member).metadata,
            dartforge_elements::model::FunctionRef::None => match p.function(f).variable.map(|v| p.variable(v).node) {
                Some(VariableRef::Field { unit, member, .. }) => &p.unit(unit).ast.member(member).metadata,
                _ => &[],
            },
        };
        meta.iter().filter_map(|m| m.name.last().map(|n| self.consulta.nome(n.sym).to_string())).collect()
    }

    /// `request.target.enclosingInterfaceElement`.
    pub(super) fn classe_envolvente_do_alvo(&self) -> Option<ClassId> {
        let c = self.no_coberto()?;
        let d = self.ancestral(c, &["ClassDeclaration", "MixinDeclaration", "EnumDeclaration", "ExtensionTypeDeclaration"])?;
        self.classe_do_no(d)
    }

    /// `_addInstanceMembers`.
    pub(super) fn membros_de_instancia(&mut self, tipo: TypeId, excluidos: &HashSet<String>, metodos: bool, setters: bool, so_super: bool) {
        let classe = match self.consulta.tabela.get(tipo) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            Type::FutureOr { .. } | Type::Null => self.consulta.core.object_class,
            _ => None,
        };
        let Some(c) = classe else { return };
        let lib = self.biblioteca();
        let membros = self.membros_da_interface(c, so_super, false);
        // `membersByName` pelo `displayName`; `bestMember`: o getter antes
        // do setter.
        let mut por_nome: Vec<(String, Vec<FunctionElementId>)> = Vec::new();
        for f in membros {
            if !self.pode_acessar_membro(f) {
                continue;
            }
            let nome = sem_igual(self.consulta.nome(self.consulta.programa.function(f).name));
            match por_nome.iter_mut().find(|(n, _)| *n == nome) {
                Some((_, v)) => v.push(f),
                None => por_nome.push((nome, vec![f])),
            }
        }
        let referencia = Some(c);
        for (nome, lista) in por_nome {
            let p = &self.consulta.programa;
            let mut melhor = lista[0];
            if p.function(melhor).kind == FunctionKind::Setter
                && let Some(&g) = lista.iter().find(|&&g| matches!(p.function(g).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
            {
                melhor = g;
            }
            let fe = p.function(melhor);
            let simbolo = fe.name;
            let e_setter = fe.kind == FunctionKind::Setter || (fe.kind == FunctionKind::ImplicitAccessor && self.consulta.nome(simbolo).ends_with('='));
            match fe.kind {
                FunctionKind::Function => {
                    if metodos {
                        let t = self.consulta.resolvedor().lookup_member(tipo, simbolo, false, lib).map(|(_, t)| t);
                        self.sugerir_metodo(melhor, t, false, None, referencia);
                    }
                }
                FunctionKind::Operator => {
                    if metodos {
                        self.sugerir_metodo(melhor, None, false, None, referencia);
                    }
                }
                FunctionKind::Getter | FunctionKind::ImplicitAccessor | FunctionKind::Setter => {
                    if (!e_setter && !excluidos.contains(&nome)) || (setters && e_setter) {
                        let busca = if e_setter { self.consulta.nomes.lookup(&nome) } else { Some(simbolo) };
                        let t = busca.and_then(|s| self.consulta.resolvedor().lookup_member(tipo, s, e_setter, lib)).map(|(_, t)| t);
                        self.sugerir_propriedade(melhor, t, false, None, referencia);
                    }
                }
                _ => {}
            }
        }
        // Os campos (sem acessor no mapa de membros): pelo getter implícito.
        let funcao = self.consulta.core.function_class;
        let e_function = Some(c) == funcao;
        let estende_function = funcao.is_some_and(|fc| distancia_de_heranca(self.consulta, c, fc).is_some_and(|d| d > 0));
        if (e_function && !so_super) || estende_function {
            self.sugerir_call();
        }
        self.membros_de_extensoes(tipo, excluidos, metodos, setters);
        self.registrar_operacao(Operacao::MembrosDeExtensao {
            tipo,
            excluidos: excluidos.iter().cloned().collect(),
            metodos,
            setters,
        });
    }

    /// `_suggestFunctionCall`.
    pub(super) fn sugerir_call(&mut self) {
        if let Some(s) = self.pontuar("call") {
            let antes = self.coletor.itens.len();
            let item = self.coletor.empurrar(grupo::MEMBRO, especie::METODO, "call".into(), "call".into(), Some("() → void".into()));
            item.exibicao = Some("call()".into());
            item.chamada = Some(Chamada::Parametros(Vec::new()));
            item.rel.fixa = Some(200);
            self.marcar(antes, s, false);
        }
    }

    /// `_addExtensionMembers`.
    pub(super) fn membros_de_extensoes(&mut self, tipo: TypeId, _excluidos: &HashSet<String>, metodos: bool, setters: bool) {
        let alvo = if matches!(self.consulta.tabela.get(tipo), Type::Null) {
            tipo
        } else {
            dartforge_types::non_nullable(tipo, &mut self.consulta.tabela)
        };
        let lib = self.biblioteca();
        let aplicaveis = {
            let Consulta { programa, nomes, tabela, core, outline, .. } = &mut *self.consulta;
            dartforge_types::extensoes_aplicaveis(programa, nomes, tabela, core, outline, lib, alvo)
        };
        for (x, _) in aplicaveis {
            let (acessores, campos, ms) = self.membros_declarados(None, Some(x));
            if metodos {
                for m in ms {
                    if !self.consulta.programa.function(m).static_ {
                        self.sugerir_metodo(m, None, false, None, None);
                    }
                }
            }
            for f in acessores {
                let fe = self.consulta.programa.function(f);
                if fe.static_ {
                    continue;
                }
                let e_setter = fe.kind == FunctionKind::Setter;
                if !e_setter || setters {
                    self.sugerir_propriedade(f, None, false, None, None);
                }
            }
            for v in campos {
                if !self.consulta.programa.variable(v).static_ {
                    self.sugerir_campo(v, None, None);
                }
            }
        }
    }

    /// `_addFieldsOfRecordType`.
    pub(super) fn campos_de_record(&mut self, tipo: TypeId, excluidos: &HashSet<String>) {
        let Type::Record { positional, named, .. } = self.consulta.tabela.get(tipo).clone() else { return };
        for (i, t) in positional.iter().enumerate() {
            self.item_campo_de_record(&format!("${}", i + 1), *t);
        }
        for (n, t) in named.iter() {
            let nome = self.consulta.nome(*n).to_string();
            if !excluidos.contains(&nome) {
                self.item_campo_de_record(&nome, *t);
            }
        }
    }

    /// `_suggestRecordField`.
    pub(super) fn item_campo_de_record(&mut self, nome: &str, tipo: TypeId) {
        if let Some(s) = self.pontuar(nome) {
            let antes = self.coletor.itens.len();
            let detalhe = self.consulta.formatar(tipo);
            let item = self.coletor.empurrar(grupo::MEMBRO, especie::VARIAVEL, nome.to_string(), nome.to_string(), Some(detalhe));
            item.rel.tipo = Some(tipo);
            self.marcar(antes, s, false);
        }
    }

    /// `_addMembersOfDartCoreObject`.
    pub(super) fn membros_de_object(&mut self) {
        let o = self.consulta.core.object;
        self.membros_de_instancia(o, &HashSet::new(), true, true, false);
    }

    /// `addInstanceMembersOfType`.
    pub(super) fn dh_membros_de_instancia(&mut self, tipo: TypeId, so_super: bool) {
        let mut t = tipo;
        for _ in 0..16 {
            match self.consulta.tabela.get(t).clone() {
                Type::TypeParameter { param, .. } => t = self.consulta.tabela.param(param).bound,
                Type::Intersection { bound, .. } => t = bound,
                _ => break,
            }
        }
        let atribuivel = self.fl().atribuivel;
        match self.consulta.tabela.get(t).clone() {
            Type::Interface { .. } | Type::ExtensionType { .. } | Type::FutureOr { .. } | Type::Null => {
                self.membros_de_instancia(t, &HashSet::new(), !atribuivel, true, so_super);
            }
            Type::Record { .. } => {
                self.campos_de_record(t, &HashSet::new());
                self.membros_de_object();
                self.membros_de_extensoes(t, &HashSet::new(), !atribuivel, true);
                self.registrar_operacao(Operacao::MembrosDeExtensao { tipo: t, excluidos: Vec::new(), metodos: !atribuivel, setters: true });
            }
            Type::Function { .. } => {
                self.sugerir_call();
                self.membros_de_object();
            }
            Type::Dynamic => self.membros_de_object(),
            _ => {}
        }
    }

    /// `addGetters`.
    pub(super) fn dh_getters(&mut self, tipo: TypeId, excluidos: &HashSet<String>) {
        match self.consulta.tabela.get(tipo).clone() {
            Type::Interface { .. } | Type::ExtensionType { .. } => self.membros_de_instancia(tipo, excluidos, false, false, false),
            Type::Record { .. } => {
                self.campos_de_record(tipo, excluidos);
                self.membros_de_object();
            }
            _ => {}
        }
    }

    /// `_addInheritedMembers`.
    pub(super) fn membros_herdados(&mut self, d: usize) {
        match self.especie(d) {
            "ExtensionDeclaration" => {
                let nome = self.decl_real(d).and_then(|x| match &x.kind {
                    ast::DeclKind::Extension(e) => e.name.map(|n| n.span.start),
                    _ => None,
                });
                let Some(x) = self.extensao_em(self.ini(d), nome) else { return };
                if self.fl().estatico {
                    return;
                }
                let on = self.consulta.outline.extensions[x.0 as usize].on;
                if matches!(self.consulta.tabela.get(on), Type::Interface { .. } | Type::ExtensionType { .. }) {
                    self.membros_de_instancia(on, &HashSet::new(), true, true, false);
                }
            }
            "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" | "ClassTypeAlias" => {
                let Some(c) = self.classe_do_no(d) else { return };
                let this = self.tipo_this(c);
                let lib = self.biblioteca();
                for f in self.membros_da_interface(c, false, true) {
                    let fe = self.consulta.programa.function(f);
                    let simbolo = fe.name;
                    match fe.kind {
                        FunctionKind::Function => {
                            let t = self.consulta.resolvedor().lookup_member(this, simbolo, false, lib).map(|(_, t)| t);
                            self.sugerir_metodo(f, t, false, None, Some(c));
                        }
                        FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                            let e_setter = self.consulta.nome(simbolo).ends_with('=');
                            let busca = if e_setter { self.consulta.nomes.lookup(self.consulta.nome(simbolo).trim_end_matches('=')) } else { Some(simbolo) };
                            let t = busca.and_then(|s| self.consulta.resolvedor().lookup_member(this, s, e_setter, lib)).map(|(_, t)| t);
                            self.sugerir_propriedade(f, t, false, None, Some(c));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // -- As declarações de topo ------------------------------------------------------

    /// Os elementos de cada categoria de uma unidade, na ordem de
    /// declaração.
    pub(super) fn elementos_da_unidade(&self, u: UnitId) -> DaUnidade {
        use dartforge_elements::model::ClassKind as K;
        let p = &self.consulta.programa;
        let ast_u = &p.unit(u).ast;
        let pos_decl = |d: ast::DeclId| ast_u.decl(d).span.start;
        let mut r = DaUnidade::default();
        let mut classes: Vec<(usize, ClassId)> = Vec::new();
        for (i, c) in p.classes.iter().enumerate() {
            if let Some(d) = c.decl
                && d.unit == u
            {
                classes.push((pos_decl(d.decl), ClassId(i as u32)));
            }
        }
        classes.sort();
        for (_, c) in classes {
            match p.class(c).kind {
                K::Class | K::MixinApplication => r.classes.push(c),
                K::Enum => r.enums.push(c),
                K::ExtensionType => r.tipos_de_extensao.push(c),
                K::Mixin => r.mixins.push(c),
            }
        }
        let mut tds: Vec<(usize, TypedefId)> =
            p.typedefs.iter().enumerate().filter(|(_, t)| t.decl.unit == u).map(|(i, t)| (pos_decl(t.decl.decl), TypedefId(i as u32))).collect();
        tds.sort();
        r.typedefs = tds.into_iter().map(|(_, t)| t).collect();
        let mut xs: Vec<(usize, ExtensionId)> =
            p.extensions.iter().enumerate().filter(|(_, x)| x.decl.unit == u).map(|(i, x)| (pos_decl(x.decl.decl), ExtensionId(i as u32))).collect();
        xs.sort();
        r.extensoes = xs.into_iter().map(|(_, x)| x).collect();
        for (i, f) in p.functions.iter().enumerate() {
            if f.class.is_some() || f.extension.is_some() {
                continue;
            }
            let dartforge_elements::model::FunctionRef::Function { unit, .. } = f.node else { continue };
            if unit != u {
                continue;
            }
            let id = FunctionElementId(i as u32);
            match f.kind {
                FunctionKind::Getter | FunctionKind::Setter => r.acessores.push(id),
                FunctionKind::Function => r.funcoes.push(id),
                _ => {}
            }
        }
        for (i, v) in p.variables.iter().enumerate() {
            if let VariableRef::TopLevel { unit, .. } = v.node
                && unit == u
            {
                r.variaveis.push(VariableId(i as u32));
            }
        }
        r
    }

    /// `_addTopLevelDeclarations` da biblioteca do pedido.
    pub(super) fn declaracoes_de_topo(&mut self) {
        let unidades = self.consulta.programa.library(self.biblioteca()).units.clone();
        let tipo = self.fl().tipo;
        for u in unidades {
            let e = self.elementos_da_unidade(u);
            for c in e.classes {
                self.sugerir_classe(c, None);
            }
            for c in e.enums {
                self.sugerir_enum(c, None);
            }
            for c in e.tipos_de_extensao {
                self.sugerir_tipo_de_extensao(c, None);
            }
            for c in e.mixins {
                self.sugerir_mixin(c, None);
            }
            for t in e.typedefs {
                self.sugerir_typedef(t, None);
            }
            if !tipo {
                for f in e.acessores {
                    let getter = self.consulta.programa.function(f).kind == FunctionKind::Getter;
                    if getter || self.getter_de_topo(f).is_none() {
                        self.sugerir_propriedade_de_topo(Element::Function(f), None);
                    }
                }
                for x in e.extensoes {
                    if self.consulta.programa.extension(x).name.is_some() {
                        self.sugerir_extensao(x, None);
                    }
                }
                for f in e.funcoes {
                    self.sugerir_funcao_de_topo(f, None);
                }
                for v in e.variaveis {
                    self.sugerir_variavel_de_topo(v, None);
                }
            }
        }
    }

    /// `correspondingGetter` de um setter de topo.
    pub(super) fn getter_de_topo(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let p = &self.consulta.programa;
        let fe = p.function(f);
        let s = self.consulta.nomes.lookup(self.consulta.nome(fe.name).trim_end_matches('='))?;
        match p.library(fe.library).declared.get(&s)?.getter {
            Some(Element::Function(g)) => Some(g),
            _ => None,
        }
    }

    /// Os imports da unidade definidora (`definingCompilationUnit.libraryImports`)
    /// na ordem, com o `dart:core` implícito por último quando nenhum o
    /// importa.
    pub(super) fn imports_da_definidora(&self) -> Vec<dartforge_elements::model::Import> {
        let p = &self.consulta.programa;
        let lib = p.library(self.biblioteca());
        let definidora = lib.units.first().copied();
        let mut v: Vec<dartforge_elements::model::Import> = lib.imports.iter().filter(|i| Some(i.unit) == definidora).cloned().collect();
        v.sort_by_key(|i| i.directive);
        if let Some(core) = p.core
            && !v.iter().any(|i| i.library == core)
            && self.biblioteca() != core
            && let Some(u) = definidora
        {
            v.push(dartforge_elements::model::Import { unit: u, directive: usize::MAX, library: core, prefix: None, deferred: false, combinators: Vec::new() });
        }
        v
    }

    /// O espaço de nomes de um import (`importElement.namespace`): o
    /// exportado da biblioteca com os combinadores, por nome.
    pub(super) fn espaco_do_import(&self, i: &dartforge_elements::model::Import) -> Vec<(SymbolId, dartforge_elements::model::Binding)> {
        let p = &self.consulta.programa;
        let mut v: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(i.library).exported.iter().map(|(&s, &b)| (s, b)).collect();
        for c in &i.combinators {
            match c {
                ast::Combinator::Show(ns) => {
                    let mostrados: HashSet<SymbolId> = ns.iter().map(|n| n.sym).collect();
                    v.retain(|(s, _)| mostrados.contains(s));
                }
                ast::Combinator::Hide(ns) => {
                    let escondidos: HashSet<SymbolId> = ns.iter().map(|n| n.sym).collect();
                    v.retain(|(s, _)| !escondidos.contains(s));
                }
            }
        }
        // A ordem do `Namespace.definedNames`: a do `exportNamespace` da
        // biblioteca (as declarações dela na ordem da fonte, unidade por
        // unidade, depois as reexportadas, export por export).
        let ordem = ordem_de_exportacao(&self.consulta.programa, i.library, &mut HashSet::new());
        let posicao: HashMap<SymbolId, usize> = ordem.iter().enumerate().map(|(k, &s)| (s, k)).collect();
        v.sort_by_key(|(s, _)| (posicao.get(s).copied().unwrap_or(usize::MAX), self.consulta.nome(*s).to_string()));
        v
    }

    /// `_addImportedDeclarations`.
    pub(super) fn declaracoes_importadas(&mut self) {
        let tipo = self.fl().tipo;
        let core = self.consulta.programa.core;
        for i in self.imports_da_definidora() {
            let prefixo = i.prefix.map(|s| self.consulta.nome(s).to_string());
            if prefixo.as_deref() == Some("_") {
                continue;
            }
            let imp = Importe { prefixo, nao_importado: false };
            let espaco = self.espaco_do_import(&i);
            self.declaracoes_externas(&espaco, &imp);
            if Some(i.library) == core && tipo {
                let nome = "Never";
                if let Some(s) = self.pontuar(nome) {
                    let antes = self.coletor.itens.len();
                    let item = self.coletor.empurrar(grupo::PALAVRA, especie::VARIAVEL, nome.into(), nome.into(), None);
                    item.rel.fixa = Some(500);
                    self.marcar(antes, s, false);
                }
            }
        }
    }

    /// `_addExternalTopLevelDeclarations`.
    pub(super) fn declaracoes_externas(&mut self, espaco: &[(SymbolId, dartforge_elements::model::Binding)], imp: &Importe) {
        use dartforge_elements::model::ClassKind as K;
        let tipo = self.fl().tipo;
        for (_, b) in espaco {
            for el in [b.getter, b.setter].into_iter().flatten() {
                match el {
                    Element::Class(c) => match self.consulta.programa.class(c).kind {
                        K::Class | K::MixinApplication => self.sugerir_classe(c, Some(imp)),
                        K::Enum => self.sugerir_enum(c, Some(imp)),
                        K::ExtensionType => self.sugerir_tipo_de_extensao(c, Some(imp)),
                        K::Mixin => self.sugerir_mixin(c, Some(imp)),
                    },
                    Element::Extension(x) => {
                        if !tipo {
                            self.sugerir_extensao(x, Some(imp));
                        }
                    }
                    Element::Typedef(t) => self.sugerir_typedef(t, Some(imp)),
                    Element::Function(f) => {
                        if !tipo {
                            match self.consulta.programa.function(f).kind {
                                FunctionKind::Function => self.sugerir_funcao_de_topo(f, Some(imp)),
                                _ => self.sugerir_propriedade_de_topo(el, Some(imp)),
                            }
                        }
                    }
                    Element::Variable(_) => {
                        // O getter sintético (o setter sintético não entra).
                        if !tipo && b.getter == Some(el) {
                            self.sugerir_propriedade_de_topo(el, Some(imp));
                        }
                    }
                    Element::Prefix(..) => {}
                }
            }
        }
    }

    /// `addImportPrefixes`.
    pub(super) fn dh_prefixos_de_import(&mut self) {
        for i in self.imports_da_definidora() {
            let Some(p) = i.prefix else { continue };
            let nome = self.consulta.nome(p).to_string();
            if !self.visivel(&nome, None) || nome.is_empty() || self.curinga(&nome) {
                continue;
            }
            if let Some(s) = self.pontuar(&nome) {
                let antes = self.coletor.itens.len();
                // O elemento é a biblioteca: `ElementKind.LIBRARY` → `File`/`Module`.
                self.coletor.empurrar(grupo::PREFIXO, 17, nome.clone(), nome, None).rel.especie = Some(crate::relevancia::Especie::Prefixo);
                self.marcar(antes, s, false);
            }
        }
    }

    /// `addDeclarationsThroughImportPrefix`.
    pub(super) fn dh_pelo_prefixo(&mut self, prefixo: SymbolId) {
        let p = &self.consulta.programa;
        let lib = p.library(self.biblioteca());
        let unidade = self.unidade;
        // Os imports com o prefixo na unidade do pedido (o fragmento que o
        // declara).
        let mut imports: Vec<dartforge_elements::model::Import> =
            lib.imports.iter().filter(|i| i.prefix == Some(prefixo) && (i.unit == unidade || lib.units.first() == Some(&i.unit))).cloned().collect();
        imports.sort_by_key(|i| (i.unit != unidade, i.directive));
        for i in imports {
            let espaco = self.espaco_do_import(&i);
            let imp = Importe { prefixo: None, nao_importado: false };
            self.declaracoes_externas(&espaco, &imp);
            if i.deferred
                && let Some(s) = self.pontuar("loadLibrary")
            {
                let antes = self.coletor.itens.len();
                let item = self.coletor.empurrar(grupo::MEMBRO, especie::FUNCAO, "loadLibrary".into(), "loadLibrary".into(), Some("() → Future<dynamic>".into()));
                item.chamada = Some(Chamada::Parametros(Vec::new()));
                item.rel.fixa = Some(200);
                self.marcar(antes, s, false);
            }
        }
    }

    /// `addFromLibrary` (nomes de `show`/`hide`): sem rastreador nem
    /// opções.
    pub(super) fn dh_da_biblioteca(&mut self, lib: LibraryId, excluidos: &HashSet<String>) {
        let p = &self.consulta.programa;
        let mut v: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(lib).exported.iter().map(|(&s, &b)| (s, b)).collect();
        v.sort_by(|a, b| self.consulta.nome(a.0).cmp(self.consulta.nome(b.0)));
        for (s, b) in v {
            let nome = self.consulta.nome(s).to_string();
            for (el, setter) in [(b.getter, false), (b.setter, true)] {
                let Some(el) = el else { continue };
                let chave = if setter { format!("{nome}=") } else { nome.clone() };
                if excluidos.contains(&chave) {
                    continue;
                }
                // `_addImportedElement`: o setter sintético não vira sugestão.
                if setter && matches!(el, Element::Variable(_)) {
                    continue;
                }
                let exibicao = match el {
                    Element::Function(f) => sem_igual(self.consulta.nome(self.consulta.programa.function(f).name)),
                    _ => nome.clone(),
                };
                if let Some(sc) = self.pontuar(&exibicao) {
                    let ident = matches!(el, Element::Function(f) if self.consulta.programa.function(f).kind == FunctionKind::Function) && self.identificador_executavel();
                    self.item_elemento(el, None, sc, ident);
                }
            }
        }
    }

    // -- Os elementos de topo ---------------------------------------------------------

    /// `_suggestClass`.
    pub(super) fn sugerir_classe(&mut self, c: ClassId, imp: Option<&Importe>) {
        let p = &self.consulta.programa;
        let cl = p.class(c);
        if cl.kind == dartforge_elements::model::ClassKind::MixinApplication && cl.decl.is_none() {
            return;
        }
        let nome = self.consulta.nome(cl.name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        let lib = self.biblioteca();
        let cl = self.consulta.programa.class(c);
        let m = cl.modifiers;
        let mesma = cl.library == lib;
        let estensivel = mesma || (!m.interface && !m.final_ && !m.sealed);
        let implementavel = mesma || (!m.base && !m.final_ && !m.sealed);
        let misturavel = mesma || !self.recurso_da_biblioteca(cl.library, Recurso::ClassModifiers) || (m.mixin && !m.interface && !m.final_ && !m.sealed);
        if (f.estensivel && !estensivel) || (f.implementavel && !implementavel) || (f.misturavel && !misturavel) {
            return;
        }
        if !(f.constante && !f.padrao_objeto)
            && !f.excluir_nomes_de_tipo
            && let Some(s) = self.pontuar(&nome)
        {
            self.item_elemento(Element::Class(c), imp, s, false);
        }
        if !f.tipo {
            self.sugerir_campos_estaticos(c, imp);
            let abstrato = m.abstract_;
            self.sugerir_construtores(c, imp, !abstrato);
        }
    }

    /// `_suggestEnum`.
    pub(super) fn sugerir_enum(&mut self, c: ClassId, imp: Option<&Importe>) {
        let nome = self.consulta.nome(self.consulta.programa.class(c).name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        if f.estensivel || f.implementavel || f.misturavel {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Class(c), imp, s, false);
        }
        if !f.tipo {
            self.sugerir_campos_estaticos(c, imp);
            self.sugerir_construtores(c, imp, false);
        }
    }

    /// `_suggestExtension`.
    pub(super) fn sugerir_extensao(&mut self, x: ExtensionId, imp: Option<&Importe>) {
        let Some(n) = self.consulta.programa.extension(x).name else { return };
        let nome = self.consulta.nome(n).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        if f.estensivel || f.implementavel || f.misturavel {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Extension(x), imp, s, false);
        }
        if !f.tipo {
            let campos = self.consulta.programa.extension(x).fields.clone();
            for v in campos {
                if self.visivel_em(self.consulta.programa.variable(v).library, self.consulta.nome(self.consulta.programa.variable(v).name)) {
                    self.sugerir_campo_estatico(v, imp, Some(nome.clone()));
                }
            }
        }
    }

    /// `_suggestExtensionType`.
    pub(super) fn sugerir_tipo_de_extensao(&mut self, c: ClassId, imp: Option<&Importe>) {
        let nome = self.consulta.nome(self.consulta.programa.class(c).name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        if f.estensivel || f.implementavel || f.misturavel {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Class(c), imp, s, false);
        }
        if !f.tipo {
            self.sugerir_campos_estaticos(c, imp);
            self.sugerir_construtores(c, imp, true);
        }
    }

    /// `_suggestMixin`.
    pub(super) fn sugerir_mixin(&mut self, c: ClassId, imp: Option<&Importe>) {
        let nome = self.consulta.nome(self.consulta.programa.class(c).name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let f = self.fl();
        let cl = self.consulta.programa.class(c);
        let implementavel = cl.library == self.biblioteca() || !cl.modifiers.base;
        if f.estensivel || (f.implementavel && !implementavel) {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Class(c), imp, s, false);
        }
        if !f.tipo {
            self.sugerir_campos_estaticos(c, imp);
        }
    }

    /// `_suggestTypeAlias`.
    pub(super) fn sugerir_typedef(&mut self, t: TypedefId, imp: Option<&Importe>) {
        let nome = self.consulta.nome(self.consulta.programa.typedef(t).name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Typedef(t), imp, s, false);
        }
        if !self.fl().tipo {
            self.construtores_do_alias(t, imp);
        }
    }

    /// `_addConstructorsForAliasedElement`.
    pub(super) fn construtores_do_alias(&mut self, t: TypedefId, imp: Option<&Importe>) {
        use dartforge_elements::model::ClassKind as K;
        let alvo = self.consulta.outline.typedefs[t.0 as usize].target_type;
        let classe = match self.consulta.tabela.get(alvo) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            _ => None,
        };
        let Some(c) = classe else { return };
        let cl = self.consulta.programa.class(c);
        match cl.kind {
            K::Class | K::MixinApplication => {
                let abstrato = cl.modifiers.abstract_;
                self.sugerir_construtores(c, imp, !abstrato);
            }
            K::ExtensionType | K::Mixin => self.sugerir_construtores(c, imp, true),
            K::Enum => {}
        }
    }

    /// `_suggestTopLevelFunction`.
    pub(super) fn sugerir_funcao_de_topo(&mut self, f: FunctionElementId, imp: Option<&Importe>) {
        let nome = self.consulta.nome(self.consulta.programa.function(f).name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let fl = self.fl();
        if fl.atribuivel || fl.constante || (fl.nao_void && self.e_void(self.retorno_de(f))) || fl.tipo {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            let ident = fl.preferir_sem_invocacao;
            self.item_elemento(Element::Function(f), imp, s, ident);
        }
    }

    /// `_suggestTopLevelProperty`: um getter/setter declarado
    /// (`Element::Function`) ou o getter sintético de uma variável
    /// (`Element::Variable`).
    pub(super) fn sugerir_propriedade_de_topo(&mut self, el: Element, imp: Option<&Importe>) {
        let p = &self.consulta.programa;
        let (nome, getter, sem_setter, constante, ret) = match el {
            Element::Variable(v) => {
                let ve = p.variable(v);
                let t = self.consulta.tipo_da_variavel(v).unwrap_or(self.consulta.core.dynamic_);
                (self.consulta.nome(ve.name).to_string(), true, ve.setter.is_none(), ve.const_, t)
            }
            Element::Function(f) => {
                let fe = p.function(f);
                let getter = fe.kind == FunctionKind::Getter;
                let ret = if getter { self.retorno_de(f) } else { self.consulta.core.void_ };
                let sem_setter = getter && self.setter_correspondente(f).is_none();
                (sem_igual(self.consulta.nome(fe.name)), getter, sem_setter, false, ret)
            }
            _ => return,
        };
        if !self.visivel(&nome, imp) {
            return;
        }
        let fl = self.fl();
        if (fl.atribuivel && getter && sem_setter) || (fl.constante && !constante) || (fl.nao_void && self.e_void(ret)) || fl.tipo {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(el, imp, s, false);
        }
    }

    /// `_suggestTopLevelVariable`.
    pub(super) fn sugerir_variavel_de_topo(&mut self, v: VariableId, imp: Option<&Importe>) {
        let ve = self.consulta.programa.variable(v);
        let nome = self.consulta.nome(ve.name).to_string();
        if !self.visivel(&nome, imp) {
            return;
        }
        let fl = self.fl();
        let ve = self.consulta.programa.variable(v);
        if (fl.atribuivel && ve.setter.is_none()) || (fl.constante && !ve.const_) || fl.tipo {
            return;
        }
        if let Some(s) = self.pontuar(&nome) {
            self.item_elemento(Element::Variable(v), imp, s, false);
        }
    }

    // -- Campos estáticos e construtores ------------------------------------------------

    /// `_suggestStaticFields` de uma classe: as constantes de enum, o
    /// `values`, os campos declarados e os getters estáticos (campos
    /// sintéticos), na ordem de `fields`.
    pub(super) fn sugerir_campos_estaticos(&mut self, c: ClassId, imp: Option<&Importe>) {
        let p = &self.consulta.programa;
        let cl = p.class(c);
        let nome_classe = self.consulta.nome(cl.name).to_string();
        let mut campos: Vec<VariableId> = cl.enum_constants.clone();
        campos.extend(cl.fields.iter().copied().filter(|v| !cl.enum_constants.contains(v)));
        let mut getters: Vec<FunctionElementId> = cl.static_members.values().copied().filter(|&f| p.function(f).kind == FunctionKind::Getter).collect();
        getters.sort();
        let values = if cl.kind == dartforge_elements::model::ClassKind::Enum {
            self.consulta.nomes.lookup("values").and_then(|s| cl.static_members.get(&s).copied())
        } else {
            None
        };
        for v in campos {
            let ve = self.consulta.programa.variable(v);
            if self.visivel_em(ve.library, self.consulta.nome(ve.name)) {
                self.sugerir_campo_estatico(v, imp, Some(nome_classe.clone()));
            }
        }
        if let Some(f) = values {
            self.sugerir_getter_estatico(f, imp, &nome_classe, true);
        }
        for g in getters {
            if Some(g) == values {
                continue;
            }
            let fe = self.consulta.programa.function(g);
            if self.visivel_em(fe.library, self.consulta.nome(fe.name)) {
                self.sugerir_getter_estatico(g, imp, &nome_classe, false);
            }
        }
    }

    /// `_suggestStaticField` de um campo declarado.
    pub(super) fn sugerir_campo_estatico(&mut self, v: VariableId, imp: Option<&Importe>, dono: Option<String>) {
        let ve = self.consulta.programa.variable(v).clone();
        let fl = self.fl();
        if !ve.static_ || (fl.atribuivel && !(ve.final_ || ve.const_)) || (fl.constante && !ve.const_) {
            return;
        }
        let Some(contexto) = self.tipo_de_contexto else { return };
        let tipo = self.consulta.tipo_da_variavel(v).unwrap_or(self.consulta.core.dynamic_);
        if !self.subtipo(tipo, contexto) {
            return;
        }
        let nome = self.consulta.nome(ve.name).to_string();
        let dono = dono.unwrap_or_default();
        let constante_de_enum = ve.class.is_some_and(|c| self.consulta.programa.class(c).enum_constants.contains(&v));
        let completion = format!("{dono}.{nome}");
        let alvo = if constante_de_enum { completion.clone() } else { nome.clone() };
        let Some(s) = self.pontuar(&alvo) else { return };
        let antes = self.coletor.itens.len();
        let detalhe = self.consulta.formatar(tipo);
        let origem = self.consulta.origem(self.consulta.inicio_da_variavel(v));
        let especie_lsp = if constante_de_enum { especie::MEMBRO_DE_ENUM } else { especie::CAMPO };
        let item = self.coletor.empurrar(grupo::MEMBRO, especie_lsp, completion.clone(), completion, Some(detalhe));
        item.origem = origem;
        item.rel.especie = Some(if constante_de_enum { crate::relevancia::Especie::SemTabela } else { crate::relevancia::Especie::Campo });
        item.rel.tipo = Some(tipo);
        self.prefixar(antes, imp);
        self.marcar(antes, s, false);
    }

    /// `_suggestStaticField` de um campo sintético (getter estático; o
    /// `values` de um enum): o getter sintético vira `FieldSuggestion` com o
    /// nome só; o declarado, `PropertyAccessSuggestion` com o nome do dono.
    pub(super) fn sugerir_getter_estatico(&mut self, g: FunctionElementId, imp: Option<&Importe>, dono: &str, sintetico: bool) {
        let fl = self.fl();
        if fl.constante || fl.atribuivel {
            return;
        }
        let Some(contexto) = self.tipo_de_contexto else { return };
        let tipo = self.retorno_de(g);
        if !self.subtipo(tipo, contexto) {
            return;
        }
        let nome = sem_igual(self.consulta.nome(self.consulta.programa.function(g).name));
        let Some(s) = self.pontuar(&nome) else { return };
        let antes = self.coletor.itens.len();
        self.item_membro(g, tipo, None, s, false);
        if !sintetico {
            for i in &mut self.coletor.itens[antes..] {
                i.inserir = format!("{dono}.{}", i.inserir);
                i.rotulo = i.inserir.clone();
            }
            self.prefixar(antes, imp);
        }
    }

    /// `isSubtypeOf`.
    pub(super) fn subtipo(&mut self, a: TypeId, b: TypeId) -> bool {
        let Consulta { tabela, outline, core, .. } = &mut *self.consulta;
        let mut env = dartforge_types::SubtypeEnv::new(tabela, &outline.hierarchy, core);
        dartforge_types::is_subtype(a, b, &mut env)
    }

    /// `_suggestConstructors`.
    pub(super) fn sugerir_construtores(&mut self, c: ClassId, imp: Option<&Importe>, nao_fabricas: bool) {
        if self.fl().atribuivel {
            return;
        }
        let lista = self.consulta.programa.class(c).construtores();
        for (n, f) in lista {
            let fe = self.consulta.programa.function(f);
            if self.visivel_em(fe.library, self.consulta.nome(n)) && (nao_fabricas || fe.factory) {
                self.sugerir_construtor(f, false, imp, false);
            }
        }
    }

    /// `_suggestConstructor`.
    pub(super) fn sugerir_construtor(&mut self, f: FunctionElementId, tem_nome_da_classe: bool, imp: Option<&Importe>, redirecionamento: bool) {
        let fl = self.fl();
        if fl.atribuivel {
            return;
        }
        let fe = self.consulta.programa.function(f).clone();
        let nome_ctor = self.consulta.nome(fe.name).to_string();
        if !self.visivel_em(fe.library, &nome_ctor) {
            return;
        }
        let Some(c) = fe.class else { return };
        let classe = self.consulta.nome(self.consulta.programa.class(self.consulta.programa.dono_da_classe(c)).name).to_string();
        // O rastreador: a classe fica conhecida (ou, não importada, tem de
        // estar visível).
        if imp.is_some_and(|i| i.nao_importado) {
            if !self.visivel(&classe, imp) {
                return;
            }
        } else {
            self.visivel(&classe, imp);
        }
        let exibicao = if nome_ctor.is_empty() { classe.clone() } else { format!("{classe}.{nome_ctor}") };
        let Some(s) = self.pontuar(&exibicao) else { return };
        let tear_off = fl.preferir_sem_invocacao || (fl.constante && !fe.const_);
        let como_new = fl.sem_nome_como_new || tear_off;
        let mut completion = nome_ctor.clone();
        if completion.is_empty() && como_new {
            completion = "new".into();
        }
        if !tem_nome_da_classe {
            completion = if completion.is_empty() { classe } else { format!("{classe}.{completion}") };
        }
        if completion.is_empty() {
            return;
        }
        self.item_construtor(f, completion, !(tear_off || redirecionamento), imp, s);
    }

    /// `addConstructorInvocations`.
    pub(super) fn dh_invocacoes_de_construtor(&mut self) {
        use dartforge_elements::model::ClassKind as K;
        let unidades = self.consulta.programa.library(self.biblioteca()).units.clone();
        let proprio = Importe { prefixo: None, nao_importado: false };
        for u in unidades {
            let e = self.elementos_da_unidade(u);
            for c in e.classes {
                let abstrato = self.consulta.programa.class(c).modifiers.abstract_;
                self.sugerir_construtores(c, Some(&proprio), !abstrato);
            }
            for c in e.enums {
                self.sugerir_construtores(c, Some(&proprio), true);
            }
            for c in e.tipos_de_extensao {
                self.sugerir_construtores(c, Some(&proprio), true);
            }
            for t in e.typedefs {
                self.construtores_do_alias(t, Some(&proprio));
            }
        }
        for i in self.imports_da_definidora() {
            let imp = Importe { prefixo: i.prefix.map(|s| self.consulta.nome(s).to_string()), nao_importado: false };
            for (_, b) in self.espaco_do_import(&i) {
                match b.getter {
                    Some(Element::Class(c)) => match self.consulta.programa.class(c).kind {
                        K::Class | K::MixinApplication => {
                            let abstrato = self.consulta.programa.class(c).modifiers.abstract_;
                            self.sugerir_construtores(c, Some(&imp), !abstrato);
                        }
                        K::ExtensionType => self.sugerir_construtores(c, Some(&imp), true),
                        _ => {}
                    },
                    Some(Element::Typedef(t)) => self.construtores_do_alias(t, Some(&imp)),
                    _ => {}
                }
            }
        }
        self.registrar_operacao(Operacao::Construtores);
    }

    /// `addConstructorNamesForElement`.
    pub(super) fn dh_nomes_de_construtor_do_elemento(&mut self, c: ClassId) {
        let lista = self.consulta.programa.class(c).construtores();
        for (_, f) in lista {
            self.sugerir_construtor(f, true, None, false);
        }
    }

    /// `addConstructorNamesForType`.
    pub(super) fn dh_nomes_de_construtor_do_tipo(&mut self, tipo: TypeId, excluir: Option<String>) {
        let c = match self.consulta.tabela.get(tipo) {
            Type::Interface { class, .. } => *class,
            Type::ExtensionType { decl, .. } => *decl,
            _ => return,
        };
        let constante = self.fl().constante;
        let lista = self.consulta.programa.class(c).construtores();
        for (n, f) in lista {
            let nome = self.consulta.nome(n).to_string();
            if !nome.is_empty() && Some(&nome) != excluir.as_ref() && !(constante && !self.consulta.programa.function(f).const_) {
                self.sugerir_construtor(f, true, None, false);
            }
        }
    }

    /// `addStaticMembersOfElement`.
    pub(super) fn dh_estaticos(&mut self, el: Element) {
        use dartforge_elements::model::ClassKind as K;
        let el = match el {
            Element::Typedef(t) => match self.consulta.tabela.get(self.consulta.outline.typedefs[t.0 as usize].target_type) {
                Type::Interface { class, .. } => Element::Class(*class),
                Type::ExtensionType { decl, .. } => Element::Class(*decl),
                _ => el,
            },
            _ => el,
        };
        match el {
            Element::Class(c) => {
                let cl = self.consulta.programa.class(c);
                let classe = cl.kind == K::Class || cl.kind == K::MixinApplication;
                let nao_fabricas = classe && !cl.modifiers.abstract_;
                let tem_construtores = cl.kind != K::Mixin;
                self.membros_estaticos(Some(c), None, tem_construtores, nao_fabricas);
            }
            Element::Extension(x) => self.membros_estaticos(None, Some(x), false, false),
            _ => {}
        }
    }

    /// `_addStaticMembers`.
    pub(super) fn membros_estaticos(&mut self, c: Option<ClassId>, x: Option<ExtensionId>, construtores: bool, nao_fabricas: bool) {
        let (acessores, campos, metodos) = self.membros_declarados(c, x);
        let lib = self.biblioteca();
        let p = &self.consulta.programa;
        let e_enum = c.is_some_and(|c| p.class(c).kind == dartforge_elements::model::ClassKind::Enum);
        for f in acessores {
            let fe = self.consulta.programa.function(f);
            if fe.static_ && self.visivel_em(fe.library, self.consulta.nome(fe.name)) {
                self.sugerir_propriedade(f, None, false, None, None);
            }
        }
        for v in campos {
            let ve = self.consulta.programa.variable(v);
            if !ve.static_ || !self.visivel_em(ve.library, self.consulta.nome(ve.name)) {
                continue;
            }
            let constante_de_enum = c.is_some_and(|c| self.consulta.programa.class(c).enum_constants.contains(&v));
            if constante_de_enum {
                let e = self.consulta.nome(self.consulta.programa.class(c.unwrap_or(ClassId(0))).name).to_string();
                let nome = self.consulta.nome(ve.name).to_string();
                if let Some(s) = self.pontuar(&format!("{e}.{nome}")) {
                    // `includeEnumName: false` → `suggestField(…, 0.0)`.
                    let antes = self.coletor.itens.len();
                    let tipo = self.consulta.tipo_da_variavel(v);
                    match self.consulta.programa.variable(v).getter {
                        Some(g) => {
                            let t = tipo.unwrap_or(self.consulta.core.dynamic_);
                            self.coletor.funcao(&*self.consulta, grupo::MEMBRO, g, t);
                        }
                        None => {
                            self.coletor.empurrar(grupo::MEMBRO, especie::MEMBRO_DE_ENUM, nome.clone(), nome, None);
                        }
                    }
                    for i in &mut self.coletor.itens[antes..] {
                        i.rel.especie = Some(crate::relevancia::Especie::Campo);
                        i.rel.distancia = Some(0);
                    }
                    self.marcar(antes, s, false);
                }
            } else {
                self.sugerir_campo(v, None, None);
            }
        }
        // O `values` sintético do enum entra entre os campos.
        if e_enum
            && let Some(c) = c
            && let Some(s) = self.consulta.nomes.lookup("values")
            && let Some(&g) = self.consulta.programa.class(c).static_members.get(&s)
        {
            self.sugerir_propriedade(g, None, false, None, None);
        }
        if !self.fl().atribuivel {
            if construtores && let Some(c) = c {
                let lista = self.consulta.programa.class(c).construtores();
                for (n, f) in lista {
                    let fe = self.consulta.programa.function(f);
                    if self.visivel_em(fe.library, self.consulta.nome(n)) && (nao_fabricas || fe.factory) {
                        self.sugerir_construtor(f, true, None, false);
                    }
                }
            }
            for m in metodos {
                let fe = self.consulta.programa.function(m);
                if fe.static_ && self.visivel_em(fe.library, self.consulta.nome(fe.name)) {
                    self.sugerir_metodo(m, None, false, None, None);
                }
            }
        }
        let _ = lib;
    }

    /// `addPossibleRedirectionsInLibrary`.
    pub(super) fn dh_redirecionamentos(&mut self, redirecionador: FunctionElementId) {
        use dartforge_elements::model::ClassKind as K;
        let Some(c) = self.consulta.programa.function(redirecionador).class else { return };
        let c = self.consulta.programa.dono_da_classe(c);
        let tipo_c = self.tipo_this(c);
        let lib = self.biblioteca();
        let unidades = self.consulta.programa.library(lib).units.clone();
        for u in unidades {
            let e = self.elementos_da_unidade(u);
            for k in e.classes {
                if self.consulta.programa.class(k).kind != K::Class && self.consulta.programa.class(k).kind != K::MixinApplication {
                    continue;
                }
                let tipo_k = self.tipo_this(k);
                if !self.subtipo(tipo_k, tipo_c) {
                    continue;
                }
                let lista = self.consulta.programa.class(k).construtores();
                for (n, f) in lista {
                    let fe = self.consulta.programa.function(f);
                    let acessivel = fe.library == lib || !self.consulta.nome(n).starts_with('_');
                    if f != redirecionador && acessivel {
                        self.sugerir_construtor(f, false, None, true);
                    }
                }
            }
        }
    }

    /// `addFieldsForInitializers`.
    pub(super) fn dh_campos_para_inicializadores(&mut self, ctor: usize, incluir: Option<VariableId>) {
        let Some(classe) = self.pai(ctor).and_then(|c| self.classe_do_no(c)) else { return };
        let p = &self.consulta.programa;
        let mut pular: HashSet<VariableId> = HashSet::new();
        let campo_por_nome = |nome: &str| p.class(classe).fields.iter().copied().find(|&v| self.consulta.nome(p.variable(v).name) == nome);
        for i in self.filhos_de(ctor, &["ConstructorFieldInitializer"]) {
            if let Some(ast::Initializer::Field { name, .. }) = self.inicializador_real(i)
                && let Some(v) = campo_por_nome(self.nome_real(*name))
            {
                pular.insert(v);
            }
        }
        if let Some(k) = self.construtor_real(ctor) {
            for prm in k.parameters.iter() {
                if prm.this_
                    && let Some(n) = prm.name
                    && let Some(v) = campo_por_nome(self.nome_real(n))
                {
                    pular.insert(v);
                }
            }
        }
        if let Some(v) = incluir {
            pular.remove(&v);
        }
        let campos = p.class(classe).fields.clone();
        for v in campos {
            let ve = self.consulta.programa.variable(v);
            if ve.static_ || pular.contains(&v) {
                continue;
            }
            let tem_inicializador = self.campo_tem_inicializador(v);
            if !(ve.final_ || ve.const_) || !tem_inicializador {
                self.sugerir_campo(v, None, None);
            }
        }
    }

    /// `field.hasInitializer`.
    pub(super) fn campo_tem_inicializador(&self, v: VariableId) -> bool {
        let p = &self.consulta.programa;
        match p.variable(v).node {
            VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => l.variables.get(index).is_some_and(|x| x.initializer.is_some()),
                _ => false,
            },
            _ => false,
        }
    }

    /// `addParametersFromSuperConstructor` (o nó é o `SuperFormalParameter`).
    pub(super) fn dh_parametros_do_super(&mut self, n: usize) {
        self.dh(Flags::default());
        let Some(prm) = self.parametro_real(n) else { return };
        if !prm.super_ {
            return;
        }
        let Some(ctor) = self.ancestral(n, &["ConstructorDeclaration"]) else { return };
        let Some(k) = self.construtor_real(ctor) else { return };
        let Some(classe) = self.pai(ctor).and_then(|c| self.classe_do_no(c)) else { return };
        let p = &self.consulta.programa;
        let Some(sup) = p.class(classe).supertype_class else { return };
        // `superConstructor`: o da invocação `super(...)`/`super.n(...)`, ou
        // o sem nome.
        let nome_sup = k
            .initializers
            .iter()
            .find_map(|i| match i {
                ast::Initializer::Super { constructor, .. } => Some(constructor.map(|c| self.nome_real(c).to_string()).unwrap_or_default()),
                _ => None,
            })
            .unwrap_or_default();
        let Some(f_sup) = p.class(p.dono_da_classe(sup)).construtores().into_iter().find(|(s, _)| self.consulta.nome(*s) == nome_sup).map(|(_, f)| f) else {
            return;
        };
        let parametros_sup = self.consulta.outline.functions[f_sup.0 as usize].parameters.clone();
        if prm.kind == ast::ParameterKind::Named {
            let mut especificados: HashSet<String> = k.parameters.iter().filter_map(|x| x.name.map(|n| self.nome_real(n).to_string())).collect();
            for i in k.initializers.iter() {
                if let ast::Initializer::Super { arguments, .. } = i {
                    for a in arguments.args.iter() {
                        if let Some(n) = a.name {
                            especificados.insert(self.nome_real(n).to_string());
                        }
                    }
                }
            }
            for ps in parametros_sup.iter() {
                if ps.kind == ast::ParameterKind::Named
                    && let Some(nome) = ps.externo.or(ps.name)
                {
                    let texto = self.consulta.nome(nome).to_string();
                    if !especificados.contains(&texto) {
                        self.item_parametro_do_super(&texto, ps.ty);
                    }
                }
            }
        } else {
            let posicionais_daqui: Vec<&ast::Parameter> = k.parameters.iter().filter(|x| x.super_ && x.kind != ast::ParameterKind::Named).collect();
            let indice = posicionais_daqui.iter().position(|x| std::ptr::eq(*x, prm));
            let posicionais_sup: Vec<_> = parametros_sup.iter().filter(|x| x.kind != ast::ParameterKind::Named).collect();
            if let Some(i) = indice
                && let Some(ps) = posicionais_sup.get(i)
                && let Some(nome) = ps.name
            {
                let texto = self.consulta.nome(nome).to_string();
                self.item_parametro_do_super(&texto, ps.ty);
            }
        }
    }

    /// `_suggestSuperParameter` (`Relevance.superFormalParameter`).
    pub(super) fn item_parametro_do_super(&mut self, nome: &str, tipo: TypeId) {
        if let Some(s) = self.pontuar(nome) {
            let antes = self.coletor.itens.len();
            let detalhe = self.consulta.formatar(tipo);
            let item = self.coletor.empurrar(grupo::LOCAL, especie::VARIAVEL, nome.to_string(), nome.to_string(), Some(detalhe));
            item.rel.fixa = Some(1000);
            self.marcar(antes, s, false);
        }
    }
}


/// O `Name` do analyzer: o nome, e a biblioteca quando é privado.
type Chave = (SymbolId, Option<dartforge_elements::model::LibraryId>);

/// Um `Map<Name, ExecutableElement>` do Dart: a ordem é a da primeira
/// inserção, e substituir mantém a posição.
#[derive(Default, Clone)]
struct MapaOrdenado {
    ordem: Vec<Chave>,
    valores: HashMap<Chave, FunctionElementId>,
}

impl MapaOrdenado {
    fn inserir(&mut self, k: Chave, v: FunctionElementId) {
        if self.valores.insert(k, v).is_none() {
            self.ordem.push(k);
        }
    }

    fn contem(&self, k: &Chave) -> bool {
        self.valores.contains_key(k)
    }

    fn entradas(&self) -> impl Iterator<Item = (Chave, FunctionElementId)> + '_ {
        self.ordem.iter().map(|k| (*k, self.valores[k]))
    }

    /// O `_addCandidates` seguido do `_findMostSpecificFromNamedCandidates`
    /// quando há um candidato só: os nomes ausentes entram no fim, e o
    /// primeiro candidato de cada nome fica.
    fn acrescentar_ausentes(&mut self, outro: &MapaOrdenado) {
        for (k, v) in outro.entradas() {
            if !self.contem(&k) {
                self.inserir(k, v);
            }
        }
    }
}

/// A `Interface` do `InheritanceManager3`, só com o que o completar lê.
#[derive(Default)]
struct Interface {
    mapa: MapaOrdenado,
    implementado: MapaOrdenado,
    super_implementado: MapaOrdenado,
    sobrescritos: MapaOrdenado,
    encaminhados: HashSet<Chave>,
}

fn chave(q: &Consulta, f: FunctionElementId) -> Chave {
    let p = &q.programa;
    let fe = p.function(f);
    let privado = q.nome(fe.name).starts_with('_');
    (fe.name, privado.then_some(fe.library))
}

/// `_getTypeMembers` (com `abstratos`) ou `_addImplemented` (sem): os
/// métodos e depois os acessores, na ordem de declaração.
fn declarados(q: &Consulta, c: ClassId, abstratos: bool) -> Vec<(Chave, FunctionElementId)> {
    let p = &q.programa;
    let mut v: Vec<FunctionElementId> = p
        .class(c)
        .instance_members
        .values()
        .copied()
        .filter(|&f| !p.function(f).static_ && (abstratos || !p.function(f).abstract_))
        .collect();
    v.sort_by_key(|&f| (!matches!(p.function(f).kind, FunctionKind::Function | FunctionKind::Operator), f));
    v.into_iter().map(|f| (chave(q, f), f)).collect()
}

fn de_object(p: &dartforge_elements::model::Program, object: Option<ClassId>, f: FunctionElementId) -> bool {
    object.is_some() && p.function(f).class.map(|c| p.dono_da_classe(c)) == object
}

/// `getInterface`: `_getInterfaceClass`, `_getInterfaceMixin` e
/// `_getInterfaceExtensionType`.
fn interface_de(
    q: &Consulta,
    object: Option<ClassId>,
    c: ClassId,
    cache: &mut HashMap<ClassId, std::rc::Rc<Interface>>,
    processando: &mut HashSet<ClassId>,
) -> std::rc::Rc<Interface> {
    if let Some(i) = cache.get(&c) {
        return i.clone();
    }
    if !processando.insert(c) {
        return std::rc::Rc::new(Interface::default());
    }
    let p = &q.programa;
    let cl = p.class(c);
    let sub = |x: ClassId, cache: &mut HashMap<ClassId, std::rc::Rc<Interface>>, processando: &mut HashSet<ClassId>| {
        interface_de(q, object, p.dono_da_classe(x), cache, processando)
    };
    let i = match cl.kind {
        ClassKind::Mixin => {
            // `superclassConstraints` (`Object` quando não há `on`).
            let restricoes: Vec<ClassId> = if cl.on_classes.is_empty() { object.into_iter().collect() } else { cl.on_classes.clone() };
            let mut super_candidatos = MapaOrdenado::default();
            for r in restricoes {
                let i = sub(r, cache, processando);
                super_candidatos.acrescentar_ausentes(&i.mapa);
            }
            let mut candidatos = super_candidatos.clone();
            for &x in &cl.interface_classes {
                let i = sub(x, cache, processando);
                candidatos.acrescentar_ausentes(&i.mapa);
            }
            let mut mapa = MapaOrdenado::default();
            for (k, f) in declarados(q, c, true) {
                mapa.inserir(k, f);
            }
            mapa.acrescentar_ausentes(&candidatos);
            let mut implementado = MapaOrdenado::default();
            for (k, f) in declarados(q, c, false) {
                implementado.inserir(k, f);
            }
            Interface { mapa, implementado, super_implementado: super_candidatos, sobrescritos: candidatos, encaminhados: HashSet::new() }
        }
        ClassKind::ExtensionType => {
            let mut declarado = MapaOrdenado::default();
            for (k, f) in declarados(q, c, false) {
                declarado.inserir(k, f);
            }
            // Os nomes precluídos: os declarados, o setter de um método e o
            // método de um setter declarados.
            let nome = |k: &Chave| q.nome(k.0).to_string();
            let mut precluidos: HashSet<String> = HashSet::new();
            for (k, f) in declarado.entradas() {
                let n = nome(&k);
                precluidos.insert(n.clone());
                match p.function(f).kind {
                    FunctionKind::Function | FunctionKind::Operator => {
                        precluidos.insert(format!("{n}="));
                    }
                    FunctionKind::Setter => {
                        precluidos.insert(n.trim_end_matches('=').to_string());
                    }
                    FunctionKind::ImplicitAccessor if n.ends_with('=') => {
                        precluidos.insert(n.trim_end_matches('=').to_string());
                    }
                    _ => {}
                }
            }
            let interfaces: Vec<ClassId> = if cl.interface_classes.is_empty() { object.into_iter().collect() } else { cl.interface_classes.clone() };
            let mut de_extensao = MapaOrdenado::default();
            let mut nao_extensao = MapaOrdenado::default();
            for x in interfaces {
                let i = sub(x, cache, processando);
                for (k, f) in i.mapa.entradas() {
                    let e_de_tipo_de_extensao = p.function(f).class.is_some_and(|d| p.class(d).kind == ClassKind::ExtensionType);
                    let alvo = if e_de_tipo_de_extensao { &mut de_extensao } else { &mut nao_extensao };
                    if !alvo.contem(&k) {
                        alvo.inserir(k, f);
                    }
                }
            }
            let mut mapa = declarado.clone();
            let mut redeclarados = MapaOrdenado::default();
            for (k, f) in de_extensao.entradas() {
                redeclarados.inserir(k, f);
                if precluidos.contains(&nome(&k)) || nao_extensao.contem(&k) {
                    continue;
                }
                mapa.inserir(k, f);
            }
            for (k, f) in nao_extensao.entradas() {
                if !redeclarados.contem(&k) {
                    redeclarados.inserir(k, f);
                }
                if precluidos.contains(&nome(&k)) || de_extensao.contem(&k) {
                    continue;
                }
                mapa.inserir(k, f);
            }
            Interface { implementado: mapa.clone(), mapa, super_implementado: MapaOrdenado::default(), sobrescritos: redeclarados, encaminhados: HashSet::new() }
        }
        ClassKind::Class | ClassKind::Enum | ClassKind::MixinApplication => {
            let mut candidatos = MapaOrdenado::default();
            let mut implementado = MapaOrdenado::default();
            let mut encaminhados_do_super = HashSet::new();
            if let Some(s) = cl.supertype_class {
                let i = sub(s, cache, processando);
                candidatos.acrescentar_ausentes(&i.mapa);
                implementado = i.implementado.clone();
                encaminhados_do_super = i.encaminhados.clone();
            }
            let mut super_implementado = implementado.clone();
            for &m in &cl.mixin_classes {
                let dono_m = p.dono_da_classe(m);
                let im = sub(m, cache, processando);
                for (k, f) in im.mapa.entradas() {
                    // O membro declarado no próprio mixin substitui o da
                    // superclasse; o que o mixin herda do `on` fica com o
                    // mais específico, que é o já presente.
                    if !candidatos.contem(&k) || p.function(f).class.map(|d| p.dono_da_classe(d)) == Some(dono_m) {
                        candidatos.inserir(k, f);
                    }
                }
                for (k, f) in im.implementado.entradas() {
                    if p.function(f).abstract_ || de_object(p, object, f) {
                        continue;
                    }
                    implementado.inserir(k, f);
                }
                super_implementado = implementado.clone();
            }
            for &x in &cl.interface_classes {
                let i = sub(x, cache, processando);
                candidatos.acrescentar_ausentes(&i.mapa);
            }
            for (k, f) in declarados(q, c, false) {
                implementado.inserir(k, f);
            }
            let mut mapa = MapaOrdenado::default();
            for (k, f) in declarados(q, c, true) {
                mapa.inserir(k, f);
            }
            mapa.acrescentar_ausentes(&candidatos);
            // Os encaminhadores de `noSuchMethod`: numa classe abstrata,
            // os da superclasse; numa concreta com `noSuchMethod` que não é
            // o de `Object`, os nomes da interface sem implementação.
            let abstrata = cl.kind == ClassKind::Class && cl.modifiers.abstract_ || cl.decl.is_none();
            let mut encaminhados = HashSet::new();
            if abstrata {
                encaminhados = encaminhados_do_super;
            } else {
                let nsm = implementado.entradas().find(|(k, _)| q.nome(k.0) == "noSuchMethod").map(|(_, f)| f);
                if let Some(n) = nsm
                    && !de_object(p, object, n)
                {
                    for (k, f) in mapa.entradas() {
                        if !implementado.contem(&k) || encaminhados_do_super.contains(&k) {
                            implementado.inserir(k, f);
                            encaminhados.insert(k);
                        }
                    }
                }
            }
            Interface { mapa, implementado, super_implementado, sobrescritos: candidatos, encaminhados }
        }
    };
    processando.remove(&c);
    let i = std::rc::Rc::new(i);
    cache.insert(c, i.clone());
    i
}

/// O lugar de um elemento de topo na biblioteca `lib`: a unidade (na ordem
/// das unidades dela), o início da declaração e a posição na lista de
/// variáveis.
fn lugar_na_biblioteca(p: &dartforge_elements::model::Program, lib: dartforge_elements::model::LibraryId, el: Element) -> Option<(usize, usize, usize)> {
    use dartforge_elements::model::{FunctionRef, VariableRef};
    let unidades = &p.library(lib).units;
    let da_unidade = |u: UnitId| unidades.iter().position(|&x| x == u);
    let da_variavel = |v: VariableId| match p.variable(v).node {
        VariableRef::TopLevel { unit, decl, index } => Some((da_unidade(unit)?, p.unit(unit).ast.decl(decl).span.start, index)),
        _ => None,
    };
    match el {
        Element::Class(c) => {
            let d = p.class(c).decl?;
            Some((da_unidade(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Typedef(x) => {
            let d = p.typedef(x).decl;
            Some((da_unidade(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Extension(x) => {
            let d = p.extension(x).decl;
            Some((da_unidade(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Function(f) => {
            let fe = p.function(f);
            if let Some(v) = fe.variable {
                return da_variavel(v);
            }
            match fe.node {
                FunctionRef::Function { unit, function } => Some((da_unidade(unit)?, p.unit(unit).ast.functions[function.0 as usize].span.start, 0)),
                _ => None,
            }
        }
        Element::Variable(v) => da_variavel(v),
        Element::Prefix(..) => None,
    }
}

fn biblioteca_do_elemento(p: &dartforge_elements::model::Program, el: Element) -> Option<dartforge_elements::model::LibraryId> {
    Some(match el {
        Element::Class(c) => p.class(c).library,
        Element::Typedef(x) => p.typedef(x).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(..) => return None,
    })
}

/// Os nomes do `exportNamespace` de `lib` na ordem do analyzer: os
/// declarados nela pela fonte, depois os de cada `export` na ordem das
/// diretivas (com os combinadores), sem repetir.
fn ordem_de_exportacao(
    p: &dartforge_elements::model::Program,
    lib: dartforge_elements::model::LibraryId,
    vistas: &mut HashSet<dartforge_elements::model::LibraryId>,
) -> Vec<SymbolId> {
    if !vistas.insert(lib) {
        return Vec::new();
    }
    let l = p.library(lib);
    let mut locais: Vec<((usize, usize, usize), SymbolId)> = l
        .exported
        .iter()
        .filter_map(|(&s, b)| {
            let el = b.getter.or(b.setter)?;
            (biblioteca_do_elemento(p, el) == Some(lib)).then_some(())?;
            Some((lugar_na_biblioteca(p, lib, el)?, s))
        })
        .collect();
    locais.sort();
    let mut ordem: Vec<SymbolId> = locais.into_iter().map(|(_, s)| s).collect();
    let mut presentes: HashSet<SymbolId> = ordem.iter().copied().collect();
    let mut exports = l.exports.clone();
    let posicao_da_unidade = |u: UnitId| l.units.iter().position(|&x| x == u).unwrap_or(usize::MAX);
    exports.sort_by_key(|e| (posicao_da_unidade(e.unit), e.directive));
    for e in exports {
        for s in ordem_de_exportacao(p, e.library, vistas) {
            let passa = e.combinators.iter().all(|c| match c {
                ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == s),
                ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == s),
            });
            if passa && l.exported.contains_key(&s) && presentes.insert(s) {
                ordem.push(s);
            }
        }
    }
    vistas.remove(&lib);
    ordem
}

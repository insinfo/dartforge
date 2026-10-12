//! Os `visit*` do `InScopeCompletionPass` (um por espécie de nó) e os
//! auxiliares `_forX` (`in_scope_completion_pass.dart:241-3929`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Flags, Inicial, Isp};
use super::*;
use dartforge_elements::model::UnitId;
use crate::arvore_analyzer::Ligacao;
use dartforge_frontend::token::Op;

/// O texto de um `completionLocation` de argumento.
fn papel_do_argumento(pai: &str) -> &'static str {
    match pai {
        "Annotation" => "annotation",
        "EnumConstantArguments" => "enumConstantArguments",
        "ExtensionOverride" => "extensionOverride",
        "FunctionExpressionInvocation" => "function",
        "InstanceCreationExpression" => "constructor",
        "MethodInvocation" => "method",
        "RedirectingConstructorInvocation" | "SuperConstructorInvocation" => "constructorRedirect",
        _ => "",
    }
}

/// Um parâmetro da invocação (`invokedFormalParameters`).
#[derive(Debug, Clone)]
pub(super) struct ParametroInvocado {
    pub nome: String,
    pub nomeado: bool,
    pub obrigatorio: bool,
    pub tipo: TypeId,
    /// `isFlutterWidgetParameter`: o construtor é de um widget do Flutter.
    pub widget: bool,
    /// `hasRequired`: a anotação `@required` do `package:meta`.
    pub anotado_required: bool,
    /// Os nomes dos posicionais do tipo de função escrito no parâmetro (o
    /// `FunctionType.parameters` do analyzer tem os nomes; o tipo do
    /// DartForge não).
    pub nomes_do_tipo: Vec<String>,
}

impl<'a, 'c> Isp<'a, 'c> {
    /// `completionNode.accept(this)`: o `visit*` da espécie (nenhum para as
    /// espécies sem `visit` próprio: o `SimpleAstVisitor` não faz nada).
    pub(super) fn visitar(&mut self, n: usize) {
        match self.especie(n) {
            "AdjacentStrings" | "DoubleLiteral" | "IntegerLiteral" | "StringInterpolation" => self.visitar_pai_se_no_ou_antes(n),
            "Annotation" => self.v_anotacao(n),
            "ArgumentList" => self.v_lista_de_argumentos(n),
            "AsExpression" => self.v_as(n),
            "AssertInitializer" => self.v_assert_inicializador(n),
            "AssertStatement" => self.v_assert(n),
            "AssignmentExpression" => {
                self.local("AssignmentExpression_rightHandSide");
                self.para_expressao(n, false, true);
            }
            "AwaitExpression" => {
                self.local("AwaitExpression_expression");
                self.para_expressao(n, false, true);
            }
            "BinaryExpression" => self.v_binaria(n),
            "Block" => self.v_bloco(n),
            "BooleanLiteral" | "NullLiteral" | "ConstructorReference" | "ExtensionOverride" | "FunctionExpressionInvocation" | "FunctionReference"
            | "SymbolLiteral" | "ThisExpression" | "TypeLiteral" => self.para_expressao(n, false, false),
            "BreakStatement" | "ContinueStatement" => self.v_break_continue(n),
            "CascadeExpression" => {
                self.local("CascadeExpression_cascadeSection");
                self.para_expressao(n, false, false);
            }
            "CaseClause" => {
                self.local("CaseClause_pattern");
                self.para_padrao(n, true);
            }
            "CastPattern" => self.v_padrao_cast(n),
            "CatchClause" => self.v_catch(n),
            "ClassDeclaration" => self.v_classe(n),
            "CommentReference" => {
                self.local("CommentReference_identifier");
                self.dh(Flags { preferir_sem_invocacao: true, ..Flags::default() });
                self.dh_lexicas(n);
            }
            "CompilationUnit" => self.v_unidade(n),
            "ConditionalExpression" => self.v_condicional(n),
            "ConstantPattern" => {
                if self.filhos(n).first().is_some_and(|&f| self.especie(f) == "SimpleIdentifier")
                    && let Some(p) = self.pai(n)
                {
                    self.visitar(p);
                }
            }
            "ConstructorDeclaration" => self.v_construtor(n),
            "ConstructorFieldInitializer" => self.v_inicializador_de_campo(n),
            "ConstructorName" => self.v_nome_de_construtor(n),
            "ConstructorSelector" => self.v_seletor_de_construtor(n),
            "DeclaredIdentifier" => {
                if let Some(p) = self.pai(n) {
                    self.visitar(p);
                }
            }
            "DeclaredVariablePattern" => self.v_padrao_de_variavel_declarada(n),
            "DefaultFormalParameter" => self.v_parametro_padrao(n),
            "DoStatement" => self.v_do(n),
            "EmptyStatement" => self.v_comando_vazio(n),
            "EnumDeclaration" => self.v_enum(n),
            "ExportDirective" => {
                if let Some(i) = self.palavra_em("export", self.ini(n), self.fim(n))
                    && self.offset <= self.span_tok(i).end
                {
                    self.para_membro_de_unidade_antes(n);
                }
            }
            "ExpressionFunctionBody" => self.v_corpo_de_expressao(n),
            "ExpressionStatement" => self.v_comando_de_expressao(n),
            "ExtendsClause" => self.v_extends(n),
            "ExtensionDeclaration" => self.v_extensao(n),
            "ExtensionOnClause" => self.v_on_de_extensao(n),
            "ExtensionTypeDeclaration" => self.v_tipo_de_extensao(n),
            "FieldDeclaration" => self.v_campo(n),
            "FieldFormalParameter" => self.v_parametro_de_campo(n),
            "ForEachPartsWithDeclaration" | "ForEachPartsWithIdentifier" => {
                self.local("ForEachPartsWithDeclaration_iterable");
                self.visitar_partes_de_for_each(n);
            }
            "ForEachPartsWithPattern" => {
                self.local("visitForEachPartsWithPattern_iterable");
                self.visitar_partes_de_for_each(n);
            }
            "ForElement" => {
                self.local("ForElement_body");
                if let Some(lit) = self.ancestral(n, &["ListLiteral", "SetOrMapLiteral"]) {
                    let elementos = self.elementos_do_literal(lit);
                    self.para_elemento_de_colecao(lit, &elementos);
                }
            }
            "FormalParameterList" => self.v_lista_de_parametros(n),
            "ForPartsWithDeclarations" => self.v_partes_de_for_com_declaracoes(n),
            "ForPartsWithExpression" => {
                if self.todo_sintetico(n)
                    && let Some(p) = self.pai(n)
                {
                    self.visitar(p);
                }
            }
            "ForStatement" => self.v_for(n),
            "FunctionDeclaration" => self.v_declaracao_de_funcao(n),
            "FunctionExpression" => self.v_expressao_de_funcao(n),
            "FunctionTypeAlias" => self.v_typedef_antigo(n),
            "FunctionTypedFormalParameter" => self.v_parametro_de_funcao(n),
            "GenericTypeAlias" => self.v_typedef(n),
            "HideCombinator" => {
                self.local("HideCombinator_hiddenName");
                self.para_combinador(n);
            }
            "ShowCombinator" => {
                self.local("ShowCombinator_shownName");
                self.para_combinador(n);
            }
            "IfElement" => self.v_if_elemento(n),
            "IfStatement" => self.v_if(n),
            "ImplementsClause" => self.v_implements(n),
            "ImportDirective" => self.v_import(n),
            "ImportPrefixReference" => self.v_referencia_de_prefixo(n),
            "IndexExpression" => self.v_indice(n),
            "InstanceCreationExpression" => self.v_criacao(n),
            "InterpolationExpression" => {
                self.local("InterpolationExpression_expression");
                let estatico = self.contexto_estatico(n);
                self.dh(Flags { estatico, nao_void: true, ..Flags::default() });
                self.dh_lexicas(n);
            }
            "IsExpression" => self.v_is(n),
            "Label" => self.v_rotulo(n),
            "LibraryDirective" => self.v_library(n),
            "ListLiteral" | "SetOrMapLiteral" => self.v_literal_de_colecao(n),
            "ListPattern" => {
                self.local("ListPattern_element");
                self.para_padrao(n, true);
            }
            "LogicalAndPattern" => {
                self.local("LogicalAndPattern_rightOperand");
                self.para_padrao(n, true);
            }
            "LogicalOrPattern" => {
                self.local("LogicalOrPattern_rightOperand");
                self.para_padrao(n, true);
            }
            "MapLiteralEntry" => self.v_entrada_de_mapa(n),
            "MapPattern" => {
                self.local("MapPatternEntry_key");
                self.para_expressao_constante(n);
            }
            "MapPatternEntry" => self.v_entrada_de_padrao_de_mapa(n),
            "MethodDeclaration" => self.v_metodo(n),
            "MethodInvocation" => self.v_invocacao(n),
            "MixinDeclaration" => self.v_mixin(n),
            "MixinOnClause" => self.v_on_de_mixin(n),
            "NamedExpression" => self.v_expressao_nomeada(n),
            "NamedType" => self.v_tipo_nomeado(n),
            "ObjectPattern" => self.v_padrao_objeto(n),
            "ParenthesizedExpression" => self.v_parenteses(n),
            "ParenthesizedPattern" => {
                self.local("ParenthesizedPattern_expression");
                self.para_padrao(n, true);
            }
            "PartDirective" | "PartOfDirective" => {
                if let Some(i) = self.palavra_em("part", self.ini(n), self.fim(n))
                    && self.offset <= self.span_tok(i).end
                {
                    self.local("CompilationUnit_directive");
                    self.para_membro_de_unidade_antes(n);
                }
            }
            "PatternAssignment" => {
                self.local("PatternAssignment_expression");
                self.para_expressao(n, false, false);
            }
            "PatternField" => self.v_campo_de_padrao(n),
            "PatternFieldName" => self.v_nome_de_campo_de_padrao(n),
            "PatternVariableDeclaration" => {
                self.local("PatternVariableDeclaration_expression");
                self.para_expressao(n, false, false);
            }
            "PostfixExpression" => self.v_posfixa(n),
            "PrefixedIdentifier" => self.v_identificador_prefixado(n),
            "PrefixExpression" => self.v_prefixa(n),
            "PropertyAccess" => self.v_acesso_a_propriedade(n),
            "RecordLiteral" => {
                self.local("RecordLiteral_fields");
                self.sugerir_campos_nomeados_de_record(n, Some(n), true);
                self.para_expressao(n, false, false);
            }
            "RecordPattern" => self.v_padrao_record(n),
            "RecordTypeAnnotation" => self.v_tipo_record(n),
            "RecordTypeAnnotationNamedField" => self.v_campo_nomeado_de_tipo_record(n),
            "RecordTypeAnnotationNamedFields" => {
                self.local("RecordTypeAnnotationNamedFields_fields");
                self.para_tipo(n, TipoOpcoes::default());
            }
            "RecordTypeAnnotationPositionalField" => {
                if let Some(t) = self.filhos(n).first().copied()
                    && self.cobre_no(t)
                {
                    self.local("RecordTypeAnnotation_positionalFields");
                    self.para_tipo(n, TipoOpcoes::default());
                }
            }
            "RedirectingConstructorInvocation" => self.v_invocacao_redirecionadora(n),
            "RelationalPattern" => self.v_padrao_relacional(n),
            "RepresentationDeclaration" => self.v_representacao(n),
            "RestPatternElement" => {
                self.local("RestPatternElement_pattern");
                self.para_padrao(n, true);
            }
            "ReturnStatement" => self.v_return(n),
            "SimpleFormalParameter" => self.v_parametro_simples(n),
            "SimpleStringLiteral" => self.v_string_simples(n),
            "SpreadElement" => {
                self.local("SpreadElement_expression");
                self.para_expressao(n, false, false);
            }
            "SuperConstructorInvocation" => self.v_invocacao_do_super(n),
            "SuperFormalParameter" => self.dh_parametros_do_super(n),
            "SwitchCase" => {
                self.local("SwitchMember_statement");
                self.para_comando(n);
            }
            "SwitchDefault" => self.v_default(n),
            "SwitchExpression" => self.v_switch_expressao(n),
            "SwitchExpressionCase" => self.v_caso_de_switch_expressao(n),
            "SwitchPatternCase" => self.v_caso_de_padrao(n),
            "SwitchStatement" => self.v_switch(n),
            "ThrowExpression" => {
                self.local("ThrowExpression_expression");
                self.para_expressao(n, false, false);
            }
            "TopLevelVariableDeclaration" => self.v_variavel_de_topo(n),
            "TryStatement" => self.v_try(n),
            "TypeArgumentList" => self.para_tipo(n, TipoOpcoes::default()),
            "TypeParameter" => self.v_parametro_de_tipo(n),
            "VariableDeclaration" => self.v_declaracao_de_variavel(n),
            "VariableDeclarationList" => self.v_lista_de_variaveis(n),
            "VariableDeclarationStatement" => self.v_comando_de_variaveis(n),
            "WhenClause" => {
                if let Some(i) = self.palavra_em("when", self.ini(n), self.fim(n))
                    && self.offset > self.span_tok(i).end
                {
                    self.local("WhenClause_expression");
                    self.para_expressao(n, false, false);
                }
            }
            "WhileStatement" => self.v_while(n),
            "WildcardPattern" => {
                if let Some(t) = self.filho(n, "NamedType")
                    && self.cobre_no(t)
                {
                    self.dh(Flags { tipo: true, ..Flags::default() });
                    self.dh_lexicas(n);
                }
            }
            "WithClause" => self.v_with(n),
            "YieldStatement" => self.v_yield(n),
            _ => {}
        }
    }

    // -- Auxiliares do Dart ------------------------------------------------------

    /// `_visitParentIfAtOrBeforeNode`.
    pub(super) fn visitar_pai_se_no_ou_antes(&mut self, n: usize) {
        if self.offset <= self.ini(n)
            && let Some(p) = self.pai(n)
        {
            self.visitar(p);
        }
    }

    /// `_forExpression`.
    pub(super) fn para_expressao(&mut self, n: usize, atribuivel: bool, nao_void: bool) {
        let e_expr = super::isp::ESPECIES_DE_EXPRESSAO.contains(&self.especie(n));
        let constante = e_expr && (self.contexto_constante(n) || self.pai(n).is_some_and(|p| self.especie(p) == "DefaultFormalParameter"));
        let estatico = self.contexto_estatico(n);
        self.kw_expressao(Some(n), constante, estatico);
        self.dh(Flags { atribuivel, constante, nao_void, estatico, ..Flags::default() });
        self.dh_lexicas(n);
    }

    /// `_forStatement`.
    pub(super) fn para_comando(&mut self, n: usize) {
        self.para_expressao(n, false, false);
        self.kw_comando(n);
    }

    /// `_forTypeAnnotation`.
    pub(super) fn para_tipo(&mut self, n: usize, o: TipoOpcoes) {
        if !(o.estensivel || o.implementavel || o.misturavel) {
            self.kw("dynamic");
            if !o.nao_void {
                self.kw("void");
            }
        }
        if self.especie(n) == "NamedType" && let Some(prefixo) = self.filho(n, "ImportPrefixReference") {
            if self.prefixo_de_import(prefixo).is_some() {
                self.dh(Flags {
                    estensivel: o.estensivel,
                    implementavel: o.implementavel,
                    misturavel: o.misturavel,
                    nao_void: o.nao_void,
                    excluidos: o.excluidos.clone(),
                    excluir_nomes_de_tipo: o.excluir_nomes_de_tipo,
                    ..Flags::default()
                });
                if let Some(p) = self.prefixo_de_import(prefixo) {
                    self.dh_pelo_prefixo(p);
                }
            }
            return;
        }
        self.dh(Flags {
            estensivel: o.estensivel,
            implementavel: o.implementavel,
            misturavel: o.misturavel,
            tipo: true,
            nao_void: o.nao_void,
            excluidos: o.excluidos,
            ..Flags::default()
        });
        self.dh_lexicas(n);
    }

    /// O prefixo de import (o nome) de um `ImportPrefixReference` ou de um
    /// identificador, se o elemento dele é um `PrefixElement`.
    pub(super) fn prefixo_de_import(&self, n: usize) -> Option<SymbolId> {
        let texto = &self.fonte[self.ini(n)..self.fim(n)];
        let nome = texto.trim_end_matches('.').trim();
        let s = self.consulta.nomes.lookup(nome)?;
        self.consulta.programa.library(self.biblioteca()).prefixes.contains_key(&s).then_some(s)
    }

    /// `_forPattern`.
    pub(super) fn para_padrao(&mut self, n: usize, constante: bool) {
        let coberto = self.no_coberto();
        if self.especie(n) == "CaseClause"
            && let Some(gp) = self.filho(n, "GuardedPattern")
            && let Some(&p) = self.filhos(gp).first()
        {
            match self.especie(p) {
                "ConstantPattern" => {
                    if let Some(&id) = self.filhos(p).first()
                        && self.especie(id) == "SimpleIdentifier"
                        && self.offset < self.ini(id)
                    {
                        self.dh(Flags { tipo: true, ..Flags::default() });
                        self.dh_lexicas(n);
                        return;
                    }
                }
                "WildcardPattern" => {
                    // O nome `_` é o último token do padrão.
                    let nome = self.fim(p).saturating_sub(1);
                    if self.offset < nome {
                        self.dh(Flags { tipo: true, ..Flags::default() });
                        self.dh_lexicas(n);
                        return;
                    }
                }
                _ => {}
            }
        }
        if let Some(c) = coberto
            && self.especie(c) == "SimpleIdentifier"
            && self.pai(c).is_some_and(|p| self.especie(p) == "ConstantPattern")
        {
            self.kw_padrao();
            self.prefere_constantes = true;
            self.dh(Flags { nao_void: true, ..Flags::default() });
            self.dh_lexicas(n);
            return;
        }
        if let Some(c) = coberto
            && self.especie(c) == "NamedType"
            && let Some(p) = self.pai(c)
        {
            let local = match self.especie(p) {
                "DeclaredVariablePattern" => Some("DeclaredVariablePattern_type"),
                "ObjectPattern" => Some("ObjectPattern_type"),
                "WildcardPattern" => Some("WildcardPattern_type"),
                _ => None,
            };
            if let Some(l) = local {
                self.local(l);
                self.visitar(c);
                return;
            }
        }
        self.kw_padrao();
        self.prefere_constantes = true;
        let estatico = self.contexto_estatico(n);
        self.dh(Flags { constante, estatico, padrao_objeto: true, ..Flags::default() });
        self.dh_lexicas(n);
    }

    /// `_forAnnotation`.
    pub(super) fn para_anotacao(&mut self, n: usize) {
        self.dh(Flags { constante: true, ..Flags::default() });
        self.dh_lexicas(n);
    }

    /// `_forClassLikeMember`.
    pub(super) fn para_membro_de_conteiner(&mut self, c: usize) {
        match self.especie(c) {
            "ClassDeclaration" => self.para_membro_de_classe(c),
            "EnumDeclaration" => self.para_membro_de_enum(c),
            "ExtensionDeclaration" => self.para_membro_de_extensao(c),
            "ExtensionTypeDeclaration" => self.para_membro_de_tipo_de_extensao(c),
            "MixinDeclaration" => self.para_membro_de_mixin(c),
            _ => {}
        }
    }

    /// `_forClassMember`.
    pub(super) fn para_membro_de_classe(&mut self, c: usize) {
        self.kw_membro_de_classe();
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(c);
        let classe = self.classe_do_no(c);
        self.sugerir_sobrescritas(classe, false);
    }

    /// `_forEnumMember`.
    pub(super) fn para_membro_de_enum(&mut self, c: usize) {
        self.kw_membro_de_enum();
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(c);
    }

    /// `_forExtensionMember`.
    pub(super) fn para_membro_de_extensao(&mut self, c: usize) {
        self.kw_membro_de_extensao(false);
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(c);
    }

    /// `_forExtensionTypeMember`.
    pub(super) fn para_membro_de_tipo_de_extensao(&mut self, c: usize) {
        self.kw_membro_de_tipo_de_extensao(false);
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(c);
    }

    /// `_forMixinMember`.
    pub(super) fn para_membro_de_mixin(&mut self, c: usize) {
        self.kw_membro_de_mixin();
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(c);
        let classe = self.classe_do_no(c);
        self.sugerir_sobrescritas(classe, false);
    }

    /// Os elementos de um literal de coleção (sem o `TypeArgumentList`).
    pub(super) fn elementos_do_literal(&self, lit: usize) -> Vec<usize> {
        self.filhos(lit).iter().copied().filter(|&f| self.especie(f) != "TypeArgumentList").collect()
    }

    /// `_forCollectionElement`.
    pub(super) fn para_elemento_de_colecao(&mut self, literal: usize, elementos: &[usize]) {
        let estatico = self.contexto_estatico(literal);
        let constante = self.contexto_constante(literal) || self.comeca_com_const(literal);
        self.kw_elemento_de_colecao(literal, elementos, constante, estatico);
        let anterior = self.elemento_antes(elementos);
        self.dh(Flags { estatico, constante, ..Flags::default() });
        self.dh_lexicas(anterior.unwrap_or(literal));
    }

    /// `_forCombinator`: os nomes exportados da biblioteca da diretiva,
    /// menos os já listados (fora o que está sob o cursor).
    pub(super) fn para_combinador(&mut self, n: usize) {
        let Some(diretiva) = self.pai(n) else { return };
        if !matches!(self.especie(diretiva), "ImportDirective" | "ExportDirective") {
            return;
        }
        let Some(lib) = self.biblioteca_da_diretiva(diretiva) else { return };
        let coberto = self.no_coberto();
        let excluidos: HashSet<String> = self
            .filhos(n)
            .iter()
            .copied()
            .filter(|&k| Some(k) != coberto)
            .map(|k| self.fonte[self.ini(k)..self.fim(k)].to_string())
            .collect();
        self.dh(Flags { preferir_sem_invocacao: true, ..Flags::default() });
        self.dh_da_biblioteca(lib, &excluidos);
    }

    /// A biblioteca referida por um `import`/`export` (`referencedLibrary`).
    pub(super) fn biblioteca_da_diretiva(&self, diretiva: usize) -> Option<LibraryId> {
        let d = self.diretiva_real(diretiva)?;
        let alvo = self.mapear(d.span.start);
        let p = &self.consulta.programa;
        let u = p.unit(self.unidade);
        let lib = p.library(u.library);
        let indice = u.unit.directives.iter().position(|x| x.span.start == alvo)?;
        lib.imports
            .iter()
            .filter(|i| i.unit == self.unidade && i.directive == indice)
            .map(|i| i.library)
            .chain(lib.exports.iter().filter(|e| e.unit == self.unidade && e.directive == indice).map(|e| e.library))
            .next()
    }

    /// `_forCompilationUnitDeclaration`.
    pub(super) fn para_declaracao_de_unidade(&mut self) {
        self.kw_declaracao_de_unidade();
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(0);
    }

    /// `_forCompilationUnitMember(unit, (before, after))`.
    pub(super) fn para_membro_de_unidade(&mut self, antes: Option<usize>, depois: Option<usize>) {
        if antes.is_none_or(|a| self.e_diretiva(a)) {
            self.local("CompilationUnit_directive");
            self.kw_diretiva(antes);
        }
        if depois.is_none_or(|d| !self.e_diretiva(d)) {
            self.local_se_vazio("CompilationUnit_declaration");
            self.para_declaracao_de_unidade();
        }
    }

    /// `_forCompilationUnitMemberBefore`.
    pub(super) fn para_membro_de_unidade_antes(&mut self, m: usize) {
        if self.pai(m) == Some(0) {
            let (a, d) = self.vizinhos_do_membro(m);
            self.para_membro_de_unidade(a, d);
        }
    }

    /// `_forConstantExpression`.
    pub(super) fn para_expressao_constante(&mut self, n: usize) {
        let em_const = super::isp::ESPECIES_DE_EXPRESSAO.contains(&self.especie(n)) && self.contexto_constante(n);
        self.kw_expressao_constante(em_const);
        let estatico = self.contexto_estatico(n);
        self.dh(Flags { constante: true, estatico, ..Flags::default() });
        self.dh_lexicas(n);
    }

    /// `_forConstructorInitializer`.
    pub(super) fn para_inicializador_de_construtor(&mut self, ctor: usize, init: Option<usize>) {
        let campo = init.and_then(|i| self.campo_do_inicializador(ctor, i));
        self.kw_inicializador_de_construtor(ctor, init);
        self.dh(Flags::default());
        self.dh_campos_para_inicializadores(ctor, campo);
    }

    /// O campo de `x = e` (`fieldName.staticElement`).
    pub(super) fn campo_do_inicializador(&self, ctor: usize, init: usize) -> Option<dartforge_elements::model::VariableId> {
        let ast::Initializer::Field { name, .. } = self.inicializador_real(init)? else { return None };
        let classe = self.pai(ctor).and_then(|c| self.classe_do_no(c))?;
        let texto = self.nome_real(*name);
        let p = &self.consulta.programa;
        p.class(classe).fields.iter().copied().find(|&v| self.consulta.nome(p.variable(v).name) == texto)
    }

    /// `_forMemberAccess`.
    pub(super) fn para_acesso_a_membro(&mut self, n: usize, tipo: TypeId, so_super: bool) {
        if so_super {
            // `request.target.dotTarget is SuperExpression`: o método que o
            // contém dá o `superMatches` e o `isNoSuchMethod`.
            self.metodo_do_super = self.ancestral(n, &["MethodDeclaration"]).and_then(|m| self.funcao_real(m)).and_then(|f| f.name).map(|x| self.nome_real(x).to_string());
        }
        let atribuivel = self.deve_ser_atribuivel(n);
        let constante = self.contexto_constante(n);
        let nao_void = self.pai(n).is_some_and(|p| self.especie(p) == "ArgumentList");
        self.dh(Flags { atribuivel, constante, nao_void, ..Flags::default() });
        self.dh_membros_de_instancia(tipo, so_super);
    }

    /// `_computeMustBeAssignable`: o nó é o lado esquerdo de uma atribuição e
    /// o cursor está na linha do operador.
    pub(super) fn deve_ser_atribuivel(&self, n: usize) -> bool {
        let Some(p) = self.pai(n) else { return false };
        if self.especie(p) != "AssignmentExpression" || self.filhos(p).first() != Some(&n) {
            return false;
        }
        let Some(op) = self.tok_depois(self.fim(n)) else { return false };
        let linha = |o: usize| self.fonte[..o.min(self.fonte.len())].matches('\n').count();
        linha(self.offset) == linha(self.span_tok(op).start)
    }

    /// `_forNameInDeclaredVariablePattern`.
    pub(super) fn para_nome_em_padrao_de_variavel(&mut self, n: usize) {
        let Some(p) = self.pai(n) else { return };
        match self.especie(p) {
            "GuardedPattern" => {
                if self.nome_do_padrao_de_variavel(n).is_some() {
                    self.kw("when");
                    if let Some(t) = self.filho(n, "NamedType") {
                        let nome = self.nome_do_tipo_nomeado(t);
                        self.ih(false);
                        self.ih_do_nome_de_tipo(&nome);
                    }
                }
            }
            "PatternField" => {
                if let Some(externo) = self.pai(p)
                    && super::isp::e_padrao(self.especie(externo))
                {
                    self.local("PatternField_pattern");
                    self.para_nome_de_campo_em_padrao(externo);
                }
            }
            _ => {}
        }
    }

    /// O nome declarado de um `DeclaredVariablePattern` (o último token).
    pub(super) fn nome_do_padrao_de_variavel(&self, n: usize) -> Option<usize> {
        let i = self.tok_antes(self.fim(n))?;
        (self.span_tok(i).start >= self.ini(n) && self.palavra(i)).then_some(i)
    }

    /// O nome (sem prefixo e sem argumentos) de um `NamedType`.
    pub(super) fn nome_do_tipo_nomeado(&self, t: usize) -> String {
        let mut ini = self.ini(t);
        if let Some(p) = self.filho(t, "ImportPrefixReference") {
            ini = self.fim(p);
        }
        let i = self.tok_depois(ini).unwrap_or(0);
        self.texto_tok(i).to_string()
    }

    /// `_forPatternFieldNameInPattern`.
    pub(super) fn para_nome_de_campo_em_padrao(&mut self, padrao: usize) {
        let excluidos = self.nomes_de_campos(padrao);
        match self.especie(padrao) {
            "ObjectPattern" => {
                let tipo = self.filho(padrao, "NamedType").and_then(|t| self.tipo_do_no_de_tipo(t));
                if let Some(t) = tipo {
                    self.dh(Flags { nao_void: true, ..Flags::default() });
                    self.dh_getters(t, &excluidos);
                }
            }
            "RecordPattern" => {
                if let Some(t) = self.tipo_casado(padrao) {
                    self.dh(Flags { nao_void: true, ..Flags::default() });
                    self.dh_getters(t, &excluidos);
                }
            }
            _ => {}
        }
    }

    /// `fieldNames` de um padrão objeto ou record: os nomes efetivos.
    pub(super) fn nomes_de_campos(&self, padrao: usize) -> HashSet<String> {
        let mut nomes = HashSet::new();
        for c in self.filhos_de(padrao, &["PatternField"]) {
            if let Some(nome) = self.nome_efetivo_do_campo(c) {
                nomes.insert(nome);
            }
        }
        nomes
    }

    /// `effectiveName`: o nome escrito, ou o da variável de `:var x`.
    pub(super) fn nome_efetivo_do_campo(&self, c: usize) -> Option<String> {
        let nome = self.filho(c, "PatternFieldName")?;
        let i = self.tok_depois(self.ini(nome))?;
        if self.palavra(i) && self.span_tok(i).end <= self.fim(nome) {
            return Some(self.texto_tok(i).to_string());
        }
        // `:var x`: o nome da variável do subpadrão.
        let sub = self.filhos(c).iter().copied().find(|&k| k != nome)?;
        if self.especie(sub) == "DeclaredVariablePattern" {
            return self.nome_do_padrao_de_variavel(sub).map(|j| self.texto_tok(j).to_string());
        }
        None
    }

    /// O tipo resolvido de um nó de tipo (`type.typeOrThrow`): o dos corpos
    /// (`tipos_de_anotacoes`) ou o do outline (`tipos_escritos`).
    pub(super) fn tipo_do_no_de_tipo(&self, t: usize) -> Option<TypeId> {
        let Ligacao::Tipo(tid) = self.lig(t) else {
            // O `NamedType` que a árvore monta para a criação sem `new`
            // (`A()`, `A.n()`): o tipo criado.
            let nome = self.pai(t).filter(|&p| self.especie(p) == "ConstructorName")?;
            let criacao = self.pai(nome).filter(|&p| self.especie(p) == "InstanceCreationExpression")?;
            let Ligacao::Expr(e) = self.lig(criacao) else { return None };
            return self.tipo_estatico(e);
        };
        let s = self.a.ty(tid).span;
        let (ini, fim) = (self.mapear(s.start), self.mapear_fim(s.end));
        let ast_c = &self.consulta.programa.unit(self.unidade).ast;
        let id = ast_c.types.iter().position(|x| x.span.start == ini && x.span.end == fim).map(|i| ast::TypeId(i as u32))?;
        let corpos = &self.consulta.corpos.units[self.unidade.0 as usize];
        corpos.tipos_de_anotacoes.get(&id).copied().or_else(|| self.consulta.outline.tipos_escritos.get(&(self.unidade, id)).copied())
    }

    /// O `matchedValueType` de um padrão.
    pub(super) fn tipo_casado(&self, padrao: usize) -> Option<TypeId> {
        let Ligacao::Padrao(pid) = self.lig(padrao) else { return None };
        let s = self.a.pattern(pid).span;
        let (ini, fim) = (self.mapear(s.start), self.mapear_fim(s.end));
        let ast_c = &self.consulta.programa.unit(self.unidade).ast;
        let corpos = &self.consulta.corpos.units[self.unidade.0 as usize];
        ast_c.patterns.iter().enumerate().find(|(_, x)| x.span.start == ini && x.span.end == fim).and_then(|(i, _)| corpos.tipos_casados.get(&ast::PatternId(i as u32)).copied())
    }

    /// `_forRedirectingConstructorInvocation`.
    pub(super) fn para_invocacao_redirecionadora(&mut self, ctor: usize) {
        let Some(conteiner) = self.pai(ctor) else { return };
        if !matches!(self.especie(conteiner), "ClassDeclaration" | "EnumDeclaration" | "ExtensionTypeDeclaration") {
            return;
        }
        let Some(classe) = self.classe_do_no(conteiner) else { return };
        let Some(this) = self.tipo_this_da_classe(classe) else { return };
        let nome = self.construtor_real(ctor).and_then(|k| k.name).map(|n| self.nome_real(n).to_string());
        let constante = self.construtor_real(ctor).is_some_and(|k| k.const_);
        self.dh(Flags { constante, ..Flags::default() });
        self.dh_nomes_de_construtor_do_tipo(this, nome);
    }

    /// `thisType` de uma classe.
    pub(super) fn tipo_this_da_classe(&mut self, c: ClassId) -> Option<TypeId> {
        tipo_da_classe(self.consulta, c)
    }

    /// `_forVariablePattern`.
    pub(super) fn para_padrao_de_variavel(&mut self) {
        self.kw_padrao_de_variavel();
    }

    /// `_forIncompletePrecedingClassMember`.
    pub(super) fn membro_incompleto_anterior(&mut self, m: usize) -> bool {
        let Some(i) = self.tok_depois(self.ini(m)) else { return false };
        if self.offset > self.span_tok(i).end {
            return false;
        }
        let Some(anterior) = self.membro_anterior(m) else { return false };
        if self.especie(anterior) == "MethodDeclaration" {
            let corpo = self.filhos(anterior).iter().copied().find(|&k| matches!(self.especie(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"));
            // O corpo todo sintético: o método sem corpo no texto.
            if corpo.is_none() {
                self.local("ClassDeclaration_member");
                self.kw_modificadores_de_corpo(None);
                return true;
            }
        }
        false
    }

    /// `_forIncompletePrecedingStatement`.
    pub(super) fn comando_incompleto_anterior(&mut self, s: usize) -> bool {
        let Some(i) = self.tok_depois(self.ini(s)) else { return false };
        if self.offset > self.span_tok(i).end {
            return false;
        }
        let Some(anterior) = self.comando_anterior(s) else { return false };
        match self.especie(anterior) {
            "IfStatement" => {
                if self.palavra_em("else", self.ini(anterior), self.fim(anterior)).is_none() {
                    self.kw("else");
                    return false;
                }
            }
            "TryStatement" => {
                if self.palavra_em("finally", self.ini(anterior), self.fim(anterior)).is_none() {
                    self.v_try(anterior);
                    return self.filhos_de(anterior, &["CatchClause"]).is_empty();
                }
            }
            _ => {}
        }
        false
    }

    /// `_handledIncompletePrecedingUnitMember`.
    pub(super) fn membro_de_unidade_incompleto(&mut self, anterior: usize) -> bool {
        match self.especie(anterior) {
            "ClassDeclaration" => {
                if self.sem_corpo(anterior) {
                    self.local("CompilationUnit_declaration");
                    self.kw_declaracao_de_classe(anterior);
                    return true;
                }
            }
            "ExtensionTypeDeclaration" => {
                if self.sem_corpo(anterior) {
                    self.local("CompilationUnit_declaration");
                    self.v_tipo_de_extensao(anterior);
                    return true;
                }
            }
            "FunctionDeclaration" => {
                self.local("CompilationUnit_declaration");
                let corpo = self.corpo_da_funcao(anterior);
                if corpo.is_none_or(|c| self.especie(c) == "EmptyFunctionBody") {
                    self.kw_modificadores_de_corpo(corpo);
                }
            }
            "ImportDirective" => {
                let tem_ponto_e_virgula = self.tok_antes(self.fim(anterior)).is_some_and(|i| self.e_op(i, Op::Semicolon));
                if !tem_ponto_e_virgula {
                    self.v_import(anterior);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// O corpo de uma função/método (o nó do corpo).
    pub(super) fn corpo_da_funcao(&self, n: usize) -> Option<usize> {
        let expr = if self.especie(n) == "FunctionDeclaration" { self.filho(n, "FunctionExpression")? } else { n };
        self.filhos(expr).iter().copied().find(|&k| matches!(self.especie(k), "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"))
    }

    /// A declaração não tem corpo (`{` e `}` sintéticos).
    pub(super) fn sem_corpo(&self, n: usize) -> bool {
        let (_, _, esq, dir) = self.chaves(n);
        esq && dir
    }

    /// As chaves do corpo de uma declaração de classe, mixin, enum,
    /// extensão ou tipo de extensão: `({, }, { sintético, } sintético)`.
    pub(super) fn chaves(&self, n: usize) -> (Span, Span, bool, bool) {
        if let Some(i) = self.tok_antes(self.fim(n))
            && self.e_op(i, Op::RBrace)
            && self.span_tok(i).start >= self.ini(n)
            && let Some(j) = self.par(i)
            && self.span_tok(j).start >= self.ini(n)
        {
            return (self.span_tok(j), self.span_tok(i), false, false);
        }
        if let Some(j) = self.op_em(Op::LBrace, self.ini(n), self.fim(n)) {
            let s = self.sintetico(self.fim(n));
            return (self.span_tok(j), s, false, true);
        }
        let s = self.sintetico(self.fim(n));
        (s, s, true, true)
    }

    /// `_handledPossibleClosure`.
    pub(super) fn tratou_possivel_closure(&mut self, e: usize) -> bool {
        let Some(prox) = self.tok_depois(self.fim(e)) else { return false };
        if self.offset > self.span_tok(prox).start {
            return false;
        }
        match self.especie(e) {
            "ParenthesizedExpression" => {
                if self.filhos(e).first().is_some_and(|&f| self.especie(f) == "SimpleIdentifier") {
                    self.kw_modificadores_de_corpo(None);
                    return true;
                }
            }
            "RecordLiteral" => {
                if self.filhos(e).iter().all(|&f| self.especie(f) == "SimpleIdentifier") {
                    self.kw_modificadores_de_corpo(None);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// `_handledRecovery` de uma `TopLevelVariableDeclaration`.
    pub(super) fn tratou_recuperacao(&mut self, decl: usize) -> bool {
        if self.pai(decl) != Some(0) {
            return false;
        }
        let Some(primeiro) = self.tok_depois(self.ini(decl)) else { return false };
        let no_primeiro = self.offset <= self.span_tok(primeiro).end;
        if no_primeiro {
            let (antes, _) = self.vizinhos_do_membro(decl);
            if let Some(a) = antes
                && self.membro_de_unidade_incompleto(a)
            {
                return true;
            }
        }
        if self.declaracao_de_topo_solta(decl) && no_primeiro {
            let (a, d) = self.vizinhos_do_membro(decl);
            self.para_membro_de_unidade(a, d);
            return true;
        }
        false
    }

    /// `TopLevelVariableDeclaration.isSingleIdentifier`: uma palavra seguida
    /// do `;` que o fasta criaria (aqui: sem `;` no texto).
    pub(super) fn declaracao_de_topo_solta(&self, decl: usize) -> bool {
        let Some(i) = self.tok_depois(self.ini(decl)) else { return false };
        if !self.palavra(i) {
            return false;
        }
        let ultimo = self.tok_antes(self.fim(decl));
        ultimo == Some(i)
    }

    /// `_locationFor(argumentList, isNamed)`.
    pub(super) fn local_de_argumentos(&self, lista: usize, nomeado: bool) -> String {
        let pai = self.pai(lista).map_or("", |p| self.especie(p));
        format!("ArgumentList_{}_{}", papel_do_argumento(pai), if nomeado { "named" } else { "unnamed" })
    }

    /// `_tryAnnotationAtEndOfClassBody`: o token deslocado é o `}` e os dois
    /// anteriores são `@` e um identificador.
    pub(super) fn anotacao_no_fim_do_corpo(&mut self, decl: usize) -> bool {
        let Some(chave) = self.tok_depois(self.offset) else { return false };
        if !self.e_op(chave, Op::RBrace) {
            return false;
        }
        let Some(id) = chave.checked_sub(1) else { return false };
        if !self.e_identificador(id) || id == 0 || !self.e_op(id - 1, Op::At) {
            return false;
        }
        self.local("Annotation_name");
        self.para_anotacao(decl);
        self.tentar_anotacao_override(id, decl);
        true
    }

    /// `_tryOverrideAnnotation`.
    pub(super) fn tentar_anotacao_override(&mut self, id: usize, decl: usize) {
        let lexema = self.texto_tok(id);
        if !lexema.is_empty() && "override".starts_with(lexema) {
            let classe = self.classe_do_no(decl);
            if classe.is_some() {
                self.sugerir_sobrescritas(classe, true);
            }
        }
    }

    /// `_suggestOverridesFor`.
    pub(super) fn sugerir_sobrescritas(&mut self, classe: Option<ClassId>, sem_arroba: bool) {
        if let Some(c) = classe {
            self.sobrescritas(c, sem_arroba);
        }
    }

    /// `_visitForEachParts`.
    pub(super) fn visitar_partes_de_for_each(&mut self, n: usize) {
        let Some(em) = self.palavra_em("in", self.ini(n), self.fim(n)) else { return };
        let s = self.span_tok(em);
        if self.cobre(s.start, s.end) {
            let anterior = em.checked_sub(1);
            if anterior.is_some_and(|i| self.e_op(i, Op::Assign)) {
                self.kw("const");
                self.kw("false");
                self.kw("null");
                self.kw("true");
            } else {
                self.kw("in");
            }
        } else {
            self.kw("await");
            let estatico = self.contexto_estatico(n);
            self.dh(Flags { estatico, ..Flags::default() });
            self.dh_lexicas(n);
        }
    }

    // -- Os parâmetros da invocação ------------------------------------------------

    /// `invokedFormalParameters` de uma `ArgumentList`.
    pub(super) fn parametros_invocados(&mut self, lista: usize) -> Option<Vec<ParametroInvocado>> {
        let pai = self.pai(lista)?;
        let consulta = &*self.consulta;
        let pelo_elemento = |f: FunctionElementId| -> Vec<ParametroInvocado> {
            consulta.outline.functions[f.0 as usize]
                .parameters
                .iter()
                .filter_map(|p| {
                    let nome = consulta.nome(p.externo.or(p.name)?).to_string();
                    Some(ParametroInvocado {
                        nome,
                        nomeado: p.kind == ast::ParameterKind::Named,
                        obrigatorio: p.required,
                        tipo: p.ty,
                        widget: false,
                        anotado_required: false,
                        nomes_do_tipo: Vec::new(),
                    })
                })
                .collect()
        };
        let pelo_tipo = |t: TypeId| -> Option<Vec<ParametroInvocado>> {
            let Type::Function { positional, optional, named, .. } = consulta.tabela.get(t).clone() else { return None };
            let mut v: Vec<ParametroInvocado> = positional
                .iter()
                .chain(optional.iter())
                .map(|&t| ParametroInvocado { nome: String::new(), nomeado: false, obrigatorio: false, tipo: t, widget: false, anotado_required: false, nomes_do_tipo: Vec::new() })
                .collect();
            for (n, t, r) in named.iter() {
                v.push(ParametroInvocado { nome: consulta.nome(*n).to_string(), nomeado: true, obrigatorio: *r, tipo: *t, widget: false, anotado_required: false, nomes_do_tipo: Vec::new() });
            }
            Some(v)
        };
        match self.especie(pai) {
            "MethodInvocation" | "FunctionExpressionInvocation" | "InstanceCreationExpression" => {
                let e = self.expr_real(pai)?;
                let c = self.expr_da_consulta(e)?;
                let corpos = &self.consulta.corpos.units[self.unidade.0 as usize];
                if let Some(Resolved::Constructor(f)) = corpos.get_resolved(c) {
                    return Some(self.enriquecido(pelo_elemento(*f), *f));
                }
                let alvo = match &self.consulta.programa.unit(self.unidade).ast.expr(c).kind {
                    ExprKind::Call { target, .. } => Some(*target),
                    _ => None,
                };
                if let Some(a) = alvo {
                    if let Some(Resolved::Constructor(f)) = corpos.get_resolved(a) {
                        return Some(self.enriquecido(pelo_elemento(*f), *f));
                    }
                    let funcao = match corpos.get_resolved(a) {
                        Some(Resolved::Element(Element::Function(f))) => Some(*f),
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => Some(*f),
                        Some(Resolved::ExtensionMember { member, .. }) => Some(*member),
                        _ => None,
                    };
                    if let Some(t) = corpos.get_type(a)
                        && let Some(v) = pelo_tipo(t)
                    {
                        return Some(match funcao {
                            Some(f) => self.nomes_do_elemento(v, f),
                            None => v,
                        });
                    }
                }
                None
            }
            "Annotation" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "EnumConstantArguments" => {
                let f = self.construtor_invocado(pai)?;
                Some(self.enriquecido(pelo_elemento(f), f))
            }
            _ => None,
        }
    }

    /// Os parâmetros declarados de um executável no AST (o da unidade dele).
    pub(super) fn parametros_ast(&self, f: FunctionElementId) -> Option<(UnitId, &[ast::Parameter])> {
        let p = &self.consulta.programa;
        match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => Some((unit, p.unit(unit).ast.function(function).parameters.as_deref()?)),
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
                _ => None,
            },
            dartforge_elements::model::FunctionRef::None => None,
        }
    }

    /// Os nomes dos posicionais de um tipo de função escrito (`void
    /// Function(int a)`) ou da forma antiga (`void f(int a)`) de um
    /// parâmetro, na unidade `u` da consulta.
    pub(super) fn nomes_do_tipo_escrito(&self, u: UnitId, prm: &ast::Parameter) -> Vec<String> {
        let unidade = self.consulta.programa.unit(u);
        let nomes = |ps: &[ast::Parameter]| -> Vec<String> {
            ps.iter().filter(|x| x.kind != ast::ParameterKind::Named).map(|x| x.name.map(|n| self.consulta.nome(n.sym).to_string()).unwrap_or_default()).collect()
        };
        if let Some(fps) = &prm.function_parameters {
            return nomes(fps);
        }
        match prm.ty.map(|t| &unidade.ast.ty(t).kind) {
            Some(ast::TypeKind::Function { parameters, .. }) => nomes(parameters),
            _ => Vec::new(),
        }
    }

    /// Completa os parâmetros pelo elemento: `widget`, `@required` e os
    /// nomes do tipo de função escrito.
    pub(super) fn enriquecido(&self, mut v: Vec<ParametroInvocado>, f: FunctionElementId) -> Vec<ParametroInvocado> {
        let p = &self.consulta.programa;
        let widget = matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) && p.function(f).class.is_some_and(|c| self.e_widget(c));
        let declarados: Vec<(UnitId, &ast::Parameter)> = match self.parametros_ast(f) {
            Some((u, ps)) => ps.iter().filter(|x| x.nome_externo().is_some()).map(|x| (u, x)).collect(),
            None => Vec::new(),
        };
        for (i, x) in v.iter_mut().enumerate() {
            x.widget = widget;
            if let Some(&(u, prm)) = declarados.get(i) {
                x.anotado_required = prm.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| self.consulta.nome(n.sym) == "required"));
                x.nomes_do_tipo = self.nomes_do_tipo_escrito(u, prm);
            }
        }
        v
    }

    /// Os nomes do tipo de função escrito nos parâmetros de `f` (os
    /// parâmetros vieram do tipo, na ordem: posicionais, opcionais,
    /// nomeados).
    pub(super) fn nomes_do_elemento(&self, mut v: Vec<ParametroInvocado>, f: FunctionElementId) -> Vec<ParametroInvocado> {
        let Some((u, ps)) = self.parametros_ast(f) else { return v };
        let posicionais: Vec<&ast::Parameter> = ps.iter().filter(|x| x.kind != ast::ParameterKind::Named).collect();
        let mut k = 0usize;
        for x in v.iter_mut() {
            if x.nomeado {
                if let Some(prm) = ps.iter().find(|q| q.kind == ast::ParameterKind::Named && q.nome_externo().is_some_and(|n| self.consulta.nome(n.sym) == x.nome)) {
                    x.nomes_do_tipo = self.nomes_do_tipo_escrito(u, prm);
                    x.anotado_required = prm.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| self.consulta.nome(n.sym) == "required"));
                }
            } else {
                if let Some(prm) = posicionais.get(k) {
                    x.nomes_do_tipo = self.nomes_do_tipo_escrito(u, prm);
                }
                k += 1;
            }
        }
        v
    }

    /// `ClassElement.isWidget`: a classe é (subtipo de) `Widget` do Flutter.
    pub(super) fn e_widget(&self, c: ClassId) -> bool {
        let p = &self.consulta.programa;
        let mut pilha = vec![c];
        let mut vistas = HashSet::new();
        while let Some(x) = pilha.pop() {
            if !vistas.insert(x) {
                continue;
            }
            let cl = p.class(x);
            if self.consulta.nome(cl.name) == "Widget" && p.library(cl.library).uri == "package:flutter/src/widgets/framework.dart" {
                return true;
            }
            pilha.extend(cl.supertype_class);
            pilha.extend(cl.mixin_classes.iter().copied());
            pilha.extend(cl.interface_classes.iter().copied());
        }
        false
    }

    /// O construtor que uma anotação, `super(...)`, `this(...)` ou os
    /// argumentos de uma constante de enum invocam.
    pub(super) fn construtor_invocado(&self, n: usize) -> Option<FunctionElementId> {
        let p = &self.consulta.programa;
        match self.especie(n) {
            "SuperConstructorInvocation" | "RedirectingConstructorInvocation" => {
                let ctor = self.pai(n)?;
                let conteiner = self.pai(ctor)?;
                let classe = self.classe_do_no(conteiner)?;
                let (ast::Initializer::Super { constructor, .. } | ast::Initializer::Redirect { constructor, .. }) = self.inicializador_real(n)? else {
                    return None;
                };
                let alvo = if self.especie(n) == "SuperConstructorInvocation" { p.class(classe).supertype_class? } else { classe };
                let nome = constructor.map(|k| self.nome_real(k).to_string()).unwrap_or_default();
                p.class(alvo).construtores().into_iter().find(|(s, _)| self.consulta.nome(*s) == nome).map(|(_, f)| f)
            }
            "EnumConstantArguments" => {
                let constante = self.pai(n)?;
                let e = self.pai(constante)?;
                let classe = self.classe_do_no(e)?;
                let nome = self.filho(n, "ConstructorSelector").map(|s| self.fonte[self.ini(s)..self.fim(s)].trim_start_matches('.').trim().to_string()).unwrap_or_default();
                p.class(classe).construtores().into_iter().find(|(s, _)| self.consulta.nome(*s) == nome).map(|(_, f)| f)
            }
            "Annotation" => {
                let texto = self.fonte[self.ini(n)..self.fim(n)].trim_start_matches('@');
                let nome: String = texto.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$' || *c == '.').collect();
                let partes: Vec<&str> = nome.split('.').collect();
                let lib = p.library(self.biblioteca());
                let classe = partes.first().and_then(|n0| self.consulta.nomes.lookup(n0)).and_then(|s| lib.scope.get(&s)).and_then(|b| match b.getter {
                    Some(Element::Class(c)) => Some(c),
                    _ => None,
                })?;
                let ctor = partes.get(1).copied().unwrap_or("");
                p.class(classe).construtores().into_iter().find(|(s, _)| self.consulta.nome(*s) == ctor).map(|(_, f)| f)
            }
            _ => None,
        }
    }
}

/// As opções de `_forTypeAnnotation`.
#[derive(Debug, Clone, Default)]
pub(super) struct TipoOpcoes {
    pub estensivel: bool,
    pub implementavel: bool,
    pub misturavel: bool,
    pub nao_void: bool,
    pub excluir_nomes_de_tipo: bool,
    pub excluidos: Vec<usize>,
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- Os `visit*` -------------------------------------------------------------

    pub(super) fn v_anotacao(&mut self, n: usize) {
        self.local("Annotation_name");
        self.para_anotacao(n);
        // `@over^` seguido de linha em branco: pedido de sobrescrita.
        let Some(m) = self.anotacao_real(n) else { return };
        let sem_construtor = m.name.len() == 1 && m.arguments.is_none();
        if !sem_construtor {
            return;
        }
        let Some(decl) = self.pai(n) else { return };
        let Some(conteiner) = self.pai(decl) else { return };
        if !super::isp::e_declaracao(self.especie(conteiner)) {
            return;
        }
        let nome = m.name[0];
        let Some(id) = self.tok_em(nome.span.start) else { return };
        let Some(prox) = self.tok_depois(nome.span.end) else { return };
        let linha = |o: usize| self.fonte[..o.min(self.fonte.len())].matches('\n').count();
        if linha(self.span_tok(prox).start) > linha(nome.span.start) + 1 {
            self.tentar_anotacao_override(id, conteiner);
        }
    }

    /// O `ast::Annotation` de um nó `Annotation`.
    pub(super) fn anotacao_real(&self, n: usize) -> Option<&'a ast::Annotation> {
        let Ligacao::Anotacao(end) = self.lig(n) else { return None };
        let todos = self
            .a
            .decls
            .iter()
            .flat_map(|d| d.metadata.iter())
            .chain(self.a.members.iter().flat_map(|m| m.metadata.iter()))
            .chain(self.cu.directives.iter().flat_map(|d| d.metadata.iter()))
            .chain(self.a.metadados_locais.iter().flat_map(|(_, m)| m.iter()))
            .chain(self.a.functions.iter().flat_map(|f| f.parameters.iter().flatten()).flat_map(|p| p.metadata.iter()))
            .chain(self.a.members.iter().flat_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) => k.parameters.iter().flat_map(|p| p.metadata.iter()).collect::<Vec<_>>(),
                _ => Vec::new(),
            }));
        for m in todos {
            if crate::arvore_analyzer::endereco(m) == end {
                return Some(m);
            }
        }
        None
    }

    pub(super) fn v_lista_de_argumentos(&mut self, n: usize) {
        let abre = self.ini(n);
        let fecha = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::RParen)).map_or(self.fim(n), |i| self.span_tok(i).start);
        if self.offset <= abre {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
            return;
        }
        if self.offset > fecha {
            self.local("ExpressionStatement_expression");
            return;
        }
        let Some(pai) = self.pai(n) else { return };
        let l = self.local_de_argumentos(n, false);
        self.local(&l);
        let argumentos: Vec<usize> = self.filhos(n).to_vec();
        // `argumentsBeforeAndAfterOffset`.
        let mut antes = None;
        let mut depois = None;
        for &a in &argumentos {
            if self.offset < self.ini(a) || (self.offset == self.ini(a) && antes.is_some_and(|p| self.fim(p) == self.offset)) {
                depois = Some(a);
                break;
            }
            antes = Some(a);
        }
        let mut indice = 0usize;
        if let Some(b) = antes {
            if self.tratou_possivel_closure(b) {
                self.para_expressao(b, false, true);
                return;
            }
            indice = argumentos.iter().position(|&k| k == b).unwrap_or(0);
            if self.offset > self.fim(b) {
                indice += 1;
            }
        }
        // `argumentContext(indice)`.
        let mut posicionais = 0usize;
        let mut usados: HashSet<String> = HashSet::new();
        for (i, &a) in argumentos.iter().enumerate() {
            if self.especie(a) == "NamedExpression" {
                if let Some(r) = self.filho(a, "Label") {
                    usados.insert(self.fonte[self.ini(r)..self.fim(r)].trim_end_matches(':').trim().to_string());
                }
            } else if i < indice {
                posicionais += 1;
            }
        }
        match self.parametros_invocados(n) {
            Some(parametros) => {
                let mut contagem_posicional = 0usize;
                let mut disponiveis = Vec::new();
                for p in &parametros {
                    if p.nomeado {
                        if !usados.contains(&p.nome) {
                            disponiveis.push(p.clone());
                        }
                    } else {
                        contagem_posicional += 1;
                    }
                }
                if posicionais < contagem_posicional {
                    self.para_expressao(pai, false, true);
                    let p = parametros[posicionais].clone();
                    if matches!(self.consulta.tabela.get(p.tipo), Type::Function { .. }) {
                        let argumento = argumentos.get(indice).copied();
                        let virgula = argumento.is_none_or(|a| !self.seguido_de_virgula(a));
                        self.sugerir_closure_com_nomes(p.tipo, &p.nomes_do_tipo, virgula);
                    }
                } else {
                    let l = self.local_de_argumentos(n, true);
                    self.local(&l);
                }
                // A vírgula depois do nome sugerido (sem tokens sintéticos: a
                // vírgula está no texto ou não está).
                let mut virgula = false;
                if let Some(d) = depois {
                    let possivel = self.tok_antes(self.ini(d));
                    match possivel {
                        Some(i) if self.e_op(i, Op::Comma) => {
                            if self.offset >= self.span_tok(i).end {
                                virgula = true;
                            }
                        }
                        _ => virgula = true,
                    }
                } else if self.especie(pai) == "InstanceCreationExpression"
                    && let Some(e) = self.expr_real(pai)
                    && let Some(t) = self.tipo_estatico(e)
                    && let Type::Interface { class, .. } = self.consulta.tabela.get(t)
                    && self.e_widget(*class)
                {
                    // `parent.isWidgetCreation`.
                    virgula = true;
                }
                let mut substituir = None;
                if antes.is_some_and(|b| self.offset == self.ini(b)) {
                    substituir = Some(0usize);
                    virgula = false;
                }
                for p in disponiveis {
                    self.sugerir_argumento_nomeado(&p, true, virgula, substituir);
                }
            }
            None => {
                if super::isp::ESPECIES_DE_EXPRESSAO.contains(&self.especie(pai)) {
                    self.para_expressao(pai, false, true);
                }
            }
        }
    }

    /// `isFollowedByComma`.
    pub(super) fn seguido_de_virgula(&self, n: usize) -> bool {
        self.tok_depois(self.fim(n)).is_some_and(|i| self.e_op(i, Op::Comma))
    }

    pub(super) fn v_as(&mut self, n: usize) {
        let Some(&expr) = self.filhos(n).first() else { return };
        if self.offset <= self.fim(expr) {
            let estatico = self.contexto_estatico(n);
            self.dh(Flags { nao_void: true, estatico, ..Flags::default() });
            self.dh_lexicas(n);
            return;
        }
        if let Some(op) = self.palavra_em("as", self.fim(expr), self.fim(n)) {
            let s = self.span_tok(op);
            if self.cobre(s.start, s.end) {
                if self.especie(expr) == "ParenthesizedExpression" {
                    self.kw_modificadores_de_corpo(None);
                } else {
                    self.kw("as");
                }
                return;
            }
        }
        let tipo = self.filhos(n).get(1).copied();
        let cobre_inicio = tipo.is_some_and(|t| self.tok_depois(self.ini(t)).is_some_and(|i| self.cobre(self.span_tok(i).start, self.span_tok(i).end)));
        if tipo.is_none() || cobre_inicio {
            self.local("AsExpression_type");
            self.para_tipo(n, TipoOpcoes { nao_void: true, ..TipoOpcoes::default() });
        }
    }

    pub(super) fn v_assert_inicializador(&mut self, n: usize) {
        let abre = self.op_em(Op::LParen, self.ini(n), self.fim(n));
        if let Some(a) = abre
            && self.span_tok(a).end <= self.offset
        {
            let virgula = self.op_em(Op::Comma, self.span_tok(a).end, self.fim(n));
            let condicao = self.filhos(n).first().copied().unwrap_or(n);
            if virgula.is_none_or(|v| self.offset <= self.span_tok(v).start) {
                self.local("AssertInitializer_condition");
            } else {
                self.local("AssertInitializer_message");
            }
            self.para_expressao(condicao, false, false);
            return;
        }
        self.local("ConstructorDeclaration_initializer");
        if let Some(ctor) = self.pai(n) {
            self.kw_inicializador_de_construtor(ctor, Some(n));
        }
    }

    pub(super) fn v_assert(&mut self, n: usize) {
        if let Some(a) = self.op_em(Op::LParen, self.ini(n), self.fim(n))
            && self.span_tok(a).end <= self.offset
        {
            let virgula = self.op_em(Op::Comma, self.span_tok(a).end, self.fim(n));
            let condicao = self.filhos(n).first().copied().unwrap_or(n);
            if virgula.is_none_or(|v| self.offset <= self.span_tok(v).start) {
                self.local("AssertStatement_condition");
            } else {
                self.local("AssertStatement_message");
            }
            self.para_expressao(condicao, false, false);
            return;
        }
        if let Some(i) = self.tok_depois(self.ini(n))
            && self.offset <= self.span_tok(i).end
        {
            self.local("Block_statement");
            self.para_comando(n);
        }
    }

    pub(super) fn v_binaria(&mut self, n: usize) {
        let (Some(&l), Some(&r)) = (self.filhos(n).first(), self.filhos(n).get(1)) else { return };
        let op = self.fonte[self.fim(l)..self.ini(r)].trim().to_string();
        self.local(&format!("BinaryExpression_{op}_rightOperand"));
        self.para_expressao(n, false, true);
    }

    pub(super) fn v_bloco(&mut self, n: usize) {
        let esquerda = self.tok_em(self.ini(n)).filter(|&i| self.e_op(i, Op::LBrace));
        if esquerda.is_none() {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
        }
        if self.offset <= self.ini(n) {
            if let Some(p) = self.pai(n)
                && self.especie(p) == "BlockFunctionBody"
                && let Some(pp) = self.pai(p)
            {
                self.visitar(pp);
            }
            return;
        }
        self.local("Block_statement");
        let comandos: Vec<usize> = self.filhos(n).to_vec();
        if let Some(anterior) = self.elemento_antes(&comandos) {
            match self.especie(anterior) {
                "TryStatement" => {
                    if self.palavra_em("finally", self.ini(anterior), self.fim(anterior)).is_none() {
                        self.kw("on");
                        self.kw("catch");
                        self.kw("finally");
                        if self.filhos_de(anterior, &["CatchClause"]).is_empty() {
                            return;
                        }
                    }
                }
                "IfStatement" => {
                    if self.palavra_em("else", self.ini(anterior), self.fim(anterior)).is_none() {
                        self.kw("else");
                    }
                }
                _ => {}
            }
        }
        self.para_comando(n);
        if self.em_catch(n) {
            self.kw("rethrow");
        }
    }

    pub(super) fn v_break_continue(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        let fim_palavra = self.span_tok(palavra).end;
        let e_break = self.especie(n) == "BreakStatement";
        if self.offset <= fim_palavra {
            self.local("Block_statement");
            self.kw(if e_break { "break" } else { "continue" });
        } else {
            let pv = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::Semicolon)).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
            if self.offset <= pv {
                self.rotulos(n);
            }
        }
    }

    pub(super) fn v_padrao_cast(&mut self, n: usize) {
        if let Some(i) = self.palavra_em("as", self.ini(n), self.fim(n))
            && self.cobre(self.span_tok(i).start, self.span_tok(i).end)
        {
            self.kw("as");
        } else {
            self.local("CastPattern_type");
            self.para_tipo(n, TipoOpcoes { nao_void: true, ..TipoOpcoes::default() });
        }
    }

    pub(super) fn v_catch(&mut self, n: usize) {
        let on = self.palavra_em("on", self.ini(n), self.fim(n));
        let catch = self.palavra_em("catch", self.ini(n), self.fim(n));
        let corpo = self.filho(n, "Block").map_or(self.fim(n), |b| self.ini(b));
        if let Some(o) = on {
            if self.offset <= self.span_tok(o).end {
                self.kw("on");
            } else if catch.is_none() && self.offset <= corpo {
                self.local("CatchClause_exceptionType");
                self.para_tipo(n, TipoOpcoes::default());
            } else if let Some(c) = catch
                && self.offset < self.span_tok(c).start
            {
                self.local("CatchClause_exceptionType");
                self.para_tipo(n, TipoOpcoes::default());
            }
        }
        if let Some(c) = catch
            && self.offset >= self.span_tok(c).start
            && self.offset <= self.span_tok(c).end
        {
            self.kw("catch");
        }
    }

    pub(super) fn v_classe(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(d) = self.decl_real(n) else { return };
        let ast::DeclKind::Class(x) = &d.kind else { return };
        let class_kw = self.palavra_em("class", self.ini(n), x.name.span.start);
        let (esq, dir, esq_s, dir_s) = self.chaves(n);
        if let Some(k) = class_kw {
            if self.offset < self.span_tok(k).start {
                self.kw_modificadores_de_classe(n);
                return;
            }
            if self.offset <= self.span_tok(k).end {
                self.kw("class");
                return;
            }
        }
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.ih_nome_de_topo(esq_s && dir_s);
        } else if self.offset <= esq.start {
            self.kw_declaracao_de_classe(n);
        } else if self.offset >= esq.end && self.offset <= dir.start {
            if self.anotacao_no_fim_do_corpo(n) {
                return;
            }
            self.local("ClassDeclaration_member");
            let membros = self.filhos_de(n, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
            let anterior = self.elemento_antes(&membros);
            let tok = match anterior {
                Some(m) => self.tok_depois(self.ini(m)),
                None => self.tok_depois(esq.end),
            };
            if tok.is_some_and(|i| self.texto_tok(i) == "final") {
                self.para_tipo(n, TipoOpcoes::default());
                return;
            }
            self.para_membro_de_classe(n);
            if let Some(m) = anterior
                && self.especie(m) == "MethodDeclaration"
                && self.corpo_da_funcao(m).is_none_or(|c| self.especie(c) == "EmptyFunctionBody")
            {
                let corpo = self.corpo_da_funcao(m);
                self.kw_modificadores_de_corpo(corpo);
            }
        } else if let Some(p) = self.pai(n) {
            self.visitar(p);
        }
    }

    pub(super) fn v_unidade(&mut self, _n: usize) {
        let (antes, depois) = self.vizinhos_do_offset();
        if let Some(a) = antes
            && self.membro_de_unidade_incompleto(a)
        {
            return;
        }
        self.para_membro_de_unidade(antes, depois);
    }

    pub(super) fn v_condicional(&mut self, n: usize) {
        let (Some(&c), Some(&t)) = (self.filhos(n).first(), self.filhos(n).get(1)) else {
            self.para_expressao(n, false, false);
            return;
        };
        let interrogacao = self.op_em(Op::Question, self.fim(c), self.ini(t));
        let dois_pontos = self.op_em(Op::Colon, self.fim(t), self.fim(n));
        if let (Some(q), Some(d)) = (interrogacao, dois_pontos) {
            if self.offset >= self.span_tok(q).end && self.offset <= self.span_tok(d).start {
                self.local("ConditionalExpression_thenExpression");
            } else if self.offset >= self.span_tok(d).end {
                self.local("ConditionalExpression_elseExpression");
            }
        }
        self.para_expressao(n, false, false);
    }

    pub(super) fn v_construtor(&mut self, n: usize) {
        let Some(k) = self.construtor_real(n) else { return };
        if self.offset <= k.class_name.span.end {
            self.local("ClassDeclaration_member");
            if let Some(p) = self.pai(n) {
                self.para_membro_de_conteiner(p);
            }
            return;
        }
        let depois_dos_parametros = self.filho(n, "FormalParameterList").map_or(k.class_name.span.end, |l| self.fim(l));
        let separador = self.tok_depois(depois_dos_parametros).filter(|&i| self.e_op(i, Op::Colon) || self.e_op(i, Op::Assign));
        let Some(sep) = separador else { return };
        if self.e_op(sep, Op::Colon) {
            let corpo = self.corpo_da_funcao(n).map_or(self.fim(n), |c| self.ini(c));
            if self.offset >= self.span_tok(sep).end && self.offset <= corpo {
                self.local("ConstructorDeclaration_initializer");
                self.para_inicializador_de_construtor(n, None);
            }
        } else {
            let Some(f) = self.construtor_declarado(n) else { return };
            let constante = self.consulta.programa.function(f).const_;
            self.dh(Flags { constante, ..Flags::default() });
            self.dh_redirecionamentos(f);
        }
    }

    pub(super) fn v_inicializador_de_campo(&mut self, n: usize) {
        let Some(ctor) = self.pai(n) else { return };
        if self.especie(ctor) != "ConstructorDeclaration" {
            return;
        }
        let igual = self.op_em(Op::Assign, self.ini(n), self.fim(n));
        let pos_igual = igual.map_or(self.sintetico(self.ini(n)).start, |i| self.span_tok(i).start);
        if self.offset <= pos_igual {
            self.local("ConstructorDeclaration_initializer");
            self.para_inicializador_de_construtor(ctor, Some(n));
        } else {
            self.local("ConstructorFieldInitializer_expression");
            self.para_expressao(n, false, true);
        }
    }

    pub(super) fn v_nome_de_construtor(&mut self, n: usize) {
        let Some(tipo) = self.filho(n, "NamedType") else { return };
        let pai = self.pai(n);
        if pai.is_some_and(|p| self.especie(p) == "ConstructorReference") {
            if let Some(c) = self.classe_do_tipo_nomeado(tipo) {
                self.dh(Flags { preferir_sem_invocacao: true, ..Flags::default() });
                self.dh_nomes_de_construtor_do_elemento(c);
                self.dh_estaticos(Element::Class(c));
            }
            return;
        }
        let Some(t) = self.tipo_do_no_de_tipo(tipo) else { return };
        if !matches!(self.consulta.tabela.get(t), Type::Interface { .. } | Type::ExtensionType { .. }) {
            return;
        }
        if let Some(p) = pai
            && self.especie(p) == "ConstructorDeclaration"
            && let Some(k) = self.construtor_real(p)
            && k.factory
            && k.redirect.is_some()
        {
            let excluir = k.name.map(|x| self.nome_real(x).to_string());
            self.dh(Flags { constante: k.const_, preferir_sem_invocacao: true, ..Flags::default() });
            self.dh_nomes_de_construtor_do_tipo(t, excluir);
            return;
        }
        self.dh(Flags::default());
        self.dh_nomes_de_construtor_do_tipo(t, None);
    }

    /// A classe que um `NamedType` nomeia.
    pub(super) fn classe_do_tipo_nomeado(&self, t: usize) -> Option<ClassId> {
        match self.consulta.tabela.get(self.tipo_do_no_de_tipo(t)?) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            _ => None,
        }
    }

    pub(super) fn v_seletor_de_construtor(&mut self, n: usize) {
        self.local("ConstructorSelector_name");
        if !self.tem_recurso(super::isp::Recurso::EnhancedEnums) {
            return;
        }
        let Some(args) = self.pai(n).filter(|&a| self.especie(a) == "EnumConstantArguments") else { return };
        let Some(constante) = self.pai(args).filter(|&c| self.especie(c) == "EnumConstantDeclaration") else { return };
        let Some(e) = self.pai(constante).filter(|&e| self.especie(e) == "EnumDeclaration") else { return };
        let Some(classe) = self.classe_do_no(e) else { return };
        self.dh(Flags { sem_nome_como_new: true, ..Flags::default() });
        self.dh_nomes_de_construtor_do_elemento(classe);
    }

    pub(super) fn v_padrao_de_variavel_declarada(&mut self, n: usize) {
        let nome = self.nome_do_padrao_de_variavel(n);
        let tipo = self.filho(n, "NamedType").or_else(|| self.filhos(n).first().copied());
        let palavra = self.tok_depois(self.ini(n)).filter(|&i| matches!(self.texto_tok(i), "var" | "final"));
        let Some(nome) = nome else {
            if tipo.is_none() && palavra.is_none() {
                self.para_tipo(n, TipoOpcoes { nao_void: true, ..TipoOpcoes::default() });
                return;
            }
            self.para_nome_em_padrao_de_variavel(n);
            return;
        };
        let s = self.span_tok(nome);
        if self.cobre(s.start, s.end) {
            self.para_nome_em_padrao_de_variavel(n);
            return;
        }
        if palavra.is_some() {
            if tipo.is_none() && self.offset < s.start {
                self.dh(Flags { tipo: true, ..Flags::default() });
                self.dh_lexicas(n);
                return;
            }
        }
        let pai = self.pai(n);
        let tem_when = pai.is_some_and(|p| self.especie(p) == "GuardedPattern" && self.filho(p, "WhenClause").is_some());
        if !tem_when {
            self.kw("when");
        }
    }

    pub(super) fn v_parametro_padrao(&mut self, n: usize) {
        let valor = self.filhos(n).get(1).copied();
        if let Some(v) = valor
            && self.cobre_no(v)
        {
            self.local("DefaultFormalParameter_defaultValue");
            self.para_expressao(v, false, true);
        } else if let Some(&p) = self.filhos(n).first() {
            self.visitar(p);
        }
    }

    pub(super) fn v_do(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
            return;
        }
        let corpo = self.filhos(n).first().copied();
        let depois_do_corpo = corpo.map_or(self.span_tok(palavra).end, |c| self.fim(c));
        let Some(abre) = self.op_em(Op::LParen, depois_do_corpo, self.fim(n)) else { return };
        let fecha = self.par(abre).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(abre).end <= self.offset && self.offset <= fecha {
            let condicao = self.filhos(n).get(1).copied();
            let ok = match condicao {
                None => true,
                Some(c) => self.offset <= self.ini(c) || self.offset == self.fim(c),
            };
            if ok {
                self.local("DoStatement_condition");
                self.para_expressao(n, false, true);
            }
        }
    }

    pub(super) fn v_comando_vazio(&mut self, n: usize) {
        if let Some(p) = self.pai(n) {
            match self.especie(p) {
                "Block" => {
                    self.local("Block_statement");
                    let comandos: Vec<usize> = self.filhos(p).to_vec();
                    if let Some(i) = comandos.iter().position(|&k| k == n)
                        && i > 0
                    {
                        let anterior = comandos[i - 1];
                        if self.especie(anterior) == "TryStatement" && self.palavra_em("finally", self.ini(anterior), self.fim(anterior)).is_none() {
                            self.kw_clausulas_de_try(true);
                            if self.filhos_de(anterior, &["CatchClause"]).is_empty() {
                                return;
                            }
                        }
                    }
                }
                "IfStatement" => {
                    let entao = self.filhos(p).iter().copied().filter(|&k| super::isp::e_comando(self.especie(k))).next();
                    if entao == Some(n) {
                        self.local("IfStatement_thenStatement");
                    } else {
                        self.local("IfStatement_elseStatement");
                    }
                }
                _ => {}
            }
        }
        if self.offset <= self.ini(n) {
            self.para_comando(n);
        }
    }

    pub(super) fn v_enum(&mut self, n: usize) {
        if !self.tem_recurso(super::isp::Recurso::EnhancedEnums) {
            return;
        }
        let Some(d) = self.decl_real(n) else { return };
        let ast::DeclKind::Enum(x) = &d.kind else { return };
        let Some(enum_kw) = self.palavra_em("enum", self.ini(n), x.name.span.start) else { return };
        if self.offset < self.span_tok(enum_kw).start {
            return;
        }
        if self.offset <= self.span_tok(enum_kw).end {
            self.kw("enum");
            return;
        }
        let (esq, dir, esq_s, dir_s) = self.chaves(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.ih_nome_de_topo(esq_s && dir_s);
            return;
        }
        if self.offset <= esq.start {
            self.kw_declaracao_de_enum(n);
            return;
        }
        if !dir_s && self.offset >= dir.end {
            return;
        }
        // O `;` depois das constantes.
        let ultima_constante = self.filhos_de(n, &["EnumConstantDeclaration"]).last().copied();
        let pv = self.op_em(Op::Semicolon, ultima_constante.map_or(esq.end, |c| self.fim(c)), dir.start);
        if let Some(p) = pv
            && self.offset >= self.span_tok(p).end
        {
            self.local("EnumDeclaration_member");
            self.para_membro_de_enum(n);
        }
    }

    pub(super) fn v_corpo_de_expressao(&mut self, n: usize) {
        let Some(&expr) = self.filhos(n).first() else { return };
        let seta = self.op_em(Op::Arrow, self.ini(n), self.ini(expr));
        if let Some(s) = seta
            && self.offset >= self.span_tok(s).end
            && self.offset <= self.fim(expr)
        {
            self.local("ExpressionFunctionBody_expression");
            self.para_expressao(expr, false, false);
        }
    }

    pub(super) fn v_comando_de_expressao(&mut self, n: usize) {
        let pai = self.pai(n);
        if pai.is_some_and(|p| matches!(self.especie(p), "SwitchPatternCase" | "SwitchCase")) {
            self.local("SwitchMember_statement");
        } else {
            self.local("Block_statement");
        }
        if self.comando_incompleto_anterior(n) && self.comando_solto(n) {
            if let Some(a) = self.comando_anterior(n)
                && self.especie(a) == "TryStatement"
            {
                return;
            }
        }
        let pv = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::Semicolon));
        if let Some(p) = pv
            && self.offset >= self.span_tok(p).end
        {
            self.para_comando(n);
            return;
        }
        let Some(&expr) = self.filhos(n).first() else {
            self.para_expressao(n, false, false);
            return;
        };
        match self.especie(expr) {
            "AsExpression" | "IsExpression" => self.visitar(expr),
            "AssignmentExpression" => {
                if let Some(&lado) = self.filhos(expr).first()
                    && self.offset <= self.fim(lado)
                {
                    match self.especie(lado) {
                        "PrefixedIdentifier" => self.visitar(lado),
                        "SimpleIdentifier" => self.para_comando(n),
                        _ => {}
                    }
                }
            }
            "CascadeExpression" => {
                if let Some(&alvo) = self.filhos(expr).first()
                    && self.offset <= self.fim(alvo)
                {
                    let estatico = self.contexto_estatico(n);
                    self.dh(Flags { nao_void: true, estatico, ..Flags::default() });
                    self.dh_lexicas(n);
                }
            }
            "InstanceCreationExpression" | "MethodInvocation" => {
                if let Some(i) = self.tok_depois(self.ini(expr))
                    && self.offset <= self.span_tok(i).end
                {
                    self.para_comando(n);
                }
            }
            "FunctionReference" => {
                if self.offset > self.fim(expr)
                    && let Some(&f) = self.filhos(expr).first()
                    && self.especie(f) == "SimpleIdentifier"
                {
                    let nome = self.fonte[self.ini(f)..self.fim(f)].to_string();
                    self.ih(false);
                    self.ih_do_nome_de_tipo(&nome);
                }
            }
            "PrefixedIdentifier" => {
                let (Some(&p), Some(&id)) = (self.filhos(expr).first(), self.filhos(expr).get(1)) else { return };
                if self.offset <= self.fim(p) {
                    let estatico = self.contexto_estatico(n);
                    self.dh(Flags { nao_void: true, estatico, ..Flags::default() });
                    self.dh_lexicas(n);
                } else if self.offset <= self.fim(id) {
                } else {
                    let nome = self.fonte[self.ini(id)..self.fim(id)].to_string();
                    self.ih(false);
                    self.ih_do_nome_de_tipo(&nome);
                }
            }
            "SimpleIdentifier" => {
                if self.offset <= self.fim(expr) {
                    self.para_comando(n);
                } else {
                    let nome = self.fonte[self.ini(expr)..self.fim(expr)].to_string();
                    self.ih(false);
                    self.ih_do_nome_de_tipo(&nome);
                }
            }
            _ => self.para_expressao(n, false, false),
        }
    }

    /// `ExpressionStatement.isSingleIdentifier`: uma palavra sem `;` no
    /// texto.
    pub(super) fn comando_solto(&self, n: usize) -> bool {
        let Some(i) = self.tok_depois(self.ini(n)) else { return false };
        self.palavra(i) && self.tok_antes(self.fim(n)) == Some(i)
    }

    pub(super) fn v_extends(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.kw("extends");
            return;
        }
        let Some(t) = self.filho(n, "NamedType") else {
            self.local("ExtendsClause_superclass");
            self.para_tipo(n, TipoOpcoes { estensivel: true, ..TipoOpcoes::default() });
            return;
        };
        let nome_ini = self.filho(t, "ImportPrefixReference").map_or(self.ini(t), |p| self.fim(p));
        let nome = self.tok_depois(nome_ini);
        if nome.is_some_and(|i| self.cobre(self.span_tok(i).start, self.span_tok(i).end)) {
            self.local("ExtendsClause_superclass");
            self.para_tipo(n, TipoOpcoes { estensivel: true, ..TipoOpcoes::default() });
        }
    }

    pub(super) fn v_extensao(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(d) = self.decl_real(n) else { return };
        let ast::DeclKind::Extension(x) = &d.kind else { return };
        let Some(ext) = self.palavra_em("extension", self.ini(n), self.fim(n)) else { return };
        if self.offset < self.span_tok(ext).start {
            return;
        }
        if self.offset <= self.span_tok(ext).end {
            self.kw("extension");
            return;
        }
        match x.name {
            Some(nome) if self.offset <= nome.span.end => {
                self.kw("on");
                if self.tem_recurso(super::isp::Recurso::InlineClass) {
                    self.texto_anotado("type");
                }
                self.ih(false);
                self.ih_nome_de_topo(false);
                return;
            }
            _ => {
                self.ih(false);
                self.ih_nome_de_topo(false);
            }
        }
        let (esq, dir, _, _) = self.chaves(n);
        if self.offset <= esq.start {
            self.local("ExtensionDeclaration_onClause");
            // `onClause.onKeyword.isSynthetic`: a cláusula sem `on` escrito.
            let on = self.palavra_em("on", self.ini(n), esq.start);
            if self.filho(n, "ExtensionOnClause").is_some() && on.is_none() {
                self.kw_declaracao_de_extensao(n);
            }
            return;
        }
        if self.offset >= esq.end && self.offset <= dir.start {
            self.local("ExtensionDeclaration_member");
            self.para_membro_de_extensao(n);
        }
    }

    pub(super) fn v_on_de_extensao(&mut self, n: usize) {
        if let Some(on) = self.tok_depois(self.ini(n))
            && self.offset <= self.span_tok(on).end
        {
            self.kw("on");
            return;
        }
        self.local("ExtensionOnClause_extendedType");
        self.para_tipo(n, TipoOpcoes::default());
    }

    pub(super) fn v_tipo_de_extensao(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(d) = self.decl_real(n) else { return };
        let ast::DeclKind::ExtensionType(x) = &d.kind else { return };
        let (esq, dir, esq_s, dir_s) = self.chaves(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.ih_nome_de_topo(esq_s && dir_s);
        } else if self.offset >= x.representation_span.end && (self.offset <= esq.start || esq_s) {
            self.kw("implements");
        } else if self.offset >= esq.end && self.offset <= dir.start {
            self.local("ExtensionTypeDeclaration_member");
            self.para_membro_de_tipo_de_extensao(n);
        }
    }

    pub(super) fn v_campo(&mut self, n: usize) {
        self.membro_incompleto_anterior(n);
        if let Some(i) = self.primeiro_depois_das_anotacoes(n)
            && self.offset <= self.span_tok(i).end
        {
            self.local("ClassDeclaration_member");
        }
        let Some(lista) = self.filho(n, "VariableDeclarationList") else { return };
        let Some(l) = self.lista_real(lista) else { return };
        match l.ty {
            None => {
                let Some(primeira) = l.variables.first() else { return };
                self.local("FieldDeclaration_fields");
                let nome = primeira.name;
                let texto = self.nome_real(nome);
                let e_palavra = PALAVRAS_DO_ANALYZER_PUB.contains(&texto);
                if l.variables.len() == 1 && e_palavra && self.offset > nome.span.end {
                    self.kw_declaracao_de_campo(n, Some(texto));
                    self.dh(Flags { tipo: true, ..Flags::default() });
                    self.dh_lexicas(n);
                } else if self.offset < nome.span.start {
                    self.kw_declaracao_de_campo(n, None);
                    self.para_tipo(n, TipoOpcoes { nao_void: primeira.initializer.is_some(), ..TipoOpcoes::default() });
                } else if self.offset <= nome.span.end {
                    self.kw_declaracao_de_campo(n, None);
                }
            }
            Some(t) => {
                let ts = self.a.ty(t).span;
                let anterior = self.membro_anterior(n);
                if self.offset <= ts.start && anterior.is_none_or(|a| self.offset >= self.fim(a)) {
                    if let Some(p) = self.pai(n) {
                        self.para_membro_de_conteiner(p);
                    }
                } else if self.offset <= ts.end {
                    if self.campo_solto(n) {
                        if let Some(p) = self.pai(n) {
                            self.para_membro_de_conteiner(p);
                        }
                    } else {
                        self.kw_declaracao_de_campo(n, None);
                        self.local("ClassDeclaration_member");
                        self.kw("var");
                        self.dh(Flags { tipo: true, ..Flags::default() });
                        self.dh_lexicas(n);
                    }
                }
            }
        }
    }

    /// `FieldDeclaration.isSingleIdentifier`: uma palavra e nenhum outro
    /// token (o `;` sintético).
    pub(super) fn campo_solto(&self, n: usize) -> bool {
        let Some(i) = self.primeiro_depois_das_anotacoes(n) else { return false };
        self.palavra(i) && self.tok_antes(self.fim(n)) == Some(i)
    }

    pub(super) fn v_parametro_de_campo(&mut self, n: usize) {
        let mut ctor = self.pai(n).and_then(|p| self.pai(p));
        if ctor.is_some_and(|c| self.especie(c) == "FormalParameterList") {
            ctor = ctor.and_then(|c| self.pai(c));
        }
        let Some(c) = ctor.filter(|&c| self.especie(c) == "ConstructorDeclaration") else { return };
        let campo = self.parametro_real(n).and_then(|p| p.name).and_then(|nome| {
            let classe = self.pai(c).and_then(|k| self.classe_do_no(k))?;
            let texto = self.nome_real(nome);
            let pr = &self.consulta.programa;
            pr.class(classe).fields.iter().copied().find(|&v| self.consulta.nome(pr.variable(v).name) == texto)
        });
        self.dh(Flags::default());
        self.dh_campos_para_inicializadores(c, campo);
    }

    pub(super) fn v_lista_de_parametros(&mut self, n: usize) {
        if self.offset >= self.fim(n)
            && let Some(p) = self.pai(n)
            && self.especie(p) == "FunctionExpression"
        {
            self.v_expressao_de_funcao(p);
            return;
        }
        self.local("FormalParameterList_parameter");
        let parametros: Vec<usize> = self.filhos(n).to_vec();
        if let Some(anterior) = self.elemento_antes(&parametros) {
            if self.parametro_incompleto(anterior) {
                self.visitar(anterior);
                return;
            }
            if self.especie(anterior) == "SimpleFormalParameter"
                && let Some(p) = self.parametro_real(anterior)
                && p.ty.is_none()
                && self.offset > self.fim(anterior)
                && let Some(nome) = p.name
            {
                let texto = self.nome_real(nome).to_string();
                self.ih(false);
                self.ih_do_nome_de_tipo(&texto);
            }
        }
        self.kw_parametro_formal(n);
        self.para_tipo(n, TipoOpcoes::default());
    }

    /// `FormalParameter.isIncomplete`.
    pub(super) fn parametro_incompleto(&self, n: usize) -> bool {
        let Some(p) = self.parametro_real(n) else { return false };
        match p.name {
            None => return true,
            Some(nome) => {
                if PALAVRAS_DO_ANALYZER_PUB.contains(&self.nome_real(nome)) {
                    return true;
                }
            }
        }
        if self.especie(n) == "DefaultFormalParameter" {
            let separador = self.op_em(Op::Assign, self.ini(n), self.fim(n).max(self.ini(n) + 1)).or_else(|| self.op_em(Op::Colon, self.ini(n), self.fim(n).max(self.ini(n) + 1)));
            if separador.is_some() && p.default_value.is_none() {
                return true;
            }
        }
        false
    }

    pub(super) fn v_partes_de_for_com_declaracoes(&mut self, n: usize) {
        let partes = self.arv.partes_de_for.get(&n).cloned().unwrap_or_default();
        let lista = partes.inicio;
        let depois_da_lista = lista.map_or(self.ini(n), |l| self.fim(l));
        let p1 = self.op_em(Op::Semicolon, depois_da_lista, self.fim(n).max(self.offset + 1));
        let p2 = p1.and_then(|a| self.op_em(Op::Semicolon, self.span_tok(a).end, usize::MAX));
        let fim_p1 = p1.map_or(self.sintetico(depois_da_lista).end, |i| self.span_tok(i).end);
        let ini_p2 = p2.map_or(self.sintetico(fim_p1).start, |i| self.span_tok(i).start);
        if self.offset >= fim_p1 && self.offset <= ini_p2 {
            self.local("ForParts_condition");
            if let Some(c) = partes.condicao
                && self.especie(c) == "SimpleIdentifier"
                && p1.is_none()
                && p2.is_none()
            {
                self.kw("in");
                return;
            }
            self.para_expressao(n, false, false);
        } else if p2.is_some_and(|i| self.offset >= self.span_tok(i).end) {
            self.local("ForParts_updater");
            self.para_expressao(n, false, false);
        }
    }

    pub(super) fn v_for(&mut self, n: usize) {
        let Some(palavra) = self.palavra_em("for", self.ini(n), self.fim(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
            return;
        }
        let abre = self.op_em(Op::LParen, self.span_tok(palavra).end, self.fim(n));
        let fecha = abre.and_then(|a| self.par(a));
        let (ab, fe) = match (abre, fecha) {
            (Some(a), Some(f)) => (self.span_tok(a).end, self.span_tok(f).start),
            (Some(a), None) => (self.span_tok(a).end, self.sintetico(self.fim(n)).start),
            _ => (usize::MAX, 0),
        };
        if self.offset >= ab && self.offset <= fe {
            self.local("ForStatement_forLoopParts");
            let Some(&partes) = self.filhos(n).first() else { return };
            match self.especie(partes) {
                "ForEachPartsWithDeclaration" => {
                    if let Some(d) = self.filho(partes, "DeclaredIdentifier") {
                        let nome = self.tok_antes(self.fim(d)).unwrap_or(0);
                        if self.offset < self.span_tok(nome).start {
                            let tipo = self.filho(d, "NamedType").or_else(|| self.filhos_de(d, &["GenericFunctionType", "RecordTypeAnnotation"]).first().copied());
                            let ok = match tipo {
                                None => true,
                                Some(t) if self.especie(t) == "NamedType" => {
                                    let nome_t = self.tok_depois(self.filho(t, "ImportPrefixReference").map_or(self.ini(t), |p| self.fim(p)));
                                    nome_t.is_some_and(|i| self.offset <= self.span_tok(i).end)
                                }
                                _ => false,
                            };
                            if ok {
                                self.para_tipo(n, TipoOpcoes::default());
                            }
                        }
                    }
                }
                "ForEachPartsWithIdentifier" => {
                    if let Some(&id) = self.filhos(partes).first()
                        && self.offset < self.ini(id)
                    {
                        self.para_tipo(n, TipoOpcoes::default());
                    }
                }
                "ForEachPartsWithPattern" | "ForPartsWithPattern" => {}
                "ForPartsWithDeclarations" => {
                    if let Some(lista) = self.filho(partes, "VariableDeclarationList")
                        && let Some(l) = self.lista_real(lista)
                    {
                        let palavra_lista = self.tok_depois(self.ini(lista)).filter(|&i| matches!(self.texto_tok(i), "var" | "final" | "const"));
                        let p1 = self.op_em(Op::Semicolon, self.fim(lista), self.fim(partes).max(self.offset + 1));
                        if l.variables.len() == 1 && palavra_lista.is_some() && p1.is_none() {
                            let depois = palavra_lista.map(|i| i + 1);
                            if let Some(d) = depois
                                && self.e_op(d, Op::LParen)
                                && let Some(f) = self.par(d)
                                && self.offset >= self.span_tok(f).end
                            {
                                self.kw("in");
                            }
                        }
                        if let Some(t) = l.ty {
                            let ts = self.a.ty(t).span;
                            if self.cobre(ts.start, ts.end)
                                && let Some(no_t) = self.filhos(lista).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(t))
                            {
                                self.visitar(no_t);
                            }
                        }
                    }
                }
                "ForPartsWithExpression" => {
                    let p1 = self.op_em(Op::Semicolon, self.ini(partes), self.fim(partes).max(self.offset + 1));
                    let inicializacao = self.arv.partes_de_for.get(&partes).and_then(|p| p.inicio);
                    if p1.is_none() && inicializacao.is_some_and(|i| self.especie(i) == "SimpleIdentifier") {
                        self.kw("final");
                        self.kw("var");
                        self.para_tipo(n, TipoOpcoes::default());
                    }
                }
                _ => {}
            }
        } else {
            self.local("ForStatement_body");
            self.para_comando(n);
        }
    }

    pub(super) fn v_declaracao_de_funcao(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
        }
        let Some(f) = self.funcao_real(n) else { return };
        let Some(nome) = f.name else { return };
        let um_token = match f.return_type {
            None => true,
            Some(r) => {
                let s = self.a.ty(r).span;
                self.tok_depois(s.start).is_some_and(|i| self.span_tok(i).end == s.end)
            }
        };
        if um_token && self.offset <= nome.span.start {
            self.local("FunctionDeclaration_returnType");
            self.para_tipo(n, TipoOpcoes::default());
        }
    }

    pub(super) fn v_expressao_de_funcao(&mut self, n: usize) {
        let parametros = self.filho(n, "FormalParameterList").map(|p| self.fim(p));
        let tipos = self.filho(n, "TypeParameterList").map(|p| self.fim(p));
        let inicio = parametros.or(tipos).unwrap_or(self.ini(n));
        let corpo = self.corpo_da_funcao(n);
        let ini_corpo = corpo.map_or(self.fim(n), |c| self.ini(c));
        if self.offset >= inicio && self.offset <= ini_corpo {
            self.kw_modificadores_de_corpo(corpo);
            let declaracao = self.pai(n);
            let unidade = declaracao.and_then(|d| self.pai(d));
            if corpo.is_some_and(|c| self.especie(c) == "EmptyFunctionBody")
                && declaracao.is_some_and(|d| self.especie(d) == "FunctionDeclaration")
                && unidade == Some(0)
            {
                self.para_declaracao_de_unidade();
            }
        }
    }

    pub(super) fn v_typedef_antigo(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(td) = self.palavra_em("typedef", self.ini(n), self.fim(n)) else { return };
        if self.offset <= self.span_tok(td).end {
            self.local("CompilationUnit_declaration");
            self.kw("typedef");
        } else if self.toks.get(td + 1).is_some_and(|t| self.offset <= t.span.end) {
            self.dh(Flags { tipo: true, ..Flags::default() });
            self.dh_lexicas(n);
        }
    }

    pub(super) fn v_parametro_de_funcao(&mut self, n: usize) {
        self.local("FormalParameterList_parameter");
        let Some(p) = self.parametro_real(n) else { return };
        match p.ty {
            Some(r) if self.offset <= self.a.ty(r).span.end => {
                if let Some(lista) = self.lista_de_parametros_de(n) {
                    self.kw_parametro_formal(lista);
                }
                self.para_tipo(n, TipoOpcoes::default());
            }
            None if p.name.is_some_and(|nm| self.offset < nm.span.start) => self.para_tipo(n, TipoOpcoes::default()),
            _ => {}
        }
    }

    /// `parentFormalParameterList`.
    pub(super) fn lista_de_parametros_de(&self, n: usize) -> Option<usize> {
        let mut p = self.pai(n)?;
        if self.especie(p) == "DefaultFormalParameter" {
            p = self.pai(p)?;
        }
        (self.especie(p) == "FormalParameterList").then_some(p)
    }

    pub(super) fn v_typedef(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        if let Some(td) = self.palavra_em("typedef", self.ini(n), self.fim(n))
            && self.cobre(self.span_tok(td).start, self.span_tok(td).end)
        {
            self.kw("typedef");
            return;
        }
        let igual = self.op_em(Op::Assign, self.ini(n), self.fim(n));
        let pv = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::Semicolon)).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if let Some(i) = igual
            && self.offset >= self.span_tok(i).end
            && self.offset <= pv
        {
            self.local("GenericTypeAlias_type");
            self.para_tipo(n, TipoOpcoes::default());
        }
    }

    pub(super) fn v_if_elemento(&mut self, n: usize) {
        let Some(&expr) = self.filhos(n).first() else { return };
        let abre = self.op_em(Op::LParen, self.ini(n), self.ini(expr) + 1);
        let fecha = abre.and_then(|a| self.par(a));
        let fe = fecha.map_or(self.sintetico(self.fim(expr)).start, |i| self.span_tok(i).start);
        let case = self.filho(n, "CaseClause");
        if self.offset > self.fim(expr) && self.offset <= fe {
            match case {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let gp = self.filho(c, "GuardedPattern");
                    if gp.is_some_and(|g| self.tem_when(g)) {
                        let sem_expressao = gp.and_then(|g| self.filho(g, "WhenClause")).is_none_or(|w| self.filhos(w).is_empty());
                        if sem_expressao {
                            let estatico = self.contexto_estatico(n);
                            self.kw_expressao(Some(n), false, estatico);
                        }
                    } else {
                        self.kw("when");
                    }
                }
            }
        } else if abre.is_some_and(|a| self.offset >= self.span_tok(a).end) && self.offset <= fe {
            self.local("IfElement_condition");
            self.para_expressao(n, false, true);
        } else if fecha.is_some_and(|f| self.offset >= self.span_tok(f).end) {
            let senao = self.palavra_em("else", self.span_tok(fecha.unwrap_or(0)).end, self.fim(n));
            if senao.is_none_or(|e| self.offset <= self.span_tok(e).start) {
                self.local("IfElement_thenElement");
            } else {
                self.local("IfElement_elseElement");
            }
            if let Some(lit) = self.ancestral(n, &["ListLiteral", "SetOrMapLiteral"]) {
                let elementos = self.elementos_do_literal(lit);
                self.para_elemento_de_colecao(lit, &elementos);
            }
        }
    }

    /// `GuardedPattern.hasWhen`.
    pub(super) fn tem_when(&self, gp: usize) -> bool {
        if self.filho(gp, "WhenClause").is_some() {
            return true;
        }
        if let Some(&p) = self.filhos(gp).first()
            && self.especie(p) == "DeclaredVariablePattern"
            && let Some(nome) = self.nome_do_padrao_de_variavel(p)
            && self.texto_tok(nome) == "when"
            && let Some(t) = self.filho(p, "NamedType")
        {
            return self.filho(t, "TypeArgumentList").is_none();
        }
        false
    }

    pub(super) fn v_if(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        let abre = self.op_em(Op::LParen, self.span_tok(palavra).end, self.fim(n));
        let fecha = abre.and_then(|a| self.par(a));
        if abre.is_some() && fecha.is_none() {
            self.kw("is");
            return;
        }
        let Some(&expr) = self.filhos(n).first() else { return };
        let fe = fecha.map_or(self.sintetico(self.fim(expr)).start, |i| self.span_tok(i).start);
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
        } else if self.offset > self.fim(expr) && self.offset <= fe {
            self.local("IfStatement_condition");
            match self.filho(n, "CaseClause") {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let gp = self.filho(c, "GuardedPattern");
                    if gp.is_some_and(|g| self.tem_when(g)) {
                        let sem_expressao = gp.and_then(|g| self.filho(g, "WhenClause")).is_none_or(|w| self.filhos(w).is_empty());
                        if sem_expressao {
                            self.para_expressao(n, false, false);
                        }
                    } else {
                        self.kw("when");
                        if let Some(g) = gp
                            && let Some(&p) = self.filhos(g).first()
                            && self.especie(p) == "ConstantPattern"
                            && let Some(&e) = self.filhos(p).first()
                            && matches!(self.especie(e), "SimpleIdentifier" | "PrefixedIdentifier" | "TypeLiteral")
                            && self.fim(e) < self.offset
                            && self.e_literal_de_tipo(e)
                        {
                            let nome = self.fonte[self.ini(e)..self.fim(e)].rsplit('.').next().unwrap_or("").to_string();
                            self.ih(false);
                            self.ih_do_nome_de_tipo(&nome);
                        }
                    }
                }
            }
        } else if abre.is_some_and(|a| self.offset >= self.span_tok(a).end) && self.offset <= fe {
            self.local("IfStatement_condition");
            self.para_expressao(n, false, true);
        } else if fecha.is_some_and(|f| self.offset >= self.span_tok(f).end) {
            let senao = fecha.and_then(|f| self.palavra_em("else", self.span_tok(f).end, self.fim(n)));
            if senao.is_none_or(|e| self.offset <= self.span_tok(e).start) {
                self.local("IfStatement_thenStatement");
            } else {
                self.local("IfStatement_elseStatement");
            }
            self.para_comando(n);
        }
    }

    /// A expressão é um `TypeLiteral` (o nome resolve a um tipo).
    pub(super) fn e_literal_de_tipo(&self, e: usize) -> bool {
        let Some(id) = self.expr_real(e) else { return false };
        matches!(self.resolvido(id), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))))
    }

    pub(super) fn v_implements(&mut self, n: usize) {
        if let Some(p) = self.tok_depois(self.ini(n))
            && self.offset <= self.span_tok(p).end
        {
            self.kw("implements");
        } else {
            self.local("ImplementsClause_interface");
            self.para_tipo(n, TipoOpcoes { implementavel: true, ..TipoOpcoes::default() });
        }
    }

    pub(super) fn v_import(&mut self, n: usize) {
        let Some(palavra) = self.palavra_em("import", self.ini(n), self.fim(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("CompilationUnit_directive");
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(uri) = self.filhos_de(n, &["SimpleStringLiteral", "StringInterpolation", "AdjacentStrings"]).first().copied() else { return };
        if self.offset <= self.ini(uri) {
            return;
        }
        if self.offset >= self.fim(uri) {
            self.local("CompilationUnit_directive");
            self.kw_diretiva_import(n);
        }
    }

    pub(super) fn v_referencia_de_prefixo(&mut self, n: usize) {
        let Some(pai) = self.pai(n) else { return };
        if self.especie(pai) != "NamedType" {
            return;
        }
        let nome_ini = self.fim(n);
        let nome = self.tok_depois(nome_ini).filter(|&i| self.span_tok(i).start < self.fim(pai).max(nome_ini + 1));
        let nome_off = nome.map_or(self.sintetico(nome_ini).start, |i| self.span_tok(i).start);
        if self.offset > nome_off {
            return;
        }
        self.local("PropertyAccess_propertyName");
        if let Some(p) = self.prefixo_de_import(n) {
            let criacao = self.pai(pai).and_then(|x| self.pai(x)).is_some_and(|x| self.especie(x) == "InstanceCreationExpression");
            self.dh(Flags { excluir_nomes_de_tipo: criacao, tipo: !criacao, nao_void: criacao, ..Flags::default() });
            self.dh_pelo_prefixo(p);
            return;
        }
        // Não é prefixo: o "prefixo" é um valor (`x.^ y`) ou um tipo.
        let texto = self.fonte[self.ini(n)..self.fim(n)].trim_end_matches('.').trim().to_string();
        let Some(s) = self.consulta.nomes.lookup(&texto) else { return };
        let lib = self.consulta.programa.library(self.biblioteca());
        let el = lib.scope.get(&s).and_then(|b| b.getter);
        match el {
            Some(Element::Class(c)) => {
                self.dh(Flags::default());
                self.dh_estaticos(Element::Class(c));
            }
            Some(Element::Extension(x)) => {
                self.dh(Flags::default());
                self.dh_estaticos(Element::Extension(x));
            }
            Some(Element::Variable(v)) => {
                if let Some(t) = self.consulta.tipo_da_variavel(v) {
                    self.dh(Flags::default());
                    self.dh_membros_de_instancia(t, false);
                }
            }
            Some(Element::Function(f)) => {
                let fe = self.consulta.programa.function(f);
                let dados = &self.consulta.outline.functions[f.0 as usize];
                let t = if matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) { dados.return_type } else { dados.signature };
                self.dh(Flags::default());
                self.dh_membros_de_instancia(t, false);
            }
            _ => {}
        }
    }

    pub(super) fn v_indice(&mut self, n: usize) {
        if let Inicial::Tok(i) = self.inicial(n)
            && self.offset <= self.span_tok(i).end
            && self.palavra(i)
        {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
            return;
        }
        let Some(&alvo) = self.filhos(n).first() else { return };
        let abre = self.op_em(Op::LBracket, self.fim(alvo), self.fim(n)).or_else(|| self.op_em(Op::QuestionDot, self.fim(alvo), self.fim(n)).map(|i| i + 1));
        let Some(a) = abre else { return };
        let fecha = self.par(a).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(a).end && self.offset <= fecha {
            self.local("IndexExpression_index");
            self.para_expressao(n, false, true);
        }
    }

    pub(super) fn v_criacao(&mut self, n: usize) {
        let palavra = self.tok_depois(self.ini(n)).filter(|&i| matches!(self.texto_tok(i), "new" | "const"));
        match palavra {
            Some(k) if self.offset > self.span_tok(k).end => {
                let nome = self.filho(n, "ConstructorName");
                let ok = match nome {
                    None => true,
                    Some(c) => self.offset < self.ini(c) || self.cobre_no(c),
                };
                if ok {
                    self.local("InstanceCreationExpression_constructorName");
                    self.dh(Flags::default());
                    self.dh_invocacoes_de_construtor();
                    self.dh_prefixos_de_import();
                }
            }
            _ => self.para_expressao(n, false, false),
        }
    }

    pub(super) fn v_is(&mut self, n: usize) {
        let Some(&expr) = self.filhos(n).first() else { return };
        let Some(op) = self.palavra_em("is", self.fim(expr), self.fim(n)) else { return };
        let s = self.span_tok(op);
        if self.cobre(s.start, s.end) {
            self.kw("is");
        } else if self.offset < s.start {
            self.para_expressao(n, false, false);
        } else if self.offset > s.end {
            self.local("IsExpression_type");
            self.dh(Flags { tipo: true, ..Flags::default() });
            self.dh_lexicas(n);
        }
    }

    pub(super) fn v_rotulo(&mut self, n: usize) {
        let fim_do_nome = self.tok_depois(self.ini(n)).map_or(self.ini(n), |i| self.span_tok(i).end);
        if self.offset >= fim_do_nome
            && let Some(p) = self.pai(n)
            && self.especie(p) == "NamedExpression"
        {
            self.visitar(p);
        }
    }

    pub(super) fn v_library(&mut self, n: usize) {
        if self.offset >= self.fim(n) && self.pai(n) == Some(0) {
            self.kw_diretiva(Some(n));
            let (_, depois) = self.vizinhos_do_membro(n);
            if depois.is_none_or(|d| !self.e_diretiva(d)) {
                self.para_declaracao_de_unidade();
            }
        }
    }

    pub(super) fn v_literal_de_colecao(&mut self, n: usize) {
        let abre = self.op_em(if self.especie(n) == "ListLiteral" { Op::LBracket } else { Op::LBrace }, self.ini(n), self.fim(n));
        let Some(a) = abre else { return };
        let fecha = self.par(a).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(a).end && self.offset <= fecha {
            self.local(if self.especie(n) == "ListLiteral" { "ListLiteral_element" } else { "SetOrMapLiteral_element" });
            let elementos = self.elementos_do_literal(n);
            self.para_elemento_de_colecao(n, &elementos);
        }
    }

    pub(super) fn v_entrada_de_mapa(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
            return;
        }
        let Some(&chave) = self.filhos(n).first() else { return };
        if let Some(s) = self.op_em(Op::Colon, self.fim(chave), self.fim(n))
            && self.offset >= self.span_tok(s).end
        {
            self.local("MapLiteralEntry_value");
            let estatico = self.contexto_estatico(n);
            self.dh(Flags { estatico, ..Flags::default() });
            self.dh_lexicas(n);
        }
    }

    pub(super) fn v_entrada_de_padrao_de_mapa(&mut self, n: usize) {
        let Some(&chave) = self.filhos(n).first() else { return };
        let sep = self.op_em(Op::Colon, self.fim(chave), self.fim(n).max(self.fim(chave) + 1));
        if sep.is_none_or(|s| self.offset <= self.span_tok(s).start) {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
            return;
        }
        self.local("MapPatternEntry_value");
        self.para_padrao(n, false);
    }

    pub(super) fn v_metodo(&mut self, n: usize) {
        self.local("ClassDeclaration_member");
        let Some(f) = self.funcao_real(n) else { return };
        let primeiro = self.primeiro_depois_das_anotacoes(n);
        let anterior = primeiro.and_then(|i| i.checked_sub(1));
        let ini = anterior.map_or(self.ini(n), |i| self.span_tok(i).start);
        if let Some(nome) = f.name
            && self.offset >= ini
            && self.offset <= nome.span.end
        {
            self.para_tipo(n, TipoOpcoes::default());
            self.kw_membro_de_classe();
        }
        let corpo = self.corpo_da_funcao(n);
        let ini_corpo = corpo.map_or(self.sintetico(self.fim(n)).start, |c| self.ini(c));
        let antes_do_corpo = self.tok_antes(ini_corpo).map_or(ini_corpo, |i| self.span_tok(i).end);
        if self.offset >= antes_do_corpo && self.offset <= ini_corpo {
            if f.modifier == ast::AsyncModifier::None {
                self.kw_modificadores_de_corpo(corpo);
            }
            if corpo.is_none_or(|c| self.especie(c) == "EmptyFunctionBody") {
                self.kw_membro_de_classe();
            }
        }
    }

    pub(super) fn v_invocacao(&mut self, n: usize) {
        if let Inicial::Tok(i) = self.inicial(n)
            && self.offset <= self.span_tok(i).end
            && self.palavra(i)
        {
            if let Some(p) = self.pai(n) {
                self.visitar(p);
            }
            return;
        }
        // O operador `.`/`?.`/`..` antes do nome do método.
        let nome = self.filhos_de(n, &["SimpleIdentifier"]).last().copied();
        let operador = nome.and_then(|m| self.tok_antes(self.ini(m))).filter(|&i| {
            self.e_op(i, Op::Dot) || self.e_op(i, Op::QuestionDot) || self.e_op(i, Op::DotDot) || self.e_op(i, Op::QuestionDotDot)
        });
        let Some(op) = operador else {
            if self.cobre_no(n) {
                let pai = self.pai(n);
                let mut nao_void = false;
                if let Some(p) = pai {
                    match self.especie(p) {
                        "ArgumentList" => {
                            let l = self.local_de_argumentos(p, false);
                            self.local(&l);
                            nao_void = true;
                        }
                        "NamedExpression" => {
                            if let Some(avo) = self.pai(p)
                                && self.especie(avo) == "ArgumentList"
                            {
                                let l = self.local_de_argumentos(avo, true);
                                self.local(&l);
                            }
                            nao_void = true;
                        }
                        "RecordLiteral" => {
                            self.local("RecordLiteral_fields");
                            nao_void = true;
                        }
                        _ => {}
                    }
                }
                self.para_expressao(n, false, nao_void);
            }
            return;
        };
        let cascata = self.e_op(op, Op::DotDot) || self.e_op(op, Op::QuestionDotDot);
        let nome_fim = nome.map_or(self.fim(n), |m| self.fim(m));
        if (cascata && self.offset == self.span_tok(op).start + 1) || (self.offset >= self.span_tok(op).end && self.offset <= nome_fim) {
            let alvo = self.filhos(n).first().copied().filter(|&a| Some(a) != nome);
            let tipo = alvo.and_then(|a| self.expr_real(a)).and_then(|e| self.tipo_estatico(e));
            if let Some(t) = tipo {
                self.para_acesso_a_membro(n, t, false);
            }
            let sem_tipo = tipo.is_none_or(|t| self.consulta.tabela.e_invalido(t) || self.e_tipo_type(t));
            if sem_tipo
                && let Some(a) = alvo
                && matches!(self.especie(a), "SimpleIdentifier" | "PrefixedIdentifier")
                && (!cascata || self.offset == self.span_tok(op).start + 1)
            {
                self.estaticos_ou_prefixo(a);
            }
        }
    }

    /// `type.isDartCoreType`.
    pub(super) fn e_tipo_type(&self, t: TypeId) -> bool {
        t == self.consulta.core.type_
    }

    /// `staticElement` do identificador alvo: classe ou tipo de extensão →
    /// estáticos; prefixo → as declarações por ele.
    pub(super) fn estaticos_ou_prefixo(&mut self, alvo: usize) {
        if let Some(p) = self.prefixo_de_import(alvo) {
            self.dh(Flags::default());
            self.dh_pelo_prefixo(p);
            return;
        }
        let Some(e) = self.expr_real(alvo) else { return };
        match self.resolvido(e) {
            Some(Resolved::Element(Element::Class(c))) => {
                self.dh(Flags::default());
                self.dh_estaticos(Element::Class(c));
            }
            Some(Resolved::Element(Element::Typedef(t))) => {
                self.dh(Flags::default());
                self.dh_estaticos(Element::Typedef(t));
            }
            _ => {}
        }
    }

    pub(super) fn v_mixin(&mut self, n: usize) {
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(d) = self.decl_real(n) else { return };
        let ast::DeclKind::Mixin(x) = &d.kind else { return };
        let Some(m) = self.palavra_em("mixin", self.ini(n), x.name.span.start) else { return };
        if self.offset < self.span_tok(m).start {
            self.kw_modificadores_de_mixin(n);
            return;
        }
        if self.offset <= self.span_tok(m).end {
            self.kw("mixin");
            return;
        }
        let (esq, dir, esq_s, dir_s) = self.chaves(n);
        if self.offset <= x.name.span.end {
            self.ih(false);
            self.ih_nome_de_topo(esq_s && dir_s);
            return;
        }
        if self.offset <= esq.start {
            self.kw_declaracao_de_mixin(n);
            return;
        }
        if self.offset >= esq.end && self.offset <= dir.start {
            self.local("MixinDeclaration_member");
            if self.anotacao_no_fim_do_corpo(n) {
                return;
            }
            self.para_membro_de_mixin(n);
            let membros = self.filhos_de(n, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
            if let Some(m) = self.elemento_antes(&membros)
                && self.especie(m) == "MethodDeclaration"
                && self.corpo_da_funcao(m).is_none_or(|c| self.especie(c) == "EmptyFunctionBody")
            {
                let corpo = self.corpo_da_funcao(m);
                self.kw_modificadores_de_corpo(corpo);
            }
        }
    }

    pub(super) fn v_on_de_mixin(&mut self, n: usize) {
        if let Some(on) = self.tok_depois(self.ini(n))
            && self.offset <= self.span_tok(on).end
        {
            self.kw("on");
        } else {
            self.para_tipo(n, TipoOpcoes::default());
        }
    }

    pub(super) fn v_expressao_nomeada(&mut self, n: usize) {
        let Some(rotulo) = self.filho(n, "Label") else { return };
        let fim_nome = self.tok_depois(self.ini(rotulo)).map_or(self.ini(rotulo), |i| self.span_tok(i).end);
        let nome_texto = self.fonte[self.ini(rotulo)..fim_nome].to_string();
        if self.offset <= fim_nome {
            let Some(pai) = self.pai(n) else { return };
            match self.especie(pai) {
                "ArgumentList" => {
                    let l = self.local_de_argumentos(pai, true);
                    self.local(&l);
                    if let Some(parametros) = self.parametros_invocados(pai) {
                        let mut usados: HashSet<String> = HashSet::new();
                        for &a in self.filhos(pai) {
                            if let Some(r) = self.filho(a, "Label") {
                                usados.insert(self.fonte[self.ini(r)..self.fim(r)].trim_end_matches(':').trim().to_string());
                            }
                        }
                        usados.remove(&nome_texto);
                        let dois_pontos = self.tok_depois(fim_nome).is_none_or(|i| !self.e_op(i, Op::Colon));
                        for p in parametros {
                            if p.nomeado && !usados.contains(&p.nome) {
                                self.sugerir_argumento_nomeado(&p, dois_pontos, false, None);
                            }
                        }
                    }
                }
                "RecordLiteral" => {
                    self.local("RecordLiteral_fields");
                    self.sugerir_campos_nomeados_de_record(n, Some(pai), false);
                }
                _ => {}
            }
        } else if self.offset >= self.fim(rotulo) {
            let em_argumentos = self.pai(n).is_some_and(|p| self.especie(p) == "ArgumentList");
            if em_argumentos {
                self.local("ArgumentList_method_named");
            }
            self.para_expressao(n, false, em_argumentos);
            // `staticParameterElement`: o tipo do parâmetro nomeado.
            if let Some(lista) = self.pai(n).filter(|&p| self.especie(p) == "ArgumentList")
                && let Some(ps) = self.parametros_invocados(lista)
                && let Some(p) = ps.into_iter().find(|p| p.nomeado && p.nome == nome_texto)
                && matches!(self.consulta.tabela.get(p.tipo), Type::Function { .. })
            {
                let virgula = !self.seguido_de_virgula(n);
                self.sugerir_closure_com_nomes(p.tipo, &p.nomes_do_tipo, virgula);
            }
        }
    }

    pub(super) fn v_tipo_nomeado(&mut self, n: usize) {
        // `prefix.x^ print(0);` recuperado como declaração de variável.
        if let Some(p) = self.filho(n, "ImportPrefixReference")
            && let Some(prefixo) = self.prefixo_de_import(p)
            && let Some(lista) = self.pai(n).filter(|&l| self.especie(l) == "VariableDeclarationList")
            && let Some(cmd) = self.pai(lista).filter(|&c| self.especie(c) == "VariableDeclarationStatement")
            && !self.tok_antes(self.fim(cmd)).is_some_and(|i| self.e_op(i, Op::Semicolon))
        {
            self.dh(Flags::default());
            self.dh_pelo_prefixo(prefixo);
            return;
        }
        if let Some(pai) = self.pai(n) {
            match self.especie(pai) {
                "ImplementsClause" => self.local("ImplementsClause_interface"),
                "TypeArgumentList" => self.local("TypeArgumentList_argument"),
                "WithClause" => self.local("WithClause_mixinType"),
                _ => {}
            }
        }
        let criacao = self.pai(n).and_then(|p| self.pai(p)).is_some_and(|x| self.especie(x) == "InstanceCreationExpression");
        self.para_tipo(n, TipoOpcoes { excluir_nomes_de_tipo: criacao, ..TipoOpcoes::default() });
    }

    pub(super) fn v_padrao_objeto(&mut self, n: usize) {
        let abre = self.op_em(Op::LParen, self.ini(n), self.fim(n));
        let Some(a) = abre else { return };
        let fecha = self.par(a).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= fecha {
            self.local("ObjectPattern_fieldName");
            let excluidos = self.nomes_de_campos(n);
            if let Some(t) = self.filho(n, "NamedType").and_then(|t| self.tipo_do_no_de_tipo(t)) {
                self.dh(Flags { nao_void: true, ..Flags::default() });
                self.dh_getters(t, &excluidos);
            }
        }
    }

    pub(super) fn v_parenteses(&mut self, n: usize) {
        let Some(&expr) = self.filhos(n).first() else { return };
        if matches!(self.especie(expr), "SimpleIdentifier" | "PrefixedIdentifier" | "PropertyAccess")
            && let Some(f) = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::RParen))
            && self.offset == self.span_tok(f).start
            && self.tok_depois(self.fim(expr)).is_some_and(|i| self.e_identificador(i) && i < f)
        {
            self.kw("is");
            return;
        }
        self.local("ParenthesizedExpression_expression");
        if self.especie(expr) == "SimpleIdentifier" {
            self.sugerir_campos_nomeados_de_record(n, None, true);
        }
        self.para_expressao(n, false, false);
    }

    pub(super) fn v_campo_de_padrao(&mut self, n: usize) {
        let nome = self.filho(n, "PatternFieldName");
        let pai = self.pai(n);
        if let Some(nm) = nome {
            let dois_pontos = self.tok_antes(self.fim(nm)).map_or(self.fim(nm), |i| self.span_tok(i).start);
            if self.offset <= dois_pontos {
                if pai.is_some_and(|p| self.especie(p) == "ObjectPattern") {
                    self.local("ObjectPattern_fieldName");
                } else {
                    self.local("PatternField_pattern");
                }
                if let Some(p) = pai {
                    self.para_nome_de_campo_em_padrao(p);
                }
                return;
            }
        }
        match nome {
            None => match pai.map(|p| self.especie(p)) {
                Some("ObjectPattern") => {
                    self.local("ObjectPattern_fieldName");
                    let p = pai.unwrap_or(n);
                    let excluidos = self.nomes_de_campos(p);
                    if let Some(t) = self.filho(p, "NamedType").and_then(|t| self.tipo_do_no_de_tipo(t)) {
                        self.dh(Flags { nao_void: true, ..Flags::default() });
                        self.dh_getters(t, &excluidos);
                    }
                }
                Some("RecordPattern") => {
                    self.local("PatternField_pattern");
                    self.para_padrao(n, true);
                }
                _ => {}
            },
            Some(nm) => {
                let tem_nome = self.tok_depois(self.ini(nm)).is_some_and(|i| self.palavra(i) && self.span_tok(i).end <= self.fim(nm));
                self.local("PatternField_pattern");
                if !tem_nome {
                    self.para_padrao_de_variavel();
                    if let Some(p) = pai {
                        self.para_nome_de_campo_em_padrao(p);
                    }
                } else {
                    self.para_padrao(n, false);
                }
            }
        }
    }

    pub(super) fn v_nome_de_campo_de_padrao(&mut self, n: usize) {
        let dois_pontos = self.tok_antes(self.fim(n)).map_or(self.fim(n), |i| self.span_tok(i).start);
        if self.offset <= dois_pontos {
            let externo = self.pai(n).and_then(|p| self.pai(p));
            if externo.is_some_and(|x| self.especie(x) == "ObjectPattern") {
                self.local("ObjectPattern_fieldName");
            } else {
                self.local("PatternField_pattern");
            }
            if let Some(x) = externo
                && super::isp::e_padrao(self.especie(x))
            {
                self.para_nome_de_campo_em_padrao(x);
            }
        }
    }

    pub(super) fn v_posfixa(&mut self, n: usize) {
        let Some(&operando) = self.filhos(n).first() else { return };
        let op = self.fonte[self.fim(operando)..self.fim(n)].trim().to_string();
        self.local(&format!("PrefixExpression_{op}_operand"));
        let atribuivel = op == "++" || op == "--";
        self.para_expressao(n, atribuivel, false);
    }

    pub(super) fn v_identificador_prefixado(&mut self, n: usize) {
        let (Some(&prefixo), Some(&id)) = (self.filhos(n).first(), self.filhos(n).get(1)) else { return };
        let ponto = self.fim(prefixo);
        if self.offset <= ponto {
            self.para_expressao(n, false, false);
            return;
        }
        let _ = id;
        self.local("PropertyAccess_propertyName");
        let tipo = self.expr_real(prefixo).and_then(|e| self.tipo_estatico(e));
        // Prefixo de import ou nome de classe: o `staticType` é nulo no
        // analyzer.
        let elemento = self.expr_real(prefixo).and_then(|e| self.resolvido(e));
        let e_tipo_ou_prefixo = matches!(elemento, Some(Resolved::Prefix(_) | Resolved::Element(Element::Class(_) | Element::Typedef(_) | Element::Extension(_))));
        match tipo {
            Some(t) if !e_tipo_ou_prefixo => {
                let so_super = self.especie(prefixo) == "SuperExpression";
                self.para_acesso_a_membro(n, t, so_super);
            }
            _ => {
                let atribuivel = self.pai(n).is_some_and(|p| self.especie(p) == "AssignmentExpression" && self.filhos(p).first() == Some(&n));
                if let Some(p) = self.prefixo_de_import(prefixo) {
                    self.dh(Flags { atribuivel, ..Flags::default() });
                    self.dh_pelo_prefixo(p);
                } else if let Some(el) = elemento {
                    let elemento = match el {
                        Resolved::Element(e) => Some(e),
                        _ => None,
                    };
                    if let Some(e) = elemento {
                        let preferir = matches!(e, Element::Class(_)) && self.tear_off_preferido();
                        self.dh(Flags { atribuivel, preferir_sem_invocacao: preferir, ..Flags::default() });
                        self.dh_estaticos(e);
                    }
                }
            }
        }
    }

    /// `request.shouldSuggestTearOff(element)`: o contexto é um tipo de
    /// função (sugere o tear-off do construtor).
    pub(super) fn tear_off_preferido(&self) -> bool {
        self.tipo_de_contexto.is_some_and(|t| matches!(self.consulta.tabela.get(t), Type::Function { .. }))
    }

    pub(super) fn v_prefixa(&mut self, n: usize) {
        self.local("PropertyAccess_propertyName");
        let op = self.tok_depois(self.ini(n)).map(|i| self.texto_tok(i)).unwrap_or("");
        let atribuivel = op == "++" || op == "--";
        self.para_expressao(n, atribuivel, false);
    }

    pub(super) fn v_acesso_a_propriedade(&mut self, n: usize) {
        let (Some(&alvo), nome) = (self.filhos(n).first(), self.filhos(n).get(1).copied()) else { return };
        let operador = nome.and_then(|m| self.tok_antes(self.ini(m))).or_else(|| self.tok_depois(self.fim(alvo)));
        let Some(op) = operador else { return };
        if self.offset <= self.span_tok(op).start {
            self.para_expressao(n, false, false);
            return;
        }
        self.local("PropertyAccess_propertyName");
        let pai = self.pai(n);
        if self.especie(alvo) == "ThisExpression" && pai.is_some_and(|p| self.especie(p) == "ConstructorFieldInitializer") {
            if let Some(p) = pai {
                self.visitar(p);
            }
            return;
        }
        let tipo = self.expr_real(alvo).and_then(|e| self.tipo_estatico(e));
        if let Some(t) = tipo {
            let so_super = self.especie(alvo) == "SuperExpression";
            self.para_acesso_a_membro(n, t, so_super);
        }
        let cascata = self.e_op(op, Op::DotDot) || self.e_op(op, Op::QuestionDotDot);
        let sem_tipo = tipo.is_none_or(|t| self.consulta.tabela.e_invalido(t) || self.e_tipo_type(t));
        if sem_tipo && matches!(self.especie(alvo), "SimpleIdentifier" | "PrefixedIdentifier") && (!cascata || self.offset == self.span_tok(op).start + 1) {
            self.estaticos_ou_prefixo(alvo);
        }
    }

    pub(super) fn v_padrao_record(&mut self, n: usize) {
        let abre = self.tok_em(self.ini(n)).filter(|&i| self.e_op(i, Op::LParen));
        let Some(a) = abre else { return };
        if self.offset == self.span_tok(a).start {
            self.local("ObjectPattern_type");
            self.dh(Flags { tipo: true, nao_void: true, ..Flags::default() });
            self.dh_lexicas(n);
            return;
        }
        let fecha = self.par(a).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= fecha {
            self.local("PatternField_pattern");
            self.kw("dynamic");
            self.para_expressao(n, false, false);
            let campos = self.filhos_de(n, &["PatternField"]);
            let alvo = campos.iter().copied().find(|&c| self.fim(c) >= self.offset);
            if let Some(c) = alvo
                && let Some(nm) = self.filho(c, "PatternFieldName")
                && self.offset <= self.tok_antes(self.fim(nm)).map_or(self.fim(nm), |i| self.span_tok(i).start)
                && let Some(t) = self.tipo_casado(n)
            {
                let excluidos = self.nomes_de_campos(n);
                self.dh(Flags { nao_void: true, ..Flags::default() });
                self.dh_getters(t, &excluidos);
            }
        }
    }

    pub(super) fn v_tipo_record(&mut self, n: usize) {
        if self.offset <= self.ini(n) {
            let mut pai = self.pai(n);
            if pai.is_some_and(|p| self.especie(p) == "DefaultFormalParameter") {
                pai = pai.and_then(|p| self.pai(p));
            }
            if let Some(p) = pai
                && super::isp::e_parametro(self.especie(p))
                && self.offset <= self.ini(p)
            {
                self.local("FormalParameterList_parameter");
                self.para_tipo(n, TipoOpcoes::default());
            }
            return;
        }
        let fecha = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::RParen)).map_or(self.fim(n), |i| self.span_tok(i).start);
        if self.offset <= fecha {
            self.local("RecordTypeAnnotation_positionalFields");
            self.para_tipo(n, TipoOpcoes::default());
        }
    }

    pub(super) fn v_campo_nomeado_de_tipo_record(&mut self, n: usize) {
        let tipo = self.filhos(n).first().copied();
        if let Some(t) = tipo
            && self.cobre_no(t)
        {
            self.local("RecordTypeAnnotationNamedFields_fields");
            self.para_tipo(n, TipoOpcoes::default());
            return;
        }
        let nome = self.tok_antes(self.fim(n));
        if let Some(i) = nome
            && self.cobre(self.span_tok(i).start, self.span_tok(i).end)
        {
            self.local("RecordTypeAnnotationNamedField_name");
            self.ih(false);
            self.ih_variavel(tipo);
        }
    }

    pub(super) fn v_invocacao_redirecionadora(&mut self, n: usize) {
        let Some(ctor) = self.pai(n).filter(|&c| self.especie(c) == "ConstructorDeclaration") else { return };
        self.local("ConstructorDeclaration_initializer");
        let Some(this) = self.tok_depois(self.ini(n)) else { return };
        let lista = self.filho(n, "ArgumentList");
        if self.offset <= self.span_tok(this).end && lista.is_none() {
            self.kw_inicializador_de_construtor(ctor, Some(n));
            return;
        }
        let ponto = self.tok_depois(self.span_tok(this).end).filter(|&i| self.e_op(i, Op::Dot));
        if let Some(p) = ponto
            && self.offset >= self.span_tok(p).end
            && self.offset <= lista.map_or(self.sintetico(self.fim(n)).start, |l| self.ini(l))
        {
            self.para_invocacao_redirecionadora(ctor);
        }
    }

    pub(super) fn v_padrao_relacional(&mut self, n: usize) {
        let Some(op) = self.tok_depois(self.ini(n)) else { return };
        let operando = self.filhos(n).first().copied();
        let prox = op + 1;
        if self.e_op(op, Op::Lt) && operando.is_none() && self.e_op(prox, Op::Gt) {
            self.local("TypeArgumentList_argument");
            self.para_tipo(n, TipoOpcoes::default());
        } else if let Some(o) = operando
            && self.especie(o) == "SimpleIdentifier"
            && self.offset >= self.span_tok(op).end
            && self.offset <= self.fim(o)
        {
            self.local("RelationalPattern_operand");
            self.para_expressao(n, false, false);
        }
    }

    pub(super) fn v_representacao(&mut self, n: usize) {
        let Some(d) = self.pai(n).and_then(|p| self.decl_real(p)) else { return };
        let ast::DeclKind::ExtensionType(x) = &d.kind else { return };
        let nome = x.representation_name;
        if self.offset <= nome.span.end {
            let tipo = self.filhos(n).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(x.representation_type));
            match tipo {
                None => {
                    let abre = self.op_em(Op::LParen, self.ini(n), self.fim(n));
                    let anotacao = abre.is_some_and(|a| self.e_op(a + 1, Op::At));
                    if anotacao {
                        self.local("Annotation_name");
                        self.para_anotacao(n);
                    } else {
                        self.local("RepresentationDeclaration_fieldType");
                        self.dh(Flags { tipo: true, ..Flags::default() });
                        self.dh_lexicas(n);
                    }
                }
                Some(t) => {
                    self.local("RepresentationDeclaration_fieldName");
                    self.ih(true);
                    self.ih_variavel(Some(t));
                }
            }
        } else {
            self.local("RepresentationDeclaration_fieldName");
            let texto = self.nome_real(nome).to_string();
            self.ih(true);
            self.ih_do_nome_de_tipo(&texto);
        }
    }

    pub(super) fn v_return(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
        } else {
            self.local("ReturnStatement_expression");
            let alvo = self.filhos(n).first().copied().unwrap_or(n);
            self.para_expressao(alvo, false, false);
        }
    }

    pub(super) fn v_parametro_simples(&mut self, n: usize) {
        let Some(p) = self.parametro_real(n) else { return };
        let lista = self.lista_de_parametros_de(n);
        if let Some(nome) = p.name
            && self.parametro_de_um_token(n)
        {
            self.local("FormalParameterList_parameter");
            let texto = self.nome_real(nome);
            if PALAVRAS_DO_ANALYZER_PUB.contains(&texto) {
                if texto == "required" && !p.covariant {
                    self.kw("covariant");
                }
                self.para_tipo(n, TipoOpcoes::default());
                return;
            }
            if let Some(l) = lista {
                self.kw_parametro_formal(l);
            }
            self.para_tipo(n, TipoOpcoes::default());
        }
        match p.ty {
            Some(t) => {
                self.local("FormalParameterList_parameter");
                let no_t = self.filhos(n).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(t));
                if let Some(nt) = no_t
                    && self.especie(nt) == "NamedType"
                    && let Some(pr) = self.filho(nt, "ImportPrefixReference")
                    && let Some(prefixo) = self.prefixo_de_import(pr)
                {
                    let nome_t = self.tok_depois(self.fim(pr));
                    if nome_t.is_some_and(|i| self.cobre(self.span_tok(i).start, self.span_tok(i).end)) {
                        self.dh(Flags { tipo: true, ..Flags::default() });
                        self.dh_pelo_prefixo(prefixo);
                    }
                }
                let ts = self.a.ty(t).span;
                let primeiro = self.tok_depois(ts.start);
                if primeiro.is_some_and(|i| self.cobre(self.span_tok(i).start, self.span_tok(i).end)) {
                    if let Some(l) = lista {
                        self.kw_parametro_formal(l);
                    }
                    self.para_tipo(n, TipoOpcoes::default());
                } else if let ast::TypeKind::Function { return_type: None, .. } = &self.a.ty(t).kind
                    && let Some(f) = self.palavra_em("Function", ts.start, ts.end)
                    && self.offset < self.span_tok(f).start
                {
                    self.para_tipo(n, TipoOpcoes::default());
                }
            }
            None => {
                let palavra = self.tok_depois(self.ini(n)).filter(|&i| matches!(self.texto_tok(i), "final" | "var" | "const"));
                if let Some(k) = palavra
                    && self.offset <= self.span_tok(k).end
                {
                    self.local("FormalParameterList_parameter");
                    if let Some(pai) = self.pai(n)
                        && self.especie(pai) == "FormalParameterList"
                    {
                        self.kw_parametro_formal(pai);
                    }
                    self.para_tipo(n, TipoOpcoes::default());
                }
            }
        }
    }

    /// `FormalParameter.isSingleIdentifier`: um só token, palavra.
    pub(super) fn parametro_de_um_token(&self, n: usize) -> bool {
        let Some(i) = self.tok_depois(self.ini(n)) else { return false };
        self.palavra(i) && self.span_tok(i).end == self.fim(n)
    }

    pub(super) fn v_string_simples(&mut self, n: usize) {
        if let Some(p) = self.pai(n)
            && matches!(self.especie(p), "Configuration" | "PartOfDirective" | "ImportDirective" | "ExportDirective" | "PartDirective" | "AugmentationImportDirective")
        {
            self.uris(n);
            return;
        }
        self.visitar_pai_se_no_ou_antes(n);
    }

    pub(super) fn v_invocacao_do_super(&mut self, n: usize) {
        let Some(ctor) = self.pai(n).filter(|&c| self.especie(c) == "ConstructorDeclaration") else { return };
        self.local("ConstructorDeclaration_initializer");
        let Some(sup) = self.tok_depois(self.ini(n)) else { return };
        let lista = self.filho(n, "ArgumentList");
        if self.offset <= self.span_tok(sup).end && lista.is_none() {
            self.kw_inicializador_de_construtor(ctor, Some(n));
            return;
        }
        let ponto = self.tok_depois(self.span_tok(sup).end).filter(|&i| self.e_op(i, Op::Dot));
        if let Some(p) = ponto
            && self.offset >= self.span_tok(p).end
            && self.offset <= lista.map_or(self.sintetico(self.fim(n)).start, |l| self.ini(l))
        {
            let Some(conteiner) = self.pai(ctor) else { return };
            if !matches!(self.especie(conteiner), "ClassDeclaration" | "EnumDeclaration") {
                return;
            }
            let Some(classe) = self.classe_do_no(conteiner) else { return };
            let supertipo = self.consulta.outline.classes.get(classe.0 as usize).and_then(|d| d.supertype);
            if let Some(st) = supertipo {
                let constante = self.construtor_real(ctor).is_some_and(|k| k.const_);
                self.dh(Flags { constante, ..Flags::default() });
                self.dh_nomes_de_construtor_do_tipo(st, None);
            }
        }
    }

    pub(super) fn v_default(&mut self, n: usize) {
        let Some(k) = self.palavra_em("default", self.ini(n), self.fim(n)) else { return };
        if self.offset <= self.span_tok(k).start {
            self.local("SwitchMember_statement");
            self.kw("case");
            self.kw_e_texto("default", ":");
        } else if self.offset <= self.span_tok(k).end {
            let dois_pontos = self.tok_depois(self.span_tok(k).end).is_some_and(|i| self.e_op(i, Op::Colon));
            if !dois_pontos {
                self.kw_e_texto("default", ":");
            } else {
                self.kw("default");
            }
        }
    }

    pub(super) fn v_switch_expressao(&mut self, n: usize) {
        let abre = self.op_em(Op::LParen, self.ini(n), self.fim(n));
        let fecha = abre.and_then(|a| self.par(a));
        if let (Some(a), Some(f)) = (abre, fecha)
            && self.offset >= self.span_tok(a).end
            && self.offset <= self.span_tok(f).start
        {
            self.local("SwitchExpression_expression");
            self.para_expressao(n, false, false);
            return;
        }
        let depois = fecha.map_or(self.ini(n), |f| self.span_tok(f).end);
        let chave = self.op_em(Op::LBrace, depois, self.fim(n));
        if let Some(c) = chave {
            let fim = self.par(c).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
            if self.offset >= self.span_tok(c).end && self.offset <= fim {
                self.local("SwitchExpression_body");
                self.para_padrao(n, true);
            }
        }
    }

    pub(super) fn v_caso_de_switch_expressao(&mut self, n: usize) {
        let gp = self.filho(n, "GuardedPattern");
        let seta = self.op_em(Op::Arrow, gp.map_or(self.ini(n), |g| self.fim(g)), self.fim(n).max(self.offset + 1));
        if seta.is_none() {
            self.local("SwitchExpression_body");
            self.para_padrao(n, true);
            return;
        }
        let Some(expr) = self.filhos(n).iter().copied().find(|&k| Some(k) != gp) else { return };
        let um_token = self.tok_depois(self.ini(expr)).is_some_and(|i| self.span_tok(i).end == self.fim(expr));
        if um_token {
            self.local("SwitchExpressionCase_expression");
            self.para_expressao(expr, false, false);
        }
    }

    pub(super) fn v_caso_de_padrao(&mut self, n: usize) {
        let coberto = self.no_coberto();
        let Some(case) = self.tok_depois(self.ini(n)) else { return };
        let gp = self.filho(n, "GuardedPattern");
        let dois_pontos = self.op_em(Op::Colon, gp.map_or(self.span_tok(case).end, |g| self.fim(g)), self.fim(n).max(self.offset + 1));
        let dp = dois_pontos.map_or(self.sintetico(gp.map_or(self.span_tok(case).end, |g| self.fim(g))).start, |i| self.span_tok(i).start);
        if self.offset <= self.span_tok(case).end {
            self.kw("case");
        } else if self.offset <= dp {
            self.local("SwitchPatternCase_pattern");
            if let Some(c) = coberto
                && self.especie(c) == "NamedType"
                && self.pai(c).is_some_and(|p| self.especie(p) == "ObjectPattern")
            {
                self.local("ObjectPattern_type");
                self.visitar(c);
                return;
            }
            let padrao = gp.and_then(|g| self.filhos(g).first().copied());
            if let Some(p) = padrao {
                if self.especie(p) == "ConstantPattern"
                    && let Some(&id) = self.filhos(p).first()
                    && self.especie(id) == "SimpleIdentifier"
                    && self.offset < self.ini(id)
                {
                    self.dh(Flags { tipo: true, ..Flags::default() });
                    self.dh_lexicas(n);
                    return;
                }
                if self.especie(p) == "WildcardPattern" && self.offset < self.fim(p).saturating_sub(1) {
                    self.dh(Flags { tipo: true, ..Flags::default() });
                    self.dh_lexicas(n);
                    return;
                }
                if self.especie(p) == "ConstantPattern"
                    && let Some(&e) = self.filhos(p).first()
                    && self.e_literal_de_tipo(e)
                    && self.fim(e) < self.offset
                {
                    let nome = self.fonte[self.ini(e)..self.fim(e)].rsplit('.').next().unwrap_or("").to_string();
                    self.ih(false);
                    self.ih_do_nome_de_tipo(&nome);
                    return;
                }
            }
            if let Some(c) = coberto
                && self.especie(c) == "NamedType"
                && let Some(pp) = self.pai(c)
            {
                match self.especie(pp) {
                    "DeclaredVariablePattern" => {
                        self.visitar(c);
                        return;
                    }
                    "ObjectPattern" => {
                        self.local("ObjectPattern_type");
                        self.visitar(c);
                        return;
                    }
                    "WildcardPattern" => {
                        self.local("WildcardPattern_type");
                        self.visitar(c);
                        return;
                    }
                    _ => {}
                }
            }
            let anterior = dois_pontos.and_then(|d| d.checked_sub(1)).or_else(|| self.tok_antes(dp));
            let Some(ant) = anterior else { return };
            let palavra = self.e_palavra_chave(ant).then(|| self.texto_tok(ant));
            match palavra {
                None => {
                    if dois_pontos.is_none() || self.cobre(self.span_tok(ant).start, self.span_tok(ant).end) {
                        self.kw("final");
                        self.kw("var");
                        match padrao {
                            Some(p) if self.especie(p) == "ConstantPattern" => {
                                let e = self.filhos(p).first().copied().unwrap_or(p);
                                self.para_expressao(e, false, true);
                            }
                            Some(p) => self.para_expressao(p, false, true),
                            None => self.para_expressao(n, false, true),
                        }
                    } else {
                        self.kw("as");
                        self.kw("when");
                    }
                }
                Some("as") => self.kw("dynamic"),
                Some("when") => {}
                Some(_) => {
                    self.kw("as");
                    self.kw("when");
                }
            }
        } else {
            self.local("SwitchMember_statement");
            let comandos: Vec<usize> = self.filhos(n).iter().copied().filter(|&k| super::isp::e_comando(self.especie(k))).collect();
            if comandos.first().is_none_or(|&c| self.offset <= self.ini(c)) {
                self.kw("case");
                self.kw_e_texto("default", ":");
            }
            self.para_comando(n);
        }
    }

    pub(super) fn v_switch(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
            return;
        }
        let abre = self.op_em(Op::LParen, self.span_tok(palavra).end, self.fim(n));
        let fecha = abre.and_then(|a| self.par(a));
        if let (Some(a), Some(f)) = (abre, fecha)
            && self.offset >= self.span_tok(a).end
            && self.offset <= self.span_tok(f).start
        {
            self.local("SwitchStatement_expression");
            self.para_expressao(n, false, true);
            return;
        }
        let depois = fecha.map_or(self.span_tok(palavra).end, |f| self.span_tok(f).end);
        let Some(c) = self.op_em(Op::LBrace, depois, self.fim(n)) else { return };
        let fim = self.par(c).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.offset >= self.span_tok(c).end && self.offset <= fim {
            self.local("SwitchMember_statement");
            let membros = self.filhos_de(n, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]);
            self.kw("case");
            self.kw_e_texto("default", ":");
            if !membros.is_empty() {
                if !membros.iter().any(|&m| self.especie(m) == "SwitchDefault") {
                    self.kw_e_texto("default", ":");
                }
                if let Some(m) = self.elemento_antes(&membros) {
                    self.para_comando(m);
                }
            }
        }
    }

    pub(super) fn v_variavel_de_topo(&mut self, n: usize) {
        if self.tratou_recuperacao(n) {
            return;
        }
        if self.offset == self.ini(n) {
            self.para_membro_de_unidade_antes(n);
            return;
        }
        let Some(lista_no) = self.filho(n, "VariableDeclarationList") else { return };
        let Some(lista) = self.lista_real(lista_no) else { return };
        let Some(primeira) = lista.variables.first() else { return };
        if self.offset > primeira.name.span.end {
            if lista.ty.is_none() {
                let texto = self.nome_real(primeira.name).to_string();
                self.ih(true);
                self.ih_do_nome_de_tipo(&texto);
            }
            return;
        }
        let external = self.palavra_em("external", self.ini(n), primeira.name.span.start).is_some();
        if !external {
            self.kw("external");
        }
        if !lista.late {
            self.kw("late");
        }
        if !lista.const_ {
            self.kw("const");
        }
        if !lista.final_ {
            self.kw("final");
        }
        if self.pai(n) == Some(0) {
            let (antes, _) = self.vizinhos_do_membro(n);
            if antes.is_none_or(|a| self.e_diretiva(a)) {
                self.local("CompilationUnit_directive");
            } else {
                self.local("CompilationUnit_declaration");
            }
        }
        self.dh(Flags { tipo: true, ..Flags::default() });
        self.dh_lexicas(n);
    }

    pub(super) fn v_try(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
            return;
        }
        let corpo = self.filho(n, "Block");
        if corpo.is_some_and(|b| self.offset >= self.fim(b)) {
            let finally = self.palavra_em("finally", self.ini(n), self.fim(n));
            match finally {
                None => {
                    let ultimo = self.filhos_de(n, &["CatchClause"]).last().copied();
                    match ultimo {
                        None => self.kw_clausulas_de_try(true),
                        Some(c) => self.kw_clausulas_de_try(self.offset >= self.fim(c)),
                    }
                }
                Some(f) if self.offset < self.span_tok(f).start => self.kw_clausulas_de_try(false),
                _ => {}
            }
        }
    }

    pub(super) fn v_parametro_de_tipo(&mut self, n: usize) {
        if let Some(lista) = self.pai(n)
            && self.especie(lista) == "TypeParameterList"
        {
            // `Future<void^>` num corpo de classe: o fasta inventa `() {}`.
            let fecha = self.tok_antes(self.fim(lista));
            let prox = fecha.and_then(|f| self.toks.get(f + 1));
            let sem_parenteses = prox.is_none_or(|t| t.kind != dartforge_frontend::token::Kind::Op(Op::LParen));
            let metodo_fantasma = self.pai(lista).is_some_and(|m| self.especie(m) == "MethodDeclaration" && self.filho(m, "FormalParameterList").is_none());
            if sem_parenteses && metodo_fantasma {
                self.local("TypeParameter_bound");
                self.para_tipo(n, TipoOpcoes { excluidos: vec![lista], ..TipoOpcoes::default() });
                return;
            }
        }
        let Some(nome) = self.tok_depois(self.ini(n)) else { return };
        let nome = if self.filhos(n).iter().any(|&k| self.especie(k) == "Annotation") {
            self.tok_depois(self.filhos(n).iter().filter(|&&k| self.especie(k) == "Annotation").map(|&k| self.fim(k)).max().unwrap_or(self.ini(n))).unwrap_or(nome)
        } else {
            nome
        };
        if self.offset <= self.span_tok(nome).end {
            return;
        }
        let extends = self.palavra_em("extends", self.span_tok(nome).end, self.fim(n));
        match extends {
            None => self.kw("extends"),
            Some(e) if self.offset <= self.span_tok(e).end => self.kw("extends"),
            Some(_) => {
                self.local("TypeParameter_bound");
                self.para_tipo(n, TipoOpcoes { nao_void: true, ..TipoOpcoes::default() });
            }
        }
    }

    pub(super) fn v_declaracao_de_variavel(&mut self, n: usize) {
        let Some(lista_no) = self.pai(n).filter(|&l| self.especie(l) == "VariableDeclarationList") else { return };
        let avo = self.pai(lista_no);
        let especie_avo = avo.map(|a| self.especie(a));
        match especie_avo {
            Some("FieldDeclaration") => {
                let a = avo.unwrap_or(n);
                if self.membro_incompleto_anterior(a) && self.campo_solto(a) {
                    return;
                }
            }
            Some("ForPartsWithDeclarations") => {
                let lista = self.lista_real(lista_no);
                let tem_igual = self.variavel_real(n).is_some_and(|v| v.initializer.is_some());
                if !tem_igual
                    && lista.is_some_and(|l| l.variables.len() == 1)
                    && lista.and_then(|l| l.ty).is_some_and(|t| matches!(self.a.ty(t).kind, ast::TypeKind::Record { .. }))
                {
                    self.kw("in");
                }
            }
            Some("TopLevelVariableDeclaration") => {
                if self.tratou_recuperacao(avo.unwrap_or(n)) {
                    return;
                }
            }
            _ => {}
        }
        let Some(v) = self.variavel_real(n) else { return };
        let Some(lista) = self.lista_real(lista_no) else { return };
        if self.offset <= v.name.span.end {
            let conteiner = avo.and_then(|a| self.pai(a));
            match lista.ty {
                None => {
                    self.local("VariableDeclarationList_type");
                    let palavra = !(lista.var_ || lista.final_ || lista.const_);
                    if palavra {
                        self.kw("const");
                        self.kw("final");
                        self.kw("var");
                    }
                    if !lista.var_ {
                        self.para_tipo(n, TipoOpcoes::default());
                    }
                }
                Some(t) => {
                    let privado = matches!(especie_avo, Some("FieldDeclaration" | "TopLevelVariableDeclaration"));
                    let no_t = self.filhos(lista_no).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(t));
                    self.ih(privado);
                    self.ih_variavel(no_t);
                }
            }
            if especie_avo == Some("FieldDeclaration") {
                let a = avo.unwrap_or(n);
                self.local("FieldDeclaration_fields");
                let ini_a = self.ini(a);
                let antes_do_nome = |s: &Self, p: &str| s.palavra_em(p, ini_a, v.name.span.start).is_some();
                if !antes_do_nome(self, "external") {
                    self.kw("external");
                }
                if !antes_do_nome(self, "static") {
                    self.kw("static");
                    if conteiner.is_some_and(|c| matches!(self.especie(c), "ClassDeclaration" | "MixinDeclaration")) {
                        if !antes_do_nome(self, "abstract") {
                            self.kw("abstract");
                        }
                        if !antes_do_nome(self, "covariant") {
                            self.kw("covariant");
                        }
                    }
                    if !lista.late && conteiner.is_none_or(|c| self.especie(c) != "ExtensionDeclaration") {
                        self.kw("late");
                    }
                }
                if self.primeiro_depois_das_anotacoes(a).is_some_and(|i| self.span_tok(i).start == v.name.span.start) {
                    self.kw("const");
                    if conteiner.is_some_and(|c| self.especie(c) == "ClassDeclaration") {
                        self.kw("factory");
                    }
                    self.kw("get");
                    self.kw("operator");
                    self.kw("set");
                }
                if self.campo_solto(a) {
                    match conteiner.map(|c| self.especie(c)) {
                        Some("ClassDeclaration") => self.local("ClassDeclaration_member"),
                        Some("EnumDeclaration") => self.local("EnumDeclaration_member"),
                        Some("ExtensionDeclaration") => self.local("ExtensionDeclaration_member"),
                        Some("MixinDeclaration") => self.local("MixinDeclaration_member"),
                        _ => {}
                    }
                    let classe = conteiner.filter(|&c| matches!(self.especie(c), "ClassDeclaration" | "MixinDeclaration")).and_then(|c| self.classe_do_no(c));
                    self.sugerir_sobrescritas(classe, false);
                }
            } else if especie_avo == Some("TopLevelVariableDeclaration") {
                let a = avo.unwrap_or(n);
                if self.palavra_em("external", self.ini(a), v.name.span.start).is_none() {
                    self.kw("external");
                }
                if !lista.late && conteiner.is_none_or(|c| self.especie(c) != "ExtensionDeclaration") {
                    self.kw("late");
                }
            }
            return;
        }
        if let Some(init) = v.initializer {
            let ini = self.a.expr(init).span.start;
            let igual = self.op_em(Op::Assign, v.name.span.end, ini.max(v.name.span.end + 1));
            if igual.is_some_and(|i| self.offset >= self.span_tok(i).end) {
                self.local("VariableDeclaration_initializer");
                self.para_expressao(n, false, true);
                if let Some(t) = self.tipo_do_local(v.name.span.start).or_else(|| lista.ty.and_then(|t| {
                    let no = self.filhos(lista_no).iter().copied().find(|&k| self.lig(k) == Ligacao::Tipo(t))?;
                    self.tipo_do_no_de_tipo(no)
                })) && matches!(self.consulta.tabela.get(t), Type::Function { .. })
                {
                    let nomes: Vec<String> = match lista.ty.map(|x| &self.a.ty(x).kind) {
                        Some(ast::TypeKind::Function { parameters, .. }) => parameters
                            .iter()
                            .filter(|x| x.kind != ast::ParameterKind::Named)
                            .map(|x| x.name.map(|n| self.nome_real(n).to_string()).unwrap_or_default())
                            .collect(),
                        _ => Vec::new(),
                    };
                    self.sugerir_closure_com_nomes(t, &nomes, false);
                }
            }
        } else if let Some(i) = self.tok_depois(v.name.span.end)
            && self.e_op(i, Op::Assign)
            && self.offset >= self.span_tok(i).end
        {
            self.local("VariableDeclaration_initializer");
            self.para_expressao(n, false, true);
        }
    }

    pub(super) fn v_lista_de_variaveis(&mut self, n: usize) {
        let Some(l) = self.lista_real(n) else { return };
        let Some(primeira) = l.variables.first() else { return };
        if self.offset <= primeira.name.span.end {
            let tipo_cobre = l.ty.is_some_and(|t| {
                let s = self.a.ty(t).span;
                self.cobre(s.start, s.end)
            });
            if (l.ty.is_none() || tipo_cobre) && !l.var_ {
                self.local("VariableDeclarationList_type");
                self.para_tipo(n, TipoOpcoes::default());
            } else if l.ty.is_some_and(|t| matches!(self.a.ty(t).kind, ast::TypeKind::Record { .. })) {
                self.kw("in");
            }
        }
    }

    pub(super) fn v_comando_de_variaveis(&mut self, n: usize) {
        self.local("Block_statement");
        if self.comando_incompleto_anterior(n) {
            return;
        }
        if let Inicial::Tok(i) = self.inicial(n)
            && self.offset <= self.span_tok(i).end
        {
            self.para_comando(n);
        } else if self.offset >= self.fim(n)
            && let Some(p) = self.pai(n)
        {
            self.para_comando(p);
        }
    }

    pub(super) fn v_while(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.para_comando(n);
            return;
        }
        let Some(a) = self.op_em(Op::LParen, self.span_tok(palavra).end, self.fim(n)) else { return };
        let fecha = self.par(a).map_or(self.sintetico(self.fim(n)).start, |i| self.span_tok(i).start);
        if self.span_tok(a).end <= self.offset && self.offset <= fecha {
            let condicao = self.filhos(n).first().copied();
            let ok = match condicao {
                None => true,
                Some(c) => self.offset <= self.ini(c) || self.offset == self.fim(c),
            };
            if ok {
                self.local("WhileStatement_condition");
                self.para_expressao(n, false, true);
            }
        }
    }

    pub(super) fn v_with(&mut self, n: usize) {
        let pai = self.pai(n);
        let Some(w) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(w).start && pai.is_some_and(|p| self.especie(p) == "ClassDeclaration") {
            if let Some(p) = pai {
                self.kw_declaracao_de_classe(p);
            }
        } else if self.offset <= self.span_tok(w).end {
            self.kw("with");
        } else {
            self.local("WithClause_mixinType");
            self.para_tipo(n, TipoOpcoes { misturavel: true, ..TipoOpcoes::default() });
        }
    }

    pub(super) fn v_yield(&mut self, n: usize) {
        let Some(palavra) = self.tok_depois(self.ini(n)) else { return };
        if self.offset <= self.span_tok(palavra).end {
            self.local("Block_statement");
            self.kw("yield");
            return;
        }
        let pv = self.tok_antes(self.fim(n)).filter(|&i| self.e_op(i, Op::Semicolon));
        if pv.is_none_or(|p| self.offset <= self.span_tok(p).end) {
            self.local("YieldStatement_expression");
            self.para_expressao(n, false, false);
        }
    }
}

/// Os lexemas de palavra-chave do analyzer (exportado para as visitas).
pub(super) const PALAVRAS_DO_ANALYZER_PUB: &[&str] = super::isp::PALAVRAS_DO_ANALYZER;

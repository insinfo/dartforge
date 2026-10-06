//! Assistências sobre membros de classe, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to final field` | `refactor.convert.getterToFinalField` | `ConvertIntoFinalField` |
//! | `Convert to getter` | `refactor.convert.finalFieldToGetter` | `ConvertIntoGetter` |
//! | `Convert to normal parameter` | `refactor.convert.toConstructorNormalParameter` | `ConvertToNormalParameter` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, StmtKind};

/// As espécies de `TypeAnnotation`.
const TIPOS: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];

impl Contexto<'_> {
    fn acao_de_membro(&self, uri: &str, titulo: &str, especie: &str, span: Span, texto: String) -> AcaoDeCodigo {
        AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span, texto }],
            diagnostico: None,
            criar_arquivo: None,
        }
    }

    /// `ConvertIntoFinalField` (convert_into_final_field.dart).
    pub(crate) fn converter_em_campo_final(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        // O getter envolvente: passa por identificadores e tipos.
        let mut getter = None;
        for n in self.com_pais(no) {
            let e = self.especie(n);
            if e == "MethodDeclaration" {
                getter = Some(n);
                break;
            }
            if e == "SimpleIdentifier" || TIPOS.contains(&e) || e == "TypeArgumentList" {
                continue;
            }
            break;
        }
        let getter = getter?;
        let Marca::Funcao(fid) = self.arvore.nos[getter].marca else { return None };
        let f = self.ast.function(fid);
        if f.kind != ast::FunctionKind::Getter {
            return None;
        }
        let nome = f.name?;
        // Sem setter correspondente (`variable.setter2`).
        let elemento = self.p.funcao_do_no(self.unidade, fid)?;
        let prog = self.p.programa();
        let fe = prog.function(elemento);
        let chave = self.p.consulta.nomes.lookup(&format!("{}=", self.p.nome(fe.name)));
        let tem_setter = chave.is_some_and(|k| match (fe.class, fe.extension) {
            (Some(c), _) => {
                let k_ = prog.class(c);
                if fe.static_ { k_.static_members.contains_key(&k) } else { k_.instance_members.contains_key(&k) }
            }
            (None, Some(x)) => {
                let x_ = prog.extension(x);
                if fe.static_ { x_.static_members.contains_key(&k) } else { x_.instance_members.contains_key(&k) }
            }
            _ => false,
        });
        if tem_setter {
            return None;
        }
        // A expressão devolvida.
        let expressao = match &f.body {
            FunctionBody::Expression(e) => Some(*e),
            FunctionBody::Block(b) => match &self.ast.stmt(*b).kind {
                StmtKind::Block(cmds) if cmds.len() == 1 => match &self.ast.stmt(cmds[0]).kind {
                    StmtKind::Return(Some(e)) => Some(*e),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }?;
        let mut codigo = String::from("final");
        if let Some(t) = f.return_type {
            let s = self.ast.ty(t).span;
            codigo.push(' ');
            codigo.push_str(&self.fonte[s.start..s.end]);
        }
        codigo.push(' ');
        codigo.push_str(&self.fonte[nome.span.start..nome.span.end]);
        let es = self.ast.expr(expressao).span;
        if !matches!(self.ast.expr(expressao).kind, ast::ExprKind::Null) {
            codigo.push_str(" = ");
            codigo.push_str(&self.fonte[es.start..es.end]);
        }
        codigo.push(';');
        // `range.startEnd(returnType ?? propertyKeywordGet, getter)`.
        let inicio_troca = match f.return_type {
            Some(t) => self.ast.ty(t).span.start,
            None => self
                .tokens
                .iter()
                .filter(|t| t.span.end <= nome.span.start && &self.fonte[t.span.start..t.span.end] == "get")
                .map(|t| t.span.start)
                .next_back()?,
        };
        let faixa = Span { start: inicio_troca, end: self.arvore.nos[getter].fim };
        Some(self.acao_de_membro(uri, "Convert to final field", "refactor.convert.getterToFinalField", faixa, codigo))
    }

    /// `ConvertIntoGetter` (convert_into_getter.dart).
    pub(crate) fn converter_em_getter(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let mut declaracao = None;
        for n in self.com_pais(no) {
            let e = self.especie(n);
            if e == "FieldDeclaration" {
                declaracao = Some(n);
                break;
            }
            if matches!(e, "SimpleIdentifier" | "VariableDeclaration" | "VariableDeclarationList" | "TypeArgumentList") || TIPOS.contains(&e) {
                continue;
            }
            break;
        }
        let declaracao = declaracao?;
        let lista = self.filhos(declaracao).iter().copied().find(|&f| self.especie(f) == "VariableDeclarationList")?;
        let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
        if variaveis.len() != 1 {
            return None;
        }
        let campo = variaveis[0];
        // O `keyword` da lista (`final`, `const` ou `var`).
        let tipo = self.filhos(lista).iter().copied().find(|&f| TIPOS.contains(&self.especie(f)));
        let limite = tipo.map_or(self.arvore.nos[campo].inicio, |t| self.arvore.nos[t].inicio);
        let palavra = self
            .tokens
            .iter()
            .find(|t| t.span.start >= self.arvore.nos[lista].inicio && t.span.end <= limite && matches!(&self.fonte[t.span.start..t.span.end], "final" | "const" | "var"))?
            .span;
        let inicializador = *self.filhos(campo).first()?;
        let nome = self.token_seguinte(self.arvore.nos[campo].inicio)?;
        let mut codigo = String::new();
        if let Some(t) = tipo {
            codigo.push_str(self.texto_do_no(t));
            codigo.push(' ');
        }
        codigo.push_str("get ");
        codigo.push_str(&self.fonte[nome.start..nome.end]);
        codigo.push_str(" => ");
        codigo.push_str(self.texto_do_no(inicializador));
        codigo.push(';');
        let faixa = Span { start: palavra.start, end: self.arvore.nos[declaracao].fim };
        Some(self.acao_de_membro(uri, "Convert to getter", "refactor.convert.finalFieldToGetter", faixa, codigo))
    }

    /// `ConvertToNormalParameter` (convert_to_normal_parameter.dart): um
    /// `this.x` direto na lista de parâmetros de um construtor vira
    /// parâmetro comum com o inicializador `x = x`.
    pub(crate) fn converter_em_parametro_normal(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let parametro = self.arvore.localizar(inicio, fim)?;
        if self.especie(parametro) != "FieldFormalParameter" {
            return None;
        }
        let lista = self.pai(parametro)?;
        if self.especie(lista) != "FormalParameterList" {
            return None;
        }
        let construtor = self.pai(lista)?;
        if self.especie(construtor) != "ConstructorDeclaration" {
            return None;
        }
        // O nome: o último token do parâmetro (`this.x`).
        let nome_span = self.token_anterior(self.arvore.nos[parametro].fim)?;
        let nome = self.fonte[nome_span.start..nome_span.end].to_string();
        // O tipo do elemento: o do parâmetro do construtor, pela posição.
        let indice = self.filhos(lista).iter().position(|&k| k == parametro)?;
        let fim_do_no = self.arvore.nos[construtor].fim;
        let mi = self.ast.members.iter().position(|m| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.end == fim_do_no)?;
        let elemento = self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32))?;
        let tipo = self.p.consulta.outline.functions.get(elemento.0 as usize)?.parameters.get(indice)?.ty;
        let mut importar = std::collections::BTreeSet::new();
        let substituto = if matches!(self.p.consulta.tabela.get(tipo), dartforge_types::Type::Dynamic) {
            nome.clone()
        } else {
            let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[parametro].inicio);
            let escrito = escritor.escrever(tipo, false).unwrap_or_default();
            importar = escritor.importar;
            format!("{escrito} {nome}")
        };
        let inicializadores: Vec<usize> = self
            .filhos(construtor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.especie(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let (em, texto) = match inicializadores.last() {
            None => (self.arvore.nos[lista].fim, format!(" : {nome} = {nome}")),
            Some(&u) => (self.arvore.nos[u].fim, format!(", {nome} = {nome}")),
        };
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        m.adicionar(uri, self.arvore.span(parametro), substituto);
        m.adicionar(uri, Span { start: em, end: em }, texto);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to normal parameter".into(),
            especie: "refactor.convert.toConstructorNormalParameter".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}

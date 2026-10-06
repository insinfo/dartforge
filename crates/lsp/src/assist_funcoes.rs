//! Assistências sobre o corpo de funções, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to async function body` | `refactor.convert.bodyToAsync` | `ConvertIntoAsyncBody` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_types::{Type, TypeId};

impl Contexto<'_> {
    /// `getEnclosingFunctionBody` (correction_producer.dart:729-747).
    pub(crate) fn corpo_envolvente(&self, no: usize) -> Option<usize> {
        let ancestral = |especie: &str| self.com_pais(no).find(|&k| self.especie(k) == especie);
        let corpo_de = |n: usize| self.filhos(n).iter().copied().rev().find(|&f| self.especie(f).ends_with("FunctionBody"));
        if let Some(f) = ancestral("FunctionExpression") {
            return corpo_de(f);
        }
        if let Some(d) = ancestral("FunctionDeclaration") {
            let f = self.filhos(d).iter().copied().find(|&k| self.especie(k) == "FunctionExpression")?;
            return corpo_de(f);
        }
        if let Some(c) = ancestral("ConstructorDeclaration") {
            return corpo_de(c);
        }
        let m = ancestral("MethodDeclaration")?;
        corpo_de(m)
    }

    /// A função do parser cujo corpo é `corpo` (a marca do pai).
    fn funcao_de_corpo(&self, corpo: usize) -> Option<ast::FunctionId> {
        match self.arvore.nos[self.pai(corpo)?].marca {
            Marca::Funcao(fid) => Some(fid),
            _ => {
                // A `FunctionExpression` de uma `FunctionDeclaration`: a marca
                // está no avô.
                let avo = self.pai(self.pai(corpo)?)?;
                match self.arvore.nos[avo].marca {
                    Marca::Funcao(fid) => Some(fid),
                    _ => None,
                }
            }
        }
    }

    /// `typeSystem.flatten`: `Future<S>` e `FutureOr<S>` dão `S`.
    fn achatar(&self, t: TypeId) -> TypeId {
        let tabela = &self.p.consulta.tabela;
        match tabela.get(t) {
            Type::FutureOr { arg, .. } => *arg,
            Type::Interface { class, args, .. } if Some(*class) == self.p.consulta.core.future_class && args.len() == 1 => args[0],
            _ => t,
        }
    }

    /// `ConvertIntoAsyncBody` (convert_into_async_body.dart) com o
    /// `convertFunctionFromSyncToAsync` do `DartFileEditBuilder`.
    pub(crate) fn converter_em_corpo_assincrono(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let corpo = self.corpo_envolvente(no)?;
        if self.especie(corpo) == "EmptyFunctionBody" {
            return None;
        }
        let pai = self.pai(corpo)?;
        if self.especie(pai) == "ConstructorDeclaration" {
            return None;
        }
        let fid = self.funcao_de_corpo(corpo)?;
        let f = self.ast.function(fid);
        // `body.keyword != null`: `async`, `async*`, `sync*`.
        if f.modifier != ast::AsyncModifier::None {
            return None;
        }
        // Só no começo do corpo: até o fim do `{` ou do `=>`.
        let primeiro = self.token_seguinte(self.arvore.nos[corpo].inicio)?;
        if inicio > primeiro.end {
            return None;
        }
        // A closure não tem tipo de retorno escrito, e o Dart não a converte.
        let declaracao = if self.especie(pai) == "FunctionExpression" {
            let d = self.pai(pai).filter(|&d| self.especie(d) == "FunctionDeclaration")?;
            Some(d)
        } else if self.especie(pai) == "MethodDeclaration" {
            Some(pai)
        } else {
            None
        };
        let mut edicoes = Vec::new();
        // `async ` no começo do corpo, com um espaço antes se o corpo está
        // colado ao token anterior.
        let colado = self.token_anterior(primeiro.start).is_some_and(|t| t.end == primeiro.start);
        edicoes.push((Span { start: primeiro.start, end: primeiro.start }, if colado { " async ".to_string() } else { "async ".to_string() }));
        let mut importar = std::collections::BTreeSet::new();
        // `_replaceReturnTypeWithFuture` → `replaceTypeWithFuture`.
        if declaracao.is_some()
            && let Some(anotacao) = f.return_type
        {
            let tipo = match self.p.funcao_do_no(self.unidade, fid) {
                Some(e) => self.p.consulta.outline.functions.get(e.0 as usize).map(|d| d.return_type),
                None => self.corpos.tipos_de_anotacoes.get(&anotacao).copied(),
            };
            if let Some(t) = tipo {
                let tabela = &self.p.consulta.tabela;
                let e_future = matches!(tabela.get(t), Type::Interface { class, .. } if Some(*class) == self.p.consulta.core.future_class);
                if !matches!(tabela.get(t), Type::Dynamic) && !e_future {
                    let valor = self.achatar(t);
                    let s = self.ast.ty(anotacao).span;
                    let mut escritor = crate::escrever_tipo::Escritor::novo(self, s.start);
                    let interno = escritor.escrever(valor, true).unwrap_or_else(|| "dynamic".to_string());
                    let future = match self.p.consulta.core.future_class {
                        Some(c) => escritor.referencia_de(dartforge_elements::model::Element::Class(c), "Future"),
                        None => "Future".to_string(),
                    };
                    edicoes.push((s, format!("{future}<{interno}>")));
                    importar = escritor.importar;
                }
            }
        }
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        for (s, texto) in edicoes {
            m.adicionar(uri, s, texto);
        }
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to async function body".into(),
            especie: "refactor.convert.bodyToAsync".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect::<Vec<Edicao>>(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}

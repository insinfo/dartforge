//! As operações do `NotImportedCompletionPass` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/not_imported_completion_pass.dart`):
//! `StaticMembersOperation` (`addNotImportedTopLevelDeclarations`),
//! `ConstructorsOperation` (`addNotImportedConstructors`) e
//! `InstanceExtensionMembersOperation` (`addNotImportedExtensionMethods`),
//! pelo mesmo `DeclarationHelper` do passe (as opções e o
//! `VisibilityTracker` dele), sobre uma biblioteca ainda não importada que
//! a consulta já carregou. Cada item novo leva o `import` que o torna
//! visível.
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Isp, Operacao};
use super::isp_declaracoes::Importe;
use super::*;
use dartforge_elements::model::ExtensionId;

impl<'a, 'c> Isp<'a, 'c> {
    /// Roda as operações registradas para a biblioteca `lib`, importada
    /// pela URI `uri`.
    pub(super) fn operacoes_nao_importadas(&mut self, lib: LibraryId, uri: &str) {
        use dartforge_elements::model::ClassKind as K;
        let imp = Importe { prefixo: None, nao_importado: true };
        let antes = self.coletor.itens.len();
        let operacoes = self.operacoes.clone();
        for op in operacoes {
            match op {
                Operacao::MembrosEstaticos => {
                    let p = &self.consulta.programa;
                    let mut espaco: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(lib).exported.iter().map(|(&s, &b)| (s, b)).collect();
                    espaco.sort_by(|a, b| self.consulta.nome(a.0).cmp(self.consulta.nome(b.0)));
                    self.declaracoes_externas(&espaco, &imp);
                }
                Operacao::Construtores => {
                    let unidades = self.consulta.programa.library(lib).units.clone();
                    for u in unidades {
                        let e = self.elementos_da_unidade(u);
                        for c in e.classes {
                            if self.consulta.programa.class(c).kind == K::MixinApplication && self.consulta.programa.class(c).decl.is_none() {
                                continue;
                            }
                            let abstrato = self.consulta.programa.class(c).modifiers.abstract_;
                            self.sugerir_construtores(c, Some(&imp), !abstrato);
                        }
                        for c in e.enums {
                            self.sugerir_construtores(c, Some(&imp), true);
                        }
                        for c in e.tipos_de_extensao {
                            self.sugerir_construtores(c, Some(&imp), true);
                        }
                        for t in e.typedefs {
                            self.construtores_do_alias(t, Some(&imp));
                        }
                    }
                }
                Operacao::MembrosDeExtensao { tipo, metodos, setters, .. } => {
                    // `applicableTo(targetType: Null ? tipo : promoteToNonNull(tipo))`.
                    let alvo = if matches!(self.consulta.tabela.get(tipo), Type::Null) { tipo } else { dartforge_types::non_nullable(tipo, &mut self.consulta.tabela) };
                    let p = &self.consulta.programa;
                    let mut exportadas: Vec<(String, ExtensionId)> = p
                        .library(lib)
                        .exported
                        .iter()
                        .filter_map(|(s, b)| match b.getter {
                            Some(Element::Extension(x)) => Some((self.consulta.nome(*s).to_string(), x)),
                            _ => None,
                        })
                        .collect();
                    exportadas.sort();
                    let candidatas: Vec<ExtensionId> = exportadas.into_iter().map(|(_, x)| x).collect();
                    let aplicaveis = {
                        let Consulta { programa, nomes, tabela, core, outline, .. } = &mut *self.consulta;
                        dartforge_types::extensoes_aplicaveis_dentre(programa, nomes, tabela, core, outline, &candidatas, alvo)
                    };
                    let biblioteca = self.biblioteca();
                    for (x, _) in aplicaveis {
                        let ext = self.consulta.programa.extension(x);
                        let visivel = ext.library == biblioteca || ext.name.is_some_and(|n| !self.consulta.nome(n).starts_with('_'));
                        if visivel {
                            self.membros_da_extensao_nao_importada(x, &imp, metodos, setters);
                        }
                    }
                }
            }
        }
        // O `import` que torna o item visível (`requiredImports`).
        let (span, texto) = crate::acoes::inserir_import(self.fonte, self.cu, uri);
        for i in &mut self.coletor.itens[antes..] {
            i.importar = Some(ImportAutomatico { uri: uri.to_string(), span, texto: texto.clone() });
            // O `detail` da lista é o do elemento; o `Auto import from` vem
            // no `completionItem/resolve` (`resolveDartCompletion`).
            i.rel.nao_importado = true;
        }
    }

    /// `addMembersFromExtensionElement`: os métodos e os acessores de
    /// instância, com a interface do tipo estendido como referência da
    /// distância de herança.
    fn membros_da_extensao_nao_importada(&mut self, x: ExtensionId, imp: &Importe, metodos: bool, setters: bool) {
        let on = self.consulta.outline.extensions[x.0 as usize].on;
        let referencia = match self.consulta.tabela.get(on) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let (acessores, _, ms) = self.membros_declarados(None, Some(x));
        let _ = (metodos, setters);
        for m in ms {
            if !self.consulta.programa.function(m).static_ {
                self.sugerir_metodo(m, None, false, Some(imp), referencia);
            }
        }
        for f in acessores {
            if !self.consulta.programa.function(f).static_ {
                self.sugerir_propriedade(f, None, false, Some(imp), referencia);
            }
        }
    }
}

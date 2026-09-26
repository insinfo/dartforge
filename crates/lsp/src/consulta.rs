//! Uma consulta semântica transitória: o programa carregado com os textos
//! vigentes do editor, o outline e os corpos inferidos pela inferência comum
//! de `crates/types` ([`dartforge_types::BodyInferrer`]). O completar e o
//! renomear leem tipos e resoluções daqui, nunca de uma inferência própria.
//!
//! Tudo pertence a uma requisição: a consulta é descartada ao responder, e
//! nada dela sobrevive à próxima versão do texto (o platô de memória do LSP).

use dartforge_elements::model::{FunctionElementId, LibraryId, Program, UnitId, VariableId};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::scope::MemberResolver;
use dartforge_types::{
    BodyInferrer, BodyTypes, CoreTypes, EscopoSondado, OutlineTypes, Type, TypeId, TypeTable,
    resolve_outline,
};

/// Programa, tipos e corpos de uma requisição.
pub(crate) struct Consulta {
    pub programa: Program,
    pub nomes: Interner,
    pub tabela: TypeTable,
    pub core: CoreTypes,
    pub outline: OutlineTypes,
    pub corpos: BodyTypes,
    /// Escopo capturado pela sonda, quando pedida e alcançada.
    pub escopo: Option<EscopoSondado>,
}

impl Consulta {
    /// Resolve o outline e infere os corpos de `bibliotecas`.
    ///
    /// `registrar_locais` liga a tabela de declarações de locais (renomear);
    /// `sonda` é o identificador cujo escopo léxico o completar quer.
    pub fn inferir(
        programa: Program,
        nomes: Interner,
        bibliotecas: &[LibraryId],
        registrar_locais: bool,
        sonda: Option<(UnitId, usize)>,
    ) -> Self {
        let mut tabela = TypeTable::new();
        let core = CoreTypes::init(&mut tabela, &programa, &nomes);
        let (mut outline, _) = resolve_outline(&programa, &nomes, &mut tabela, &core);
        let (corpos, escopo) = {
            let mut inferidor =
                BodyInferrer::new(&programa, &nomes, &mut tabela, &core, &mut outline);
            inferidor.apenas_bibliotecas = Some(bibliotecas.iter().map(|l| l.0).collect());
            inferidor.registrar_locais = registrar_locais;
            inferidor.sonda_escopo = sonda;
            let (corpos, _, escopo) = inferidor.infer_all_com_sonda();
            (corpos, escopo)
        };
        Self {
            programa,
            nomes,
            tabela,
            core,
            outline,
            corpos,
            escopo,
        }
    }

    /// Texto de um tipo como o Dart o escreve.
    pub fn formatar(&self, tipo: TypeId) -> String {
        self.tabela.format(tipo, &self.nomes, &self.programa)
    }

    /// Texto de um símbolo.
    pub fn nome(&self, simbolo: SymbolId) -> &str {
        self.nomes.resolve(simbolo)
    }

    /// Busca de membros sobre receptores estáticos (a mesma de `crates/types`).
    pub fn resolvedor(&mut self) -> MemberResolver<'_> {
        MemberResolver::new(
            &self.programa,
            &self.outline,
            &self.nomes,
            &self.outline.hierarchy,
            &mut self.tabela,
            &self.core,
        )
    }

    /// Tipo de uma variável de topo ou campo: o escrito, senão o inferido.
    pub fn tipo_da_variavel(&self, variavel: VariableId) -> Option<TypeId> {
        let dados = self.outline.variables.get(variavel.0 as usize)?;
        dados.declared_type.or(dados.inferred)
    }

    /// Detalhe de uma função (`(int x, {String? nome}) → void`) a partir da
    /// assinatura `assinatura` (já substituída pelo receptor, quando há) e dos
    /// nomes dos parâmetros declarados.
    pub fn detalhe_de_funcao(&self, funcao: FunctionElementId, assinatura: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.tabela.get(assinatura).clone()
        else {
            return self.formatar(assinatura);
        };
        let dados = self.outline.functions.get(funcao.0 as usize);
        let nomes_posicionais: Vec<Option<SymbolId>> = dados
            .map(|d| {
                d.parameters
                    .iter()
                    .filter(|p| p.kind != dartforge_frontend::ast::ParameterKind::Named)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let mut partes = Vec::new();
        for (i, t) in positional.iter().enumerate() {
            partes.push(self.parametro(*t, nomes_posicionais.get(i).copied().flatten()));
        }
        if !optional.is_empty() {
            let opcionais: Vec<String> = optional
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    self.parametro(
                        *t,
                        nomes_posicionais
                            .get(positional.len() + i)
                            .copied()
                            .flatten(),
                    )
                })
                .collect();
            partes.push(format!("[{}]", opcionais.join(", ")));
        }
        if !named.is_empty() {
            // A ordem da declaração, não a canônica do tipo (por símbolo).
            let mut nomeados: Vec<(usize, String)> = named
                .iter()
                .map(|(n, t, obrigatorio)| {
                    let ordem = dados
                        .and_then(|d| {
                            d.parameters
                                .iter()
                                .position(|p| p.externo.or(p.name) == Some(*n))
                        })
                        .unwrap_or(usize::MAX);
                    let prefixo = if *obrigatorio { "required " } else { "" };
                    (
                        ordem,
                        format!("{prefixo}{} {}", self.formatar(*t), self.nome(*n)),
                    )
                })
                .collect();
            nomeados.sort();
            partes.push(format!(
                "{{{}}}",
                nomeados
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        format!("({}) → {}", partes.join(", "), self.formatar(ret))
    }

    fn parametro(&self, tipo: TypeId, nome: Option<SymbolId>) -> String {
        match nome {
            Some(n) => format!("{} {}", self.formatar(tipo), self.nome(n)),
            None => self.formatar(tipo),
        }
    }
}

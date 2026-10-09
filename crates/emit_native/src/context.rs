//! Contexto de compilação da crate `dartforge-emit-native`.

use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::{BodyTypes, Resolved};
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};


pub struct Context<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
    /// Identidade do marcador do SDK, nunca de uma classe homônima do usuário.
    finalizable_class: Option<dartforge_elements::model::ClassId>,
    pub entry_lib: Option<LibraryId>,
    /// Id de classe do runtime de cada classe do programa (P2): a ordem do
    /// caminho estável (`nome_da_biblioteca`, nome da classe), a partir de 1,
    /// pulando a faixa 1000–1012 das classes de erro do runtime. `None` para
    /// as classes do SDK.
    pub ids_de_classe: Vec<Option<u32>>,
    /// Com o SDK da fonte, os ids das classes do SDK vêm desta tabela
    /// (`sdk_modulo::ids_de_classe_do_sdk`), a mesma do módulo do SDK: o
    /// programa carrega só as bibliotecas que importa, e numerar pelo que
    /// carregou daria ids diferentes dos do SDK compilado.
    pub ids_fixos_do_sdk: Option<std::sync::Arc<TabelaDeIds>>,
    /// As formas de record com campo nomeado do programa (P3): número de
    /// posicionais e os nomes, ordenados — cada uma é uma "classe" de record
    /// com id `ID_BASE_DE_FORMA + índice`.
    pub formas_de_record: Vec<(usize, Vec<String>)>,
    /// Diretório da biblioteca de entrada: as bibliotecas `file:` são
    /// nomeadas pelo caminho relativo a ele (estável entre máquinas).
    raiz: Option<std::path::PathBuf>,
    /// Este contexto baixa uma biblioteca do SDK em um objeto separado.
    pub biblioteca_sdk: bool,
    /// Por biblioteca: o corpo das funções dela é compilado (as do programa;
    /// com [`Context::com_sdk_da_fonte`], também as do SDK da fonte).
    pub compiladas: Vec<bool>,
    /// Por biblioteca: as funções dela são baixadas **neste** módulo (as do
    /// programa; num módulo do SDK da fonte, só a biblioteca dele). As outras
    /// compiladas moram em outro objeto e são chamadas pelo símbolo.
    pub no_modulo: Vec<bool>,
    /// P6: as bibliotecas do SDK compiladas da fonte com o programa
    /// (`fonte.rs`); o `is_sdk` delas já está desligado na cópia do
    /// `Program`. Vazio para quem não usa `dart:async`.
    pub da_fonte: std::collections::HashSet<LibraryId>,
    /// RTI: a posição de cada classe do SDK (sem id de classe do heap) na
    /// ordem do caminho estável — o id RTI é `0x2000_0000 +` ela
    /// (`lower::rti`, `Context::id_rti`).
    pub ids_rti_sdk: std::collections::HashMap<dartforge_elements::model::ClassId, u32>,
    /// P6: o programa usa `dart:async` e tem o laço de eventos depois do
    /// `main` (com o SDK da fonte o `dart:async` é o do módulo em cache, e
    /// `da_fonte` fica vazio).
    pub usa_dart_async: bool,
    /// P6: os símbolos das funções da fonte com corpo (`lower::com_corpo_da_fonte`).
    pub com_corpo_da_fonte: std::sync::OnceLock<std::collections::HashSet<String>>,
    /// `dart:ffi`: o layout das structs e unions do programa (`lower::ffi::compostos`).
    pub compostos_ffi: std::sync::OnceLock<Option<crate::lower::ffi::Compostos>>,
    /// Os tipos de extensão apagados (`apagamento.rs`), calculados antes do
    /// contexto; vazio sem tipo de extensão.
    pub te: crate::apagamento::TiposDeExtensao,
    /// J05: com informação de depuração, o início de cada linha de cada
    /// unidade (pelo índice da unidade), para as posições das instruções.
    /// Ligado também pelo rastro simbólico (§13.14), que usa as posições sem
    /// o DWARF.
    pub depuracao: Option<Vec<Vec<u32>>>,
    /// J05: as posições viram tabelas de linha do depurador (`--depuracao`);
    /// sem isto, só o rastro simbólico as usa.
    pub dwarf: bool,
    /// O rastro simbólico (§13.14) está ligado: o lowering marca as funções
    /// (quadros ocultos, corpos `async`, elos de quem espera) e insere a
    /// pilha do rastro.
    pub rastro: bool,
    /// Recarga do JIT (J03): os ids que a geração viva deu às classes do
    /// programa, por `(biblioteca, classe)`. A mesma classe fica com o mesmo
    /// id; uma classe nova ganha um id acima de todos eles (os objetos vivos
    /// no heap guardam o id).
    ids_anteriores: Option<std::collections::HashMap<(String, String), u32>>,
    /// O mundo fechado do programa (`mundo_nativo.rs`): a função ou o
    /// global do programa fora dele vira corpo que lança. `None`: sem poda
    /// (os módulos do SDK, `DARTFORGE_SEM_PODA_DO_PROGRAMA=1`).
    pub mundo: Option<dartforge_mundo::Mundo>,
    /// Memória de `sdk_fonte::implementacoes` por (classe, nome): a busca
    /// percorre TODAS as classes do programa e a linearização de cada uma, e
    /// é feita a cada acesso a membro pelo despacho por classe. Num programa
    /// real (o new_sali/backend: milhares de classes, centenas de milhares
    /// de acessos) era quadrática e dominava a emissão (minutos). O
    /// resultado só depende do programa e das bibliotecas compiladas, fixos
    /// depois da construção do contexto.
    pub memoria_implementacoes:
        std::sync::RwLock<std::collections::HashMap<(dartforge_elements::model::ClassId, SymbolId), std::sync::Arc<Vec<crate::lower::sdk_fonte::Implementacao>>>>,
    /// Os subtipos de cada classe (`Context::subtipos`), calculados na
    /// primeira consulta.
    pub memoria_subtipos:
        std::sync::RwLock<std::collections::HashMap<dartforge_elements::model::ClassId, std::sync::Arc<Vec<dartforge_elements::model::ClassId>>>>,
    /// Idem para `sdk_fonte::implementacoes_por_classe`.
    pub memoria_implementacoes_por_classe: std::sync::RwLock<
        std::collections::HashMap<
            (dartforge_elements::model::ClassId, SymbolId),
            Option<std::sync::Arc<Vec<(i64, crate::lower::sdk_fonte::Implementacao)>>>,
        >,
    >,
}

/// O nome da variável de um padrão `:x`/`:var x`/`:x?`/`:x as T`.
pub fn nome_de_variavel_do_padrao(
    ast: &dartforge_frontend::ast::Ast,
    mut p: dartforge_frontend::ast::PatternId,
) -> Option<SymbolId> {
    use dartforge_frontend::ast::PatternKind;
    loop {
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, .. } => return Some(name.sym),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => p = *x,
            _ => return None,
        }
    }
}

/// Primeiro id de classe das formas de record com campo nomeado (bem acima
/// dos ids das classes do programa).
pub const ID_BASE_DE_FORMA: u32 = 0x4000_0000;

/// Escapa uma parte de um símbolo estável: letras, dígitos e `_` ficam; o
/// resto vira `$` e dois dígitos hexadecimais por byte UTF-8. O `.` separa as
/// partes, então nunca aparece cru dentro de uma — o símbolo é injetivo.
pub fn escapar(parte: &str) -> String {
    let mut s = String::with_capacity(parte.len());
    for b in parte.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' {
            s.push(b as char);
        } else {
            s.push_str(&format!("${b:02x}"));
        }
    }
    s
}

impl<'a> Context<'a> {
    /// A função `f` do programa ficou fora do mundo fechado (vira corpo que
    /// lança). Só funções de bibliotecas do programa, com nó de função
    /// (métodos, de topo, acessores); construtores nunca.
    pub fn funcao_podada(&self, f: usize) -> bool {
        let Some(m) = &self.mundo else { return false };
        let func = &self.program.functions[f];
        !self.program.library(func.library).is_sdk
            && matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. })
            && !m.funcao(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// O campo de instância `v` de uma classe do programa ficou fora do
    /// mundo fechado (ninguém o lê nem grava).
    pub fn campo_podado(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.mundo else { return false };
        let var = &self.program.variables[v.0 as usize];
        if self.program.library(var.library).is_sdk {
            return false;
        }
        // O mundo acompanha os acessores implícitos do campo (membros de
        // instância com `variable`), não a variável.
        let Some(c) = var.class else { return false };
        let mut acessores = self.program.classes[c.0 as usize]
            .instance_members
            .values()
            .filter(|f| self.program.functions[f.0 as usize].variable == Some(v))
            .peekable();
        acessores.peek().is_some() && acessores.all(|f| !m.funcao(*f))
    }

    /// O membro de instância `f` de uma classe do programa (método, acessor
    /// explícito ou implícito de campo) ficou fora do mundo fechado: sem
    /// entrada na tabela de métodos nem adaptador.
    pub fn membro_podado(&self, f: usize) -> bool {
        let Some(m) = &self.mundo else { return false };
        let func = &self.program.functions[f];
        // Só o que tem corpo escrito (`FunctionRef::Function`) ou é acessor
        // de campo: os membros sintéticos (os encaminhadores de
        // `noSuchMethod` que o CFE criaria) não passam pelo mundo, e sem
        // entrada na tabela a chamada caía no `NoSuchMethodError`
        // (corpus/js/223_nosuchmethod_argumentos).
        let com_no = matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. }) || func.variable.is_some();
        com_no
            && !self.program.library(func.library).is_sdk
            && !matches!(
                func.kind,
                dartforge_elements::model::FunctionKind::Constructor | dartforge_elements::model::FunctionKind::SyntheticConstructor
            )
            && !m.funcao(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// O global `v` do programa ficou fora do mundo fechado.
    pub fn global_podado(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.mundo else { return false };
        let var = &self.program.variables[v.0 as usize];
        // Uma `const` fica sempre: ela é valor padrão de parâmetro de membro
        // abstrato, que o encaminhador de `noSuchMethod` gerado pelo lowering
        // lê e o mundo não vê (o `Pintor.padrao` de
        // corpus/js/223_nosuchmethod_argumentos); o getter é pequeno.
        !var.const_ && !self.program.library(var.library).is_sdk && !m.variavel(v)
    }

    /// As classes do programa que são subtipo de `cid` (ela inclusive), na
    /// ordem dos ids de elemento — o que as buscas por implementação
    /// percorriam varrendo TODAS as classes a cada acesso a membro.
    pub fn subtipos(&self, cid: dartforge_elements::model::ClassId) -> std::sync::Arc<Vec<dartforge_elements::model::ClassId>> {
        if let Some(r) = self.memoria_subtipos.read().unwrap_or_else(|e| e.into_inner()).get(&cid) {
            return r.clone();
        }
        let r: std::sync::Arc<Vec<_>> = std::sync::Arc::new(
            (0..self.program.classes.len() as u32)
                .map(dartforge_elements::model::ClassId)
                .filter(|&k| crate::lower::membros::subclasse_de(self, k, cid))
                .collect(),
        );
        self.memoria_subtipos.write().unwrap_or_else(|e| e.into_inner()).insert(cid, r.clone());
        r
    }

    /// Liga a informação de depuração (J05): indexa as linhas das unidades.
    pub fn ligar_depuracao(&mut self) {
        let linhas = self
            .program
            .units
            .iter()
            .map(|u| {
                std::iter::once(0)
                    .chain(u.source.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i as u32 + 1))
                    .collect()
            })
            .collect();
        self.depuracao = Some(linhas);
    }

    /// A url do script da unidade no rastro da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    /// §13.14): a da unidade (`file:///…`, `package:x/y.dart`); no SDK, a da
    /// biblioteca para a unidade que a define (`dart:core`) e
    /// `dart:<biblioteca>/<arquivo>` para as partes (`dart:core/list.dart`),
    /// com `-patch` nas da VM e da sobreposição nativa
    /// (`dart:core-patch/growable_array.dart`).
    pub fn url_do_rastro(&self, unit: dartforge_elements::model::UnitId) -> String {
        let u = self.program.unit(unit);
        let biblioteca = &self.program.library(u.library).uri;
        let Some(nome) = biblioteca.strip_prefix("dart:") else { return u.uri.clone() };
        if u.uri == *biblioteca {
            return biblioteca.clone();
        }
        let arquivo = u
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| u.uri.rsplit('/').next().unwrap_or_default().to_string());
        let remendo = u
            .path
            .as_ref()
            .is_some_and(|p| p.components().any(|c| matches!(c.as_os_str().to_str(), Some("_internal" | "sdk_nativo"))));
        if remendo { format!("dart:{nome}-patch/{arquivo}") } else { format!("dart:{nome}/{arquivo}") }
    }

    /// `(linha, coluna)`, a partir de 1, do byte `offset` da unidade `unit`,
    /// com a depuração ligada.
    pub fn linha_e_coluna(&self, unit: dartforge_elements::model::UnitId, offset: usize) -> Option<(u32, u32)> {
        let inicios = self.depuracao.as_ref()?.get(unit.0 as usize)?;
        let offset = u32::try_from(offset).ok()?;
        let linha = inicios.partition_point(|&i| i <= offset);
        let inicio = inicios[linha.checked_sub(1)?];
        Some((linha as u32, offset - inicio + 1))
    }

    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Self {
        let raiz = program.entry.and_then(|l| {
            let u = *program.library(l).units.first()?;
            program.unit(u).path.as_ref()?.parent().map(|p| p.to_path_buf())
        });
        let mut ctx = Self {
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            finalizable_class: program.classes.iter().enumerate().find_map(|(i, c)| {
                (program.library(c.library).uri == "dart:ffi" && interner.resolve(c.name) == "Finalizable")
                    .then_some(dartforge_elements::model::ClassId(i as u32))
            }),
            entry_lib: program.entry,
            ids_de_classe: Vec::new(),
            ids_fixos_do_sdk: None,
            formas_de_record: Vec::new(),
            raiz,
            biblioteca_sdk: false,
            compiladas: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            no_modulo: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            da_fonte: std::collections::HashSet::new(),
            ids_rti_sdk: std::collections::HashMap::new(),
            usa_dart_async: false,
            com_corpo_da_fonte: std::sync::OnceLock::new(),
            compostos_ffi: std::sync::OnceLock::new(),
            memoria_implementacoes: Default::default(),
            mundo: None,
            memoria_subtipos: Default::default(),
            memoria_implementacoes_por_classe: Default::default(),
            te: crate::apagamento::TiposDeExtensao::default(),
            ids_anteriores: None,
            depuracao: None,
            dwarf: false,
            rastro: false,
        };
        // Formas de record com campo nomeado: literais, padrões e tipos de
        // todas as unidades do programa (o conjunto inteiro, antes do
        // lowering — um acesso `r.x` sem tipo testa todas as que têm `x`).
        let mut formas = std::collections::BTreeSet::new();
        for u in &program.units {
            if program.library(u.library).is_sdk {
                continue;
            }
            for e in &u.ast.exprs {
                if let dartforge_frontend::ast::ExprKind::Record { positional, named, .. } = &e.kind
                    && !named.is_empty()
                {
                    let mut n: Vec<String> = named.iter().map(|(k, _)| interner.resolve(k.sym).to_string()).collect();
                    n.sort();
                    formas.insert((positional.len(), n));
                }
            }
            for p in &u.ast.patterns {
                if let dartforge_frontend::ast::PatternKind::Record { fields } = &p.kind {
                    let mut npos = 0;
                    let mut n = Vec::new();
                    for f in fields.iter() {
                        match f.name {
                            Some(k) => n.push(interner.resolve(k.sym).to_string()),
                            None => {
                                // `:x` — o nome é o da variável dentro do campo.
                                let texto = &u.source[f.span.start as usize..f.span.end as usize];
                                if texto.trim_start().starts_with(':')
                                    && let Some(s) = nome_de_variavel_do_padrao(&u.ast, f.pattern)
                                {
                                    n.push(interner.resolve(s).to_string());
                                } else {
                                    npos += 1;
                                }
                            }
                        }
                    }
                    if !n.is_empty() {
                        n.sort();
                        formas.insert((npos, n));
                    }
                }
            }
            for t in &u.ast.types {
                if let dartforge_frontend::ast::TypeKind::Record { positional, named } = &t.kind
                    && !named.is_empty()
                {
                    let mut n: Vec<String> = named.iter().map(|(k, _)| interner.resolve(k.sym).to_string()).collect();
                    n.sort();
                    formas.insert((positional.len(), n));
                }
            }
        }
        ctx.formas_de_record = formas.into_iter().collect();
        ctx.numerar_classes();
        ctx
    }

    /// Liga o SDK da fonte (P5c): as bibliotecas de `BIBLIOTECAS_DA_FONTE`
    /// passam a ter corpo compilado e classes com id.
    pub fn com_sdk_da_fonte(mut self) -> Self {
        for (i, l) in self.program.libraries.iter().enumerate() {
            if let Some(nome) = l.uri.strip_prefix("dart:")
                && crate::sdk_modulo::BIBLIOTECAS_DA_FONTE.contains(&nome)
            {
                self.compiladas[i] = true;
            }
        }
        self.numerar_classes();
        self
    }

    /// [`Context::com_sdk_da_fonte`] com os ids das classes do SDK fixados
    /// pela tabela do SDK compilado.
    pub fn com_sdk_da_fonte_e_ids(mut self, ids: std::sync::Arc<TabelaDeIds>) -> Self {
        self.ids_fixos_do_sdk = Some(ids);
        self.com_sdk_da_fonte()
    }

    /// O módulo de uma biblioteca do SDK da fonte (P5c): só ela é baixada
    /// aqui; o programa e as outras bibliotecas ficam de fora.
    pub fn so_a_biblioteca(mut self, lib: LibraryId) -> Self {
        self.biblioteca_sdk = true;
        self.no_modulo = vec![false; self.program.libraries.len()];
        self.no_modulo[lib.0 as usize] = true;
        self
    }

    /// Recarga do JIT (J03): numera as classes do programa mantendo os ids
    /// da geração viva (`ids`, de [`ids_do_ir`]).
    pub fn com_ids_anteriores(mut self, ids: std::collections::HashMap<(String, String), u32>) -> Self {
        self.ids_anteriores = Some(ids);
        self.numerar_classes();
        self
    }

    /// Os ids das classes do programa, `(id, biblioteca, classe)`. Numa
    /// recarga, também os da geração viva cuja classe não está nesta (J03):
    /// um objeto dela pode estar vivo, e a classe pode voltar — o id fica
    /// reservado para ela de geração em geração.
    pub fn ids_do_programa(&self) -> Vec<(u32, String, String)> {
        let mut ids: Vec<(u32, String, String)> = self
            .program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.program.library(c.library).is_sdk)
            .filter_map(|(i, c)| {
                let id = self.ids_de_classe.get(i).copied().flatten()?;
                Some((id, self.nome_da_biblioteca(c.library), self.interner.resolve(c.name).to_string()))
            })
            .collect();
        if let Some(anteriores) = &self.ids_anteriores {
            let presentes: std::collections::HashSet<(String, String)> =
                ids.iter().map(|(_, l, c)| (l.clone(), c.clone())).collect();
            let mut reservados: Vec<(u32, String, String)> = anteriores
                .iter()
                .filter(|(chave, _)| !presentes.contains(*chave))
                .map(|((l, c), id)| (*id, l.clone(), c.clone()))
                .collect();
            reservados.sort();
            ids.extend(reservados);
        }
        ids
    }

    /// O layout dos objetos de cada classe do programa (J03, ver
    /// `hir::Module::campos_do_programa`).
    pub fn campos_do_programa(&self) -> Vec<(u32, usize, Vec<crate::hir::CampoDoLayout>)> {
        (0..self.program.classes.len() as u32)
            .map(dartforge_elements::model::ClassId)
            .filter(|&c| !self.program.library(self.program.class(c).library).is_sdk)
            .filter_map(|c| {
                let id = self.id_de_classe(c)?;
                let campos = crate::lower::membros::layout(self, c)
                    .into_iter()
                    .map(|v| {
                        let var = &self.program.variables[v.0 as usize];
                        let t = crate::lower::membros::tipo_da_variavel(self, v);
                        let anulavel = matches!(self.table.get(t), Type::Dynamic | Type::Void | Type::Null)
                            || self.table.get(t).is_declared_nullable();
                        crate::hir::CampoDoLayout {
                            nome: self.symbol_name(var.name).to_string(),
                            anulavel,
                            late: var.late,
                            tipo: self.table.format(t, self.interner, self.program),
                        }
                    })
                    .collect();
                Some((id, crate::lower::enums::base_do_layout(self, c), campos))
            })
            .collect()
    }

    /// O corpo das funções da biblioteca é compilado?
    pub fn biblioteca_compilada(&self, lib: LibraryId) -> bool {
        self.compiladas[lib.0 as usize]
    }

    /// As funções da biblioteca são baixadas neste módulo?
    pub fn biblioteca_no_modulo(&self, lib: LibraryId) -> bool {
        self.no_modulo[lib.0 as usize]
    }

    /// A classe `nome` de `dart:<lib>` (SDK da fonte), se carregada.
    pub fn classe_do_sdk(&self, lib: &str, nome: &str) -> Option<dartforge_elements::model::ClassId> {
        let uri = format!("dart:{lib}");
        let sym = self.interner.lookup(nome)?;
        self.program
            .classes
            .iter()
            .position(|c| c.name == sym && self.program.library(c.library).uri == uri)
            .map(|i| dartforge_elements::model::ClassId(i as u32))
    }

    /// Ids de classe estáveis (P2): as classes compiladas pela ordem do
    /// caminho — as do SDK primeiro (grupo 0), numa faixa que só depende do
    /// SDK, depois as do programa —, a partir de 128, pulando 1000–1012; as
    /// classes de `layout::cid::DO_SDK` têm o cid fixo (1–65). Com a tabela
    /// do SDK (`ids_fixos_do_sdk`), as do SDK vêm dela e as do programa
    /// começam depois da maior.
    fn numerar_classes(&mut self) {
        let program = self.program;
        let mut ids = vec![None; program.classes.len()];
        let mut chaves: Vec<(String, String, usize)> = Vec::new();
        // 1–127 são dos cids fixos (docs/NATIVO-ESPACO-UNIFICADO.md §2.4).
        let mut prox = dartforge_runtime::layout::PRIMEIRO_CID_LIVRE as u32;
        match self.ids_fixos_do_sdk.clone() {
            Some(tabela) => {
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiladas[c.library.0 as usize] {
                        continue;
                    }
                    let lib = self.nome_da_biblioteca(c.library);
                    let nome = self.interner.resolve(c.name).to_string();
                    if program.library(c.library).is_sdk {
                        let id = tabela.ids.get(&(lib, nome)).copied().unwrap_or_else(|| {
                            panic!(
                                "classe {}::{} sem id na tabela do SDK compilado (cache desatualizado)",
                                self.nome_da_biblioteca(c.library),
                                self.interner.resolve(c.name)
                            )
                        });
                        ids[i] = Some(id);
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        // Um tipo de extensão não tem objeto no heap: o
                        // valor é o da representação.
                        chaves.push((lib, nome, i));
                    }
                }
                prox = tabela.proximo;
            }
            None => {
                let sdk = ids_das_classes_do_sdk(program, self.interner, &self.compiladas);
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiladas[c.library.0 as usize] {
                        continue;
                    }
                    if program.library(c.library).is_sdk {
                        ids[i] = sdk.ids.get(&(self.nome_da_biblioteca(c.library), self.interner.resolve(c.name).to_string())).copied();
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        chaves.push((self.nome_da_biblioteca(c.library), self.interner.resolve(c.name).to_string(), i));
                    }
                }
                prox = prox.max(sdk.proximo);
            }
        }
        chaves.sort();
        // Recarga (J03): a classe que a geração viva já numerou fica com o
        // id dela; as novas vêm depois do maior (nunca o de uma que sumiu:
        // um objeto dela pode estar vivo).
        if let Some(anteriores) = &self.ids_anteriores {
            if let Some(&maior) = anteriores.values().max() {
                prox = prox.max(maior + 1);
            }
            chaves.retain(|(lib, nome, i)| match anteriores.get(&(lib.clone(), nome.clone())) {
                Some(&id) => {
                    ids[*i] = Some(id);
                    false
                }
                None => true,
            });
        }
        for (_, _, i) in chaves {
            if (1000..=1012).contains(&prox) {
                prox = 1013;
            }
            ids[i] = Some(prox);
            prox += 1;
        }
        self.ids_de_classe = ids;
        // RTI: as classes do SDK sem id do heap (as não compiladas), na
        // ordem do caminho estável.
        let mut sdk: Vec<(String, String, usize)> = program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| program.library(c.library).is_sdk && !self.compiladas[c.library.0 as usize])
            .map(|(i, c)| (self.nome_da_biblioteca(c.library), self.interner.resolve(c.name).to_string(), i))
            .collect();
        sdk.sort();
        self.ids_rti_sdk = sdk
            .into_iter()
            .enumerate()
            .map(|(k, (_, _, i))| (dartforge_elements::model::ClassId(i as u32), k as u32))
            .collect();
    }

    /// O nome estável de uma biblioteca (P2): `dart:x` e `package:a/b.dart`
    /// como estão; `file:` pelo caminho relativo ao diretório da biblioteca
    /// de entrada, com `/` (o mesmo programa tem os mesmos símbolos em
    /// qualquer máquina e checkout).
    pub fn nome_da_biblioteca(&self, lib: LibraryId) -> String {
        let l = self.program.library(lib);
        if !l.uri.starts_with("file:") {
            return l.uri.clone();
        }
        let caminho = l
            .units
            .first()
            .and_then(|u| self.program.unit(*u).path.clone());
        if let (Some(c), Some(r)) = (caminho, self.raiz.as_ref())
            && let Ok(rel) = c.strip_prefix(r)
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
        l.uri.clone()
    }

    /// Id de classe da forma de record `(npos, nomes)`, se o programa a tem.
    pub fn id_da_forma(&self, npos: usize, nomes: &[String]) -> Option<u32> {
        self.formas_de_record
            .iter()
            .position(|(p, n)| *p == npos && n.as_slice() == nomes)
            .map(|i| ID_BASE_DE_FORMA + i as u32)
    }

    /// Id de classe do runtime de uma classe do programa.
    pub fn id_de_classe(&self, cid: dartforge_elements::model::ClassId) -> Option<u32> {
        self.ids_de_classe.get(cid.0 as usize).copied().flatten()
    }

    pub fn symbol_name(&self, sym: SymbolId) -> &str {
        self.interner.resolve(sym)
    }

    /// O tipo estático de `expr`, **apagado** (`apagamento.rs`): é o tipo
    /// que o valor tem em tempo de execução, o que decide representação,
    /// caminhos rápidos e testes de tipo.
    pub fn get_type(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        self.get_type_bruto(unit, expr).map(|t| self.te.apagar(t))
    }

    /// O tipo estático de `expr` como a inferência o deu, com os tipos de
    /// extensão: só a rota dos membros deles e os argumentos de tipo desses
    /// membros o usam.
    pub fn get_type_bruto(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_type(expr)
        } else {
            None
        }
    }

    /// O tipo apagado de `t` (ver [`Context::get_type`]).
    pub fn apagar(&self, t: TypeId) -> TypeId {
        self.te.apagar(t)
    }

    /// `c` é um tipo de extensão (apagado em tempo de execução).
    pub fn e_tipo_de_extensao(&self, c: dartforge_elements::model::ClassId) -> bool {
        self.program.classes[c.0 as usize].kind == dartforge_elements::model::ClassKind::ExtensionType
    }

    pub fn get_resolved(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<&Resolved> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_resolved(expr)
        } else {
            None
        }
    }

    pub fn is_int(&self, ty: TypeId) -> bool {
        if self.core.int_class.is_some() && ty == self.core.int {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "int"
            }
            _ => false,
        }
    }

    pub fn is_double(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "double"
            }
            _ => false,
        }
    }

    pub fn is_bool(&self, ty: TypeId) -> bool {
        if self.core.bool_class.is_some() && ty == self.core.bool_ {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "bool"
            }
            _ => false,
        }
    }

    pub fn is_string(&self, ty: TypeId) -> bool {
        if self.core.string_class.is_some() && ty == self.core.string {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "String"
            }
            _ => false,
        }
    }

    pub fn is_list(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "List"
            }
            _ => false,
        }
    }

    pub fn is_map(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "Map"
            }
            _ => false,
        }
    }

    pub fn is_void(&self, ty: TypeId) -> bool {
        ty == self.core.void_
    }

    /// Representação de um tipo Dart (R1, `docs/NATIVO-PLANO.md` §6.2).
    ///
    /// Só `int`, `double` e `bool` **não anuláveis** são escalares; qualquer
    /// tipo anulável (`int?` inclusive), `num`, `Object`, `dynamic` e
    /// parâmetros de tipo são `Ref` — `Type` inclusive: é o objeto canônico
    /// do RTI (`lower/rti.rs`).
    /// O tipo HIR do retorno da função `fid` (declarado ou inferido). Uma
    /// função `void` de corpo `=> e` síncrono devolve o valor de `e` (`Ref`):
    /// o `void` é só estático, e quem chama por `dynamic` ou `Function`
    /// recebe o valor, como na VM (o `handleSpace` do `intl`, `void f() =>
    /// cond ? '' : erro()`, guardado num `Map<String, Function>` e chamado
    /// para escrever o resultado; docs/NATIVO-PROJETOS-REAIS.md C23). Quem
    /// chama direto ignora o valor.
    pub fn retorno_hir(&self, fid: usize) -> crate::hir::Type {
        let Some(d) = self.outline.functions.get(fid) else { return crate::hir::Type::Ref };
        let t = self.to_hir_type(d.return_type);
        if t == crate::hir::Type::Void && self.void_de_seta(fid) { crate::hir::Type::Ref } else { t }
    }

    /// `void f() => e;` síncrono, função ou método comum (não getter, setter,
    /// operador nem construtor). Ver [`Self::retorno_hir`].
    fn void_de_seta(&self, fid: usize) -> bool {
        let f = &self.program.functions[fid];
        // O `main` é chamado pela entrada como `void` (`lower/mod.rs`).
        if f.kind != dartforge_elements::model::FunctionKind::Function
            || (f.class.is_none() && self.symbol_name(f.name) == "main")
        {
            return false;
        }
        let dartforge_elements::model::FunctionRef::Function { unit, function } = f.node else { return false };
        let funcao = self.program.unit(unit).ast.function(function);
        matches!(funcao.body, dartforge_frontend::ast::FunctionBody::Expression(_))
            && funcao.modifier == dartforge_frontend::ast::AsyncModifier::None
    }

    pub fn to_hir_type(&self, ty: TypeId) -> crate::hir::Type {
        let ty = self.apagar(ty);
        if self.is_void(ty) {
            return crate::hir::Type::Void;
        }
        let Type::Interface { class, nullable, .. } = self.table.get(ty) else {
            return crate::hir::Type::Ref;
        };
        let classe = &self.program.classes[class.0 as usize];
        if *nullable || !self.program.library(classe.library).is_sdk {
            return crate::hir::Type::Ref;
        }
        match self.symbol_name(classe.name) {
            "int" => crate::hir::Type::I64,
            "double" => crate::hir::Type::F64,
            "bool" => crate::hir::Type::I1,
            _ => crate::hir::Type::Ref,
        }
    }

    /// Tipo declarado (ou inferido) da variável local cujo nome começa em
    /// `offset` (R6).
    pub fn tipo_local(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.tipo_local_semantico(unit, offset).map(|t| self.apagar(t))
    }

    /// Classifica a obrigação estática; None ainda exige prova no lowering.
    pub(crate) fn classificar_finalizavel(&self, tipo: TypeId) -> Option<bool> {
        crate::finalizaveis::classificar(self.table, &self.outline.hierarchy, self.finalizable_class, tipo)
    }

    /// Obrigação de um receptor cuja identidade de classe já é conhecida.
    pub(crate) fn classificar_classe_finalizavel(&self, classe: dartforge_elements::model::ClassId) -> Option<bool> {
        crate::finalizaveis::classificar_classe(&self.outline.hierarchy, self.finalizable_class, classe)
    }

    /// Tipo estático de this no membro: classe ou tipo on de uma extensão.
    pub(crate) fn classificar_this(&self, fid: usize) -> Option<bool> {
        let f = self.program.functions.get(fid)?;
        if let Some(e) = f.extension {
            self.classificar_finalizavel(self.outline.extensions.get(e.0 as usize)?.on)
        } else {
            f.class.and_then(|c| self.classificar_classe_finalizavel(c))
        }
    }

    /// Tipo Dart original da declaração, antes do apagamento para representação.
    /// Obrigações léxicas dependem da identidade estática, inclusive de tipos
    /// de extensão; ausência na tabela não prova ausência dessas obrigações.
    pub(crate) fn tipo_local_semantico(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.bodies.units.get(unit.0 as usize)?.tipo_local(offset)
    }
}

/// Os ids das classes do SDK compilado: `(biblioteca, classe) → id`, e o
/// primeiro id livre depois deles (onde começam as do programa).
#[derive(Debug, Clone, Default)]
pub struct TabelaDeIds {
    pub ids: std::collections::HashMap<(String, String), u32>,
    pub proximo: u32,
}

impl TabelaDeIds {
    /// A tabela em texto: uma linha `id\tbiblioteca\tclasse` por classe e
    /// a última `proximo\t<n>`.
    pub fn para_texto(&self) -> String {
        let mut v: Vec<_> = self.ids.iter().collect();
        v.sort_by_key(|(_, id)| **id);
        let mut t = String::new();
        for ((lib, nome), id) in v {
            t.push_str(&format!("{id}\t{lib}\t{nome}\n"));
        }
        t.push_str(&format!("proximo\t{}\n", self.proximo));
        t
    }

    /// Lê o texto de [`TabelaDeIds::para_texto`].
    pub fn de_texto(t: &str) -> Option<Self> {
        let mut tabela = TabelaDeIds::default();
        for linha in t.lines() {
            let mut partes = linha.split('\t');
            let a = partes.next()?;
            if a == "proximo" {
                tabela.proximo = partes.next()?.parse().ok()?;
                continue;
            }
            let id: u32 = a.parse().ok()?;
            let lib = partes.next()?.to_string();
            let nome = partes.next()?.to_string();
            tabela.ids.insert((lib, nome), id);
        }
        (tabela.proximo > 0).then_some(tabela)
    }
}

/// Numera as classes das bibliotecas do SDK que `compiladas` marca, pela
/// ordem (biblioteca, classe), a partir de 1, pulando 1000–1012 (os ids do
/// runtime). É a fonte única dos ids do SDK: o módulo do SDK a calcula com
/// todas as bibliotecas carregadas, e o programa recebe a mesma tabela.
pub fn ids_das_classes_do_sdk(program: &Program, interner: &Interner, compiladas: &[bool]) -> TabelaDeIds {
    let mut chaves: Vec<(String, String)> = program
        .classes
        .iter()
        .filter(|c| compiladas[c.library.0 as usize] && program.library(c.library).is_sdk)
        .map(|c| (program.library(c.library).uri.clone(), interner.resolve(c.name).to_string()))
        .collect();
    chaves.sort();
    let mut tabela = TabelaDeIds::default();
    // As classes que o runtime conhece têm o cid fixo do contrato de layout
    // (docs/NATIVO-ESPACO-UNIFICADO.md §2.4, `layout::cid::DO_SDK`): 1–127 ficam
    // reservados (quem não existe no SDK carregado fica sem instâncias) e as
    // demais começam em `PRIMEIRO_CID_LIVRE`, pulando 1000–1012.
    let fixos: std::collections::HashMap<(&str, &str), u32> =
        dartforge_runtime::layout::cid::DO_SDK.iter().map(|&(c, lib, nome)| ((lib, nome), c as u32)).collect();
    let mut prox = dartforge_runtime::layout::PRIMEIRO_CID_LIVRE as u32;
    for k in chaves {
        if let Some(&c) = fixos.get(&(k.0.as_str(), k.1.as_str())) {
            tabela.ids.insert(k, c);
            continue;
        }
        if (1000..=1012).contains(&prox) {
            prox = 1013;
        }
        tabela.ids.insert(k, prox);
        prox += 1;
    }
    tabela.proximo = prox;
    tabela
}

/// Os ids das classes do programa escritos no IR por uma geração
/// (`; df.classe <id> <biblioteca> <classe>`, com as partes escapadas por
/// [`escapar`]), por `(biblioteca, classe)`.
pub fn ids_do_ir(ir: &str) -> std::collections::HashMap<(String, String), u32> {
    ir.lines()
        .filter_map(|l| {
            let mut p = l.strip_prefix("; df.classe ")?.split(' ');
            let id = p.next()?.parse().ok()?;
            Some(((desescapar(p.next()?)?, desescapar(p.next()?)?), id))
        })
        .collect()
}

/// O inverso de [`escapar`].
pub fn desescapar(parte: &str) -> Option<String> {
    let b = parte.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(parte.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(v).ok()
}

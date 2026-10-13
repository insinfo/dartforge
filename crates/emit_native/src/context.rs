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
    /// caminho estável (`library_name`, nome da classe), a partir de 1,
    /// pulando a faixa 1000–1012 das classes de erro do runtime. `None` para
    /// as classes do SDK.
    pub class_ids: Vec<Option<u32>>,
    /// Com o SDK da fonte, os ids das classes do SDK vêm desta tabela
    /// (`sdk_modulo::sdk_class_ids`), a mesma do módulo do SDK: o
    /// programa carrega só as bibliotecas que importa, e numerar pelo que
    /// carregou daria ids diferentes dos do SDK compilado.
    pub fixed_sdk_ids: Option<std::sync::Arc<IdTable>>,
    /// As formas de record com campo nomeado do programa (P3): número de
    /// posicionais e os nomes, ordenados — cada uma é uma "classe" de record
    /// com id `SHAPE_BASE_ID + índice`.
    pub record_shapes: Vec<(usize, Vec<String>)>,
    /// Diretório da biblioteca de entrada: as bibliotecas `file:` são
    /// nomeadas pelo caminho relativo a ele (estável entre máquinas).
    root: Option<std::path::PathBuf>,
    /// Este contexto baixa uma biblioteca do SDK em um objeto separado.
    pub sdk_library: bool,
    /// Produz fatos de ownership somente para a política --memoria=arc.
    pub arc_memory: bool,
    /// Por biblioteca: o corpo das funções dela é compilado (as do programa;
    /// com [`Context::with_source_sdk`], também as do SDK da fonte).
    pub compiled: Vec<bool>,
    /// Por biblioteca: as funções dela são baixadas **neste** módulo (as do
    /// programa; num módulo do SDK da fonte, só a biblioteca dele). As outras
    /// compiladas moram em outro objeto e são chamadas pelo símbolo.
    pub in_module: Vec<bool>,
    /// P6: as bibliotecas do SDK compiladas da fonte com o programa
    /// (`source.rs`); o `is_sdk` delas já está desligado na cópia do
    /// `Program`. Vazio para quem não usa `dart:async`.
    pub from_source: std::collections::HashSet<LibraryId>,
    /// RTI: a posição de cada classe do SDK (sem id de classe do heap) na
    /// ordem do caminho estável — o id RTI é `0x2000_0000 +` ela
    /// (`lower::rti`, `Context::id_rti`).
    pub sdk_rti_ids: std::collections::HashMap<dartforge_elements::model::ClassId, u32>,
    /// P6: o programa usa `dart:async` e tem o laço de eventos depois do
    /// `main` (com o SDK da fonte o `dart:async` é o do módulo em cache, e
    /// `from_source` fica vazio).
    pub uses_dart_async: bool,
    /// P6: os símbolos das funções da fonte com corpo (`lower::with_source_body`).
    pub with_source_body: std::sync::OnceLock<std::collections::HashSet<String>>,
    /// `dart:ffi`: o layout das structs e unions do programa (`lower::ffi::composites`).
    pub ffi_composites: std::sync::OnceLock<Option<crate::lower::ffi::Composites>>,
    /// Os tipos de extensão apagados (`erasure.rs`), calculados antes do
    /// contexto; vazio sem tipo de extensão.
    pub te: crate::erasure::ExtensionTypes,
    /// J05: com informação de depuração, o início de cada linha de cada
    /// unidade (pelo índice da unidade), para as posições das instruções.
    /// Ligado também pelo rastro simbólico (§13.14), que usa as posições sem
    /// o DWARF.
    pub debug_info: Option<Vec<Vec<u32>>>,
    /// J05: as posições viram tabelas de linha do depurador (`--debug_info`);
    /// sem isto, só o rastro simbólico as usa.
    pub dwarf: bool,
    /// O rastro simbólico (§13.14) está ligado: o lowering marca as funções
    /// (quadros ocultos, corpos `async`, elos de quem espera) e insere a
    /// pilha do rastro.
    pub stack_trace: bool,
    /// Recarga do JIT (J03): os ids que a geração viva deu às classes do
    /// programa, por `(library, class)`. A mesma classe fica com o mesmo
    /// id; uma classe nova ganha um id acima de todos eles (os objetos vivos
    /// no heap guardam o id).
    previous_ids: Option<std::collections::HashMap<(String, String), u32>>,
    /// O mundo fechado do programa (`native_world.rs`): a função ou o
    /// global do programa fora dele vira corpo que lança. `None`: sem poda
    /// (os módulos do SDK, `DARTFORGE_SEM_PODA_DO_PROGRAMA=1`).
    pub world: Option<dartforge_world::Mundo>,
    /// Memória de `sdk_source::implementations` por (classe, nome): a busca
    /// percorre TODAS as classes do programa e a linearização de cada uma, e
    /// é feita a cada acesso a membro pelo despacho por classe. Num programa
    /// real (o new_sali/backend: milhares de classes, centenas de milhares
    /// de acessos) era quadrática e dominava a emissão (minutos). O
    /// resultado só depende do programa e das bibliotecas compiladas, fixos
    /// depois da construção do contexto.
    pub implementations_memo:
        std::sync::RwLock<std::collections::HashMap<(dartforge_elements::model::ClassId, SymbolId), std::sync::Arc<Vec<crate::lower::sdk_source::Implementation>>>>,
    /// Os subtipos de cada classe (`Context::subtypes`), calculados na
    /// primeira consulta.
    pub subtypes_memo:
        std::sync::RwLock<std::collections::HashMap<dartforge_elements::model::ClassId, std::sync::Arc<Vec<dartforge_elements::model::ClassId>>>>,
    /// Idem para `sdk_source::implementations_by_class`.
    pub implementations_by_class_memo: std::sync::RwLock<
        std::collections::HashMap<
            (dartforge_elements::model::ClassId, SymbolId),
            Option<std::sync::Arc<Vec<(i64, crate::lower::sdk_source::Implementation)>>>,
        >,
    >,
}

/// O nome da variável de um padrão `:x`/`:var x`/`:x?`/`:x as T`.
pub fn pattern_variable_name(
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
pub const SHAPE_BASE_ID: u32 = 0x4000_0000;

/// Escapa uma parte de um símbolo estável: letras, dígitos e `_` ficam; o
/// resto vira `$` e dois dígitos hexadecimais por byte UTF-8. O `.` separa as
/// partes, então nunca aparece cru dentro de uma — o símbolo é injetivo.
pub fn escape_str(part: &str) -> String {
    let mut s = String::with_capacity(part.len());
    for b in part.bytes() {
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
    pub fn pruned_function(&self, f: usize) -> bool {
        let Some(m) = &self.world else { return false };
        let func = &self.program.functions[f];
        !self.program.library(func.library).is_sdk
            && matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. })
            && !m.funcao(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// O campo de instância `v` de uma classe do programa ficou fora do
    /// mundo fechado (ninguém o lê nem grava).
    pub fn pruned_field(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.world else { return false };
        let var = &self.program.variables[v.0 as usize];
        if self.program.library(var.library).is_sdk {
            return false;
        }
        // O mundo acompanha os acessores implícitos do campo (membros de
        // instância com `variable`), não a variável.
        let Some(c) = var.class else { return false };
        let mut accessors = self.program.classes[c.0 as usize]
            .instance_members
            .values()
            .filter(|f| self.program.functions[f.0 as usize].variable == Some(v))
            .peekable();
        accessors.peek().is_some() && accessors.all(|f| !m.funcao(*f))
    }

    /// O membro de instância `f` de uma classe do programa (método, acessor
    /// explícito ou implícito de campo) ficou fora do mundo fechado: sem
    /// entrada na tabela de métodos nem adaptador.
    pub fn pruned_member(&self, f: usize) -> bool {
        let Some(m) = &self.world else { return false };
        let func = &self.program.functions[f];
        // Só o que tem corpo escrito (`FunctionRef::Function`) ou é acessor
        // de campo: os membros sintéticos (os encaminhadores de
        // `noSuchMethod` que o CFE criaria) não passam pelo mundo, e sem
        // entrada na tabela a chamada caía no `NoSuchMethodError`
        // (corpus/js/223_nosuchmethod_argumentos).
        let with_node = matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. }) || func.variable.is_some();
        with_node
            && !self.program.library(func.library).is_sdk
            && !matches!(
                func.kind,
                dartforge_elements::model::FunctionKind::Constructor | dartforge_elements::model::FunctionKind::SyntheticConstructor
            )
            && !m.funcao(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// O global `v` do programa ficou fora do mundo fechado.
    pub fn pruned_global(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.world else { return false };
        let var = &self.program.variables[v.0 as usize];
        // Uma `const` fica sempre: ela é valor padrão de parâmetro de membro
        // abstrato, que o encaminhador de `noSuchMethod` gerado pelo lowering
        // lê e o mundo não vê (o `Pintor.pattern` de
        // corpus/js/223_nosuchmethod_argumentos); o getter é pequeno.
        !var.const_ && !self.program.library(var.library).is_sdk && !m.variavel(v)
    }

    /// As classes do programa que são subtipo de `cid` (ela inclusive), na
    /// ordem dos ids de elemento — o que as buscas por implementação
    /// percorriam varrendo TODAS as classes a cada acesso a membro.
    pub fn subtypes(&self, cid: dartforge_elements::model::ClassId) -> std::sync::Arc<Vec<dartforge_elements::model::ClassId>> {
        if let Some(r) = self.subtypes_memo.read().unwrap_or_else(|e| e.into_inner()).get(&cid) {
            return r.clone();
        }
        let r: std::sync::Arc<Vec<_>> = std::sync::Arc::new(
            (0..self.program.classes.len() as u32)
                .map(dartforge_elements::model::ClassId)
                .filter(|&k| crate::lower::members::subclass_of(self, k, cid))
                .collect(),
        );
        self.subtypes_memo.write().unwrap_or_else(|e| e.into_inner()).insert(cid, r.clone());
        r
    }

    /// Liga a informação de depuração (J05): indexa as linhas das unidades.
    pub fn link_debug(&mut self) {
        let lines = self
            .program
            .units
            .iter()
            .map(|u| {
                std::iter::once(0)
                    .chain(u.source.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i as u32 + 1))
                    .collect()
            })
            .collect();
        self.debug_info = Some(lines);
    }

    /// A url do script da unidade no rastro da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    /// §13.14): a da unidade (`file:///…`, `package:x/y.dart`); no SDK, a da
    /// biblioteca para a unidade que a define (`dart:core`) e
    /// `dart:<library>/<file>` para as partes (`dart:core/list.dart`),
    /// com `-patch` nas da VM e da sobreposição nativa
    /// (`dart:core-patch/growable_array.dart`).
    pub fn stack_trace_url(&self, unit: dartforge_elements::model::UnitId) -> String {
        let u = self.program.unit(unit);
        let library = &self.program.library(u.library).uri;
        let Some(name) = library.strip_prefix("dart:") else { return u.uri.clone() };
        if u.uri == *library {
            return library.clone();
        }
        let file = u
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| u.uri.rsplit('/').next().unwrap_or_default().to_string());
        let patch = u
            .path
            .as_ref()
            .is_some_and(|p| p.components().any(|c| matches!(c.as_os_str().to_str(), Some("_internal" | "native_sdk"))));
        if patch { format!("dart:{name}-patch/{file}") } else { format!("dart:{name}/{file}") }
    }

    /// `(line, column)`, a partir de 1, do byte `offset` da unidade `unit`,
    /// com a depuração ligada.
    pub fn line_and_column(&self, unit: dartforge_elements::model::UnitId, offset: usize) -> Option<(u32, u32)> {
        let starts = self.debug_info.as_ref()?.get(unit.0 as usize)?;
        let offset = u32::try_from(offset).ok()?;
        let line = starts.partition_point(|&i| i <= offset);
        let start = starts[line.checked_sub(1)?];
        Some((line as u32, offset - start + 1))
    }

    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Self {
        let root = program.entry.and_then(|l| {
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
            class_ids: Vec::new(),
            fixed_sdk_ids: None,
            record_shapes: Vec::new(),
            root,
            sdk_library: false,
            arc_memory: false,
            compiled: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            in_module: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            from_source: std::collections::HashSet::new(),
            sdk_rti_ids: std::collections::HashMap::new(),
            uses_dart_async: false,
            with_source_body: std::sync::OnceLock::new(),
            ffi_composites: std::sync::OnceLock::new(),
            implementations_memo: Default::default(),
            world: None,
            subtypes_memo: Default::default(),
            implementations_by_class_memo: Default::default(),
            te: crate::erasure::ExtensionTypes::default(),
            previous_ids: None,
            debug_info: None,
            dwarf: false,
            stack_trace: false,
        };
        // Formas de record com campo nomeado: literais, padrões e tipos de
        // todas as unidades do programa (o conjunto inteiro, antes do
        // lowering — um acesso `r.x` sem tipo testa todas as que têm `x`).
        let mut shapes = std::collections::BTreeSet::new();
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
                    shapes.insert((positional.len(), n));
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
                                let text = &u.source[f.span.start as usize..f.span.end as usize];
                                if text.trim_start().starts_with(':')
                                    && let Some(s) = pattern_variable_name(&u.ast, f.pattern)
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
                        shapes.insert((npos, n));
                    }
                }
            }
            for t in &u.ast.types {
                if let dartforge_frontend::ast::TypeKind::Record { positional, named } = &t.kind
                    && !named.is_empty()
                {
                    let mut n: Vec<String> = named.iter().map(|(k, _)| interner.resolve(k.sym).to_string()).collect();
                    n.sort();
                    shapes.insert((positional.len(), n));
                }
            }
        }
        ctx.record_shapes = shapes.into_iter().collect();
        ctx.number_classes();
        ctx
    }

    /// Liga o SDK da fonte (P5c): as bibliotecas de `SOURCE_LIBRARIES`
    /// passam a ter corpo compilado e classes com id.
    pub fn with_source_sdk(mut self) -> Self {
        for (i, l) in self.program.libraries.iter().enumerate() {
            if let Some(name) = l.uri.strip_prefix("dart:")
                && crate::sdk_modulo::SOURCE_LIBRARIES.contains(&name)
            {
                self.compiled[i] = true;
            }
        }
        self.number_classes();
        self
    }

    /// [`Context::with_source_sdk`] com os ids das classes do SDK fixados
    /// pela tabela do SDK compilado.
    pub fn with_source_sdk_and_ids(mut self, ids: std::sync::Arc<IdTable>) -> Self {
        self.fixed_sdk_ids = Some(ids);
        self.with_source_sdk()
    }

    /// O módulo de uma biblioteca do SDK da fonte (P5c): só ela é baixada
    /// aqui; o programa e as outras bibliotecas ficam de fora.
    pub fn only_the_library(mut self, lib: LibraryId) -> Self {
        self.sdk_library = true;
        self.in_module = vec![false; self.program.libraries.len()];
        self.in_module[lib.0 as usize] = true;
        self
    }

    /// Recarga do JIT (J03): numera as classes do programa mantendo os ids
    /// da geração viva (`ids`, de [`ir_ids`]).
    pub fn with_previous_ids(mut self, ids: std::collections::HashMap<(String, String), u32>) -> Self {
        self.previous_ids = Some(ids);
        self.number_classes();
        self
    }

    /// Os ids das classes do programa, `(id, library, class)`. Numa
    /// recarga, também os da geração viva cuja classe não está nesta (J03):
    /// um objeto dela pode estar vivo, e a classe pode voltar — o id fica
    /// reservado para ela de geração em geração.
    pub fn program_ids(&self) -> Vec<(u32, String, String)> {
        let mut ids: Vec<(u32, String, String)> = self
            .program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.program.library(c.library).is_sdk)
            .filter_map(|(i, c)| {
                let id = self.class_ids.get(i).copied().flatten()?;
                Some((id, self.library_name(c.library), self.interner.resolve(c.name).to_string()))
            })
            .collect();
        if let Some(previous_ones) = &self.previous_ids {
            let present_set: std::collections::HashSet<(String, String)> =
                ids.iter().map(|(_, l, c)| (l.clone(), c.clone())).collect();
            let mut reserved: Vec<(u32, String, String)> = previous_ones
                .iter()
                .filter(|(key, _)| !present_set.contains(*key))
                .map(|((l, c), id)| (*id, l.clone(), c.clone()))
                .collect();
            reserved.sort();
            ids.extend(reserved);
        }
        ids
    }

    /// O layout dos objetos de cada classe do programa (J03, ver
    /// `hir::Module::program_fields`).
    pub fn program_fields(&self) -> Vec<(u32, usize, Vec<crate::hir::LayoutField>)> {
        (0..self.program.classes.len() as u32)
            .map(dartforge_elements::model::ClassId)
            .filter(|&c| !self.program.library(self.program.class(c).library).is_sdk)
            .filter_map(|c| {
                let id = self.class_id_of(c)?;
                let fields = crate::lower::members::layout(self, c)
                    .into_iter()
                    .map(|v| {
                        let var = &self.program.variables[v.0 as usize];
                        let t = crate::lower::members::variable_type(self, v);
                        let nullable = matches!(self.table.get(t), Type::Dynamic | Type::Void | Type::Null)
                            || self.table.get(t).is_declared_nullable();
                        crate::hir::LayoutField {
                            name: self.symbol_name(var.name).to_string(),
                            nullable,
                            late: var.late,
                            ty: self.table.format(t, self.interner, self.program),
                        }
                    })
                    .collect();
                Some((id, crate::lower::enums::layout_base(self, c), fields))
            })
            .collect()
    }

    /// O corpo das funções da biblioteca é compilado?
    pub fn compiled_library(&self, lib: LibraryId) -> bool {
        self.compiled[lib.0 as usize]
    }

    /// As funções da biblioteca são baixadas neste módulo?
    pub fn library_in_module(&self, lib: LibraryId) -> bool {
        self.in_module[lib.0 as usize]
    }

    /// A classe `name` de `dart:<lib>` (SDK da fonte), se carregada.
    pub fn sdk_class(&self, lib: &str, name: &str) -> Option<dartforge_elements::model::ClassId> {
        let uri = format!("dart:{lib}");
        let sym = self.interner.lookup(name)?;
        self.program
            .classes
            .iter()
            .position(|c| c.name == sym && self.program.library(c.library).uri == uri)
            .map(|i| dartforge_elements::model::ClassId(i as u32))
    }

    /// Ids de classe estáveis (P2): as classes compiladas pela ordem do
    /// caminho — as do SDK primeiro (grupo 0), numa faixa que só depende do
    /// SDK, depois as do programa —, a partir de 128, pulando 1000–1012; as
    /// classes de `layout::cid::FROM_SDK` têm o cid fixo (1–65). Com a tabela
    /// do SDK (`fixed_sdk_ids`), as do SDK vêm dela e as do programa
    /// começam depois da maior.
    fn number_classes(&mut self) {
        let program = self.program;
        let mut ids = vec![None; program.classes.len()];
        let mut keys: Vec<(String, String, usize)> = Vec::new();
        // 1–127 são dos cids fixos (docs/NATIVO-ESPACO-UNIFICADO.md §2.4).
        let mut next = dartforge_runtime::layout::PRIMEIRO_CID_LIVRE as u32;
        match self.fixed_sdk_ids.clone() {
            Some(table) => {
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiled[c.library.0 as usize] {
                        continue;
                    }
                    let lib = self.library_name(c.library);
                    let name = self.interner.resolve(c.name).to_string();
                    if program.library(c.library).is_sdk {
                        let id = table.ids.get(&(lib, name)).copied().unwrap_or_else(|| {
                            panic!(
                                "classe {}::{} sem id na tabela do SDK compilado (cache desatualizado)",
                                self.library_name(c.library),
                                self.interner.resolve(c.name)
                            )
                        });
                        ids[i] = Some(id);
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        // Um tipo de extensão não tem objeto no heap: o
                        // valor é o da representação.
                        keys.push((lib, name, i));
                    }
                }
                next = table.next_item;
            }
            None => {
                let sdk = sdk_class_ids(program, self.interner, &self.compiled);
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiled[c.library.0 as usize] {
                        continue;
                    }
                    if program.library(c.library).is_sdk {
                        ids[i] = sdk.ids.get(&(self.library_name(c.library), self.interner.resolve(c.name).to_string())).copied();
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        keys.push((self.library_name(c.library), self.interner.resolve(c.name).to_string(), i));
                    }
                }
                next = next.max(sdk.next_item);
            }
        }
        keys.sort();
        // Recarga (J03): a classe que a geração viva já numerou fica com o
        // id dela; as novas vêm depois do maior (nunca o de uma que sumiu:
        // um objeto dela pode estar vivo).
        if let Some(previous_ones) = &self.previous_ids {
            if let Some(&larger) = previous_ones.values().max() {
                next = next.max(larger + 1);
            }
            keys.retain(|(lib, name, i)| match previous_ones.get(&(lib.clone(), name.clone())) {
                Some(&id) => {
                    ids[*i] = Some(id);
                    false
                }
                None => true,
            });
        }
        for (_, _, i) in keys {
            if (1000..=1012).contains(&next) {
                next = 1013;
            }
            ids[i] = Some(next);
            next += 1;
        }
        self.class_ids = ids;
        // RTI: as classes do SDK sem id do heap (as não compiladas), na
        // ordem do caminho estável.
        let mut sdk: Vec<(String, String, usize)> = program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| program.library(c.library).is_sdk && !self.compiled[c.library.0 as usize])
            .map(|(i, c)| (self.library_name(c.library), self.interner.resolve(c.name).to_string(), i))
            .collect();
        sdk.sort();
        self.sdk_rti_ids = sdk
            .into_iter()
            .enumerate()
            .map(|(k, (_, _, i))| (dartforge_elements::model::ClassId(i as u32), k as u32))
            .collect();
    }

    /// O nome estável de uma biblioteca (P2): `dart:x` e `package:a/b.dart`
    /// como estão; `file:` pelo caminho relativo ao diretório da biblioteca
    /// de entrada, com `/` (o mesmo programa tem os mesmos símbolos em
    /// qualquer máquina e checkout).
    pub fn library_name(&self, lib: LibraryId) -> String {
        let l = self.program.library(lib);
        if !l.uri.starts_with("file:") {
            return l.uri.clone();
        }
        let path = l
            .units
            .first()
            .and_then(|u| self.program.unit(*u).path.clone());
        if let (Some(c), Some(r)) = (path, self.root.as_ref())
            && let Ok(rel) = c.strip_prefix(r)
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
        l.uri.clone()
    }

    /// Id de classe da forma de record `(npos, names)`, se o programa a tem.
    pub fn shape_id(&self, npos: usize, names: &[String]) -> Option<u32> {
        self.record_shapes
            .iter()
            .position(|(p, n)| *p == npos && n.as_slice() == names)
            .map(|i| SHAPE_BASE_ID + i as u32)
    }

    /// Id de classe do runtime de uma classe do programa.
    pub fn class_id_of(&self, cid: dartforge_elements::model::ClassId) -> Option<u32> {
        self.class_ids.get(cid.0 as usize).copied().flatten()
    }

    pub fn symbol_name(&self, sym: SymbolId) -> &str {
        self.interner.resolve(sym)
    }

    /// O tipo estático de `expr`, **apagado** (`erasure.rs`): é o tipo
    /// que o valor tem em tempo de execução, o que decide representação,
    /// caminhos rápidos e testes de tipo.
    pub fn get_type(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        self.get_type_raw(unit, expr).map(|t| self.te.erase(t))
    }

    /// O tipo estático de `expr` como a inferência o deu, com os tipos de
    /// extensão: só a rota dos membros deles e os argumentos de tipo desses
    /// membros o usam.
    pub fn get_type_raw(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_type(expr)
        } else {
            None
        }
    }

    /// O tipo apagado de `t` (ver [`Context::get_type`]).
    pub fn erase(&self, t: TypeId) -> TypeId {
        self.te.erase(t)
    }

    /// `c` é um tipo de extensão (apagado em tempo de execução).
    pub fn is_extension_type(&self, c: dartforge_elements::model::ClassId) -> bool {
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
    pub fn hir_return(&self, fid: usize) -> crate::hir::Type {
        let Some(d) = self.outline.functions.get(fid) else { return crate::hir::Type::Ref };
        let t = self.to_hir_type(d.return_type);
        if t == crate::hir::Type::Void && self.arrow_void(fid) { crate::hir::Type::Ref } else { t }
    }

    /// `void f() => e;` síncrono, função ou método comum (não getter, setter,
    /// operador nem construtor). Ver [`Self::hir_return`].
    fn arrow_void(&self, fid: usize) -> bool {
        let f = &self.program.functions[fid];
        // O `main` é chamado pela entrada como `void` (`lower/mod.rs`).
        if f.kind != dartforge_elements::model::FunctionKind::Function
            || (f.class.is_none() && self.symbol_name(f.name) == "main")
        {
            return false;
        }
        let dartforge_elements::model::FunctionRef::Function { unit, function } = f.node else { return false };
        let func_decl = self.program.unit(unit).ast.function(function);
        matches!(func_decl.body, dartforge_frontend::ast::FunctionBody::Expression(_))
            && func_decl.modifier == dartforge_frontend::ast::AsyncModifier::None
    }

    pub fn to_hir_type(&self, ty: TypeId) -> crate::hir::Type {
        let ty = self.erase(ty);
        if self.is_void(ty) {
            return crate::hir::Type::Void;
        }
        let Type::Interface { class, nullable, .. } = self.table.get(ty) else {
            return crate::hir::Type::Ref;
        };
        let cls = &self.program.classes[class.0 as usize];
        if *nullable || !self.program.library(cls.library).is_sdk {
            return crate::hir::Type::Ref;
        }
        match self.symbol_name(cls.name) {
            "int" => crate::hir::Type::I64,
            "double" => crate::hir::Type::F64,
            "bool" => crate::hir::Type::I1,
            _ => crate::hir::Type::Ref,
        }
    }

    /// Tipo declarado (ou inferido) da variável local cujo nome começa em
    /// `offset` (R6).
    pub fn local_type(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.semantic_local_type(unit, offset).map(|t| self.erase(t))
    }

    /// Classifica a obrigação estática; None ainda exige prova no lowering.
    pub(crate) fn classify_finalizable(&self, ty: TypeId) -> Option<bool> {
        crate::finalizables::classify(self.table, &self.outline.hierarchy, self.finalizable_class, ty)
    }

    /// Obrigação de um receptor cuja identidade de classe já é conhecida.
    pub(crate) fn classify_finalizable_class(&self, cls: dartforge_elements::model::ClassId) -> Option<bool> {
        crate::finalizables::classify_class(&self.outline.hierarchy, self.finalizable_class, cls)
    }

    /// Tipo estático de this no membro: classe ou tipo on de uma extensão.
    pub(crate) fn classify_this(&self, fid: usize) -> Option<bool> {
        let f = self.program.functions.get(fid)?;
        if let Some(e) = f.extension {
            self.classify_finalizable(self.outline.extensions.get(e.0 as usize)?.on)
        } else {
            f.class.and_then(|c| self.classify_finalizable_class(c))
        }
    }

    /// Tipo Dart original da declaração, antes do apagamento para representação.
    /// Obrigações léxicas dependem da identidade estática, inclusive de tipos
    /// de extensão; ausência na tabela não prova ausência dessas obrigações.
    pub(crate) fn semantic_local_type(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.bodies.units.get(unit.0 as usize)?.tipo_local(offset)
    }
}

/// Os ids das classes do SDK compilado: `(library, class) → id`, e o
/// primeiro id livre depois deles (onde começam as do programa).
#[derive(Debug, Clone, Default)]
pub struct IdTable {
    pub ids: std::collections::HashMap<(String, String), u32>,
    pub next_item: u32,
}

impl IdTable {
    /// A tabela em texto: uma linha `id\tbiblioteca\tclasse` por classe e
    /// a última `next_item\t<n>`.
    pub fn to_text(&self) -> String {
        let mut v: Vec<_> = self.ids.iter().collect();
        v.sort_by_key(|(_, id)| **id);
        let mut t = String::new();
        for ((lib, name), id) in v {
            t.push_str(&format!("{id}\t{lib}\t{name}\n"));
        }
        t.push_str(&format!("proximo\t{}\n", self.next_item));
        t
    }

    /// Lê o texto de [`IdTable::to_text`].
    pub fn from_string(t: &str) -> Option<Self> {
        let mut table = IdTable::default();
        for line in t.lines() {
            let mut parts = line.split('\t');
            let a = parts.next()?;
            if a == "proximo" {
                table.next_item = parts.next()?.parse().ok()?;
                continue;
            }
            let id: u32 = a.parse().ok()?;
            let lib = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            table.ids.insert((lib, name), id);
        }
        (table.next_item > 0).then_some(table)
    }
}

/// Numera as classes das bibliotecas do SDK que `compiled` marca, pela
/// ordem (biblioteca, classe), a partir de 1, pulando 1000–1012 (os ids do
/// runtime). É a fonte única dos ids do SDK: o módulo do SDK a calcula com
/// todas as bibliotecas carregadas, e o programa recebe a mesma tabela.
pub fn sdk_class_ids(program: &Program, interner: &Interner, compiled: &[bool]) -> IdTable {
    let mut keys: Vec<(String, String)> = program
        .classes
        .iter()
        .filter(|c| compiled[c.library.0 as usize] && program.library(c.library).is_sdk)
        .map(|c| (program.library(c.library).uri.clone(), interner.resolve(c.name).to_string()))
        .collect();
    keys.sort();
    let mut table = IdTable::default();
    // As classes que o runtime conhece têm o cid fixo do contrato de layout
    // (docs/NATIVO-ESPACO-UNIFICADO.md §2.4, `layout::cid::FROM_SDK`): 1–127 ficam
    // reservados (quem não existe no SDK carregado fica sem instâncias) e as
    // demais começam em `PRIMEIRO_CID_LIVRE`, pulando 1000–1012.
    let fixed_set: std::collections::HashMap<(&str, &str), u32> =
        dartforge_runtime::layout::cid::DO_SDK.iter().map(|&(c, lib, name)| ((lib, name), c as u32)).collect();
    let mut next = dartforge_runtime::layout::PRIMEIRO_CID_LIVRE as u32;
    for k in keys {
        if let Some(&c) = fixed_set.get(&(k.0.as_str(), k.1.as_str())) {
            table.ids.insert(k, c);
            continue;
        }
        if (1000..=1012).contains(&next) {
            next = 1013;
        }
        table.ids.insert(k, next);
        next += 1;
    }
    table.next_item = next;
    table
}

/// Os ids das classes do programa escritos no IR por uma geração
/// (`; df.class <id> <library> <class>`, com as partes escapadas por
/// [`escape_str`]), por `(library, class)`.
pub fn ir_ids(ir: &str) -> std::collections::HashMap<(String, String), u32> {
    ir.lines()
        .filter_map(|l| {
            let mut p = l.strip_prefix("; df.classe ")?.split(' ');
            let id = p.next()?.parse().ok()?;
            Some(((unescape(p.next()?)?, unescape(p.next()?)?), id))
        })
        .collect()
}

/// O inverso de [`escape_str`].
pub fn unescape(part: &str) -> Option<String> {
    let b = part.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(part.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(v).ok()
}

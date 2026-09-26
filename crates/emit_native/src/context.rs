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
    /// P5c: o SDK é compilado da fonte (`sdk_modulo`). As bibliotecas de
    /// [`crate::sdk_modulo::BIBLIOTECAS_DA_FONTE`] têm corpo compilado e
    /// classes com id, como as do programa.
    pub sdk_da_fonte: bool,
    /// Este contexto baixa uma biblioteca do SDK em um objeto separado.
    pub biblioteca_sdk: bool,
    /// Por biblioteca: o corpo das funções dela é compilado (as do programa;
    /// com `sdk_da_fonte`, também as do SDK da fonte).
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
    pub com_corpo_da_fonte: std::cell::OnceCell<std::collections::HashSet<String>>,
    /// `dart:ffi`: o layout das structs e unions do programa (`lower::ffi::compostos`).
    pub compostos_ffi: std::cell::OnceCell<Option<crate::lower::ffi::Compostos>>,
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
            entry_lib: program.entry,
            ids_de_classe: Vec::new(),
            ids_fixos_do_sdk: None,
            formas_de_record: Vec::new(),
            raiz,
            sdk_da_fonte: false,
            biblioteca_sdk: false,
            compiladas: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            no_modulo: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            da_fonte: std::collections::HashSet::new(),
            ids_rti_sdk: std::collections::HashMap::new(),
            usa_dart_async: false,
            com_corpo_da_fonte: std::cell::OnceCell::new(),
            compostos_ffi: std::cell::OnceCell::new(),
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
        self.sdk_da_fonte = true;
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
    /// SDK, depois as do programa —, a partir de 1, pulando 1000–1012. Com
    /// a tabela do SDK (`ids_fixos_do_sdk`), as do SDK vêm dela e as do
    /// programa começam depois da maior.
    fn numerar_classes(&mut self) {
        let program = self.program;
        let mut ids = vec![None; program.classes.len()];
        let mut chaves: Vec<(String, String, usize)> = Vec::new();
        let mut prox = 1u32;
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
                    } else {
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
                    } else {
                        chaves.push((self.nome_da_biblioteca(c.library), self.interner.resolve(c.name).to_string(), i));
                    }
                }
                prox = prox.max(sdk.proximo);
            }
        }
        chaves.sort();
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

    pub fn get_type(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_type(expr)
        } else {
            None
        }
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
    pub fn to_hir_type(&self, ty: TypeId) -> crate::hir::Type {
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
    let mut prox = 1u32;
    for k in chaves {
        if (1000..=1012).contains(&prox) {
            prox = 1013;
        }
        tabela.ids.insert(k, prox);
        prox += 1;
    }
    tabela.proximo = prox;
    tabela
}

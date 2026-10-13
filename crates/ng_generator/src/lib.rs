//! ngdart generator: produces the `.template.dart` files that `build_runner`
//! produces, without `build_runner`.
//!
//! The ngdart compiler turns component + HTML template into imperative Dart
//! (`ViewXComp0.build()`, factories, injection). `build_runner` only
//! orchestrates this — and it is `build_runner`, with `build_web_compilers`,
//! that costs the minute and a half of compiling new_sali. Generating here, the
//! whole cycle is ours.
//!
//! Rule: **do not touch ngdart or the application**. What matters is not
//! producing identical text, but the same ABI — the same public symbols
//! (`XNgFactory`, `createXFactory`, `ViewX0`) with the same semantics. Where
//! the output is equal to the official one, even better: it can be compared
//! byte for byte, and the 284 files that build_runner already wrote in new_sali
//! serve as an oracle, as `dartdevc` does for the emitter.
//!
//! Progress is gradual and measured: what we cannot generate yet is left out
//! and keeps coming from `build_runner`. The application works at every step
//! and the scoreboard says exactly where we are.
pub mod component;
pub mod css;
pub mod csslib;
pub mod dialect;
pub mod directives;
pub mod dom;
mod entities;
pub mod expr;
pub mod html;
pub mod incremental;
mod injector;
pub mod metadata;
pub mod micro;
pub mod resolucao;
pub mod sass;
pub mod selector;
pub mod shadow_css;
pub mod view;

use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
pub use incremental::{ConsultaNg, SaidaArquivo, analisar_arquivo, gerar_arquivo};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use view::{Motivo, Recusa, recusa};

/// Header that the official compiler writes in every generated file.
pub const CABECALHO: &str = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\n";

/// What a library has of interest to the generator.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Achados {
    /// `@Component`s read in full — the view comes from them.
    pub componentes: Vec<component::Componente>,
    /// `@Directive`s, read as the official compiler sees them from their users
    /// (selector, inputs, outputs, constructor).
    pub diretivas: Vec<component::Componente>,
    /// `@Pipe`s, with what their users need to know about them.
    pub pipes: Vec<component::Pipe>,
    /// The top-level variables with `@GenerateInjector`, in source order.
    pub injetores: Vec<String>,
    /// `@Directive`s with `@HostBinding`: each one gets a
    /// `DirectiveChangeDetector` in the generated file.
    pub hospedeiras: Vec<Hospedeira>,
    /// `@Directive`s with `@HostBinding`/`@HostListener` that inherit from
    /// something (`extends`, `with`): the official compiler also collects the
    /// inherited members, which cannot be seen from here.
    pub hospedeiro_herdado: bool,
}

/// A `@Directive` with `@HostBinding`: the official compiler generates the
/// `XNgCd` class for it (`requiresDirectiveChangeDetector`, in
/// `compile_metadata.dart`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hospedeira {
    pub classe: String,
    /// `(binding name, member)` of each `@HostBinding` — `class.x`, `attr.x`,
    /// property or, without an argument, the member name —, in the order the
    /// official compiler collects them (`DirectiveVisitor`, in
    /// `angular_compiler/analyzer/view/directive.dart`): accessors, then
    /// methods, then fields — each group in declaration order.
    pub ligacoes: Vec<(String, String)>,
    /// `style.x`: the member type as written (`String`, `int?`…), which decides
    /// between `toString()`, `?.toString()` and the direct value
    /// (`visitStyleBinding`).
    pub tipos_de_estilo: std::collections::HashMap<String, String>,
    /// Some binding outside what we know how to translate: the reason.
    pub recusada: Option<String>,
    /// The `final` members (`isImmutable`): written once, in `if (firstCheck)`,
    /// without an `_expr_N` field.
    pub imutaveis: std::collections::HashSet<String>,
    /// The static members of the class itself: read through the class
    /// (`StaticRead`), always immutable and `dynamic` (case j134).
    pub estaticos: std::collections::HashSet<String>,
}

/// Reads the `@HostBinding`s of a `@Directive` class; `None` if there are none.
fn hospedeira(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Option<Hospedeira> {
    let membros = component::tipos_dos_membros(arvore, fonte, interner, classe);
    let mut tipos_de_estilo = std::collections::HashMap::new();
    let mut acessores = Vec::new();
    let mut campos = Vec::new();
    let mut imutaveis = std::collections::HashSet::new();
    let mut recusada: Option<String> = None;
    let recusar = |motivo: String, recusada: &mut Option<String>| {
        recusada.get_or_insert(motivo);
    };
    let mut alguma = false;
    for &m in &classe.members {
        let membro = arvore.member(m);
        for a in membro.metadata.iter() {
            if nome_da_anotacao(a, interner) != "HostBinding" {
                continue;
            }
            alguma = true;
            // Without an argument (`Some(None)`), the binding name is that of
            // the member itself (`bindingName ?? memberName`).
            let nome = match a.arguments.as_ref().map(|args| &args.args[..]) {
                None | Some([]) => Some(None),
                Some([x]) if x.name.is_none() => match &arvore.expr(x.value).kind {
                    ast::ExprKind::String(lit) => {
                        lit.constant_value().map(|s| Some(s.to_string_lossy()))
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(nome) = nome else {
                recusar(
                    "@HostBinding with a name that is not a string".into(),
                    &mut recusada,
                );
                continue;
            };
            // `class.x`, `attr.x`, property and `style.x`
            // ([`view::forma_do_hospedeiro`]); `style.x` by the written type of
            // the field (not `final`: the immutable one would be written once),
            // as in the component.
            let aceita = |n: &str, membro: &str, campo: bool| match view::forma_do_hospedeiro(n) {
                Ok(view::FormaDoHospedeiro::Estilo { .. }) => {
                    let tipo = membros
                        .get(membro)
                        .map(|m| m.tipo.trim())
                        .unwrap_or_default();
                    let base = tipo.trim_end_matches('?');
                    campo
                        && !base.is_empty()
                        && !matches!(base, "dynamic" | "var" | "Object" | "Never")
                }
                Ok(_) => true,
                Err(_) => false,
            };
            match &membro.kind {
                // A `final` field is immutable and is written once, on the
                // first check (`isImmutable`); a static one reads through the
                // class, not yet.
                ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                    let membro = interner.resolve(l.variables[0].name.sym).to_string();
                    let nome = nome.unwrap_or_else(|| membro.clone());
                    let imutavel = l.final_ || l.const_;
                    if imutavel {
                        imutaveis.insert(membro.clone());
                    }
                    if !aceita(&nome, &membro, !imutavel) {
                        recusar(
                            format!("@HostBinding('{nome}') outside the known shapes"),
                            &mut recusada,
                        );
                    }
                    if let Some(m) = membros.get(&membro) {
                        tipos_de_estilo.insert(membro.clone(), m.tipo.trim().to_string());
                    }
                    campos.push((nome, membro));
                }
                ast::MemberKind::Method(f) => {
                    let funcao = arvore.function(*f);
                    match (funcao.kind, funcao.name) {
                        (ast::FunctionKind::Getter, Some(n)) if !funcao.static_ => {
                            let membro = interner.resolve(n.sym).to_string();
                            let nome = nome.unwrap_or_else(|| membro.clone());
                            if !aceita(&nome, &membro, false) {
                                recusar(
                                    format!("@HostBinding('{nome}') outside the known shapes"),
                                    &mut recusada,
                                );
                            }
                            acessores.push((nome, membro));
                        }
                        _ => recusar(
                            "@HostBinding on a method, setter or static getter".into(),
                            &mut recusada,
                        ),
                    }
                }
                // An immutable static outside `class.x`/`style.x`:
                // `hostAttribute`, which does not go into the `XNgCd`
                // (`_computeHostBindingImmutability`).
                ast::MemberKind::Field(l)
                    if l.static_
                        && (l.final_ || l.const_)
                        && !nome.as_deref().is_some_and(|n| {
                            n.starts_with("class.") || n.starts_with("style.")
                        }) => {}
                ast::MemberKind::Field(l) if l.static_ => {
                    recusar("@HostBinding on a static field".into(), &mut recusada)
                }
                _ => recusar("@HostBinding on a member outside the shape".into(), &mut recusada),
            }
        }
    }
    acessores.extend(campos);
    // Without `hostProperties` (only `hostAttributes`, or nothing) there is no
    // `XNgCd` (`requiresDirectiveChangeDetector`).
    if !alguma || (acessores.is_empty() && recusada.is_none()) {
        return None;
    }
    // The official map is keyed by binding name: repeating the name overwrites
    // the value and keeps the position of the first.
    let mut vistos = std::collections::HashSet::new();
    if !acessores.iter().all(|(c, _)| vistos.insert(c.clone())) {
        recusar("@HostBinding with a repeated name".into(), &mut recusada);
    }
    // A generic type changes the class declaration (`XNgCd<T>`); not yet.
    if !classe.type_params.is_empty() {
        recusar("@HostBinding on a generic directive".into(), &mut recusada);
    }
    Some(Hospedeira {
        classe: interner.resolve(classe.name.sym).to_string(),
        ligacoes: acessores,
        tipos_de_estilo,
        recusada,
        imutaveis,
        estaticos: Default::default(),
    })
}

impl Achados {
    /// Without any of this the generated file is just the header and the
    /// `import` of itself — 120 of the 284 files of new_sali/frontend.
    pub fn trivial(&self) -> bool {
        self.componentes.is_empty()
            && self.diretivas.is_empty()
            && self.pipes.is_empty()
            && self.injetores.is_empty()
    }
}

/// Effective name of an annotation: `@Component`, `@ng.Component` and
/// `@Component.new` all count as `Component`. The library prefix is lowercase
/// by convention and the class uppercase; that is what tells `p.Component` from
/// `Component.new`.
pub(crate) fn nome_da_anotacao(a: &ast::Annotation, interner: &Interner) -> String {
    for parte in &a.name {
        let texto = interner.resolve(parte.sym);
        if texto.starts_with(char::is_uppercase) {
            return texto.to_string();
        }
    }
    a.name
        .first()
        .map(|n| interner.resolve(n.sym).to_string())
        .unwrap_or_default()
}

/// Scans the top-level declarations of an already parsed unit.
pub fn achar(
    arvore: &ast::Ast,
    unidade: &ast::CompilationUnit,
    fonte: &str,
    interner: &Interner,
) -> Achados {
    let mut achados = Achados::default();
    for &id in &unidade.declarations {
        let decl = arvore.decl(id);
        // `@Component` and `@Directive` only exist on classes, but
        // `@GenerateInjector` sits on a top-level variable
        // (`@GenerateInjector([...]) final InjectorFactory injector = …`),
        // which is how new_sali sets up injection. That is why the scan does
        // not filter by declaration kind.
        let alvo = match &decl.kind {
            ast::DeclKind::Class(c) => interner.resolve(c.name.sym).to_string(),
            _ => String::new(),
        };
        for a in decl.metadata.iter() {
            match nome_da_anotacao(a, interner).as_str() {
                "Component" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        let mut c = component::ler_componente(arvore, fonte, interner, classe, a);
                        c.link_de_deteccao = decl.metadata.iter().any(|m| {
                            m.name.last().map(|n| interner.resolve(n.sym))
                                == Some("changeDetectionLink")
                        });
                        achados.componentes.push(c);
                    }
                }
                "Directive" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        achados
                            .diretivas
                            .push(component::ler_diretiva(arvore, fonte, interner, classe, a));
                    }
                    // Only `@HostBinding` changes the directive's file;
                    // `@HostListener` goes to its users. But the official
                    // compiler collects both in the superclasses too.
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        let herda = classe.extends.is_some() || !classe.with.is_empty();
                        let anotada = classe.members.iter().any(|&m| {
                            arvore.member(m).metadata.iter().any(|a| {
                                matches!(
                                    nome_da_anotacao(a, interner).as_str(),
                                    "HostBinding" | "HostListener"
                                )
                            })
                        });
                        achados.hospedeiro_herdado |= herda && anotada;
                        achados
                            .hospedeiras
                            .extend(hospedeira(arvore, fonte, interner, classe));
                    }
                }
                "Pipe" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        achados
                            .pipes
                            .push(component::ler_pipe(arvore, fonte, interner, classe, a));
                    }
                }
                "GenerateInjector" => {
                    if let ast::DeclKind::Variables(l) = &decl.kind {
                        achados.injetores.extend(
                            l.variables
                                .iter()
                                .map(|v| interner.resolve(v.name.sym).to_string()),
                        );
                    } else {
                        achados.injetores.push(alvo.clone());
                    }
                }
                _ => {}
            }
        }
    }
    achados
}

/// Generated file of a library with nothing Angular.
pub fn template_trivial(nome_do_arquivo: &str) -> String {
    format!("{CABECALHO}import '{nome_do_arquivo}';\n")
}

/// Where we are: how many files the generator already covers and which ones are
/// missing.
#[derive(Debug, Default)]
pub struct Placar {
    /// Arquivos `.dart` examinados.
    pub examinados: usize,
    /// Generated by us.
    pub gerados: usize,
    /// Left to whoever knows how to generate them (today, `build_runner`).
    pub pendentes: Vec<PathBuf>,
    /// Quantos pendentes por motivo da primeira recusa.
    pub motivos: std::collections::BTreeMap<Motivo, usize>,
    /// Complete set of refusals (reason and sub-shape) of each pending file,
    /// collected by the same emission that generates. It is how one knows how
    /// many files a new shape really unlocks.
    pub conjuntos: Vec<std::collections::BTreeSet<Recusa>>,
    /// How many times each shape not understood appears (`@HostBinding`,
    /// `providers: [..]`…), counting all of those of each pending component.
    pub nao_entendidos: std::collections::BTreeMap<String, usize>,
}

impl Placar {
    pub fn resumo(&self) -> String {
        format!(
            "ngdart: {}/{} generated by us",
            self.gerados, self.examinados
        )
    }
}

/// Caminho do gerado ao lado da fonte: `foo.dart` -> `foo.template.dart`.
pub fn caminho_do_template(fonte: &Path) -> PathBuf {
    let nome = fonte.file_name().unwrap_or_default().to_string_lossy();
    let base = nome.strip_suffix(".dart").unwrap_or(&nome);
    fonte.with_file_name(format!("{base}.template.dart"))
}

/// The package being generated: the name is used for the `asset:` URIs that
/// ngdart uses in development-mode messages.
#[derive(Default)]
pub struct Pacote {
    pub nome: String,
    pub raiz: PathBuf,
    /// The `.css` stylesheets that `sass_builder` generated in this build (the
    /// engine reads them from its output), by path: the ngdart shim starts from
    /// them, as in the official build — in the style (`outputStyle`) the
    /// project asked for. Without it, the shim compiles the `.scss` next to it
    /// on its own.
    pub folhas_geradas: std::collections::HashMap<PathBuf, String>,
}

impl Pacote {
    /// Path of a file inside the package, with slashes: `lib/src/x/f.dart`.
    fn relativo(&self, arquivo: &Path) -> String {
        arquivo
            .strip_prefix(&self.raiz)
            .unwrap_or(arquivo)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }
}

/// Generates what it can for the `.dart` files of `diretorios`, accumulating
/// into `c`.
pub fn gerar_em(
    pacote: &Pacote,
    diretorios: &[PathBuf],
    interner: &mut Interner,
    c: &mut Construtor,
    placar: &mut Placar,
    programa: Option<&resolucao::Resolvedor>,
) {
    let resolvedor = programa.map(|r| r as &dyn resolucao::Resolucao);
    // Two passes: the first reads and parses everything, the second generates.
    // It is the first that builds the package's component index — without it
    // there is no way to know that `<a02-texto-estatico>` is a component and
    // which class implements it.
    let mut arquivos: Vec<(PathBuf, String, Achados)> = Vec::new();
    for dir in diretorios {
        let mut pilha = vec![dir.clone()];
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                    continue;
                }
                let nome = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if !nome.ends_with(".dart") || nome.ends_with(".template.dart") {
                    continue;
                }
                placar.examinados += 1;
                let Ok(fonte) = std::fs::read_to_string(&p) else {
                    placar.pendentes.push(p);
                    continue;
                };
                let tokens = dartforge_frontend::lexer::lex(&fonte);
                let analisada = dartforge_frontend::parser::parse_lexed(&fonte, tokens, interner);
                // A part (`part of`) is not a library: the official compiler
                // generates nothing for it (`resolver.isLibrary`), and what it
                // declares goes into the `.template.dart` of the owning
                // library.
                if e_parte(&analisada.unit) {
                    placar.examinados -= 1;
                    continue;
                }
                let achados = achar(&analisada.ast, &analisada.unit, &fonte, interner);
                arquivos.push((p, nome, achados));
            }
        }
    }

    let indice = Indice::montar(pacote, &arquivos, programa);

    for (p, nome, achados) in &arquivos {
        let (texto, entradas, extras) =
            match gerar_interno(pacote, p, nome, achados, resolvedor, interner, &indice) {
                Ok(x) => x,
                Err(primeira) => {
                    // A shape the generator does not cover yet: it stays with
                    // build_runner, and the application compiles all the same.
                    *placar.motivos.entry(primeira.motivo).or_default() += 1;
                    // Every shape present counts, not only the one that
                    // refused.
                    for c in &achados.componentes {
                        for r in &c.nao_entendidos {
                            *placar.nao_entendidos.entry(r.forma.clone()).or_default() += 1;
                        }
                    }
                    placar.conjuntos.push(motivos_do_arquivo(
                        pacote, p, achados, primeira, resolvedor, &indice, interner,
                    ));
                    placar.pendentes.push(p.clone());
                    continue;
                }
            };
        c.por(caminho_do_template(p), texto, "ngdart", entradas.clone());
        for (destino, conteudo) in extras {
            c.por(destino, conteudo, "ngdart", entradas.clone());
        }
        placar.gerados += 1;
    }

    // The stylesheets, one by one, like the `StylesheetCompiler`: every `.css`
    // of the directories — the written one, the one `sass_builder` generated in
    // this build and, outside the engine, the one of each `.scss` that is not a
    // partial.
    for css in folhas_de(pacote, diretorios) {
        let entrada = if css.is_file() {
            css.clone()
        } else {
            css.with_extension("scss")
        };
        for (destino, r) in gerar_folha(pacote, &css) {
            match r {
                Ok(conteudo) => c.por(destino, conteudo, "ngdart", vec![entrada.clone()]),
                Err(recusa) => {
                    *placar.motivos.entry(recusa.motivo).or_default() += 1;
                    placar.conjuntos.push(std::iter::once(recusa).collect());
                    placar.pendentes.push(destino);
                }
            }
        }
    }
}

/// The `.css` files for which ngdart generates stylesheets, in order: those on
/// disk, those `sass_builder` generated in this build and those of the
/// non-partial `.scss` files without a `.css`.
fn folhas_de(pacote: &Pacote, diretorios: &[PathBuf]) -> Vec<PathBuf> {
    let mut v = std::collections::BTreeSet::new();
    for dir in diretorios {
        let mut pilha = vec![dir.clone()];
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                    continue;
                }
                let nome = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if nome.ends_with(".css") {
                    v.insert(p);
                } else if nome.ends_with(".scss") && !nome.starts_with('_') {
                    v.insert(p.with_extension("css"));
                }
            }
        }
        v.extend(
            pacote
                .folhas_geradas
                .keys()
                .filter(|k| k.starts_with(dir))
                .cloned(),
        );
    }
    v.into_iter().collect()
}

/// The unit is a part (`part of`), not a library.
pub fn e_parte(unit: &dartforge_frontend::ast::CompilationUnit) -> bool {
    unit.directives.iter().any(|d| {
        matches!(
            d.kind,
            dartforge_frontend::ast::DirectiveKind::PartOf { .. }
        )
    })
}

/// The components and directives of the package, by library and class.
///
/// The official compiler asks the `build` asset graph for this; here it comes
/// from the first pass. It is what allows matching `<a02-texto-estatico>` with
/// the class that declares that selector and emitting the child view — and
/// knowing that a `<form>` gets `NgForm`.
///
/// It is incremental: [`Indice::atualizar`] and [`Indice::remover`] replace
/// what one file put in, without rereading the rest — the build engine keeps it
/// alive during the session.
#[derive(Default)]
pub struct Indice {
    por_classe: std::collections::HashMap<(String, String), view::Filho>,
    /// `@Directive` by (library URI, class): the selector and what the emitter
    /// needs to instantiate it.
    diretivas: std::collections::HashMap<(String, String), component::Componente>,
    /// `@Pipe` por (URI da biblioteca, classe).
    pipes: std::collections::HashMap<(String, String), component::Pipe>,
    /// The metadata of each `@Directive`, read from the program
    /// (`metadata.rs`): it is through them that the emitter instantiates the
    /// directive on a node. Without a program, empty — and every matched
    /// directive is refused.
    metadados: std::collections::HashMap<(String, String), std::sync::Arc<directives::Diretiva>>,
    /// The `@GenerateInjector`s by (library URI, variable), read from the
    /// program (`metadata::ler_injetores`), or the reason they could not be
    /// read.
    injetores: std::collections::HashMap<
        (String, String),
        Result<std::sync::Arc<metadata::Injetor>, String>,
    >,
    /// The keys each file put in, for `remover`/`atualizar`.
    por_arquivo: std::collections::HashMap<PathBuf, Vec<(String, String)>>,
}

impl Indice {
    /// An empty index.
    pub fn novo() -> Indice {
        Indice::default()
    }

    fn montar(
        pacote: &Pacote,
        arquivos: &[(PathBuf, String, Achados)],
        programa: Option<&resolucao::Resolvedor>,
    ) -> Self {
        // With the program loaded, the index covers all packages: an
        // `<li-select>` from limitless_ui is as much a component as one of the
        // project itself. Without it, only what the file scan saw.
        let mut indice = match programa {
            Some(r) => Indice::do_programa(r),
            None => Indice::novo(),
        };
        for (caminho, _nome, achados) in arquivos {
            let Some(uri) = uri_de_biblioteca(pacote, caminho) else {
                continue;
            };
            indice.juntar(&uri, Some(caminho), achados, programa, false);
        }
        indice
    }

    /// The index of all libraries loaded in the program (dependencies
    /// included), each one registered by its file: afterwards,
    /// [`Indice::atualizar`] replaces only what changes.
    pub fn do_programa(r: &resolucao::Resolvedor) -> Indice {
        let mut indice = Indice::novo();
        let interner = r.interner();
        for (uri, arvore, unidade, fonte, caminho) in r.bibliotecas() {
            let achados = achar(arvore, unidade, fonte, interner);
            indice.juntar(&uri, caminho, &achados, Some(r), true);
        }
        indice
    }

    /// The findings of `arquivo` go into the index, replacing the previous
    /// ones. With the program, the directive metadata is also reread from it
    /// (and the components' injection resolved); without it, the components
    /// keep unresolved injection and the directives no metadata — which refuses
    /// whoever uses them.
    pub fn atualizar(
        &mut self,
        pacote: &Pacote,
        arquivo: &Path,
        achados: &Achados,
        programa: Option<&resolucao::Resolvedor>,
    ) {
        self.remover(arquivo);
        let Some(uri) = uri_de_biblioteca(pacote, arquivo) else {
            return;
        };
        self.juntar(&uri, Some(arquivo), achados, programa, true);
    }

    /// Removes from the index everything `arquivo` had put in.
    pub fn remover(&mut self, arquivo: &Path) {
        let Some(chaves) = self
            .por_arquivo
            .remove(&dartforge_elements::gerado::chave(arquivo))
        else {
            return;
        };
        for k in chaves {
            self.por_classe.remove(&k);
            self.diretivas.remove(&k);
            self.pipes.remove(&k);
            self.metadados.remove(&k);
            self.injetores.remove(&k);
        }
    }

    /// The components `comp` can use in its template, in the order of the
    /// expanded `directives:` (the directives are left out).
    pub fn filhos_de(
        &self,
        comp: &component::Componente,
        fonte: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> Vec<view::Filho> {
        self.diretivas_de(comp, fonte, resolvedor)
            .0
            .into_iter()
            .filter_map(|u| u.filho)
            .collect()
    }

    /// Who declares exactly this selector: (library, class). With more than
    /// one, the one with the smallest key — the answer does not depend on the
    /// map order.
    pub fn declarante(&self, seletor: &str) -> Option<(String, String)> {
        self.por_classe
            .iter()
            .filter(|(_, f)| f.seletor == seletor)
            .map(|(k, _)| k)
            .chain(
                self.diretivas
                    .iter()
                    .filter(|(_, d)| d.seletor == seletor)
                    .map(|(k, _)| k),
            )
            .min()
            .cloned()
    }

    /// Adds the findings of a library. `substitui`: directives and pipes
    /// replace whatever is there (otherwise, what is already there stays — the
    /// file pass does not overwrite what the program read).
    fn juntar(
        &mut self,
        uri: &str,
        caminho: Option<&Path>,
        achados: &Achados,
        programa: Option<&resolucao::Resolvedor>,
        substitui: bool,
    ) {
        let mut chaves = Vec::new();
        for comp in &achados.componentes {
            if comp.seletor.is_empty() {
                continue;
            }
            indexar(
                &mut self.por_classe,
                uri,
                comp,
                caminho,
                programa.map(|r| r as &dyn resolucao::Resolucao),
            );
            let k = (uri.to_string(), comp.classe.clone());
            // The component's metadata: with it, its `providers:`
            // (`ExistingProvider`, like those of directives) go into the node
            // of whoever uses it, and "child with providers" stops being a
            // refusal.
            if let Some(r) = programa
                && let Some(m) = r
                    .classe_por_uri(uri, &comp.classe)
                    .and_then(|id| crate::metadata::ler(r, id))
                && let Some(f) = self.por_classe.get_mut(&k)
            {
                // The template node writes the child's `providers:` as the host
                // view does; what is not emitted yet (a dependency from outside
                // the node, a provider requested on the node itself) is refused
                // by the view, with the reason.
                if crate::view::provedores_escreviveis(&m).is_ok() {
                    f.pendencias.retain(|p| p.forma != "child with providers");
                }
                // The constructor parameters from the metadata (`@Inject`,
                // `@Attribute`, opaque tokens): they replace the ones read from
                // the text and the refusal they caused.
                let recusa_do_texto = comp
                    .parametros
                    .iter()
                    .find_map(|p| injetado(p, caminho, Some(r as &dyn resolucao::Resolucao)).err());
                match parametros_dos_metadados(comp, &m) {
                    Ok(ps) => {
                        if let Some(forma) = recusa_do_texto {
                            f.pendencias.retain(|p| p.forma != forma);
                        }
                        f.parametros = ps;
                    }
                    // The metadata says better what is missing: its reason
                    // replaces that of the text.
                    Err(motivo) => {
                        if let Some(forma) = recusa_do_texto {
                            for p in f.pendencias.iter_mut().filter(|p| p.forma == forma) {
                                p.forma = motivo.to_string();
                            }
                        }
                    }
                }
                // The inheriting child: `@Input`, `@Output`, `@HostBinding` and
                // hooks from the metadata, which walks up the supertypes like
                // the official `_collectInheritableMetadata`. The inherited
                // content query is not written yet.
                // The hooks always come from the program (indirect interface:
                // `implements X`, `X extends OnInit`).
                if m.fora.is_empty() {
                    f.ganchos = m.ganchos;
                }
                if comp.herda && m.fora.is_empty() {
                    f.entradas = m
                        .entradas
                        .iter()
                        .map(|e| component::Entrada {
                            nome: e.nome.clone(),
                            campo: e.membro.clone(),
                        })
                        .collect();
                    f.saidas = m.saidas.clone();
                    f.ganchos = m.ganchos;
                    f.hospedeiro = m.ligacoes_do_hospedeiro.iter().any(|(_, membro)| {
                        !comp
                            .ligacoes_do_hospedeiro
                            .iter()
                            .any(|l| l.estatico && l.membro == *membro)
                    });
                    if m.consultas_de_conteudo.is_empty() {
                        f.pendencias
                            .retain(|p| p.forma != "child that inherits (extends/with)");
                    }
                }
                f.metadados = Some(std::sync::Arc::new(m));
            }
            chaves.push(k);
        }
        for d in &achados.diretivas {
            let k = (uri.to_string(), d.classe.clone());
            if let Some(r) = programa
                && let Some(m) = r
                    .classe_por_uri(uri, &d.classe)
                    .and_then(|id| crate::metadata::ler(r, id))
            {
                self.metadados.insert(k.clone(), std::sync::Arc::new(m));
            }
            if substitui {
                self.diretivas.insert(k.clone(), d.clone());
            } else {
                self.diretivas.entry(k.clone()).or_insert_with(|| d.clone());
            }
            chaves.push(k);
        }
        if !achados.injetores.is_empty()
            && let Some(r) = programa
        {
            for (nome, lido) in crate::metadata::ler_injetores(r, uri) {
                let k = (uri.to_string(), nome);
                self.injetores
                    .insert(k.clone(), lido.map(std::sync::Arc::new));
                chaves.push(k);
            }
        }
        for p in &achados.pipes {
            let k = (uri.to_string(), p.classe.clone());
            if substitui {
                self.pipes.insert(k.clone(), p.clone());
            } else {
                self.pipes.entry(k.clone()).or_insert_with(|| p.clone());
            }
            chaves.push(k);
        }
        if let Some(c) = caminho {
            self.por_arquivo
                .entry(dartforge_elements::gerado::chave(c))
                .or_default()
                .extend(chaves);
        }
    }
    /// The pipes this component uses, in the order of `pipes:` expanded
    /// depth-first (the constant lists, like `commonPipes`, through the
    /// semantic database). The official compiler looks the name up from the end
    /// to the start (`_findPipeMeta`), so the order matters. A class that is
    /// not a pipe is left out; a name that does not resolve comes back as a
    /// refusal.
    fn pipes_de(
        &self,
        comp: &component::Componente,
        arquivo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> (Vec<view::PipeUsado>, Vec<Recusa>) {
        let mut saida = Vec::new();
        let mut fora = Vec::new();
        if comp.pipes_ilegiveis {
            fora.push(recusa(
                Motivo::PipesUsados,
                "pipes: with an item that is not a name",
            ));
        }
        let mut pilha: Vec<(String, PathBuf, u32)> = comp
            .pipes
            .iter()
            .rev()
            .map(|n| (n.clone(), arquivo.to_path_buf(), 0))
            .collect();
        while let Some((nome, escopo, profundidade)) = pilha.pop() {
            if profundidade > 32 {
                fora.push(recusa(Motivo::PipesUsados, "pipes: circular list"));
                break;
            }
            let simples = nome.rsplit('.').next().unwrap_or(&nome).to_string();
            match resolvedor.and_then(|r| r.designado(&escopo, &nome)) {
                Some(resolucao::Designado::Classe { uri }) => {
                    if let Some(p) = self.pipes.get(&(uri.clone(), simples)) {
                        saida.push(view::PipeUsado {
                            uri,
                            pipe: p.clone(),
                        });
                    }
                }
                Some(resolucao::Designado::Lista { itens, escopo }) => {
                    for item in itens.iter().rev() {
                        pilha.push((item.clone(), escopo.clone(), profundidade + 1));
                    }
                }
                None => fora.push(recusa(Motivo::PipesUsados, "pipes: unresolved name")),
            }
        }
        (saida, fora)
    }

    /// The directives and components this component uses, in the official
    /// order: `directives:` expanded depth-first (the constant lists through
    /// the semantic database), without repetition (`removeDuplicates`). A class
    /// that is neither a directive nor a component is ignored, as in the
    /// official compiler (`typeDeclarationOf(value)?.accept(...)` returns
    /// `null`). What cannot be resolved comes back as a refusal: without the
    /// whole list one cannot know what matches each element.
    fn diretivas_de(
        &self,
        comp: &component::Componente,
        arquivo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> (Vec<view::Usada>, Vec<Recusa>) {
        let mut saida = Vec::new();
        let mut vistos = std::collections::HashSet::new();
        let mut fora = Vec::new();
        if comp.tipos_de_diretiva_ilegiveis {
            fora.push(recusa(
                Motivo::NaoEntendido,
                "directiveTypes: in an unreadable shape",
            ));
        }
        if comp.diretivas_ilegiveis {
            fora.push(recusa(
                Motivo::DiretivaPorSeletor,
                "directives: with an item that is not a name",
            ));
        }
        for nome in &comp.diretivas {
            self.expandir(
                nome,
                arquivo,
                resolvedor,
                &mut saida,
                &mut vistos,
                &mut fora,
                0,
            );
        }
        (saida, fora)
    }

    #[allow(clippy::too_many_arguments)]
    fn expandir(
        &self,
        nome: &str,
        escopo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
        saida: &mut Vec<view::Usada>,
        vistos: &mut std::collections::HashSet<(String, String)>,
        fora: &mut Vec<Recusa>,
        profundidade: u32,
    ) {
        if profundidade > 32 {
            fora.push(recusa(
                Motivo::DiretivaPorSeletor,
                "directives: circular list",
            ));
            return;
        }
        let simples = nome.rsplit('.').next().unwrap_or(nome).to_string();
        match resolvedor.and_then(|r| r.designado(escopo, nome)) {
            Some(resolucao::Designado::Classe { uri }) => {
                let chave = (uri.clone(), simples.clone());
                if !vistos.insert(chave.clone()) {
                    return;
                }
                if let Some(f) = self.por_classe.get(&chave) {
                    saida.push(view::Usada {
                        classe: simples,
                        uri,
                        seletores: selector::Seletor::analisar(&f.seletor),
                        filho: Some(f.clone()),
                        diretiva: None,
                    });
                } else if let Some(d) = self.diretivas.get(&chave) {
                    saida.push(view::Usada {
                        classe: simples,
                        uri,
                        seletores: selector::Seletor::analisar(&d.seletor),
                        filho: None,
                        diretiva: self.metadados.get(&chave).cloned(),
                    });
                }
            }
            Some(resolucao::Designado::Lista { itens, escopo }) => {
                for item in &itens {
                    self.expandir(
                        item,
                        &escopo,
                        resolvedor,
                        saida,
                        vistos,
                        fora,
                        profundidade + 1,
                    );
                }
            }
            None => {
                // No answer from the semantic database: only a component with a
                // unique name in the package (the earlier path, without a
                // program).
                let por_nome: Vec<&view::Filho> = self
                    .por_classe
                    .values()
                    .filter(|f| f.classe == simples)
                    .collect();
                match por_nome.as_slice() {
                    [f] if !nome.contains('.') => {
                        let chave = (f.uri_dart.clone(), f.classe.clone());
                        if vistos.insert(chave) {
                            saida.push(view::Usada {
                                classe: f.classe.clone(),
                                uri: f.uri_dart.clone(),
                                seletores: selector::Seletor::analisar(&f.seletor),
                                filho: Some((*f).clone()),
                                diretiva: None,
                            });
                        }
                    }
                    _ => fora.push(recusa(
                        Motivo::DiretivaPorSeletor,
                        "directives: unresolved name",
                    )),
                }
            }
        }
    }
}

/// The components the emitter matches by tag: those whose selector is only an
/// element name. The rest of the list only serves the directive guard.
fn filhos_por_tag(usadas: &[view::Usada]) -> std::collections::HashMap<String, view::Filho> {
    let mut saida = std::collections::HashMap::new();
    // Compound selector (`page-header-comp,pg-header`): each alternative that
    // is just the tag matches the element with that tag. The other alternatives
    // are left to the directive guard.
    for u in usadas {
        let Some(f) = &u.filho else {
            continue;
        };
        for s in &u.seletores {
            if let Some(tag) = s.so_tag() {
                saida.entry(tag.to_string()).or_insert_with(|| f.clone());
            }
        }
    }
    saida
}

/// The constructor parameters of a child from the metadata read from the
/// program (`_getCompileDiDependencyMetadata`: the named ones are left out;
/// `@Inject(token)` and `@Attribute('name')` resolved), as the official
/// compiler resolves them on its node. A generic type without `@Inject` and the
/// annotated element built-ins are not translated yet.
fn parametros_dos_metadados(
    comp: &component::Componente,
    meta: &directives::Diretiva,
) -> Result<Vec<view::Injetado>, String> {
    use directives::Token;
    if let Some(f) = meta.fora.first() {
        return Err(format!("incomplete child metadata: {f}"));
    }
    let textuais: Vec<&component::Parametro> =
        comp.parametros.iter().filter(|p| !p.nomeado).collect();
    if textuais.len() != meta.dependencias.len() {
        return Err("child parameters without the metadata".into());
    }
    let mut saida = Vec::new();
    for d in &meta.dependencias {
        if let Some(nome) = &d.atributo {
            saida.push(view::Injetado::Atributo(nome.clone()));
            continue;
        }
        let anotado = d.opcional || d.proprio || d.hospedeiro || d.pular;
        let embutido = match &d.token {
            Token::Elemento | Token::Detector => true,
            Token::Classe { uri, classe } => embutido_do_ngdart(uri, classe),
            _ => false,
        };
        if embutido && anotado {
            return Err("annotated element built-in in the child constructor".into());
        }
        saida.push(match &d.token {
            Token::Elemento => view::Injetado::Elemento,
            Token::Detector => view::Injetado::Detector,
            Token::Classe { uri, classe } if embutido_do_ngdart(uri, classe) => {
                match classe.as_str() {
                    "ViewContainerRef" | "ComponentLoader" => view::Injetado::Container,
                    _ => return Err("ngdart token in the child constructor".into()),
                }
            }
            Token::Classe { uri, .. } if uri.starts_with("dart:") => {
                return Err("SDK type in the child constructor".into());
            }
            t => view::Injetado::Servico {
                token: t.clone(),
                opcional: d.opcional,
                proprio: d.proprio,
                hospedeiro: d.hospedeiro,
                pular: d.pular,
            },
        });
    }
    Ok(saida)
}

/// The node built-ins that the official `_getLocalDependency` returns without
/// looking for a provider (`ElementRef`, `ChangeDetectorRef`, `NgContentRef`,
/// `TemplateRef`, `ViewContainerRef`, `ComponentLoader`, `Injector`); another
/// ngdart class (`NgZone`, …) is a service like any other.
fn embutido_do_ngdart(uri: &str, classe: &str) -> bool {
    uri.starts_with("package:ngdart/")
        && matches!(
            classe,
            "ElementRef"
                | "ChangeDetectorRef"
                | "NgContentRef"
                | "TemplateRef"
                | "ViewContainerRef"
                | "ComponentLoader"
                | "Injector"
        )
}

/// How the official compiler resolves a constructor parameter of a child on its
/// node: the node, the child's view or a service from outside the view. The
/// rest (another ngdart token, `@Inject`, `@Self`, generic, named) not yet.
fn injetado(
    p: &component::Parametro,
    caminho: Option<&Path>,
    resolvedor: Option<&dyn resolucao::Resolucao>,
) -> Result<view::Injetado, &'static str> {
    let tipo = p
        .tipo
        .as_deref()
        .ok_or("untyped parameter in the child constructor")?;
    if p.nomeado {
        return Err("named parameter in the child constructor");
    }
    if p.outra_anotacao {
        return Err("@Inject/@Attribute in the child constructor");
    }
    let tipo = view::tipo_do_token(tipo);
    let uri = match (caminho, resolvedor) {
        (Some(c), Some(r)) => r.uri_do_tipo(c, tipo),
        _ => None,
    }
    .ok_or("unresolved type injected into the child")?;
    let simples = tipo.rsplit('.').next().unwrap_or(tipo).to_string();
    // The element built-ins (the node, the view, the `ViewContainer`) with
    // `@Self`/`@Host`/`@SkipSelf`/`@Optional`: no case yet.
    let embutido = crate::dialect::no_embutido(&uri, &simples).is_some()
        || embutido_do_ngdart(&uri, &simples);
    if embutido && p.anotado {
        return Err("annotated element built-in in the child constructor");
    }
    if crate::dialect::no_embutido(&uri, &simples).is_some() {
        return Ok(view::Injetado::Elemento);
    }
    if embutido_do_ngdart(&uri, &simples) {
        return match simples.as_str() {
            "ChangeDetectorRef" if !p.opcional => Ok(view::Injetado::Detector),
            // The node gets a `ViewContainer` (`requiresViewContainer`), which
            // the child receives (case j47).
            "ViewContainerRef" | "ComponentLoader" => Ok(view::Injetado::Container),
            _ => Err("ngdart token in the child constructor"),
        };
    }
    if uri.starts_with("dart:") {
        return Err("SDK type in the child constructor");
    }
    Ok(view::Injetado::Servico {
        token: directives::Token::Classe {
            uri,
            classe: simples,
        },
        opcional: p.opcional,
        proprio: p.proprio,
        hospedeiro: p.hospedeiro,
        pular: p.pular,
    })
}

/// Puts a component in the index, with what the emitter needs from it.
fn indexar(
    por_classe: &mut std::collections::HashMap<(String, String), view::Filho>,
    uri: &str,
    comp: &component::Componente,
    caminho: Option<&Path>,
    resolvedor: Option<&dyn resolucao::Resolucao>,
) {
    if comp.seletor.is_empty() {
        return;
    }
    // If the child projects content, its user calls `createAndProject`.
    let nos = match (&comp.template, &comp.template_url) {
        (Some(t), _) => Some(html::analisar(t)),
        (None, Some(u)) => caminho
            .and_then(|c| c.parent().map(|d| d.join(u)))
            .and_then(|c| std::fs::read_to_string(c).ok())
            .map(|t| html::analisar(&t)),
        (None, None) => None,
    };
    // `ngContentSelectors`: the `select` of each `<ng-content>`, `*` without
    // it.
    let projecoes: Vec<String> = nos
        .as_deref()
        .map(html::projecoes)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.unwrap_or_else(|| "*".to_string()))
        .collect();
    // What in the child changes the code of its user and is not written yet.
    let em_filho = |f: &str| recusa(Motivo::LigacaoEmFilho, f);
    let mut pendencias = Vec::new();
    let mut parametros = Vec::new();
    for p in &comp.parametros {
        match injetado(p, caminho, resolvedor) {
            Ok(x) => parametros.push(x),
            Err(f) => {
                pendencias.push(em_filho(f));
                break;
            }
        }
    }
    // The content queries, with the target resolved in the child's file. An
    // ngdart type (`TemplateRef`, core directives) would match what the emitter
    // puts in the content (`*ngIf`): not yet.
    let mut consultas = Vec::new();
    match &comp.consultas_de_conteudo {
        None => pendencias.push(em_filho("child with @ContentChild outside the shape")),
        Some(lidas) => {
            for q in lidas {
                // `read:` changes the value read: the node
                // (`HtmlElement`/`Element`) or another provider of the node, of
                // a package class. The rest (`ViewContainerRef`, `TemplateRef`,
                // tokens) not yet.
                let leitura = match &q.leitura {
                    None => None,
                    Some(nome) => {
                        let uri = match (caminho, resolvedor) {
                            (Some(c), Some(r)) => r.uri_do_tipo(c, nome),
                            _ => None,
                        };
                        let simples = nome.rsplit('.').next().unwrap_or(nome);
                        match uri {
                            Some(u) if crate::dialect::no_embutido(&u, simples).is_some() => {
                                Some(view::LeituraDaConsulta::Elemento)
                            }
                            Some(u)
                                if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") =>
                            {
                                Some(view::LeituraDaConsulta::Classe(u, simples.to_string()))
                            }
                            _ => {
                                pendencias.push(em_filho("child with @ContentChild(.., read:)"));
                                break;
                            }
                        }
                    }
                };
                if q.referencia {
                    consultas.push(view::ConsultaDoFilho {
                        campo: q.campo.clone(),
                        lista: q.lista,
                        alvo: view::AlvoDeConsulta::Referencia(q.alvo.clone()),
                        descendentes: q.descendentes,
                        leitura,
                    });
                    continue;
                }
                let uri = match (caminho, resolvedor) {
                    (Some(c), Some(r)) => r.uri_do_tipo(c, &q.alvo),
                    _ => None,
                };
                match uri {
                    Some(u) if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") => {
                        let simples = q.alvo.rsplit('.').next().unwrap_or(&q.alvo);
                        consultas.push(view::ConsultaDoFilho {
                            campo: q.campo.clone(),
                            lista: q.lista,
                            alvo: view::AlvoDeConsulta::Classe(u, simples.to_string()),
                            descendentes: q.descendentes,
                            leitura,
                        });
                    }
                    _ => {
                        pendencias.push(em_filho("child with @ContentChild of an unresolved type"));
                        break;
                    }
                }
            }
        }
    }
    if comp.com_provedores {
        pendencias.push(em_filho("child with providers"));
    }
    // Inherited hooks, `@Input` and `@Output` cannot be seen from here (the
    // official compiler collects them through the supertypes).
    if comp.herda {
        pendencias.push(em_filho("child that inherits (extends/with)"));
    }
    if comp.template.is_none() && comp.template_url.is_some() && nos.is_none() {
        pendencias.push(em_filho("child template not found"));
    }
    por_classe.insert(
        (uri.to_string(), comp.classe.clone()),
        view::Filho {
            classe: comp.classe.clone(),
            seletor: comp.seletor.clone(),
            uri_dart: uri.to_string(),
            uri_template: uri.replace(".dart", ".template.dart"),
            projecoes,
            entradas: comp.entradas.clone(),
            ganchos: comp.ganchos,
            on_push: comp.on_push,
            link_de_deteccao: comp.link_de_deteccao,
            // The static one goes into the constructor of the child's view:
            // without another one, the child has no `detectHostChanges` (case
            // j101).
            hospedeiro: comp.liga_hospedeiro
                && (comp.ligacoes_do_hospedeiro.is_empty()
                    || comp.ligacoes_do_hospedeiro.iter().any(|l| !l.estatico)),
            saidas: comp.saidas.clone(),
            atributos_do_hospedeiro: comp
                .ligacoes_do_hospedeiro
                .iter()
                .filter(|l| l.estatico && l.imutavel)
                .filter(|l| !l.nome.starts_with("class.") && !l.nome.starts_with("style."))
                .map(|l| {
                    let nome = l.nome.strip_prefix("attr.").unwrap_or(&l.nome);
                    (nome.to_string(), l.membro.clone())
                })
                .collect(),
            parametros,
            consultas,
            pendencias,
            metadados: None,
        },
    );
}

/// Package name, read from the root `pubspec.yaml`.
///
/// It is the authoritative source: the folder name does not work
/// (`limitless_ui/example` declares `name: limitless_ui_example`) and comparing
/// the `rootUri` of `package_config.json` with the root fails because of
/// trailing slashes and canonicalization.
pub fn nome_do_pacote(raiz: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(raiz.join("pubspec.yaml")).ok()?;
    for linha in texto.lines() {
        // `name:` at the top level, without indentation.
        let Some(valor) = linha.strip_prefix("name:") else {
            continue;
        };
        if linha.starts_with(char::is_whitespace) {
            continue;
        }
        let nome = valor.trim().trim_matches(['\'', '"']).trim();
        if !nome.is_empty() {
            return Some(nome.to_string());
        }
    }
    None
}

/// `package:` URI of a file of the package, when it is in `lib/`.
fn uri_de_biblioteca(pacote: &Pacote, caminho: &Path) -> Option<String> {
    let rel = pacote.relativo(caminho);
    let dentro = rel.strip_prefix("lib/")?;
    // In canonical form, that of the index (`package:ngx_forms/` → `ngforms`).
    Some(dialect::canonica_string(format!(
        "package:{}/{dentro}",
        pacote.nome
    )))
}

/// What comes out of a file: the `.template.dart`, the files that feed it and
/// those generated separately (the `.css.shim.dart` of each stylesheet).
pub(crate) type Gerado = (String, Vec<PathBuf>, Vec<(PathBuf, String)>);

/// The directives of the file that get an `XNgCd`, in source order: the
/// bindings are those of the metadata read from the program (the official
/// `hostProperties`, supertypes first), including inherited ones — and a
/// directive that only inherits bindings gets one too (case j84). For an
/// inherited member, the immutability (a `final` field would be written once)
/// and, for `style.x`, the type are asked of the resolver. Without the
/// metadata, only the bindings of the class itself, and an inheriting directive
/// is refused.
fn hospedeiras_efetivas(
    pacote: &Pacote,
    fonte: &Path,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    indice: &Indice,
) -> Result<Vec<Hospedeira>, Recusa> {
    let uri = uri_de_biblioteca(pacote, fonte);
    let metadados = |classe: &str| {
        uri.as_ref()
            .and_then(|u| indice.metadados.get(&(u.clone(), classe.to_string())))
            .filter(|m| m.fora.is_empty())
    };
    let tem_todos = achados
        .diretivas
        .iter()
        .all(|d| metadados(&d.classe).is_some());
    if !tem_todos {
        if achados.hospedeiro_herdado {
            return Err(recusa(
                Motivo::HostBindingEmDiretiva,
                "@HostBinding/@HostListener on an inheriting directive",
            ));
        }
        return Ok(achados.hospedeiras.clone());
    }
    let mut saida = Vec::new();
    for d in &achados.diretivas {
        let Some(m) = metadados(&d.classe) else {
            continue;
        };
        if m.ligacoes_do_hospedeiro.is_empty() {
            continue;
        }
        let estaticos: std::collections::HashSet<String> =
            m.estaticos_do_hospedeiro.iter().cloned().collect();
        let propria = achados.hospedeiras.iter().find(|h| h.classe == d.classe);
        if let Some(h) = propria
            && h.ligacoes == m.ligacoes_do_hospedeiro
            && estaticos.is_empty()
        {
            saida.push(h.clone());
            continue;
        }
        let mut h = Hospedeira {
            classe: d.classe.clone(),
            ligacoes: m.ligacoes_do_hospedeiro.clone(),
            tipos_de_estilo: std::collections::HashMap::new(),
            recusada: None,
            imutaveis: std::collections::HashSet::new(),
            estaticos: estaticos.clone(),
        };
        for (nome, membro) in &m.ligacoes_do_hospedeiro {
            let forma = view::forma_do_hospedeiro(nome);
            if estaticos.contains(membro) {
                if let Err(f) = forma {
                    h.recusada.get_or_insert(f);
                }
                continue;
            }
            let final_ = resolvedor.and_then(|r| r.membro_final(fonte, &d.classe, membro));
            if final_ == Some(true) {
                h.imutaveis.insert(membro.clone());
            }
            match forma {
                Err(f) => {
                    h.recusada.get_or_insert(f);
                }
                Ok(_) if final_.is_none() => {
                    h.recusada.get_or_insert(format!(
                        "inherited @HostBinding('{nome}') without a readable declaration"
                    ));
                }
                Ok(view::FormaDoHospedeiro::Estilo { .. }) if final_ == Some(true) => {
                    h.recusada
                        .get_or_insert(format!("@HostBinding('{nome}') on a final field"));
                }
                Ok(view::FormaDoHospedeiro::Estilo { .. }) => {
                    // The written type, from the program (which walks up to the
                    // class that declares the member), otherwise that of the
                    // class itself.
                    let tipo = resolvedor
                        .and_then(|r| r.tipo_do_membro(fonte, &d.classe, membro))
                        .map(|(t, _)| t)
                        .or_else(|| propria.and_then(|x| x.tipos_de_estilo.get(membro).cloned()));
                    let base = tipo.as_deref().map(|t| t.trim().trim_end_matches('?'));
                    match (tipo.clone(), base) {
                        (Some(t), Some(b))
                            if !b.is_empty()
                                && !matches!(b, "dynamic" | "var" | "Object" | "Never") =>
                        {
                            h.tipos_de_estilo.insert(membro.clone(), t);
                        }
                        _ => {
                            h.recusada.get_or_insert(format!(
                                "@HostBinding('{nome}') of unknown type"
                            ));
                        }
                    }
                }
                Ok(_) => {}
            }
        }
        saida.push(h);
    }
    Ok(saida)
}

/// Contents of the `.template.dart` of a file, when we know how to generate it,
/// with the files that feed it (the `.dart` and the template's `.html`).
pub(crate) fn gerar_interno(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<Gerado, Recusa> {
    // An annotation of the file with a name that does not resolve is not
    // constant: the official compiler fails to compile it ("Arguments of a
    // constant creation must be constant expressions") and generates nothing.
    if let Some(uri) = uri_de_biblioteca(pacote, fonte) {
        let metadados = achados
            .diretivas
            .iter()
            .filter_map(|d| {
                indice
                    .metadados
                    .get(&(uri.clone(), d.classe.clone()))
                    .cloned()
            })
            .chain(achados.componentes.iter().filter_map(|c| {
                indice
                    .por_classe
                    .get(&(uri.clone(), c.classe.clone()))
                    .and_then(|f| f.metadados.clone())
            }));
        for m in metadados {
            if let Some(f) = m.fora.iter().find(|f| f.contains("unresolved name")) {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!(
                        "annotation of {} with an unresolved name ({f}): the official compiler fails",
                        m.classe
                    ),
                ));
            }
        }
    }
    // The same holds for whoever lists that class in `directives:`: the
    // official compiler reads the metadata of each directive used when
    // compiling the component, and its failure is the component's (the
    // `material_date_time_picker` with the `MaterialInputComponent` of a
    // nonexistent `NG_VALIDATORS`).
    for comp in &achados.componentes {
        let (usadas, _) = indice.diretivas_de(comp, fonte, resolvedor);
        for u in &usadas {
            let m = u
                .filho
                .as_ref()
                .and_then(|f| f.metadados.clone())
                .or_else(|| u.diretiva.clone());
            if let Some(m) = m
                && let Some(f) = m.fora.iter().find(|f| f.contains("unresolved name"))
            {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!(
                        "directives: {} has an annotation with an unresolved name ({f}): the official compiler fails",
                        m.classe
                    ),
                ));
            }
        }
    }
    if achados.injetores.is_empty() {
        return gerar_visoes(
            pacote,
            fonte,
            nome_do_arquivo,
            achados,
            resolvedor,
            nomes,
            indice,
        );
    }
    // `buildGeneratedCode`: the injectors' imports right after the import of
    // the file itself, before those of the views; their code at the end.
    let uri = uri_de_biblioteca(pacote, fonte)
        .ok_or_else(|| recusa(Motivo::Injetor, "@GenerateInjector outside a package"))?;
    let mut injetores = Vec::new();
    for nome in &achados.injetores {
        match indice.injetores.get(&(uri.clone(), nome.clone())) {
            Some(Ok(i)) => injetores.push(i.clone()),
            Some(Err(f)) => return Err(recusa(Motivo::Injetor, f.clone())),
            None => return Err(recusa(Motivo::Injetor, "@GenerateInjector sem programa")),
        }
    }
    let (mut texto, entradas, extras) = gerar_visoes(
        pacote,
        fonte,
        nome_do_arquivo,
        achados,
        resolvedor,
        nomes,
        indice,
    )?;
    let refs: Vec<&metadata::Injetor> = injetores.iter().map(|i| i.as_ref()).collect();
    let (imports, corpo) = injector::emitir(&refs).map_err(|f| recusa(Motivo::Injetor, f))?;
    let marca = format!("import '{nome_do_arquivo}';\n");
    let depois = texto
        .find(&marca)
        .map(|i| i + marca.len())
        .ok_or_else(|| recusa(Motivo::Injetor, "generated file without the import of itself"))?;
    texto.insert_str(depois, &imports);
    texto.push('\n');
    texto.push_str(&corpo);
    Ok((texto, entradas, extras))
}

/// The `.template.dart` of the views (and of the `XNgCd`s) of a file, without
/// the injectors.
fn gerar_visoes(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<Gerado, Recusa> {
    if achados.componentes.is_empty() && achados.diretivas.is_empty() && achados.pipes.is_empty() {
        return Ok((
            template_trivial(nome_do_arquivo),
            vec![fonte.to_path_buf()],
            Vec::new(),
        ));
    }
    // Directives and pipes do not generate a view: their file is the trivial
    // one, unless a directive has `@HostBinding` — then the official compiler
    // generates its `DirectiveChangeDetector`.
    // The `XNgCd` of each directive of the file, with the inherited bindings
    // (case j84).
    let hospedeiras = hospedeiras_efetivas(pacote, fonte, achados, resolvedor, indice)?;
    if achados.componentes.is_empty() {
        if hospedeiras.is_empty() {
            return Ok((
                template_trivial(nome_do_arquivo),
                vec![fonte.to_path_buf()],
                Vec::new(),
            ));
        }
        // The `XNgCd` classes in source order, with the shared import table; a
        // directive without `@HostBinding` and a pipe generate nothing.
        if let Some(m) = hospedeiras.iter().find_map(|h| h.recusada.clone()) {
            return Err(recusa(Motivo::HostBindingEmDiretiva, m));
        }
        let hs: Vec<&Hospedeira> = hospedeiras.iter().collect();
        return Ok((
            view::detector_de_diretivas(&hs, nome_do_arquivo),
            vec![fonte.to_path_buf()],
            Vec::new(),
        ));
    }
    // A directive without `@HostBinding`/`@HostListener` and a pipe generate
    // nothing in the file (case j29); one with `@HostBinding` gets the `XNgCd`
    // class after the components (case j45).
    if let Some(m) = hospedeiras.iter().find_map(|h| h.recusada.clone()) {
        return Err(recusa(Motivo::HostBindingEmDiretiva, m));
    }
    // Several components: there is a single import table and the fragments come
    // out in source order (case i24); the import of each one's stylesheet is
    // allocated when its `styles$X` list is written (case j44).
    let mut imp = view::Importacoes::default();
    let mut trechos = Vec::new();
    let mut entradas = vec![fonte.to_path_buf()];
    for comp in &achados.componentes {
        let (trecho, html) = trecho_do_componente(
            pacote,
            fonte,
            nome_do_arquivo,
            comp,
            resolvedor,
            nomes,
            indice,
            &mut imp,
        )?;
        trechos.push(trecho);
        entradas.extend(html);
    }
    for h in &hospedeiras {
        trechos.push(view::classe_ngcd(h, nome_do_arquivo, &mut imp));
    }
    Ok((
        view::montar_arquivo(nome_do_arquivo, &imp, &trechos),
        entradas,
        Vec::new(),
    ))
}

/// The `Local` of a component: where it lives and its metadata.
fn local_do_componente<'a>(
    pacote: &'a Pacote,
    fonte: &'a Path,
    relativo: &'a str,
    nome_do_arquivo: &'a str,
    comp: &component::Componente,
    indice: &Indice,
) -> view::Local<'a> {
    view::Local {
        pacote: &pacote.nome,
        relativo,
        arquivo: nome_do_arquivo,
        caminho: fonte,
        raiz: &pacote.raiz,
        url_do_template: url_do_template(pacote, fonte, comp),
        metadados: uri_de_biblioteca(pacote, fonte).and_then(|uri| {
            indice
                .por_classe
                .get(&(uri, comp.classe.clone()))
                .and_then(|f| f.metadados.clone())
        }),
    }
}

/// The template of a component, already parsed: the text of the `.html` (or
/// that of the annotation, with the positions shifted to those of the `.dart`)
/// and the file that feeds it.
fn template_do_componente(
    fonte: &Path,
    comp: &component::Componente,
) -> Result<(Vec<html::No>, Option<PathBuf>), Recusa> {
    let ausente = || recusa(Motivo::TemplateAusente, "templateUrl not found");
    Ok(match (&comp.template, &comp.template_url) {
        (Some(t), _) => {
            let (mut nos, erro) = html::analisar_no_modo(t, comp.preservar_espacos);
            if let Some(e) = erro {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!("invalid template: {e}"),
                ));
            }
            if let Some(k) = comp.deslocamento_do_template {
                html::deslocar(&mut nos, k);
            }
            (nos, None)
        }
        (None, Some(url)) => {
            let caminho = fonte.parent().ok_or_else(ausente)?.join(url);
            let texto = std::fs::read_to_string(&caminho).map_err(|_| ausente())?;
            let (nos, erro) = html::analisar_no_modo(&texto, comp.preservar_espacos);
            if let Some(e) = erro {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!("invalid template: {e}"),
                ));
            }
            (nos, Some(caminho))
        }
        (None, None) => (html::analisar(""), None),
    })
}

/// The fragment of a component in the `.template.dart`, with its `.html`.
#[allow(clippy::too_many_arguments)]
fn trecho_do_componente(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    comp: &component::Componente,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
    imp: &mut view::Importacoes,
) -> Result<(String, Option<PathBuf>), Recusa> {
    if let Some(r) = comp.nao_entendidos.first() {
        return Err(r.clone());
    }
    let (nos, arquivo_html) = template_do_componente(fonte, comp)?;
    let relativo = pacote.relativo(fonte);
    let local = local_do_componente(pacote, fonte, &relativo, nome_do_arquivo, comp, indice);
    let (usadas, fora) = indice.diretivas_de(comp, fonte, resolvedor);
    if let Some(r) = fora.into_iter().next() {
        return Err(r);
    }
    let filhos = filhos_por_tag(&usadas);
    // The `pipes:` list only matters if the template uses a pipe (case b19):
    // its refusal goes along and the view decides.
    let (pipes, fora) = indice.pipes_de(comp, fonte, resolvedor);
    let pipes = match fora.into_iter().next() {
        Some(r) => Err(r),
        None => Ok(pipes),
    };
    let texto = view::trecho_de_componente(
        comp, &local, &nos, resolvedor, nomes, &filhos, &usadas, &pipes, imp,
    )?;
    // The stylesheet is another output ([`gerar_folha`], per `.css`, like the
    // official `StylesheetCompiler`): the template only imports its module, and
    // the contents do not change it. Here we only check that it exists.
    for url in &comp.style_urls {
        // `package:this/x.css` is `lib/x.css`; one from another package is an
        // output of that package's build, which the template only imports by
        // URI.
        let css = match url.strip_prefix("package:").and_then(|r| r.split_once('/')) {
            Some((p, resto)) if p == pacote.nome => pacote.raiz.join("lib").join(resto),
            Some(_) => continue,
            None => fonte
                .parent()
                .ok_or_else(|| recusa(Motivo::Estilos, "stylesheet outside lib/"))?
                .join(url),
        };
        if texto_da_folha(pacote, &css).is_none() {
            return Err(recusa(Motivo::Estilos, "stylesheet not found"));
        }
    }
    Ok((texto, arquivo_html))
}

/// `package:` URI of the template file — what the official compiler writes in
/// the `REF` comment of each binding. Only for components in `lib/` with
/// `templateUrl`; with the template written in the annotation the reference is
/// different.
fn url_do_template(pacote: &Pacote, fonte: &Path, comp: &component::Componente) -> Option<String> {
    // Template in the annotation: the reference is the `.dart` (`asset:`), with
    // its positions — only when the literal allows counting them.
    if comp.template.is_some() {
        comp.deslocamento_do_template?;
        return Some(format!("asset:{}/{}", pacote.nome, pacote.relativo(fonte)));
    }
    let url = comp.template_url.as_ref()?;
    let html = fonte.parent()?.join(url);
    let rel = pacote.relativo(&html);
    let dentro_de_lib = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{dentro_de_lib}", pacote.nome))
}

/// Does a component's stylesheet compile (Sass and shim)? It is the same
/// computation the generator does; the scoreboard uses it so as not to mark as
/// pending what already comes out.
/// The stylesheet text of a component with `ViewEncapsulation.none`, which goes
/// without a shim into the `.css.dart`: the written `.css`, as is —
/// ngcompiler's `compileStylesheet` only removes the `@import`s
/// (`extractStyleUrls`) and escapes the string.
///
/// # Errors
///
/// The refused shape, when the text is not known byte for byte: the stylesheet
/// comes from `sass_builder` (the `.css` does not exist; the formatting of its
/// output is not ours), has `@import` (it becomes another entry in the list) or
/// cannot be read.
/// The text of a `.css` stylesheet, as the official `StylesheetCompiler` reads
/// it: the output of `sass_builder` in this build (the engine passes it in
/// [`Pacote::folhas_geradas`]), the written `.css`, or — outside the engine,
/// without `sass_builder` — the `.scss` next to it compiled here. `None`
/// without any.
pub(crate) fn texto_da_folha(pacote: &Pacote, css: &Path) -> Option<Result<String, view::Motivo>> {
    // `buildStep.readAsString` decodes with `utf8.decode`, which strips the
    // leading BOM (the `compressed` output of dart-sass with a non-ASCII
    // character starts with it).
    let sem_bom = |t: String| match t.strip_prefix('\u{FEFF}') {
        Some(resto) => resto.to_string(),
        None => t,
    };
    if let Some(t) = pacote.folhas_geradas.get(css) {
        return Some(Ok(sem_bom(t.clone())));
    }
    if let Ok(t) = std::fs::read_to_string(css) {
        return Some(Ok(sem_bom(t)));
    }
    let scss = css.with_extension("scss");
    let fonte = std::fs::read_to_string(&scss).ok()?;
    Some(sass::compilar_em(&fonte, scss.parent()).map(sem_bom))
}

/// The outputs of the ngdart `StylesheetCompiler` for a `.css` stylesheet
/// (`compileStylesheet`, `stylesheet_compiler/builder.dart`): the
/// `.css.shim.dart`, with the CSS rewritten for emulated encapsulation, and the
/// `.css.dart`, with it as is. Each resolvable `@import` leaves the text and
/// becomes the module of the imported stylesheet, before it
/// (`extractStyleUrls`, `_compileStyles`); the text goes through the emitter's
/// `escapeSingleQuoteString`. Each output comes with the contents or the reason
/// for not generating it.
pub fn gerar_folha(pacote: &Pacote, css: &Path) -> Vec<(PathBuf, Result<String, Recusa>)> {
    let destino = |sufixo: &str| {
        css.with_file_name(format!(
            "{}{sufixo}",
            css.file_name().unwrap_or_default().to_string_lossy()
        ))
    };
    let texto = match texto_da_folha(pacote, css) {
        Some(Ok(t)) => Ok(t),
        Some(Err(_)) => Err(recusa(
            Motivo::Estilos,
            "Sass the native compiler does not translate",
        )),
        None => Err(recusa(Motivo::Estilos, "stylesheet not found")),
    };
    let asset = format!("asset:{}/{}", pacote.nome, pacote.relativo(css));
    let texto = texto.map(|t| extrair_imports(&asset, &t));
    let saida = |sufixo: &str, conteudo: &dyn Fn(&str) -> Result<String, Recusa>| {
        texto.clone().and_then(|(estilo, urls)| {
            let modulo = format!("{asset}{sufixo}");
            let mut cabeca = String::new();
            let mut itens = Vec::new();
            for (k, url) in urls.iter().enumerate() {
                let importado = format!("{url}{sufixo}");
                let caminho = resolucao::asset_de_uri(&importado, "", Path::new(""))
                    .or_else(|| importado.starts_with("asset:").then(|| importado.clone()))
                    .and_then(|alvo| resolucao::caminho_do_import(&modulo, &alvo))
                    .ok_or_else(|| {
                        recusa(Motivo::Estilos, "stylesheet @import without an import path")
                    })?;
                cabeca.push_str(&format!("import '{caminho}' as import{k};\n"));
                itens.push(format!("import{k}.styles"));
            }
            itens.push(view::literal(&conteudo(&estilo)?));
            Ok(format!("{cabeca}{}", lista_de_estilos(&itens)))
        })
    };
    vec![
        (
            destino(".shim.dart"),
            saida(".shim.dart", &|t| {
                css::shim(t).map_err(|_| {
                    recusa(
                        Motivo::Estilos,
                        "CSS the ngdart shim refuses (the official compiler throws)",
                    )
                })
            }),
        ),
        (destino(".dart"), saida(".dart", &|t| Ok(t.to_string()))),
    ]
}

/// `final List<Object> styles = [..];` as the emitter writes it (the
/// `.css.dart` does not go through `DartFormatter`): with one item, on one
/// line; with more, `[` and the items on the next line, indented two spaces,
/// separated by `,` and wrapping when the line goes past 80 (`visitAllObjects`
/// with `keepOnSameLine`, `currentLineLength` = indentation level +
/// characters), and `]` and `;` on their own lines.
fn lista_de_estilos(itens: &[String]) -> String {
    if let [item] = itens {
        return format!("final List<Object> styles = [{item}];");
    }
    let mut linhas = vec!["final List<Object> styles = [".to_string()];
    let mut linha = String::new();
    for (k, item) in itens.iter().enumerate() {
        linha.push_str(item);
        if k + 1 < itens.len() {
            // The test looks at the line before the separator (in UTF-16 code
            // units, like Dart's `String.length`); the comma goes in anyway.
            let quebra = 1 + linha.encode_utf16().count() > 80;
            linha.push(',');
            if quebra {
                linhas.push(format!("  {linha}"));
                linha.clear();
            }
        }
    }
    linhas.push(format!("  {linha}"));
    linhas.push("]".to_string());
    linhas.push(";".to_string());
    linhas.join("\n")
}

/// `extractStyleUrls`: each `@import` (the official `_cssImportRe`,
/// `@import\s+(?:url\()?\s*(?:(?:['"]([^'"]*))|([^;\)\s]*))[^;]*;?`) with a
/// resolvable URL — relative, `package:` or `asset:` — leaves the text, and the
/// URL resolved against that of the stylesheet goes into the list; one with
/// another scheme or absolute stays.
fn extrair_imports(asset: &str, css: &str) -> (String, Vec<String>) {
    let mut saida = String::with_capacity(css.len());
    let mut urls = Vec::new();
    let b = css.as_bytes();
    let mut i = 0;
    let mut copiado = 0;
    while let Some(k) = css[i..].find("@import") {
        let ini = i + k;
        let mut j = ini + "@import".len();
        // `\s+`
        let espacos = css[j..].len() - css[j..].trim_start().len();
        if espacos == 0 {
            i = j;
            continue;
        }
        j += espacos;
        if css[j..].starts_with("url(") {
            j += 4;
        }
        j += css[j..].len() - css[j..].trim_start().len();
        let url = if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
            j += 1;
            let f = css[j..].find(['\'', '"']).map_or(css.len(), |f| j + f);
            let u = css[j..f].to_string();
            j = f;
            u
        } else {
            let f = css[j..]
                .find(|c: char| c == ';' || c == ')' || c.is_whitespace())
                .map_or(css.len(), |f| j + f);
            let u = css[j..f].to_string();
            j = f;
            u
        };
        // `[^;]*;?`
        let fim = match css[j..].find(';') {
            Some(f) => j + f + 1,
            None => css.len(),
        };
        if resolvivel(&url) {
            urls.push(resolver_url(asset, &url));
            saida.push_str(&css[copiado..ini]);
            copiado = fim;
        }
        i = fim.max(ini + 1);
    }
    saida.push_str(&css[copiado..]);
    (saida, urls)
}

/// `isStyleUrlResolvable`.
fn resolvivel(url: &str) -> bool {
    if url.is_empty() || url.starts_with('/') {
        return false;
    }
    match url.find(':') {
        Some(k) if !url[..k].contains(['/', '?', '#']) && k > 0 => {
            matches!(&url[..k], "package" | "asset")
        }
        _ => true,
    }
}

/// `Uri.parse(base).resolve(url)` for stylesheet URLs: with a scheme, the URL
/// itself; relative, against the base's folder, without `.` and `..`.
fn resolver_url(base: &str, url: &str) -> String {
    if url.starts_with("package:") || url.starts_with("asset:") {
        return url.to_string();
    }
    let (esquema, caminho) = base.split_once(':').unwrap_or(("", base));
    let pasta = caminho.rsplit_once('/').map_or("", |(p, _)| p);
    let mut partes: Vec<&str> = pasta.split('/').filter(|p| !p.is_empty()).collect();
    for p in url.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                partes.pop();
            }
            p => partes.push(p),
        }
    }
    format!("{esquema}:{}", partes.join("/"))
}

/// Set of refusals of a pending file, for the scoreboard: the first one, and
/// all those the emission in collection mode finds.
fn motivos_do_arquivo(
    pacote: &Pacote,
    fonte: &Path,
    achados: &Achados,
    primeira: Recusa,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    indice: &Indice,
    nomes: &mut Interner,
) -> std::collections::BTreeSet<Recusa> {
    let mut fora = std::collections::BTreeSet::new();
    fora.insert(primeira);
    let relativo = pacote.relativo(fonte);
    let nome = fonte
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    for comp in &achados.componentes {
        let Ok((nos, _)) = template_do_componente(fonte, comp) else {
            continue;
        };
        let local = local_do_componente(pacote, fonte, &relativo, &nome, comp, indice);
        let (usadas, fora_da_lista) = indice.diretivas_de(comp, fonte, resolvedor);
        fora.extend(fora_da_lista);
        let filhos = filhos_por_tag(&usadas);
        let (pipes, fora_dos_pipes) = indice.pipes_de(comp, fonte, resolvedor);
        let pipes = match fora_dos_pipes.into_iter().next() {
            Some(r) => Err(r),
            None => Ok(pipes),
        };
        fora.extend(view::coletar(
            comp, &local, &nos, resolvedor, nomes, &filhos, &usadas, &pipes,
        ));
    }
    fora
}

/// Generates for a whole package (`lib/`, `web/`, `test/`), completing what it
/// cannot generate yet with a fallback generation — today, what `build_runner`
/// left on disk.
///
/// It is this mix that lets the migration happen in steps: with each new shape
/// the generator learns, a file leaves the fallback and enters ours, and the
/// application keeps compiling the whole time.
pub fn gerar_com_apoio(
    pacote: &Pacote,
    interner: &mut Interner,
    apoio: Option<&Geracao>,
    programa: Option<&resolucao::Resolvedor>,
) -> (Arc<Geracao>, Placar) {
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> = ["lib", "web", "test"]
        .iter()
        .map(|d| pacote.raiz.join(d))
        .filter(|d| d.is_dir())
        .collect();
    gerar_em(pacote, &dirs, interner, &mut c, &mut placar, programa);
    if let Some(apoio) = apoio {
        // The fallback only enters where we do not generate: pending files of
        // this package and everything outside it.
        for (caminho, f) in apoio.iter() {
            if c.contem(caminho) {
                continue;
            }
            c.por(
                caminho.clone(),
                f.conteudo.to_string(),
                f.gerador,
                f.entradas.to_vec(),
            );
        }
    }
    let g = c.concluir(1).unwrap_or_else(|erros| {
        for m in erros {
            eprintln!("erro do gerador ngdart: {m}");
        }
        Arc::new(Geracao::default())
    });
    (g, placar)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Each `.css` gives the `.css.shim.dart` and the `.css.dart` (the text as
    /// is); the stylesheet from the `sass_builder` output wins over the disk,
    /// and `@import` is refused in both.
    #[test]
    fn gerar_folha_da_as_duas_saidas() {
        let dir = tempfile::tempdir().expect("temporary dir");
        let a = dir.path().join("a.css");
        std::fs::write(&a, ".a { content: '\\e939'; }\n").unwrap();
        let b = dir.path().join("b.css");
        std::fs::write(&b, "@import 'c.css';\n").unwrap();
        let mut pacote = Pacote::default();
        let saidas = gerar_folha(&pacote, &a);
        assert_eq!(saidas[0].0, dir.path().join("a.css.shim.dart"));
        assert_eq!(saidas[1].0, dir.path().join("a.css.dart"));
        assert_eq!(
            saidas[1].1.as_deref(),
            Ok("final List<Object> styles = ['.a { content: \\'\\\\e939\\'; }\\n'];")
        );
        assert!(gerar_folha(&pacote, &b).iter().all(|(_, r)| r.is_err()));
        pacote.folhas_geradas.insert(a.clone(), ".x{}".to_string());
        assert_eq!(
            gerar_folha(&pacote, &a)[1].1.as_deref(),
            Ok("final List<Object> styles = ['.x{}'];")
        );
        let z = dir.path().join("z.css");
        assert!(gerar_folha(&pacote, &z).iter().all(|(_, r)| r.is_err()));
    }

    fn achados_de(fonte: &str) -> Achados {
        let mut i = Interner::new();
        let t = dartforge_frontend::lexer::lex(fonte);
        let u = dartforge_frontend::parser::parse_lexed(fonte, t, &mut i);
        achar(&u.ast, &u.unit, fonte, &i)
    }

    #[test]
    fn biblioteca_sem_angular_e_trivial() {
        assert!(achados_de("class A {}\nint x = 1;\n").trivial());
    }

    #[test]
    fn acha_componente_com_e_sem_prefixo() {
        let a = achados_de("@Component(selector: 'x')\nclass XComp {}\n");
        assert_eq!(a.componentes[0].classe, "XComp");
        assert_eq!(a.componentes[0].seletor, "x");
        let b = achados_de("@ng.Component(selector: 'x')\nclass YComp {}\n");
        assert_eq!(b.componentes[0].classe, "YComp");
    }

    #[test]
    fn acha_diretiva_e_pipe() {
        let a = achados_de("@Directive(selector: '[d]')\nclass D {}\n@Pipe('p')\nclass P {}\n");
        assert_eq!(a.diretivas.len(), 1);
        assert_eq!(a.diretivas[0].classe, "D");
        assert_eq!(a.diretivas[0].seletor, "[d]");
        assert_eq!(a.pipes.len(), 1);
        assert_eq!(a.pipes[0].classe, "P");
        assert_eq!(a.pipes[0].nome, "p");
        assert!(a.pipes[0].puro);
        assert!(!a.trivial());
    }

    /// Bytes exatos do compilador oficial, conferidos em
    /// `loading.template.dart` do new_sali/frontend.
    #[test]
    fn trivial_byte_a_byte() {
        let esperado = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\nimport 'loading.dart';\n";
        assert_eq!(template_trivial("loading.dart"), esperado);
    }

    /// new_sali sets up injection like this; if this slips, we generate an
    /// empty file in place of a whole injector.
    #[test]
    fn acha_injetor_em_variavel_de_topo() {
        let a = achados_de(
            "@GenerateInjector([])
final InjectorFactory injector = self.injector$Injector;
",
        );
        assert_eq!(a.injetores.len(), 1);
        assert!(!a.trivial());
    }

    /// The build engine index: a file goes in, is found by its selector, is
    /// replaced and goes out, without touching the others.
    #[test]
    fn indice_incremental() {
        let pacote = Pacote {
            folhas_geradas: Default::default(),
            nome: "p".into(),
            raiz: PathBuf::from("/r"),
        };
        let a = Path::new("/r/lib/a.dart");
        let b = Path::new("/r/lib/src/b.dart");
        let mut indice = Indice::novo();
        indice.atualizar(
            &pacote,
            a,
            &achados_de("@Component(selector: 'x-a', template: '')\nclass A {}\n"),
            None,
        );
        indice.atualizar(
            &pacote,
            b,
            &achados_de("@Directive(selector: '[b]')\nclass B {}\n"),
            None,
        );
        assert_eq!(
            indice.declarante("x-a"),
            Some(("package:p/a.dart".into(), "A".into()))
        );
        assert_eq!(
            indice.declarante("[b]"),
            Some(("package:p/src/b.dart".into(), "B".into()))
        );
        // O arquivo mudou de seletor: o antigo some.
        indice.atualizar(
            &pacote,
            a,
            &achados_de("@Component(selector: 'x-novo', template: '')\nclass A {}\n"),
            None,
        );
        assert_eq!(indice.declarante("x-a"), None);
        assert!(indice.declarante("x-novo").is_some());
        indice.remover(a);
        assert_eq!(indice.declarante("x-novo"), None);
        assert!(indice.declarante("[b]").is_some());
    }

    #[test]
    fn caminho_ao_lado_da_fonte() {
        assert_eq!(
            caminho_do_template(Path::new("/p/lib/a/foo.dart")),
            PathBuf::from("/p/lib/a/foo.template.dart")
        );
    }
}

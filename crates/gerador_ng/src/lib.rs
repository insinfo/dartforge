//! Gerador do ngdart: produz os `.template.dart` que o `build_runner` produz,
//! sem `build_runner`.
//!
//! O compilador do ngdart transforma componente + template HTML em Dart
//! imperativo (`ViewXComp0.build()`, fábricas, injeção). O `build_runner` só
//! orquestra isso — e é ele, com o `build_web_compilers`, que custa o minuto e
//! meio de compilação do new_sali. Gerando aqui, o ciclo inteiro fica nosso.
//!
//! Regra: **não mexer no ngdart nem na aplicação**. O que importa não é sair
//! texto idêntico, e sim a mesma ABI — os mesmos símbolos públicos
//! (`XNgFactory`, `createXFactory`, `ViewX0`) com a mesma semântica. Onde sair
//! igual ao oficial, melhor ainda: dá para comparar byte a byte, e os 284
//! arquivos que o build_runner já escreveu no new_sali servem de oráculo, como
//! o `dartdevc` serve para o emissor.
//!
//! O avanço é gradual e medido: o que ainda não sabemos gerar fica de fora e
//! continua vindo do `build_runner`. A aplicação funciona em todos os passos e
//! o placar diz exatamente onde estamos.
pub mod componente;
pub mod css;
pub mod csslib;
pub mod diretivas;
pub mod dom;
mod entidades;
pub mod expr;
pub mod html;
pub mod incremental;
mod injetor;
pub mod metadados;
pub mod micro;
pub mod resolucao;
pub mod sass;
pub mod seletor;
pub mod shadow_css;
pub mod visao;

use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
pub use incremental::{ConsultaNg, SaidaArquivo, analisar_arquivo, gerar_arquivo};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use visao::{Motivo, Recusa, recusa};

/// Cabeçalho que o compilador oficial escreve em todo arquivo gerado.
pub const CABECALHO: &str = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\n";

/// O que uma biblioteca tem de interessante para o gerador.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Achados {
    /// `@Component` lidos por inteiro — é deles que sai a visão.
    pub componentes: Vec<componente::Componente>,
    /// `@Directive`, lidas como o oficial as vê de quem as usa (seletor,
    /// entradas, saídas, construtor).
    pub diretivas: Vec<componente::Componente>,
    /// `@Pipe`, com o que quem o usa precisa saber dele.
    pub pipes: Vec<componente::Pipe>,
    /// As variáveis de topo com `@GenerateInjector`, na ordem do fonte.
    pub injetores: Vec<String>,
    /// `@Directive` com `@HostBinding`: cada uma ganha um
    /// `DirectiveChangeDetector` no arquivo gerado.
    pub hospedeiras: Vec<Hospedeira>,
    /// `@Directive` com `@HostBinding`/`@HostListener` que herda de alguém
    /// (`extends`, `with`): o oficial coleta também os membros herdados, que
    /// daqui não se veem.
    pub hospedeiro_herdado: bool,
}

/// Uma `@Directive` com `@HostBinding`: o oficial gera para ela a classe
/// `XNgCd` (`requiresDirectiveChangeDetector`, em `compile_metadata.dart`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hospedeira {
    pub classe: String,
    /// `(nome da ligação, membro)` de cada `@HostBinding` — `class.x`,
    /// `attr.x`, propriedade ou, sem argumento, o nome do membro —, na ordem
    /// em que o oficial os coleta (`DirectiveVisitor`, em
    /// `angular_compiler/analyzer/view/directive.dart`): acessores, depois
    /// métodos, depois campos — cada grupo em ordem de declaração.
    pub ligacoes: Vec<(String, String)>,
    /// `style.x`: o tipo do membro como escrito (`String`, `int?`…), que
    /// decide entre `toString()`, `?.toString()` e o valor direto
    /// (`visitStyleBinding`).
    pub tipos_de_estilo: std::collections::HashMap<String, String>,
    /// Alguma ligação fora do que sabemos traduzir: o motivo.
    pub recusada: Option<String>,
    /// Os membros `final` (`isImmutable`): escritos uma vez, no
    /// `if (firstCheck)`, sem campo `_expr_N`.
    pub imutaveis: std::collections::HashSet<String>,
}

/// Lê os `@HostBinding` de uma classe `@Directive`; `None` se não há.
fn hospedeira(
    arvore: &ast::Ast,
    fonte: &str,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Option<Hospedeira> {
    let membros = componente::tipos_dos_membros(arvore, fonte, interner, classe);
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
            // Sem argumento (`Some(None)`), o nome da ligação é o do próprio
            // membro (`bindingName ?? memberName`).
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
                    "@HostBinding com nome que não é texto".into(),
                    &mut recusada,
                );
                continue;
            };
            // `class.x`, `attr.x`, propriedade e `style.x`
            // ([`visao::forma_do_hospedeiro`]); o `style.x` pelo tipo escrito
            // do campo (não `final`: o imutável seria escrito uma vez), como
            // no componente.
            let aceita = |n: &str, membro: &str, campo: bool| match visao::forma_do_hospedeiro(n) {
                Ok(visao::FormaDoHospedeiro::Estilo { .. }) => {
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
                // Campo `final` é imutável e é escrito uma vez, na primeira
                // checagem (`isImmutable`); estático lê pela classe, ainda
                // não.
                ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                    let membro = interner.resolve(l.variables[0].name.sym).to_string();
                    let nome = nome.unwrap_or_else(|| membro.clone());
                    let imutavel = l.final_ || l.const_;
                    if imutavel {
                        imutaveis.insert(membro.clone());
                    }
                    if !aceita(&nome, &membro, !imutavel) {
                        recusar(
                            format!("@HostBinding('{nome}') fora das formas"),
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
                                    format!("@HostBinding('{nome}') fora das formas"),
                                    &mut recusada,
                                );
                            }
                            acessores.push((nome, membro));
                        }
                        _ => recusar(
                            "@HostBinding em método, setter ou getter estático".into(),
                            &mut recusada,
                        ),
                    }
                }
                ast::MemberKind::Field(l) if l.static_ => {
                    recusar("@HostBinding em campo estático".into(), &mut recusada)
                }
                _ => recusar("@HostBinding em membro fora da forma".into(), &mut recusada),
            }
        }
    }
    if !alguma {
        return None;
    }
    acessores.extend(campos);
    // O mapa do oficial é por nome de ligação: repetir o nome sobrescreve o
    // valor e mantém a posição do primeiro.
    let mut vistos = std::collections::HashSet::new();
    if !acessores.iter().all(|(c, _)| vistos.insert(c.clone())) {
        recusar("@HostBinding com nome repetido".into(), &mut recusada);
    }
    // Tipo genérico muda a declaração da classe (`XNgCd<T>`); ainda não.
    if !classe.type_params.is_empty() {
        recusar("@HostBinding em diretiva genérica".into(), &mut recusada);
    }
    Some(Hospedeira {
        classe: interner.resolve(classe.name.sym).to_string(),
        ligacoes: acessores,
        tipos_de_estilo,
        recusada,
        imutaveis,
    })
}

impl Achados {
    /// Sem nada disto o arquivo gerado é só o cabeçalho e o `import` de si
    /// mesmo — 120 dos 284 arquivos do new_sali/frontend.
    pub fn trivial(&self) -> bool {
        self.componentes.is_empty()
            && self.diretivas.is_empty()
            && self.pipes.is_empty()
            && self.injetores.is_empty()
    }
}

/// Nome efetivo de uma anotação: `@Component`, `@ng.Component` e
/// `@Component.new` contam todos como `Component`. O prefixo de biblioteca vem
/// em minúscula por convenção e a classe em maiúscula; é o que distingue
/// `p.Component` de `Component.new`.
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

/// Varre as declarações de topo de uma unidade já analisada.
pub fn achar(
    arvore: &ast::Ast,
    unidade: &ast::CompilationUnit,
    fonte: &str,
    interner: &Interner,
) -> Achados {
    let mut achados = Achados::default();
    for &id in &unidade.declarations {
        let decl = arvore.decl(id);
        // `@Component` e `@Directive` só existem em classe, mas
        // `@GenerateInjector` fica numa variável de topo
        // (`@GenerateInjector([...]) final InjectorFactory injector = …`), que é
        // como o new_sali monta a injeção. Por isso a varredura não filtra por
        // tipo de declaração.
        let alvo = match &decl.kind {
            ast::DeclKind::Class(c) => interner.resolve(c.name.sym).to_string(),
            _ => String::new(),
        };
        for a in decl.metadata.iter() {
            match nome_da_anotacao(a, interner).as_str() {
                "Component" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        achados.componentes.push(componente::ler_componente(
                            arvore, fonte, interner, classe, a,
                        ));
                    }
                }
                "Directive" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        achados
                            .diretivas
                            .push(componente::ler_diretiva(arvore, fonte, interner, classe, a));
                    }
                    // Só `@HostBinding` muda o arquivo da diretiva; o
                    // `@HostListener` vai para quem a usa. Mas o oficial
                    // coleta os dois também nas superclasses.
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
                            .push(componente::ler_pipe(arvore, fonte, interner, classe, a));
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

/// Arquivo gerado de uma biblioteca sem nada de Angular.
pub fn template_trivial(nome_do_arquivo: &str) -> String {
    format!("{CABECALHO}import '{nome_do_arquivo}';\n")
}

/// Onde estamos: quantos arquivos o gerador já cobre e quais faltam.
#[derive(Debug, Default)]
pub struct Placar {
    /// Arquivos `.dart` examinados.
    pub examinados: usize,
    /// Gerados por nós.
    pub gerados: usize,
    /// Deixados para quem souber gerar (hoje, o `build_runner`).
    pub pendentes: Vec<PathBuf>,
    /// Quantos pendentes por motivo da primeira recusa.
    pub motivos: std::collections::BTreeMap<Motivo, usize>,
    /// Conjunto completo de recusas (motivo e sub-forma) de cada pendente,
    /// coletado pela mesma emissão que gera. É por ele que se sabe quantos
    /// arquivos uma forma nova destrava de verdade.
    pub conjuntos: Vec<std::collections::BTreeSet<Recusa>>,
    /// Quantas vezes cada forma não entendida aparece (`@HostBinding`,
    /// `providers: [..]`…), contando todas as de cada componente pendente.
    pub nao_entendidos: std::collections::BTreeMap<String, usize>,
}

impl Placar {
    pub fn resumo(&self) -> String {
        format!(
            "ngdart: {}/{} gerados por nós",
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

/// O pacote sendo gerado: o nome vale para as URIs `asset:` que o ngdart usa
/// nas mensagens de modo de desenvolvimento.
#[derive(Default)]
pub struct Pacote {
    pub nome: String,
    pub raiz: PathBuf,
    /// As folhas `.css` que o `sass_builder` gerou nesta build (o motor as
    /// lê da saída dele), pelo caminho: é delas que o shim do ngdart parte,
    /// como no oficial — no estilo (`outputStyle`) que o projeto pediu. Sem
    /// ela, o shim compila o `.scss` ao lado por conta própria.
    pub folhas_geradas: std::collections::HashMap<PathBuf, String>,
}

impl Pacote {
    /// Caminho de um arquivo dentro do pacote, com barras: `lib/src/x/f.dart`.
    fn relativo(&self, arquivo: &Path) -> String {
        arquivo
            .strip_prefix(&self.raiz)
            .unwrap_or(arquivo)
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }
}

/// Gera o que sabe gerar para os `.dart` de `diretorios`, acumulando em `c`.
pub fn gerar_em(
    pacote: &Pacote,
    diretorios: &[PathBuf],
    interner: &mut Interner,
    c: &mut Construtor,
    placar: &mut Placar,
    programa: Option<&resolucao::Resolvedor>,
) {
    let resolvedor = programa.map(|r| r as &dyn resolucao::Resolucao);
    // Duas passadas: a primeira lê e analisa tudo, a segunda gera. É a
    // primeira que monta o índice de componentes do pacote — sem ele não dá
    // para saber que `<a02-texto-estatico>` é um componente e qual classe o
    // implementa.
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
                // Uma parte (`part of`) não é biblioteca: o oficial não gera
                // nada para ela (`resolver.isLibrary`), e o que ela declara
                // entra no `.template.dart` da biblioteca dona.
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
                    // Forma que o gerador ainda não cobre: fica com o
                    // build_runner, e a aplicação compila do mesmo jeito.
                    *placar.motivos.entry(primeira.motivo).or_default() += 1;
                    // Toda forma presente conta, não só a que recusou.
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

    // As folhas, uma a uma, como o `StylesheetCompiler`: todo `.css` dos
    // diretórios — o escrito, o que o `sass_builder` gerou nesta build e,
    // fora do motor, o de cada `.scss` que não é parcial.
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

/// Os `.css` de que o ngdart gera folhas, em ordem: os do disco, os que o
/// `sass_builder` gerou nesta build e os dos `.scss` não parciais sem `.css`.
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

/// A unidade é uma parte (`part of`), não uma biblioteca.
pub fn e_parte(unit: &dartforge_frontend::ast::CompilationUnit) -> bool {
    unit.directives.iter().any(|d| {
        matches!(
            d.kind,
            dartforge_frontend::ast::DirectiveKind::PartOf { .. }
        )
    })
}

/// Os componentes e diretivas do pacote, por biblioteca e classe.
///
/// O oficial pergunta isso ao grafo de assets do `build`; aqui vem da
/// primeira passada. É o que permite casar `<a02-texto-estatico>` com a
/// classe que declara esse seletor e emitir a visão-filha — e saber que um
/// `<form>` recebe `NgForm`.
///
/// É incremental: [`Indice::atualizar`] e [`Indice::remover`] trocam o que
/// um arquivo pôs, sem reler o resto — o motor de build o mantém vivo na
/// sessão.
#[derive(Default)]
pub struct Indice {
    por_classe: std::collections::HashMap<(String, String), visao::Filho>,
    /// `@Directive` por (URI da biblioteca, classe): o seletor e o que o
    /// emissor precisa para instanciá-la.
    diretivas: std::collections::HashMap<(String, String), componente::Componente>,
    /// `@Pipe` por (URI da biblioteca, classe).
    pipes: std::collections::HashMap<(String, String), componente::Pipe>,
    /// Os metadados de cada `@Directive`, lidos do programa
    /// (`metadados.rs`): é por eles que o emissor instancia a diretiva num
    /// nó. Sem programa, vazio — e toda diretiva casada é recusada.
    metadados: std::collections::HashMap<(String, String), std::sync::Arc<diretivas::Diretiva>>,
    /// Os `@GenerateInjector` por (URI da biblioteca, variável), lidos do
    /// programa (`metadados::ler_injetores`), ou o motivo de não se lerem.
    injetores: std::collections::HashMap<
        (String, String),
        Result<std::sync::Arc<metadados::Injetor>, String>,
    >,
    /// As chaves que cada arquivo pôs, para `remover`/`atualizar`.
    por_arquivo: std::collections::HashMap<PathBuf, Vec<(String, String)>>,
}

impl Indice {
    /// Um índice vazio.
    pub fn novo() -> Indice {
        Indice::default()
    }

    fn montar(
        pacote: &Pacote,
        arquivos: &[(PathBuf, String, Achados)],
        programa: Option<&resolucao::Resolvedor>,
    ) -> Self {
        // Com o programa carregado, o índice cobre todos os pacotes: um
        // `<li-select>` do limitless_ui é tão componente quanto um do próprio
        // projeto. Sem ele, só o que a varredura de arquivos viu.
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

    /// O índice de todas as bibliotecas carregadas no programa (as
    /// dependências inclusive), cada uma registrada pelo seu arquivo: depois,
    /// [`Indice::atualizar`] troca só o que muda.
    pub fn do_programa(r: &resolucao::Resolvedor) -> Indice {
        let mut indice = Indice::novo();
        let interner = r.interner();
        for (uri, arvore, unidade, fonte, caminho) in r.bibliotecas() {
            let achados = achar(arvore, unidade, fonte, interner);
            indice.juntar(uri, caminho, &achados, Some(r), true);
        }
        indice
    }

    /// Os achados de `arquivo` entram no índice, no lugar dos anteriores. Com
    /// o programa, os metadados das diretivas também são relidos dele (e a
    /// injeção dos componentes resolvida); sem, os componentes ficam com a
    /// injeção sem resolução e as diretivas sem metadados — o que recusa quem
    /// os usa.
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

    /// Tira do índice tudo o que `arquivo` tinha posto.
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

    /// Os componentes que `comp` pode usar no template, na ordem de
    /// `directives:` expandida (as diretivas ficam de fora).
    pub fn filhos_de(
        &self,
        comp: &componente::Componente,
        fonte: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> Vec<visao::Filho> {
        self.diretivas_de(comp, fonte, resolvedor)
            .0
            .into_iter()
            .filter_map(|u| u.filho)
            .collect()
    }

    /// Quem declara exatamente este seletor: (biblioteca, classe). Com mais
    /// de um, o de menor chave — a resposta não depende da ordem do mapa.
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

    /// Põe os achados de uma biblioteca. `substitui`: diretivas e pipes
    /// trocam o que houver (senão, o que já está fica — a passada de arquivos
    /// não sobrescreve o que o programa leu).
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
            // Os metadados do componente: com eles, os `providers:` dele
            // (`ExistingProvider`, como os das diretivas) entram no nó de
            // quem o usa, e o "filho com providers" deixa de ser recusa.
            if let Some(r) = programa
                && let Some(m) = r
                    .classe_por_uri(uri, &comp.classe)
                    .and_then(|id| crate::metadados::ler(r, id))
                && let Some(f) = self.por_classe.get_mut(&k)
            {
                // O nó de template escreve os `providers:` do filho como a
                // hospedeira; o que ainda não sai (dependência de fora do
                // nó, provedor pedido no próprio nó) é recusado pela visão,
                // com o motivo.
                if crate::visao::provedores_escreviveis(&m).is_ok() {
                    f.pendencias.retain(|p| p.forma != "filho com providers");
                }
                // Os parâmetros do construtor pelos metadados (`@Inject`,
                // `@Attribute`, tokens opacos): trocam os lidos do texto e a
                // recusa que eles deram.
                if let Ok(ps) = parametros_dos_metadados(comp, &m) {
                    let recusa_do_texto = comp.parametros.iter().find_map(|p| {
                        injetado(p, caminho, Some(r as &dyn resolucao::Resolucao)).err()
                    });
                    if let Some(forma) = recusa_do_texto {
                        f.pendencias.retain(|p| p.forma != forma);
                    }
                    f.parametros = ps;
                }
                // O filho que herda: `@Input`, `@Output`, `@HostBinding` e
                // ganchos pelos metadados, que sobem os supertipos como o
                // `_collectInheritableMetadata` do oficial. A consulta de
                // conteúdo herdada ainda não se escreve.
                if comp.herda && m.fora.is_empty() {
                    f.entradas = m
                        .entradas
                        .iter()
                        .map(|e| componente::Entrada {
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
                            .retain(|p| p.forma != "filho que herda (extends/with)");
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
                    .and_then(|id| crate::metadados::ler(r, id))
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
            for (nome, lido) in crate::metadados::ler_injetores(r, uri) {
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
    /// Os pipes que este componente usa, na ordem de `pipes:` expandida em
    /// profundidade (as listas constantes, como `commonPipes`, pelo banco
    /// semântico). O oficial procura o nome do fim para o começo
    /// (`_findPipeMeta`), então a ordem importa. Classe que não é pipe fica
    /// de fora; nome que não se resolve volta como recusa.
    fn pipes_de(
        &self,
        comp: &componente::Componente,
        arquivo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> (Vec<visao::PipeUsado>, Vec<Recusa>) {
        let mut saida = Vec::new();
        let mut fora = Vec::new();
        if comp.pipes_ilegiveis {
            fora.push(recusa(
                Motivo::PipesUsados,
                "pipes: com item que não é nome",
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
                fora.push(recusa(Motivo::PipesUsados, "pipes: lista circular"));
                break;
            }
            let simples = nome.rsplit('.').next().unwrap_or(&nome).to_string();
            match resolvedor.and_then(|r| r.designado(&escopo, &nome)) {
                Some(resolucao::Designado::Classe { uri }) => {
                    if let Some(p) = self.pipes.get(&(uri.clone(), simples)) {
                        saida.push(visao::PipeUsado {
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
                None => fora.push(recusa(Motivo::PipesUsados, "pipes: nome não resolvido")),
            }
        }
        (saida, fora)
    }

    /// As diretivas e componentes que este componente usa, na ordem do
    /// oficial: `directives:` expandida em profundidade (as listas
    /// constantes pelo banco semântico), sem repetição (`removeDuplicates`).
    /// Classe que não é diretiva nem componente é ignorada, como no oficial
    /// (`typeDeclarationOf(value)?.accept(...)` devolve `null`). O que não
    /// se consegue resolver volta como recusa: sem a lista inteira não se
    /// sabe o que casa com cada elemento.
    fn diretivas_de(
        &self,
        comp: &componente::Componente,
        arquivo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> (Vec<visao::Usada>, Vec<Recusa>) {
        let mut saida = Vec::new();
        let mut vistos = std::collections::HashSet::new();
        let mut fora = Vec::new();
        if comp.diretivas_ilegiveis {
            fora.push(recusa(
                Motivo::DiretivaPorSeletor,
                "directives: com item que não é nome",
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
        saida: &mut Vec<visao::Usada>,
        vistos: &mut std::collections::HashSet<(String, String)>,
        fora: &mut Vec<Recusa>,
        profundidade: u32,
    ) {
        if profundidade > 32 {
            fora.push(recusa(
                Motivo::DiretivaPorSeletor,
                "directives: lista circular",
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
                    saida.push(visao::Usada {
                        classe: simples,
                        uri,
                        seletores: seletor::Seletor::analisar(&f.seletor),
                        filho: Some(f.clone()),
                        diretiva: None,
                    });
                } else if let Some(d) = self.diretivas.get(&chave) {
                    saida.push(visao::Usada {
                        classe: simples,
                        uri,
                        seletores: seletor::Seletor::analisar(&d.seletor),
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
                // Sem resposta do banco semântico: só um componente de nome
                // único no pacote (o caminho de antes, sem programa).
                let por_nome: Vec<&visao::Filho> = self
                    .por_classe
                    .values()
                    .filter(|f| f.classe == simples)
                    .collect();
                match por_nome.as_slice() {
                    [f] if !nome.contains('.') => {
                        let chave = (f.uri_dart.clone(), f.classe.clone());
                        if vistos.insert(chave) {
                            saida.push(visao::Usada {
                                classe: f.classe.clone(),
                                uri: f.uri_dart.clone(),
                                seletores: seletor::Seletor::analisar(&f.seletor),
                                filho: Some((*f).clone()),
                                diretiva: None,
                            });
                        }
                    }
                    _ => fora.push(recusa(
                        Motivo::DiretivaPorSeletor,
                        "directives: nome não resolvido",
                    )),
                }
            }
        }
    }
}

/// Os componentes que o emissor casa pela tag: os de seletor só com nome de
/// elemento. O resto da lista só serve à guarda de diretivas.
fn filhos_por_tag(usadas: &[visao::Usada]) -> std::collections::HashMap<String, visao::Filho> {
    let mut saida = std::collections::HashMap::new();
    // Seletor composto (`page-header-comp,pg-header`): cada alternativa que
    // é só a tag casa o elemento com essa tag. As outras alternativas ficam
    // com a guarda de diretivas.
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

/// Os parâmetros do construtor de um filho pelos metadados lidos do
/// programa (`_getCompileDiDependencyMetadata`: os nomeados ficam de fora;
/// `@Inject(token)` e `@Attribute('nome')` resolvidos), como o oficial os
/// resolve no nó dele. Um tipo genérico sem `@Inject` e os embutidos do
/// elemento anotados ainda não se traduzem.
fn parametros_dos_metadados(
    comp: &componente::Componente,
    meta: &diretivas::Diretiva,
) -> Result<Vec<visao::Injetado>, &'static str> {
    use diretivas::Token;
    if !meta.fora.is_empty() {
        return Err("metadados do filho incompletos");
    }
    let textuais: Vec<&componente::Parametro> =
        comp.parametros.iter().filter(|p| !p.nomeado).collect();
    if textuais.len() != meta.dependencias.len() {
        return Err("parâmetros do filho sem os metadados");
    }
    let mut saida = Vec::new();
    for (p, d) in textuais.iter().zip(&meta.dependencias) {
        if !p.outra_anotacao && p.tipo.as_deref().is_some_and(|t| t.contains('<')) {
            return Err("token genérico no construtor do filho");
        }
        if let Some(nome) = &d.atributo {
            saida.push(visao::Injetado::Atributo(nome.clone()));
            continue;
        }
        let anotado = d.opcional || d.proprio || d.hospedeiro || d.pular;
        let embutido = match &d.token {
            Token::Elemento | Token::Detector => true,
            Token::Classe { uri, .. } => uri.starts_with("package:ngdart/"),
            _ => false,
        };
        if embutido && anotado {
            return Err("embutido do elemento anotado no construtor do filho");
        }
        saida.push(match &d.token {
            Token::Elemento => visao::Injetado::Elemento,
            Token::Detector => visao::Injetado::Detector,
            Token::Classe { uri, classe } if uri.starts_with("package:ngdart/") => {
                match classe.as_str() {
                    "ViewContainerRef" => visao::Injetado::Container,
                    _ => return Err("token do ngdart no construtor do filho"),
                }
            }
            Token::Classe { uri, .. } if uri.starts_with("dart:") => {
                return Err("tipo do SDK no construtor do filho");
            }
            t => visao::Injetado::Servico {
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

/// Como o oficial resolve um parâmetro do construtor de um filho no nó dele:
/// o nó, a visão do filho ou um serviço de fora da visão. O resto (outro
/// token do ngdart, `@Inject`, `@Self`, genérico, nomeado) ainda não.
fn injetado(
    p: &componente::Parametro,
    caminho: Option<&Path>,
    resolvedor: Option<&dyn resolucao::Resolucao>,
) -> Result<visao::Injetado, &'static str> {
    let tipo = p
        .tipo
        .as_deref()
        .ok_or("parâmetro sem tipo no construtor do filho")?;
    if p.nomeado {
        return Err("parâmetro nomeado no construtor do filho");
    }
    if p.outra_anotacao {
        return Err("@Inject/@Attribute no construtor do filho");
    }
    if tipo.contains('<') {
        return Err("token genérico no construtor do filho");
    }
    let tipo = tipo.trim_end_matches('?');
    let uri = match (caminho, resolvedor) {
        (Some(c), Some(r)) => r.uri_do_tipo(c, tipo),
        _ => None,
    }
    .ok_or("tipo injetado no filho sem resolução")?;
    let simples = tipo.rsplit('.').next().unwrap_or(tipo).to_string();
    // Os embutidos do elemento (o nó, a visão, o `ViewContainer`) com
    // `@Self`/`@Host`/`@SkipSelf`/`@Optional`: ainda sem caso.
    let embutido = (uri == "dart:html" && matches!(simples.as_str(), "Element" | "HtmlElement"))
        || uri.starts_with("package:ngdart/");
    if embutido && p.anotado {
        return Err("embutido do elemento anotado no construtor do filho");
    }
    if uri == "dart:html" && matches!(simples.as_str(), "Element" | "HtmlElement") {
        return Ok(visao::Injetado::Elemento);
    }
    if uri.starts_with("package:ngdart/") {
        return match simples.as_str() {
            "ChangeDetectorRef" if !p.opcional => Ok(visao::Injetado::Detector),
            // O nó ganha um `ViewContainer` (`requiresViewContainer`), que o
            // filho recebe (caso j47).
            "ViewContainerRef" => Ok(visao::Injetado::Container),
            _ => Err("token do ngdart no construtor do filho"),
        };
    }
    if uri.starts_with("dart:") {
        return Err("tipo do SDK no construtor do filho");
    }
    Ok(visao::Injetado::Servico {
        token: diretivas::Token::Classe {
            uri,
            classe: simples,
        },
        opcional: p.opcional,
        proprio: p.proprio,
        hospedeiro: p.hospedeiro,
        pular: p.pular,
    })
}

/// Põe um componente no índice, com o que o emissor precisa dele.
fn indexar(
    por_classe: &mut std::collections::HashMap<(String, String), visao::Filho>,
    uri: &str,
    comp: &componente::Componente,
    caminho: Option<&Path>,
    resolvedor: Option<&dyn resolucao::Resolucao>,
) {
    if comp.seletor.is_empty() {
        return;
    }
    // Se o filho projeta conteúdo, quem o usa chama `createAndProject`.
    let nos = match (&comp.template, &comp.template_url) {
        (Some(t), _) => Some(html::analisar(t)),
        (None, Some(u)) => caminho
            .and_then(|c| c.parent().map(|d| d.join(u)))
            .and_then(|c| std::fs::read_to_string(c).ok())
            .map(|t| html::analisar(&t)),
        (None, None) => None,
    };
    // `ngContentSelectors`: o `select` de cada `<ng-content>`, `*` sem ele.
    let projecoes: Vec<String> = nos
        .as_deref()
        .map(html::projecoes)
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.unwrap_or_else(|| "*".to_string()))
        .collect();
    // O que no filho muda o código de quem o usa e ainda não é escrito.
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
    // As consultas de conteúdo, com o alvo resolvido no arquivo do filho.
    // Tipo do ngdart (`TemplateRef`, diretivas do núcleo) casaria com o que
    // o emissor põe no conteúdo (`*ngIf`): ainda não.
    let mut consultas = Vec::new();
    match &comp.consultas_de_conteudo {
        None => pendencias.push(em_filho("filho com @ContentChild fora da forma")),
        Some(lidas) => {
            for q in lidas {
                // `read:` troca o valor lido: o nó (`HtmlElement`/`Element`)
                // ou outro provedor do nó, de classe do pacote. O resto
                // (`ViewContainerRef`, `TemplateRef`, tokens) ainda não.
                let leitura = match &q.leitura {
                    None => None,
                    Some(nome) => {
                        let uri = match (caminho, resolvedor) {
                            (Some(c), Some(r)) => r.uri_do_tipo(c, nome),
                            _ => None,
                        };
                        let simples = nome.rsplit('.').next().unwrap_or(nome);
                        match uri {
                            Some(u)
                                if u == "dart:html"
                                    && matches!(simples, "HtmlElement" | "Element") =>
                            {
                                Some(visao::LeituraDaConsulta::Elemento)
                            }
                            Some(u)
                                if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") =>
                            {
                                Some(visao::LeituraDaConsulta::Classe(u, simples.to_string()))
                            }
                            _ => {
                                pendencias.push(em_filho("filho com @ContentChild(.., read:)"));
                                break;
                            }
                        }
                    }
                };
                if q.referencia {
                    consultas.push(visao::ConsultaDoFilho {
                        campo: q.campo.clone(),
                        lista: q.lista,
                        alvo: visao::AlvoDeConsulta::Referencia(q.alvo.clone()),
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
                        consultas.push(visao::ConsultaDoFilho {
                            campo: q.campo.clone(),
                            lista: q.lista,
                            alvo: visao::AlvoDeConsulta::Classe(u, simples.to_string()),
                            descendentes: q.descendentes,
                            leitura,
                        });
                    }
                    _ => {
                        pendencias.push(em_filho("filho com @ContentChild de tipo não resolvido"));
                        break;
                    }
                }
            }
        }
    }
    if comp.com_provedores {
        pendencias.push(em_filho("filho com providers"));
    }
    // Ganchos, `@Input` e `@Output` herdados não se veem daqui (o oficial
    // os coleta pelos supertipos).
    if comp.herda {
        pendencias.push(em_filho("filho que herda (extends/with)"));
    }
    if comp.template.is_none() && comp.template_url.is_some() && nos.is_none() {
        pendencias.push(em_filho("template do filho não encontrado"));
    }
    por_classe.insert(
        (uri.to_string(), comp.classe.clone()),
        visao::Filho {
            classe: comp.classe.clone(),
            seletor: comp.seletor.clone(),
            uri_dart: uri.to_string(),
            uri_template: uri.replace(".dart", ".template.dart"),
            projecoes,
            entradas: comp.entradas.clone(),
            ganchos: comp.ganchos,
            on_push: comp.on_push,
            // O estático sai no construtor da visão do filho: sem outro, o
            // filho não tem `detectHostChanges` (caso j101).
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

/// Nome do pacote, lido do `pubspec.yaml` da raiz.
///
/// É a fonte autoritativa: o nome da pasta não serve (`limitless_ui/example`
/// declara `name: limitless_ui_example`) e comparar `rootUri` do
/// `package_config.json` com a raiz falha por barra final e canonicalização.
pub fn nome_do_pacote(raiz: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(raiz.join("pubspec.yaml")).ok()?;
    for linha in texto.lines() {
        // `name:` no primeiro nível, sem indentação.
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

/// URI `package:` de um arquivo do pacote, quando ele está em `lib/`.
fn uri_de_biblioteca(pacote: &Pacote, caminho: &Path) -> Option<String> {
    let rel = pacote.relativo(caminho);
    let dentro = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{dentro}", pacote.nome))
}

/// O que sai de um arquivo: o `.template.dart`, os arquivos que o alimentam
/// e os gerados à parte (o `.css.shim.dart` de cada folha).
pub(crate) type Gerado = (String, Vec<PathBuf>, Vec<(PathBuf, String)>);

/// As diretivas do arquivo que ganham `XNgCd`, na ordem do fonte: as
/// ligações são as dos metadados lidos do programa (`hostProperties` do
/// oficial, supertipos primeiro), também as herdadas — e a diretiva que só
/// herda ligações também ganha o seu (caso j84). O membro herdado tem a
/// imutabilidade (campo `final` seria escrito uma vez) e, no `style.x`, o
/// tipo perguntados ao resolvedor. Sem os metadados, só as ligações da
/// própria classe, e a diretiva que herda é recusada.
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
                "@HostBinding/@HostListener em diretiva que herda",
            ));
        }
        return Ok(achados.hospedeiras.clone());
    }
    let mut saida = Vec::new();
    for d in &achados.diretivas {
        let Some(m) = metadados(&d.classe) else {
            continue;
        };
        if m.hospedeiro_estatico {
            return Err(recusa(
                Motivo::HostBindingEmDiretiva,
                "@HostBinding em membro estático de diretiva",
            ));
        }
        if m.ligacoes_do_hospedeiro.is_empty() {
            continue;
        }
        let propria = achados.hospedeiras.iter().find(|h| h.classe == d.classe);
        if let Some(h) = propria
            && h.ligacoes == m.ligacoes_do_hospedeiro
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
        };
        for (nome, membro) in &m.ligacoes_do_hospedeiro {
            let forma = visao::forma_do_hospedeiro(nome);
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
                        "@HostBinding('{nome}') herdado sem declaração legível"
                    ));
                }
                Ok(visao::FormaDoHospedeiro::Estilo { .. }) if final_ == Some(true) => {
                    h.recusada
                        .get_or_insert(format!("@HostBinding('{nome}') em campo final"));
                }
                Ok(visao::FormaDoHospedeiro::Estilo { .. }) => {
                    // O tipo escrito, pelo programa (que sobe até a classe
                    // que declara o membro), senão o da própria classe.
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
                                "@HostBinding('{nome}') de tipo desconhecido"
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

/// Conteúdo do `.template.dart` de um arquivo, quando sabemos gerá-lo, com os
/// arquivos que o alimentam (o `.dart` e o `.html` do template).
pub(crate) fn gerar_interno(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<Gerado, Recusa> {
    // Uma anotação do arquivo com nome que não se resolve não é constante:
    // o oficial falha ao compilá-la ("Arguments of a constant creation must
    // be constant expressions") e não gera nada.
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
            if let Some(f) = m.fora.iter().find(|f| f.contains("nome não resolvido")) {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!(
                        "anotação de {} com nome não resolvido ({f}): o oficial falha",
                        m.classe
                    ),
                ));
            }
        }
    }
    // O mesmo vale para quem lista essa classe em `directives:`: o oficial
    // lê os metadados de cada diretiva usada ao compilar o componente, e a
    // falha dela é a dele (o `material_date_time_picker` com o
    // `MaterialInputComponent` de `NG_VALIDATORS` inexistente).
    for comp in &achados.componentes {
        let (usadas, _) = indice.diretivas_de(comp, fonte, resolvedor);
        for u in &usadas {
            let m = u
                .filho
                .as_ref()
                .and_then(|f| f.metadados.clone())
                .or_else(|| u.diretiva.clone());
            if let Some(m) = m
                && let Some(f) = m.fora.iter().find(|f| f.contains("nome não resolvido"))
            {
                return Err(recusa(
                    Motivo::NaoEntendido,
                    format!(
                        "directives: {} tem anotação com nome não resolvido ({f}): o oficial falha",
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
    // `buildGeneratedCode`: os imports dos injetores logo depois do import
    // do próprio arquivo, antes dos das visões; o código deles no fim.
    let uri = uri_de_biblioteca(pacote, fonte)
        .ok_or_else(|| recusa(Motivo::Injetor, "@GenerateInjector fora de um pacote"))?;
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
    let refs: Vec<&metadados::Injetor> = injetores.iter().map(|i| i.as_ref()).collect();
    let (imports, corpo) = injetor::emitir(&refs).map_err(|f| recusa(Motivo::Injetor, f))?;
    let marca = format!("import '{nome_do_arquivo}';\n");
    let depois = texto
        .find(&marca)
        .map(|i| i + marca.len())
        .ok_or_else(|| recusa(Motivo::Injetor, "arquivo gerado sem o import de si mesmo"))?;
    texto.insert_str(depois, &imports);
    texto.push('\n');
    texto.push_str(&corpo);
    Ok((texto, entradas, extras))
}

/// O `.template.dart` das visões (e dos `XNgCd`) de um arquivo, sem os
/// injetores.
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
    // Diretiva e pipe não geram visão: o arquivo deles é o trivial, a não
    // ser que uma diretiva tenha `@HostBinding` — aí o oficial gera o
    // `DirectiveChangeDetector` dela.
    // O `XNgCd` de cada diretiva do arquivo, com as ligações herdadas
    // (caso j84).
    let hospedeiras = hospedeiras_efetivas(pacote, fonte, achados, resolvedor, indice)?;
    if achados.componentes.is_empty() {
        if hospedeiras.is_empty() {
            return Ok((
                template_trivial(nome_do_arquivo),
                vec![fonte.to_path_buf()],
                Vec::new(),
            ));
        }
        // As classes `XNgCd` na ordem do fonte, com a tabela de imports
        // compartilhada; diretiva sem `@HostBinding` e pipe não geram nada.
        if let Some(m) = hospedeiras.iter().find_map(|h| h.recusada.clone()) {
            return Err(recusa(Motivo::HostBindingEmDiretiva, m));
        }
        let hs: Vec<&Hospedeira> = hospedeiras.iter().collect();
        return Ok((
            visao::detector_de_diretivas(&hs, nome_do_arquivo),
            vec![fonte.to_path_buf()],
            Vec::new(),
        ));
    }
    // Diretiva sem `@HostBinding`/`@HostListener` e pipe não geram nada no
    // arquivo (caso j29); a com `@HostBinding` ganha a classe `XNgCd` depois
    // dos componentes (caso j45).
    if let Some(m) = hospedeiras.iter().find_map(|h| h.recusada.clone()) {
        return Err(recusa(Motivo::HostBindingEmDiretiva, m));
    }
    // Vários componentes: a tabela de imports é uma só e os trechos saem na
    // ordem do fonte (caso i24); o import da folha de cada um é alocado
    // quando a lista `styles$X` dele é escrita (caso j44).
    let mut imp = visao::Importacoes::default();
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
        trechos.push(visao::classe_ngcd(h, nome_do_arquivo, &mut imp));
    }
    Ok((
        visao::montar_arquivo(nome_do_arquivo, &imp, &trechos),
        entradas,
        Vec::new(),
    ))
}

/// O `Local` de um componente: onde ele mora e os metadados dele.
fn local_do_componente<'a>(
    pacote: &'a Pacote,
    fonte: &'a Path,
    relativo: &'a str,
    nome_do_arquivo: &'a str,
    comp: &componente::Componente,
    indice: &Indice,
) -> visao::Local<'a> {
    visao::Local {
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

/// O template de um componente, já analisado: o texto do `.html` (ou o da
/// anotação, com as posições deslocadas para as do `.dart`) e o arquivo
/// que o alimenta.
fn template_do_componente(
    fonte: &Path,
    comp: &componente::Componente,
) -> Result<(Vec<html::No>, Option<PathBuf>), Recusa> {
    let ausente = || recusa(Motivo::TemplateAusente, "templateUrl não encontrado");
    Ok(match (&comp.template, &comp.template_url) {
        (Some(t), _) => {
            let mut nos = html::analisar(t);
            if let Some(k) = comp.deslocamento_do_template {
                html::deslocar(&mut nos, k);
            }
            (nos, None)
        }
        (None, Some(url)) => {
            let caminho = fonte.parent().ok_or_else(ausente)?.join(url);
            let texto = std::fs::read_to_string(&caminho).map_err(|_| ausente())?;
            (html::analisar(&texto), Some(caminho))
        }
        (None, None) => (html::analisar(""), None),
    })
}

/// O trecho de um componente no `.template.dart`, com o `.html` dele.
#[allow(clippy::too_many_arguments)]
fn trecho_do_componente(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    comp: &componente::Componente,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
    imp: &mut visao::Importacoes,
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
    // A lista de `pipes:` só pesa se o template usa pipe (caso b19): a
    // recusa dela vai junto e é a visão que decide.
    let (pipes, fora) = indice.pipes_de(comp, fonte, resolvedor);
    let pipes = match fora.into_iter().next() {
        Some(r) => Err(r),
        None => Ok(pipes),
    };
    let texto = visao::trecho_de_componente(
        comp, &local, &nos, resolvedor, nomes, &filhos, &usadas, &pipes, imp,
    )?;
    // A folha é outra saída ([`gerar_folha`], por `.css`, como o
    // `StylesheetCompiler` do oficial): o template só importa o módulo
    // dela, e o conteúdo não o muda. Aqui só se confere que ela existe.
    for url in &comp.style_urls {
        let css = fonte
            .parent()
            .ok_or_else(|| recusa(Motivo::Estilos, "folha fora de lib/"))?
            .join(url);
        if texto_da_folha(pacote, &css).is_none() {
            return Err(recusa(Motivo::Estilos, "folha não encontrada"));
        }
    }
    Ok((texto, arquivo_html))
}

/// URI `package:` do arquivo do template — o que o oficial escreve no
/// comentário `REF` de cada ligação. Só para componentes em `lib/` com
/// `templateUrl`; com template escrito na anotação a referência é outra.
fn url_do_template(pacote: &Pacote, fonte: &Path, comp: &componente::Componente) -> Option<String> {
    // Template na anotação: a referência é o `.dart` (`asset:`), com as
    // posições dele — só quando o literal permite contá-las.
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

/// A folha de um componente compila (Sass e shim)? É a mesma conta que o
/// gerador faz; o placar usa para não marcar como pendente o que já sai.
/// O texto da folha de um componente com `ViewEncapsulation.none`, que vai
/// sem shim no `.css.dart`: o `.css` escrito, tal qual — o `compileStylesheet`
/// do ngcompiler só tira os `@import` (`extractStyleUrls`) e escapa a
/// string.
///
/// # Erros
///
/// A forma recusada, quando o texto não é conhecido byte a byte: a folha
/// vem do `sass_builder` (o `.css` não existe; a formatação da saída dele
/// não é a nossa), tem `@import` (vira outra entrada na lista) ou não se lê.
/// O texto de uma folha `.css`, como o `StylesheetCompiler` do oficial o
/// lê: a saída do `sass_builder` nesta build (o motor a passa em
/// [`Pacote::folhas_geradas`]), o `.css` escrito, ou — fora do motor, sem o
/// `sass_builder` — o `.scss` ao lado compilado aqui. `None` sem nenhum.
pub(crate) fn texto_da_folha(pacote: &Pacote, css: &Path) -> Option<Result<String, visao::Motivo>> {
    // `buildStep.readAsString` decodifica com `utf8.decode`, que tira o BOM
    // do começo (a saída `compressed` do dart-sass com caractere não-ASCII
    // começa com ele).
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

/// As saídas do `StylesheetCompiler` do ngdart para uma folha `.css`
/// (`compileStylesheet`, `stylesheet_compiler/builder.dart`): o
/// `.css.shim.dart`, com o CSS reescrito para o encapsulamento emulado, e o
/// `.css.dart`, com ele como está. Cada `@import` resolúvel sai do texto e
/// vira o módulo da folha importada, antes dele (`extractStyleUrls`,
/// `_compileStyles`); o texto vai no `escapeSingleQuoteString` do emissor.
/// Cada saída vem com o conteúdo ou o motivo de não gerá-la.
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
            "Sass que o compilador nativo não traduz",
        )),
        None => Err(recusa(Motivo::Estilos, "folha não encontrada")),
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
                        recusa(Motivo::Estilos, "@import de folha sem caminho de import")
                    })?;
                cabeca.push_str(&format!("import '{caminho}' as import{k};\n"));
                itens.push(format!("import{k}.styles"));
            }
            itens.push(visao::literal(&conteudo(&estilo)?));
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
                        "CSS que o shim do ngdart recusa (o oficial lança)",
                    )
                })
            }),
        ),
        (destino(".dart"), saida(".dart", &|t| Ok(t.to_string()))),
    ]
}

/// `final List<Object> styles = [..];` como o emissor o escreve (o
/// `.css.dart` não passa pelo `DartFormatter`): com um item, numa linha;
/// com mais, `[` e os itens na linha seguinte, dois espaços para dentro,
/// separados por `,` e quebrando quando a linha passa de 80
/// (`visitAllObjects` com `keepOnSameLine`, `currentLineLength` = nível de
/// indentação + caracteres), e `]` e `;` em linhas próprias.
fn lista_de_estilos(itens: &[String]) -> String {
    if let [item] = itens {
        return format!("final List<Object> styles = [{item}];");
    }
    let mut linhas = vec!["final List<Object> styles = [".to_string()];
    let mut linha = String::new();
    for (k, item) in itens.iter().enumerate() {
        linha.push_str(item);
        if k + 1 < itens.len() {
            // O teste vê a linha antes do separador (em unidades UTF-16,
            // como o `String.length` do Dart); a vírgula vai de todo jeito.
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

/// `extractStyleUrls`: cada `@import` (a `_cssImportRe` do oficial,
/// `@import\s+(?:url\()?\s*(?:(?:['"]([^'"]*))|([^;\)\s]*))[^;]*;?`) com
/// URL resolúvel — relativa, `package:` ou `asset:` — sai do texto, e a URL
/// resolvida contra a da folha entra na lista; a de outro esquema ou
/// absoluta fica.
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

/// `Uri.parse(base).resolve(url)` para as URLs de folha: com esquema, ela
/// mesma; relativa, contra a pasta da base, sem `.` e `..`.
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

/// Conjunto de recusas de um arquivo pendente, para o placar: a primeira,
/// e todas as que a emissão em modo de coleta encontra.
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
        fora.extend(visao::coletar(
            comp, &local, &nos, resolvedor, nomes, &filhos, &usadas, &pipes,
        ));
    }
    fora
}

/// Gera para um pacote inteiro (`lib/`, `web/`, `test/`), completando o que
/// ainda não sabe gerar com uma geração de apoio — hoje, o que o
/// `build_runner` deixou no disco.
///
/// É esta mistura que deixa a migração acontecer em passos: a cada forma nova
/// que o gerador aprende, um arquivo sai do apoio e entra no nosso, e a
/// aplicação continua compilando o tempo todo.
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
        // O apoio entra só onde não geramos: pendentes deste pacote e tudo o
        // que é de fora dele.
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

    /// Cada `.css` dá o `.css.shim.dart` e o `.css.dart` (o texto tal
    /// qual); a folha da saída do `sass_builder` vale sobre o disco, e o
    /// `@import` é recusado nas duas.
    #[test]
    fn gerar_folha_da_as_duas_saidas() {
        let dir = tempfile::tempdir().expect("temporário");
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

    /// O new_sali monta a injeção assim; se isto escapar, geramos um arquivo
    /// vazio no lugar de um injetor inteiro.
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

    /// O índice do motor de build: um arquivo entra, é achado pelo seletor,
    /// é trocado e sai, sem mexer no dos outros.
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

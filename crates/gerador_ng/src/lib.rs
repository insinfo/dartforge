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
pub mod dom;
pub mod expr;
pub mod html;
pub mod micro;
pub mod resolucao;
pub mod sass;
pub mod seletor;
pub mod visao;

use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_frontend::ast;
use dartforge_intern::Interner;
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
    /// `@GenerateInjector` em qualquer declaração de topo.
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
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Hospedeira {
    pub classe: String,
    /// `(classe CSS, membro)` de cada `@HostBinding('class.x')`, na ordem em
    /// que o oficial os coleta (`DirectiveVisitor`, em
    /// `angular_compiler/analyzer/view/directive.dart`): acessores, depois
    /// métodos, depois campos — cada grupo em ordem de declaração.
    pub classes: Vec<(String, String)>,
    /// Alguma ligação fora do que sabemos traduzir.
    pub recusada: bool,
}

/// Lê os `@HostBinding` de uma classe `@Directive`; `None` se não há.
fn hospedeira(
    arvore: &ast::Ast,
    interner: &Interner,
    classe: &ast::ClassDecl,
) -> Option<Hospedeira> {
    let mut acessores = Vec::new();
    let mut campos = Vec::new();
    let mut recusada = false;
    let mut alguma = false;
    for &m in &classe.members {
        let membro = arvore.member(m);
        for a in membro.metadata.iter() {
            if nome_da_anotacao(a, interner) != "HostBinding" {
                continue;
            }
            alguma = true;
            // Sem argumento, o nome da ligação é o do próprio membro — uma
            // ligação de propriedade, fora do subconjunto.
            let nome = a.arguments.as_ref().and_then(|args| match &args.args[..] {
                [x] if x.name.is_none() => match &arvore.expr(x.value).kind {
                    ast::ExprKind::String(lit) => lit.constant_value().map(|s| s.to_string_lossy()),
                    _ => None,
                },
                _ => None,
            });
            // Só `class.x`. `attr.`, `style.` e propriedade têm cada um a sua
            // chamada e ficam de fora até terem caso no corpus.
            let Some(classe_css) = nome.as_deref().and_then(|n| n.strip_prefix("class.")) else {
                recusada = true;
                continue;
            };
            match &membro.kind {
                // Campo `final` é imutável e seria escrito uma vez, na
                // primeira checagem (`isImmutable`); estático lê pela classe.
                // Nenhum dos dois ainda.
                ast::MemberKind::Field(l)
                    if !l.static_ && !l.final_ && !l.const_ && l.variables.len() == 1 =>
                {
                    let membro = interner.resolve(l.variables[0].name.sym).to_string();
                    campos.push((classe_css.to_string(), membro));
                }
                ast::MemberKind::Method(f) => {
                    let funcao = arvore.function(*f);
                    match (funcao.kind, funcao.name) {
                        (ast::FunctionKind::Getter, Some(n)) if !funcao.static_ => {
                            let membro = interner.resolve(n.sym).to_string();
                            acessores.push((classe_css.to_string(), membro));
                        }
                        _ => recusada = true,
                    }
                }
                _ => recusada = true,
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
        recusada = true;
    }
    // Tipo genérico muda a declaração da classe (`XNgCd<T>`); ainda não.
    if !classe.type_params.is_empty() {
        recusada = true;
    }
    Some(Hospedeira {
        classe: interner.resolve(classe.name.sym).to_string(),
        classes: acessores,
        recusada,
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
                            .extend(hospedeira(arvore, interner, classe));
                    }
                }
                "Pipe" => {
                    if let ast::DeclKind::Class(classe) = &decl.kind {
                        achados
                            .pipes
                            .push(componente::ler_pipe(arvore, fonte, interner, classe, a));
                    }
                }
                "GenerateInjector" => achados.injetores.push(alvo.clone()),
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
pub struct Pacote {
    pub nome: String,
    pub raiz: PathBuf,
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
                let achados = achar(&analisada.ast, &analisada.unit, &fonte, interner);
                arquivos.push((p, nome, achados));
            }
        }
    }

    let indice = Indice::montar(pacote, &arquivos, programa);

    for (p, nome, achados) in &arquivos {
        let (texto, entradas, extras) =
            match gerar_arquivo(pacote, p, nome, achados, resolvedor, interner, &indice) {
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
}

/// Os componentes e diretivas do pacote, por biblioteca e classe.
///
/// O oficial pergunta isso ao grafo de assets do `build`; aqui vem da
/// primeira passada. É o que permite casar `<a02-texto-estatico>` com a
/// classe que declara esse seletor e emitir a visão-filha — e saber que um
/// `<form>` recebe `NgForm`.
#[derive(Default)]
pub struct Indice {
    por_classe: std::collections::HashMap<(String, String), visao::Filho>,
    /// `@Directive` por (URI da biblioteca, classe): o seletor e o que o
    /// emissor precisa para instanciá-la.
    diretivas: std::collections::HashMap<(String, String), componente::Componente>,
    /// `@Pipe` por (URI da biblioteca, classe).
    pipes: std::collections::HashMap<(String, String), componente::Pipe>,
    /// Mesmo índice por nome de classe, para quando não há banco semântico
    /// (o teste do corpus). Só vale quando o nome é único no pacote — com
    /// duas classes de mesmo nome, resolver pelo nome seria chute.
    por_nome: std::collections::HashMap<String, Vec<visao::Filho>>,
}

impl Indice {
    fn montar(
        pacote: &Pacote,
        arquivos: &[(PathBuf, String, Achados)],
        programa: Option<&resolucao::Resolvedor>,
    ) -> Self {
        let mut por_classe = std::collections::HashMap::new();
        let mut diretivas = std::collections::HashMap::new();
        let mut pipes = std::collections::HashMap::new();
        // Com o programa carregado, o índice cobre todos os pacotes: um
        // `<li-select>` do limitless_ui é tão componente quanto um do próprio
        // projeto. Sem ele, só o que a varredura de arquivos viu.
        if let Some(r) = programa {
            let interner = r.interner();
            for (uri, arvore, unidade, fonte, caminho) in r.bibliotecas() {
                let achados = achar(arvore, unidade, fonte, interner);
                for comp in &achados.componentes {
                    indexar(&mut por_classe, uri, comp, caminho, Some(r));
                }
                for d in achados.diretivas {
                    diretivas.insert((uri.to_string(), d.classe.clone()), d);
                }
                for p in achados.pipes {
                    pipes.insert((uri.to_string(), p.classe.clone()), p);
                }
            }
        }
        for (caminho, _nome, achados) in arquivos {
            let Some(uri) = uri_de_biblioteca(pacote, caminho) else {
                continue;
            };
            for comp in &achados.componentes {
                if comp.seletor.is_empty() {
                    continue;
                }
                indexar(
                    &mut por_classe,
                    &uri,
                    comp,
                    Some(caminho),
                    programa.map(|r| r as &dyn resolucao::Resolucao),
                );
            }
            for d in &achados.diretivas {
                diretivas
                    .entry((uri.clone(), d.classe.clone()))
                    .or_insert_with(|| d.clone());
            }
            for p in &achados.pipes {
                pipes
                    .entry((uri.clone(), p.classe.clone()))
                    .or_insert_with(|| p.clone());
            }
        }
        let mut por_nome: std::collections::HashMap<String, Vec<visao::Filho>> =
            std::collections::HashMap::new();
        for ((_, classe), f) in &por_classe {
            por_nome.entry(classe.clone()).or_default().push(f.clone());
        }
        Indice {
            por_classe,
            diretivas,
            pipes,
            por_nome,
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
                    });
                } else if let Some(d) = self.diretivas.get(&chave) {
                    saida.push(visao::Usada {
                        classe: simples,
                        uri,
                        seletores: seletor::Seletor::analisar(&d.seletor),
                        filho: None,
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
                match self.por_nome.get(&simples).map(Vec::as_slice) {
                    Some([f]) if !nome.contains('.') => {
                        let chave = (f.uri_dart.clone(), f.classe.clone());
                        if vistos.insert(chave) {
                            saida.push(visao::Usada {
                                classe: f.classe.clone(),
                                uri: f.uri_dart.clone(),
                                seletores: seletor::Seletor::analisar(&f.seletor),
                                filho: Some(f.clone()),
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
    for u in usadas {
        let (Some(f), [s]) = (&u.filho, u.seletores.as_slice()) else {
            continue;
        };
        if let Some(tag) = s.so_tag() {
            saida.entry(tag.to_string()).or_insert_with(|| f.clone());
        }
    }
    saida
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
    if p.anotado && !p.opcional {
        return Err("@Inject/@Self/@Attribute no construtor do filho");
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
    if uri == "dart:html" && matches!(simples.as_str(), "Element" | "HtmlElement") {
        return Ok(visao::Injetado::Elemento);
    }
    if uri.starts_with("package:ngdart/") {
        return if simples == "ChangeDetectorRef" && !p.opcional {
            Ok(visao::Injetado::Detector)
        } else {
            Err("token do ngdart no construtor do filho")
        };
    }
    if uri.starts_with("dart:") {
        return Err("tipo do SDK no construtor do filho");
    }
    Ok(visao::Injetado::Servico {
        uri,
        classe: simples,
        opcional: p.opcional,
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
    if comp.liga_hospedeiro {
        pendencias.push(em_filho("filho com @HostBinding"));
    }
    // As consultas de conteúdo, com o alvo resolvido no arquivo do filho.
    // Tipo do ngdart (`TemplateRef`, diretivas do núcleo) casaria com o que
    // o emissor põe no conteúdo (`*ngIf`): ainda não.
    let mut consultas = Vec::new();
    match &comp.consultas_de_conteudo {
        None => pendencias.push(em_filho("filho com @ContentChild fora da forma")),
        Some(lidas) => {
            for q in lidas {
                if q.referencia {
                    consultas.push((
                        q.campo.clone(),
                        q.lista,
                        visao::AlvoDeConsulta::Referencia(q.alvo.clone()),
                    ));
                    continue;
                }
                let uri = match (caminho, resolvedor) {
                    (Some(c), Some(r)) => r.uri_do_tipo(c, &q.alvo),
                    _ => None,
                };
                match uri {
                    Some(u) if !u.starts_with("package:ngdart/") && !u.starts_with("dart:") => {
                        let simples = q.alvo.rsplit('.').next().unwrap_or(&q.alvo);
                        consultas.push((
                            q.campo.clone(),
                            q.lista,
                            visao::AlvoDeConsulta::Classe(u, simples.to_string()),
                        ));
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
            saidas: comp.saidas.clone(),
            parametros,
            consultas,
            pendencias,
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
type Gerado = (String, Vec<PathBuf>, Vec<(PathBuf, String)>);

/// Conteúdo do `.template.dart` de um arquivo, quando sabemos gerá-lo, com os
/// arquivos que o alimentam (o `.dart` e o `.html` do template).
fn gerar_arquivo(
    pacote: &Pacote,
    fonte: &Path,
    nome_do_arquivo: &str,
    achados: &Achados,
    resolvedor: Option<&dyn resolucao::Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<Gerado, Recusa> {
    if achados.trivial() {
        return Ok((
            template_trivial(nome_do_arquivo),
            vec![fonte.to_path_buf()],
            Vec::new(),
        ));
    }
    if !achados.injetores.is_empty() {
        return Err(recusa(Motivo::Injetor, "@GenerateInjector"));
    }
    // Diretiva e pipe não geram visão: o arquivo deles é o trivial, a não
    // ser que uma diretiva tenha `@HostBinding` — aí o oficial gera o
    // `DirectiveChangeDetector` dela.
    if achados.componentes.is_empty() {
        if achados.hospedeiro_herdado {
            return Err(recusa(
                Motivo::HostBindingEmDiretiva,
                "@HostBinding/@HostListener em diretiva que herda",
            ));
        }
        return match achados.hospedeiras.as_slice() {
            [] => Ok((
                template_trivial(nome_do_arquivo),
                vec![fonte.to_path_buf()],
                Vec::new(),
            )),
            // Uma diretiva só no arquivo: com mais de uma classe gerada a
            // numeração dos imports passa a ser compartilhada, e isso ainda
            // não tem caso no corpus.
            [h] if !h.recusada && achados.diretivas.len() == 1 && achados.pipes.is_empty() => Ok((
                visao::detector_de_diretiva(h, nome_do_arquivo),
                vec![fonte.to_path_buf()],
                Vec::new(),
            )),
            _ => Err(recusa(
                Motivo::HostBindingEmDiretiva,
                "@HostBinding fora de `class.x` ou várias diretivas no arquivo",
            )),
        };
    }
    if !achados.diretivas.is_empty() || !achados.pipes.is_empty() {
        return Err(recusa(
            Motivo::DiretivaOuPipe,
            "componente com diretiva ou pipe no arquivo",
        ));
    }
    if achados.componentes.len() != 1 {
        return Err(recusa(
            Motivo::VariosComponentes,
            "vários componentes no arquivo",
        ));
    }
    let comp = &achados.componentes[0];
    if let Some(r) = comp.nao_entendidos.first() {
        return Err(r.clone());
    }
    let ausente = || recusa(Motivo::TemplateAusente, "templateUrl não encontrado");
    let (template, arquivo_html) = match (&comp.template, &comp.template_url) {
        (Some(t), _) => (t.clone(), None),
        (None, Some(url)) => {
            let caminho = fonte.parent().ok_or_else(ausente)?.join(url);
            let texto = std::fs::read_to_string(&caminho).map_err(|_| ausente())?;
            (texto, Some(caminho))
        }
        (None, None) => (String::new(), None),
    };
    let relativo = pacote.relativo(fonte);
    let local = visao::Local {
        pacote: &pacote.nome,
        relativo: &relativo,
        arquivo: nome_do_arquivo,
        caminho: fonte,
        raiz: &pacote.raiz,
        url_do_template: url_do_template(pacote, fonte, comp),
    };
    let nos = html::analisar(&template);
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
    let texto = visao::template_de_componente(
        comp, &local, &nos, resolvedor, nomes, &filhos, &usadas, &pipes,
    )?;
    let mut entradas = vec![fonte.to_path_buf()];
    entradas.extend(arquivo_html);
    // A folha compilada é um arquivo à parte, como o oficial gera: o
    // `<nome>.css.shim.dart` que o template importa.
    let mut extras = Vec::new();
    let folha = |f: &str| recusa(Motivo::Estilos, f);
    for url in &comp.style_urls {
        let css = fonte
            .parent()
            .ok_or_else(|| folha("folha fora de lib/"))?
            .join(url);
        // O `.css` do `styleUrls` quase nunca existe no disco: quem o produz
        // é o `sass_builder`, a partir do `.scss` ao lado. Fazemos os dois.
        let (texto_css, entrada) = match std::fs::read_to_string(&css) {
            Ok(t) => (t, css.clone()),
            Err(_) => {
                let scss = css.with_extension("scss");
                let fonte_scss =
                    std::fs::read_to_string(&scss).map_err(|_| folha("folha não encontrada"))?;
                (
                    sass::compilar(&fonte_scss)
                        .map_err(|_| folha("Sass ou CSS fora do subconjunto"))?,
                    scss,
                )
            }
        };
        let shim = css::shim(&texto_css).map_err(|_| folha("Sass ou CSS fora do subconjunto"))?;
        let destino = css.with_file_name(format!(
            "{}.shim.dart",
            css.file_name().unwrap_or_default().to_string_lossy()
        ));
        extras.push((destino, format!("final List<Object> styles = ['{shim}'];")));
        entradas.push(entrada);
    }
    Ok((texto, entradas, extras))
}

/// URI `package:` do arquivo do template — o que o oficial escreve no
/// comentário `REF` de cada ligação. Só para componentes em `lib/` com
/// `templateUrl`; com template escrito na anotação a referência é outra.
fn url_do_template(pacote: &Pacote, fonte: &Path, comp: &componente::Componente) -> Option<String> {
    let url = comp.template_url.as_ref()?;
    let html = fonte.parent()?.join(url);
    let rel = pacote.relativo(&html);
    let dentro_de_lib = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{dentro_de_lib}", pacote.nome))
}

/// A folha de um componente compila (Sass e shim)? É a mesma conta que o
/// gerador faz; o placar usa para não marcar como pendente o que já sai.
pub(crate) fn estilo_compila(fonte: &Path, url: &str) -> bool {
    let Some(dir) = fonte.parent() else {
        return false;
    };
    let css = dir.join(url);
    let texto = match std::fs::read_to_string(&css) {
        Ok(t) => t,
        Err(_) => {
            let scss = css.with_extension("scss");
            match std::fs::read_to_string(&scss)
                .ok()
                .map(|f| sass::compilar_em(&f, scss.parent()))
            {
                Some(Ok(c)) => c,
                _ => return false,
            }
        }
    };
    css::shim(&texto).is_ok()
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
    if achados.componentes.len() != 1 {
        return fora;
    }
    let comp = &achados.componentes[0];
    let template = match (&comp.template, &comp.template_url) {
        (Some(t), _) => t.clone(),
        (None, Some(url)) => match fonte.parent().map(|d| d.join(url)) {
            Some(c) => std::fs::read_to_string(c).unwrap_or_default(),
            None => String::new(),
        },
        (None, None) => String::new(),
    };
    let relativo = pacote.relativo(fonte);
    let nome = fonte
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let local = visao::Local {
        pacote: &pacote.nome,
        relativo: &relativo,
        arquivo: &nome,
        caminho: fonte,
        raiz: &pacote.raiz,
        url_do_template: url_do_template(pacote, fonte, comp),
    };
    let (usadas, fora_da_lista) = indice.diretivas_de(comp, fonte, resolvedor);
    fora.extend(fora_da_lista);
    let filhos = filhos_por_tag(&usadas);
    let (pipes, fora_dos_pipes) = indice.pipes_de(comp, fonte, resolvedor);
    let pipes = match fora_dos_pipes.into_iter().next() {
        Some(r) => Err(r),
        None => Ok(pipes),
    };
    fora.extend(visao::coletar(
        comp,
        &local,
        &html::analisar(&template),
        resolvedor,
        nomes,
        &filhos,
        &usadas,
        &pipes,
    ));
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

    #[test]
    fn caminho_ao_lado_da_fonte() {
        assert_eq!(
            caminho_do_template(Path::new("/p/lib/a/foo.dart")),
            PathBuf::from("/p/lib/a/foo.template.dart")
        );
    }
}

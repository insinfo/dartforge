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
pub mod dom;
pub mod expr;
pub mod html;
pub mod resolucao;
pub mod visao;

use dartforge_elements::gerado::{Construtor, Geracao};
use visao::Motivo;
use dartforge_frontend::ast;
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cabeçalho que o compilador oficial escreve em todo arquivo gerado.
pub const CABECALHO: &str = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\n";

/// O que uma biblioteca tem de interessante para o gerador.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Achados {
    /// `@Component` lidos por inteiro — é deles que sai a visão.
    pub componentes: Vec<componente::Componente>,
    pub diretivas: Vec<String>,
    pub pipes: Vec<String>,
    /// `@GenerateInjector` em qualquer declaração de topo.
    pub injetores: Vec<String>,
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
    a.name.first().map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default()
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
                "Directive" => achados.diretivas.push(alvo.clone()),
                "Pipe" => achados.pipes.push(alvo.clone()),
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
    /// Quantos pendentes por motivo — o mapa que ordena o trabalho.
    pub motivos: std::collections::BTreeMap<Motivo, usize>,
    /// Conjunto completo de motivos de cada pendente. É por ele que se sabe
    /// quantos arquivos uma forma nova destrava de verdade.
    pub conjuntos: Vec<std::collections::BTreeSet<Motivo>>,
    /// Quantas vezes cada forma não entendida aparece (`@HostListener`,
    /// `@ViewChild`, ciclo de vida…).
    pub nao_entendidos: std::collections::BTreeMap<String, usize>,
}

impl Placar {
    pub fn resumo(&self) -> String {
        format!("ngdart: {}/{} gerados por nós", self.gerados, self.examinados)
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
    resolvedor: Option<&dyn resolucao::Resolucao>,
) {
    // Duas passadas: a primeira lê e analisa tudo, a segunda gera. É a
    // primeira que monta o índice de componentes do pacote — sem ele não dá
    // para saber que `<a02-texto-estatico>` é um componente e qual classe o
    // implementa.
    let mut arquivos: Vec<(PathBuf, String, Achados)> = Vec::new();
    for dir in diretorios {
        let mut pilha = vec![dir.clone()];
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else { continue };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                    continue;
                }
                let nome = p.file_name().unwrap_or_default().to_string_lossy().to_string();
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

    let indice = Indice::montar(pacote, &arquivos);

    for (p, nome, achados) in &arquivos {
        let (texto, entradas) =
            match gerar_arquivo(pacote, p, nome, achados, resolvedor, interner, &indice) {
                Ok(x) => x,
                Err(motivo) => {
                    // Forma que o gerador ainda não cobre: fica com o
                    // build_runner, e a aplicação compila do mesmo jeito.
                    *placar.motivos.entry(motivo).or_default() += 1;
                    for c in &achados.componentes {
                        if let Some(forma) = &c.nao_entendido {
                            *placar.nao_entendidos.entry(forma.clone()).or_default() += 1;
                        }
                    }
                    placar.conjuntos.push(motivos_do_arquivo(
                        pacote, p, achados, motivo, resolvedor,
                    ));
                    placar.pendentes.push(p.clone());
                    continue;
                }
            };
        c.por(caminho_do_template(p), texto, "ngdart", entradas);
        placar.gerados += 1;
    }
}

/// Os componentes do pacote, por biblioteca e classe.
///
/// O oficial pergunta isso ao grafo de assets do `build`; aqui vem da
/// primeira passada. É o que permite casar `<a02-texto-estatico>` com a
/// classe que declara esse seletor e emitir a visão-filha.
#[derive(Default)]
pub struct Indice {
    por_classe: std::collections::HashMap<(String, String), visao::Filho>,
    /// Mesmo índice por nome de classe, para quando não há banco semântico
    /// (o teste do corpus). Só vale quando o nome é único no pacote — com
    /// duas classes de mesmo nome, resolver pelo nome seria chute.
    por_nome: std::collections::HashMap<String, Vec<visao::Filho>>,
}

impl Indice {
    fn montar(pacote: &Pacote, arquivos: &[(PathBuf, String, Achados)]) -> Self {
        let mut por_classe = std::collections::HashMap::new();
        for (caminho, _nome, achados) in arquivos {
            let Some(uri) = uri_de_biblioteca(pacote, caminho) else { continue };
            for comp in &achados.componentes {
                if comp.seletor.is_empty() {
                    continue;
                }
                let projeta = comp
                    .template_url
                    .as_ref()
                    .and_then(|u| caminho.parent().map(|d| d.join(u)))
                    .and_then(|c| std::fs::read_to_string(c).ok())
                    .map(|t| html::tem_projecao(&html::analisar(&t)))
                    .unwrap_or_else(|| {
                        comp.template
                            .as_ref()
                            .map(|t| html::tem_projecao(&html::analisar(t)))
                            .unwrap_or(false)
                    });
                por_classe.insert(
                    (uri.clone(), comp.classe.clone()),
                    visao::Filho {
                        classe: comp.classe.clone(),
                        seletor: comp.seletor.clone(),
                        uri_dart: uri.clone(),
                        uri_template: uri.replace(".dart", ".template.dart"),
                        projeta,
                        entradas: comp
                            .membros
                            .iter()
                            .map(|(n, m)| (n.clone(), m.clone()))
                            .collect(),
                    },
                );
            }
        }
        let mut por_nome: std::collections::HashMap<String, Vec<visao::Filho>> =
            std::collections::HashMap::new();
        for ((_, classe), f) in &por_classe {
            por_nome.entry(classe.clone()).or_default().push(f.clone());
        }
        Indice { por_classe, por_nome }
    }

    /// Os filhos que este componente pode usar: os nomes de `directives:` que
    /// resolvem para um componente do índice, por seletor.
    fn filhos_de(
        &self,
        comp: &componente::Componente,
        arquivo: &Path,
        resolvedor: Option<&dyn resolucao::Resolucao>,
    ) -> std::collections::HashMap<String, visao::Filho> {
        let mut saida = std::collections::HashMap::new();
        for nome in &comp.diretivas {
            let achado = resolvedor
                .and_then(|r| r.uri_do_tipo(arquivo, nome))
                .and_then(|uri| self.por_classe.get(&(uri, nome.clone())))
                .or_else(|| match self.por_nome.get(nome) {
                    Some(v) if v.len() == 1 => v.first(),
                    _ => None,
                });
            if let Some(f) = achado {
                saida.insert(f.seletor.clone(), f.clone());
            }
        }
        saida
    }
}

/// URI `package:` de um arquivo do pacote, quando ele está em `lib/`.
fn uri_de_biblioteca(pacote: &Pacote, caminho: &Path) -> Option<String> {
    let rel = pacote.relativo(caminho);
    let dentro = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{dentro}", pacote.nome))
}

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
) -> Result<(String, Vec<PathBuf>), Motivo> {
    if achados.trivial() {
        return Ok((template_trivial(nome_do_arquivo), vec![fonte.to_path_buf()]));
    }
    if !achados.injetores.is_empty() {
        return Err(Motivo::Injetor);
    }
    if !achados.diretivas.is_empty() || !achados.pipes.is_empty() {
        return Err(Motivo::DiretivaOuPipe);
    }
    if achados.componentes.len() != 1 {
        return Err(Motivo::VariosComponentes);
    }
    let comp = &achados.componentes[0];
    if comp.nao_entendido.is_some() {
        return Err(Motivo::NaoEntendido);
    }
    let (template, arquivo_html) = match (&comp.template, &comp.template_url) {
        (Some(t), _) => (t.clone(), None),
        (None, Some(url)) => {
            let caminho = fonte.parent().ok_or(Motivo::TemplateAusente)?.join(url);
            let texto = std::fs::read_to_string(&caminho).map_err(|_| Motivo::TemplateAusente)?;
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
    let filhos = indice.filhos_de(comp, fonte, resolvedor);
    let texto = visao::template_de_componente(comp, &local, &nos, resolvedor, nomes, &filhos)?;
    let mut entradas = vec![fonte.to_path_buf()];
    entradas.extend(arquivo_html);
    Ok((texto, entradas))
}

/// URI `package:` do arquivo do template — o que o oficial escreve no
/// comentário `REF` de cada ligação. Só para componentes em `lib/` com
/// `templateUrl`; com template escrito na anotação a referência é outra.
fn url_do_template(
    pacote: &Pacote,
    fonte: &Path,
    comp: &componente::Componente,
) -> Option<String> {
    let url = comp.template_url.as_ref()?;
    let html = fonte.parent()?.join(url);
    let rel = pacote.relativo(&html);
    let dentro_de_lib = rel.strip_prefix("lib/")?;
    Some(format!("package:{}/{dentro_de_lib}", pacote.nome))
}

/// Conjunto de motivos de um arquivo pendente, para o placar.
fn motivos_do_arquivo(
    pacote: &Pacote,
    fonte: &Path,
    achados: &Achados,
    primeiro: Motivo,
    resolvedor: Option<&dyn resolucao::Resolucao>,
) -> std::collections::BTreeSet<Motivo> {
    let mut fora = std::collections::BTreeSet::new();
    fora.insert(primeiro);
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
    let nome = fonte.file_name().unwrap_or_default().to_string_lossy().to_string();
    let local = visao::Local {
        pacote: &pacote.nome,
        relativo: &relativo,
        arquivo: &nome,
        caminho: fonte,
        raiz: &pacote.raiz,
        url_do_template: url_do_template(pacote, fonte, comp),
    };
    fora.extend(visao::motivos(comp, &local, &html::analisar(&template), resolvedor));
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
    resolvedor: Option<&dyn resolucao::Resolucao>,
) -> (Arc<Geracao>, Placar) {
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> = ["lib", "web", "test"]
        .iter()
        .map(|d| pacote.raiz.join(d))
        .filter(|d| d.is_dir())
        .collect();
    gerar_em(pacote, &dirs, interner, &mut c, &mut placar, resolvedor);
    if let Some(apoio) = apoio {
        // O apoio entra só onde não geramos: pendentes deste pacote e tudo o
        // que é de fora dele.
        for (caminho, f) in apoio.iter() {
            if c.contem(caminho) {
                continue;
            }
            c.por(caminho.clone(), f.conteudo.to_string(), f.gerador, f.entradas.to_vec());
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
        assert_eq!(a.diretivas, vec!["D".to_string()]);
        assert_eq!(a.pipes, vec!["P".to_string()]);
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
        let a = achados_de("@GenerateInjector([])
final InjectorFactory injector = self.injector$Injector;
");
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

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
use dartforge_elements::gerado::{Construtor, Geracao};
use dartforge_frontend::ast;
use dartforge_frontend::parser::Parsed;
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Cabeçalho que o compilador oficial escreve em todo arquivo gerado.
pub const CABECALHO: &str = "// **************************************************************************\n// Generator: AngularDart Compiler\n// **************************************************************************\n\n";

/// O que uma biblioteca tem de interessante para o gerador.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Achados {
    pub componentes: Vec<String>,
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
fn nome_da_anotacao(a: &ast::Annotation, interner: &Interner) -> String {
    for parte in &a.name {
        let texto = interner.resolve(parte.sym);
        if texto.starts_with(char::is_uppercase) {
            return texto.to_string();
        }
    }
    a.name.first().map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default()
}

/// Varre as declarações de topo de uma unidade já analisada.
pub fn achar(analisada: &Parsed, interner: &Interner) -> Achados {
    let mut achados = Achados::default();
    for &id in &analisada.unit.declarations {
        let decl = analisada.ast.decl(id);
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
                "Component" => achados.componentes.push(alvo.clone()),
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

/// Gera o que sabe gerar para os `.dart` de `diretorios`, acumulando em `c`.
pub fn gerar_em(
    diretorios: &[PathBuf],
    interner: &mut Interner,
    c: &mut Construtor,
    placar: &mut Placar,
) {
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
                let achados = achar(&analisada, interner);
                if !achados.trivial() {
                    // Visões ainda não: fica com o build_runner.
                    placar.pendentes.push(p);
                    continue;
                }
                let destino = caminho_do_template(&p);
                c.por(destino, template_trivial(&nome), "ngdart", vec![p]);
                placar.gerados += 1;
            }
        }
    }
}

/// Gera para um pacote inteiro (`lib/`, `web/`, `test/`), completando o que
/// ainda não sabe gerar com uma geração de apoio — hoje, o que o
/// `build_runner` deixou no disco.
///
/// É esta mistura que deixa a migração acontecer em passos: a cada forma nova
/// que o gerador aprende, um arquivo sai do apoio e entra no nosso, e a
/// aplicação continua compilando o tempo todo.
pub fn gerar_com_apoio(
    raiz: &Path,
    interner: &mut Interner,
    apoio: Option<&Geracao>,
) -> (Arc<Geracao>, Placar) {
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> =
        ["lib", "web", "test"].iter().map(|d| raiz.join(d)).filter(|d| d.is_dir()).collect();
    gerar_em(&dirs, interner, &mut c, &mut placar);
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
        achar(&u, &i)
    }

    #[test]
    fn biblioteca_sem_angular_e_trivial() {
        assert!(achados_de("class A {}\nint x = 1;\n").trivial());
    }

    #[test]
    fn acha_componente_com_e_sem_prefixo() {
        let a = achados_de("@Component(selector: 'x')\nclass XComp {}\n");
        assert_eq!(a.componentes, vec!["XComp".to_string()]);
        let b = achados_de("@ng.Component(selector: 'x')\nclass YComp {}\n");
        assert_eq!(b.componentes, vec!["YComp".to_string()]);
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

//! Aceite do cache de expansões (docs/MACROS-PROTOCOLO.md §6) e da recarga
//! por diferença, sem VM: um executor falso implementa em Rust três macros
//! determinísticas que usam o protocolo de verdade (pedido com a pré-busca
//! dos membros, `macro.consulta` inclusive com resposta negativa, resultado
//! estruturado montado e recarregado pelo hospedeiro).
//!
//! * `@Rotulo` (declarações e definições) — declara `rotulo()` na classe; a
//!   saída só depende do nome da classe;
//! * `@Campos` (declarações) — declara um getter por campo (pré-busca) e
//!   pergunta pela função de topo `ajuda` (a resposta "não existe" também é
//!   dependência); emite um aviso;
//! * `@Consome` (declarações) — lê os métodos de `A` (que `@Rotulo` aumenta)
//!   e declara `comToJson()` ou `semToJson()`.
//!
//! Cada cenário compila uma vez, edita o fonte e compila de novo com o mesmo
//! cache; a segunda compilação tem de dar **o mesmo** que uma compilação
//! limpa (textos, ordem das expansões, diagnósticos) e só falar com o
//! executor quando algo observado mudou.
use dartforge_elements::load::load_lenient_gerados;
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::unidades::CacheUnidades;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::cache::CacheDeMacros;
use dartforge_macros_host::executor::{Disponibilidade, ExecutorMacros, Fase, PedidoDeExecucao, ServicoDeConsultas};
use dartforge_macros_host::montagem::{Codigo, Parte, Resultado};
use dartforge_macros_host::protocolo::Apresentacao;
use dartforge_macros_host::{Saida, aplicar, aplicar_incremental};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn sdk() -> Option<SdkLayout> {
    let lib = SdkLayout::discover()?;
    let mut sdk = SdkLayout::load(&lib, "dartdevc").ok()?;
    sdk.versao_corrente = LanguageVersion::new(3, 6);
    sdk.experimentos = vec![Feature::Macros];
    Some(sdk)
}

/// O executor falso. `permitido: false` reprova o teste se ele for tocado.
struct Falso {
    permitido: bool,
    instancias: Vec<String>,
    /// `(macro, fase, alvo)` de cada execução, na ordem.
    execucoes: Vec<(String, Fase, String)>,
    iniciado: bool,
    /// A macro que lança exceção ao executar (o erro de uma macro).
    falhar: Option<&'static str>,
}

impl Falso {
    fn novo() -> Self {
        Falso { permitido: true, instancias: Vec::new(), execucoes: Vec::new(), iniciado: false, falhar: None }
    }
    fn proibido() -> Self {
        Falso { permitido: false, ..Falso::novo() }
    }
    fn tocar(&self) {
        assert!(self.permitido, "o executor não podia ser tocado nesta compilação");
    }
}

fn texto(s: &str) -> Parte {
    Parte::Texto(s.into())
}

fn membro(partes: Vec<Parte>) -> Codigo {
    Codigo { tipo: "declaration".into(), partes }
}

/// `resolverIdentificador` de `dart:core`.
fn core(consultas: &mut dyn ServicoDeConsultas, nome: &str) -> u64 {
    let r = consultas.consultar("resolverIdentificador", &json!({"uri": "dart:core", "nome": nome})).expect(nome);
    r["id"].as_u64().unwrap()
}

impl ExecutorMacros for Falso {
    fn disponibilidade(&self) -> Disponibilidade {
        self.tocar();
        Disponibilidade::Disponivel
    }
    fn iniciar(&mut self) -> Result<Apresentacao, String> {
        self.tocar();
        self.iniciado = true;
        Ok(Apresentacao { executor: "falso".into(), abi: String::new(), macros: Vec::new() })
    }
    fn instanciar(&mut self, macro_: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        self.tocar();
        self.instancias.push(macro_.to_string());
        let interfaces: &[&str] = match macro_.rsplit('#').next() {
            Some("Rotulo") => &["ClassDeclarationsMacro", "ClassDefinitionMacro"],
            _ => &["ClassDeclarationsMacro"],
        };
        Ok((self.instancias.len() as u64, interfaces.iter().map(|s| s.to_string()).collect()))
    }
    fn executar(&mut self, p: &PedidoDeExecucao, consultas: &mut dyn ServicoDeConsultas) -> Result<Resultado, String> {
        self.tocar();
        let macro_ = self.instancias[(p.instancia - 1) as usize].rsplit('#').next().unwrap().to_string();
        let classe = p.alvo["ident"]["id"].as_u64().unwrap();
        let nome = p.alvo["ident"]["nome"].as_str().unwrap().to_string();
        let uri = p.alvo["lib"]["uri"].as_str().unwrap().to_string();
        self.execucoes.push((macro_.clone(), p.fase, nome.clone()));
        if self.falhar == Some(macro_.as_str()) {
            return Err(format!("{macro_}: exceção na macro"));
        }
        let mut r = Resultado::default();
        match (macro_.as_str(), p.fase) {
            ("Rotulo", Fase::Declaracoes) => {
                let string = core(consultas, "String");
                r.tipos = vec![(classe, vec![membro(vec![texto("  external "), Parte::Ident(string), texto(" rotulo();")])])];
            }
            ("Rotulo", Fase::Definicoes) => {
                let string = core(consultas, "String");
                let corpo = format!(" rotulo() => '{nome}';");
                r.tipos = vec![(classe, vec![membro(vec![texto("  augment "), Parte::Ident(string), texto(&corpo)])])];
            }
            ("Campos", Fase::Declaracoes) => {
                let int = core(consultas, "int");
                let campos = p.modelo["membros"][classe.to_string()]["campos"].as_array().cloned().unwrap_or_default();
                let mut membros = Vec::new();
                for c in &campos {
                    let n = c["ident"]["nome"].as_str().unwrap();
                    membros.push(membro(vec![texto("  external "), Parte::Ident(int), texto(&format!(" get campo_{n};"))]));
                }
                // A resposta negativa ("não existe") também é dependência.
                let ajuda = consultas.consultar("resolverIdentificador", &json!({"uri": uri, "nome": "ajuda"}));
                let marca = if ajuda.is_ok() { "comAjuda" } else { "semAjuda" };
                membros.push(membro(vec![texto(&format!("  external void {marca}();"))]));
                r.tipos = vec![(classe, membros)];
                r.diagnosticos =
                    vec![json!({"severidade": "warning", "mensagem": {"texto": format!("{nome}: {} campos", campos.len())}})];
            }
            ("Consome", Fase::Declaracoes) => {
                let a = consultas.consultar("resolverIdentificador", &json!({"uri": uri, "nome": "A"})).unwrap();
                let metodos = consultas.consultar("membros", &json!({"dono": a["id"], "tipo": "metodos"})).unwrap();
                let tem = metodos.as_array().unwrap().iter().any(|m| m["ident"]["nome"] == "toJson");
                let int = core(consultas, "int");
                let nome = if tem { " comToJson();" } else { " semToJson();" };
                r.tipos = vec![(classe, vec![membro(vec![texto("  external "), Parte::Ident(int), texto(nome)])])];
            }
            _ => {}
        }
        Ok(r)
    }
    fn encerrar(&mut self) {
        self.tocar();
        self.iniciado = false;
    }
}

/// O fonte do caso, com as edições de cada cenário.
#[derive(Clone, Default)]
struct Fonte {
    corpo_de_m: &'static str,
    campo_extra_em_a: bool,
    to_json_em_a: bool,
    campo_extra_em_b: bool,
    ajuda: bool,
}

impl Fonte {
    fn texto(&self) -> String {
        let mut s = String::from("import 'macros.dart';\n\n");
        if self.ajuda {
            s.push_str("void ajuda() {}\n\n");
        }
        s.push_str("@Rotulo()\nclass A {\n  final int a;\n");
        if self.campo_extra_em_a {
            s.push_str("  final int outro = 2;\n");
        }
        s.push_str(&format!("  A(this.a);\n  void m() {{ {} }}\n", self.corpo_de_m));
        if self.to_json_em_a {
            s.push_str("  Map<String, Object?> toJson() => {};\n");
        }
        s.push_str("}\n\n@Campos()\nclass B {\n  final int x;\n");
        if self.campo_extra_em_b {
            s.push_str("  final String y = '';\n");
        }
        s.push_str("  B(this.x);\n}\n\n@Consome()\nclass C {}\n\nvoid main() {}\n");
        s
    }
}

/// Grava a biblioteca das macros (as aplicações ficam em outra, como exige
/// a linguagem) e devolve o caminho da entrada.
fn diretorio(dir: &Path) -> PathBuf {
    std::fs::write(
        dir.join("macros.dart"),
        "macro class Rotulo { const Rotulo(); }\nmacro class Campos { const Campos(); }\nmacro class Consome { const Consome(); }\n",
    )
    .unwrap();
    dir.join("main.dart")
}

/// O que uma compilação produziu, para comparar incremental com limpa.
#[derive(Debug, PartialEq)]
struct Produto {
    textos: Vec<(PathBuf, String)>,
    avisos: Vec<String>,
    expansoes: Vec<(String, Fase)>,
}

fn produto(s: &Saida) -> Produto {
    Produto {
        textos: s.textos.iter().map(|t| (t.caminho.clone(), t.texto.clone())).collect(),
        avisos: s.avisos.iter().map(|d| format!("{:?} {}", d.span, d.message)).collect(),
        expansoes: s.expansoes.iter().map(|e| (e.aplicacao.to_string(), e.fase)).collect(),
    }
}

fn compilar(entrada: &Path, sdk: &SdkLayout, executor: &mut dyn ExecutorMacros, cache: Option<&mut CacheDeMacros>) -> Saida {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(entrada, sdk, None, &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut carregar = |i: &mut Interner, g, u: &mut CacheUnidades| load_lenient_gerados(entrada, sdk, None, i, None, Some(u), g);
    match aplicar_incremental(p, &mut nomes, None, &mut carregar, executor, cache) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    }
}

/// Compila `fonte` do zero (cache novo, executor novo).
fn limpa(entrada: &Path, sdk: &SdkLayout, fonte: &Fonte) -> Produto {
    std::fs::write(entrada, fonte.texto()).unwrap();
    let mut cache = CacheDeMacros::novo("falso");
    produto(&compilar(entrada, sdk, &mut Falso::novo(), Some(&mut cache)))
}

struct Cenario {
    _dir: tempfile::TempDir,
    entrada: PathBuf,
    sdk: SdkLayout,
    cache: CacheDeMacros,
}

impl Cenario {
    /// Compila `inicial` uma vez, enchendo o cache.
    fn novo(inicial: &Fonte) -> Option<Cenario> {
        let Some(sdk) = sdk() else {
            eprintln!("sem SDK: pulado");
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        let entrada = diretorio(dir.path());
        std::fs::write(&entrada, inicial.texto()).unwrap();
        let mut cache = CacheDeMacros::novo("falso");
        let mut executor = Falso::novo();
        let s = compilar(&entrada, &sdk, &mut executor, Some(&mut cache));
        assert_eq!(s.macros_executadas, 4, "{:?}", executor.execucoes);
        assert_eq!(s.medicao.reutilizadas, 0);
        assert!(!executor.iniciado, "a sessão encerra o executor");
        Some(Cenario { _dir: dir, entrada, sdk, cache })
    }

    /// Recompila `fonte` com o cache e confere contra a compilação limpa.
    fn recompilar(&mut self, fonte: &Fonte, executor: &mut Falso) -> Saida {
        std::fs::write(&self.entrada, fonte.texto()).unwrap();
        let s = compilar(&self.entrada, &self.sdk, executor, Some(&mut self.cache));
        let incremental = produto(&s);
        assert_eq!(incremental, limpa(&self.entrada, &self.sdk, fonte), "incremental difere da compilação limpa");
        s
    }
}

#[test]
fn recompilacao_sem_mudanca_nao_fala_com_o_executor() {
    let fonte = Fonte::default();
    let Some(mut c) = Cenario::novo(&fonte) else { return };
    let s = c.recompilar(&fonte, &mut Falso::proibido());
    assert_eq!(s.macros_executadas, 0);
    assert_eq!(s.medicao.reutilizadas, 4);
    assert!(!s.medicao.executor_iniciado);
    assert!(s.expansoes.iter().all(|e| e.reutilizada));
    // A segunda também (o cache continua cheio).
    let s = c.recompilar(&fonte, &mut Falso::proibido());
    assert_eq!(s.medicao.reutilizadas, 4);
}

#[test]
fn edicao_nao_observada_reutiliza() {
    let Some(mut c) = Cenario::novo(&Fonte::default()) else { return };
    // Corpo de método: nenhuma macro vê corpo.
    let fonte = Fonte { corpo_de_m: "print('outro corpo');", ..Fonte::default() };
    let s = c.recompilar(&fonte, &mut Falso::proibido());
    assert_eq!((s.macros_executadas, s.medicao.reutilizadas), (0, 4));
}

#[test]
fn campo_novo_invalida_so_quem_o_observa() {
    let Some(mut c) = Cenario::novo(&Fonte::default()) else { return };
    let fonte = Fonte { campo_extra_em_b: true, ..Fonte::default() };
    let mut executor = Falso::novo();
    let s = c.recompilar(&fonte, &mut executor);
    assert_eq!(executor.execucoes, vec![("Campos".to_string(), Fase::Declaracoes, "B".to_string())]);
    assert_eq!(s.medicao.reutilizadas, 3);
    assert!(s.textos[0].texto.contains("campo_y"), "{}", s.textos[0].texto);
}

#[test]
fn membro_antes_inexistente_invalida() {
    // `ajuda` não existia: a resposta negativa era dependência de @Campos.
    let Some(mut c) = Cenario::novo(&Fonte::default()) else { return };
    let fonte = Fonte { ajuda: true, ..Fonte::default() };
    let mut executor = Falso::novo();
    let s = c.recompilar(&fonte, &mut executor);
    assert_eq!(executor.execucoes, vec![("Campos".to_string(), Fase::Declaracoes, "B".to_string())]);
    assert!(s.textos[0].texto.contains("comAjuda"));

    // `toJson` em `A`: muda o modelo de @Rotulo (que roda de novo e dá o
    // mesmo) e a resposta de `membros` que @Consome leu (que roda de novo e
    // muda).
    let fonte = Fonte { ajuda: true, to_json_em_a: true, ..Fonte::default() };
    let mut executor = Falso::novo();
    let s = c.recompilar(&fonte, &mut executor);
    assert_eq!(
        executor.execucoes,
        vec![
            ("Rotulo".to_string(), Fase::Declaracoes, "A".to_string()),
            ("Consome".to_string(), Fase::Declaracoes, "C".to_string()),
            ("Rotulo".to_string(), Fase::Definicoes, "A".to_string()),
        ]
    );
    assert!(s.textos[0].texto.contains("comToJson"));
}

#[test]
fn saida_igual_nao_invalida_consumidores() {
    let Some(mut c) = Cenario::novo(&Fonte::default()) else { return };
    // Campo novo em `A`: @Rotulo roda de novo (o modelo dele mudou) e dá o
    // mesmo resultado; @Consome lê os métodos de `A`, que não mudaram, e
    // não roda.
    let fonte = Fonte { campo_extra_em_a: true, ..Fonte::default() };
    let mut executor = Falso::novo();
    let s = c.recompilar(&fonte, &mut executor);
    assert_eq!(
        executor.execucoes,
        vec![
            ("Rotulo".to_string(), Fase::Declaracoes, "A".to_string()),
            ("Rotulo".to_string(), Fase::Definicoes, "A".to_string()),
        ]
    );
    assert_eq!(s.medicao.reexecutadas_iguais, 2);
    assert_eq!(s.medicao.reutilizadas, 2);
}

#[test]
fn implementacao_editada_invalida_as_aplicacoes_dela() {
    let fonte = Fonte::default();
    let Some(mut c) = Cenario::novo(&fonte) else { return };
    // O fonte da biblioteca das macros é a identidade da implementação.
    let macros = c.entrada.with_file_name("macros.dart");
    let mut texto = std::fs::read_to_string(&macros).unwrap();
    texto.push_str("// versão 2\n");
    std::fs::write(&macros, texto).unwrap();
    let mut executor = Falso::novo();
    let s = c.recompilar(&fonte, &mut executor);
    assert_eq!((s.macros_executadas, s.medicao.reutilizadas), (4, 0));
    assert_eq!(s.medicao.reexecutadas_iguais, 4);
}

#[test]
fn sem_cache_e_com_cache_dao_o_mesmo() {
    // A recarga completa de `aplicar` e a por diferença de
    // `aplicar_incremental` produzem o mesmo programa e o mesmo texto.
    let Some(sdk) = sdk() else { return };
    let dir = tempfile::tempdir().unwrap();
    let entrada = diretorio(dir.path());
    let fonte = Fonte { campo_extra_em_b: true, to_json_em_a: true, ..Fonte::default() };
    std::fs::write(&entrada, fonte.texto()).unwrap();
    let mut nomes = Interner::new();
    let (p, _) = load_lenient_gerados(&entrada, &sdk, None, &mut nomes, None, None, None);
    let mut carregar = |i: &mut Interner, g| load_lenient_gerados(&entrada, &sdk, None, i, None, None, g);
    let completa = aplicar(p, &mut nomes, None, &mut carregar, &mut Falso::novo()).unwrap_or_else(|_| panic!("erro"));
    let por_diferenca = compilar(&entrada, &sdk, &mut Falso::novo(), None);
    assert_eq!(produto(&completa), produto(&por_diferenca));
    assert!(por_diferenca.medicao.unidades_reaproveitadas > 0);
    assert_eq!(completa.program.units.len(), por_diferenca.program.units.len());
}

/// Compila sem entrar em pânico no erro: o que o hospedeiro devolve.
fn tentar(entrada: &Path, sdk: &SdkLayout, executor: &mut dyn ExecutorMacros, cache: &mut CacheDeMacros) -> Result<Saida, String> {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(entrada, sdk, None, &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let mut carregar = |i: &mut Interner, g, u: &mut CacheUnidades| load_lenient_gerados(entrada, sdk, None, i, None, Some(u), g);
    aplicar_incremental(p, &mut nomes, None, &mut carregar, executor, Some(cache))
        .map_err(|ds| ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n"))
}

/// B07: uma macro que lança exceção faz a compilação falhar com o erro dela,
/// e o cache continua sendo o da última geração válida — voltar ao fonte
/// anterior não executa nada e dá o mesmo texto de antes; corrigir a macro
/// reexecuta só a aplicação que falhou.
#[test]
fn erro_na_macro_preserva_a_ultima_geracao_valida() {
    let inicial = Fonte::default();
    let Some(mut c) = Cenario::novo(&inicial) else { return };
    let antes = produto(&compilar(&c.entrada, &c.sdk, &mut Falso::proibido(), Some(&mut c.cache)));

    // Campo novo em `B` acorda o @Campos, que agora lança.
    let editada = Fonte { campo_extra_em_b: true, ..Fonte::default() };
    std::fs::write(&c.entrada, editada.texto()).unwrap();
    let mut quebrado = Falso::novo();
    quebrado.falhar = Some("Campos");
    let erro = tentar(&c.entrada, &c.sdk, &mut quebrado, &mut c.cache).err().expect("a exceção da macro vira erro");
    assert!(erro.contains("Campos: exceção na macro"), "{erro}");

    // De volta ao fonte válido: nada executa e o texto é o da última
    // geração válida.
    std::fs::write(&c.entrada, inicial.texto()).unwrap();
    let s = tentar(&c.entrada, &c.sdk, &mut Falso::proibido(), &mut c.cache).expect("fonte válido");
    assert_eq!(s.medicao.reutilizadas, 4);
    assert_eq!(produto(&s), antes, "a falha estragou a geração válida");

    // A macro corrigida: só a aplicação observada roda de novo, e o
    // resultado é o de uma compilação limpa.
    let mut executor = Falso::novo();
    let s = c.recompilar(&editada, &mut executor);
    assert_eq!(executor.execucoes, vec![("Campos".to_string(), Fase::Declaracoes, "B".to_string())]);
    assert!(s.textos[0].texto.contains("campo_y"));
}

/// B07: com o `main.macro.dart` materializado nas duas formas (a do 3.6.2,
/// `import augment` + `augment library`, e a atual, `part` + `part of`), a
/// biblioteca não roda as macros de novo: nenhuma execução e nenhuma
/// augmentation nova — as declarações não saem duas vezes.
#[test]
fn materializado_nas_duas_formas_nao_duplica() {
    let Some(sdk) = sdk() else { return };
    let dir = tempfile::tempdir().unwrap();
    let entrada = diretorio(dir.path());
    let fonte = Fonte::default().texto();
    std::fs::write(&entrada, &fonte).unwrap();
    let mut cache = CacheDeMacros::novo("falso");
    let s = compilar(&entrada, &sdk, &mut Falso::novo(), Some(&mut cache));
    let (caminho, texto) = (s.textos[0].caminho.clone(), s.textos[0].texto.clone());
    assert!(caminho.ends_with("main.macro.dart"), "{}", caminho.display());
    let corpo = texto.split_once('\n').map(|(_, r)| r).unwrap();
    for (cabecalho, diretiva) in [
        ("augment library 'main.dart';", "import augment 'main.macro.dart';"),
        ("part of 'main.dart';", "part 'main.macro.dart';"),
    ] {
        std::fs::write(&caminho, format!("{cabecalho}\n{corpo}")).unwrap();
        std::fs::write(&entrada, fonte.replacen("import 'macros.dart';", &format!("import 'macros.dart';\n{diretiva}"), 1)).unwrap();
        let mut nomes = Interner::new();
        let (p, d) = load_lenient_gerados(&entrada, &sdk, None, &mut nomes, None, None, None);
        assert!(d.is_empty(), "{diretiva}: {d:?}");
        let mut carregar =
            |i: &mut Interner, g, u: &mut CacheUnidades| load_lenient_gerados(&entrada, &sdk, None, i, None, Some(u), g);
        let s = aplicar_incremental(p, &mut nomes, None, &mut carregar, &mut Falso::proibido(), None)
            .unwrap_or_else(|ds| panic!("{diretiva}: {:?}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>()));
        assert_eq!(s.macros_executadas, 0, "{diretiva}");
        assert!(s.textos.is_empty(), "{diretiva}: augmentation montada de novo sobre a materializada");
    }
}

//! Contrato do backend nativo AOT para os recursos de Dart de produção:
//! variáveis e constantes de topo, membros estáticos, construtores nomeados com
//! lista de inicialização e `super` explícito, operador ternário e apagamento de
//! genéricos.
//!
//! As saídas esperadas foram produzidas por `dart run` com o Dart 3.6.2 e
//! conferidas no Dart 3.13.4; estão copiadas sem edição. Onde o DartForge diverge
//! do SDK — a ordem de inicialização de estáticos — as duas saídas aparecem em
//! constantes separadas e a divergência é afirmada por teste, não escondida. O
//! contrato está em `docs/NATIVO-PRODUCAO.md`.
use dartforge_compiler::{compile, compile_llvm};
use std::path::PathBuf;

/// Ordem de inicialização com efeito observável em cada inicializador.
const ORDEM: &str = r"
int rastro(String nome, int valor) {
  print(nome);
  return valor;
}

int topo = rastro('topo', 1);
final int fixo = rastro('fixo', 2);
const int constante = 3;

class Base {
  static int contador = rastro('Base.contador', 10);
  static const String etiqueta = 'base';
  static int dobro(int n) => n * 2;
}

class Derivada extends Base {
  static int contador = rastro('Derivada.contador', 20);
}

void main() {
  print('main');
  print(topo);
  print(fixo);
  print(constante);
  print(Base.contador);
  print(Derivada.contador);
  print(Base.etiqueta);
  print(Base.dobro(21));
  topo = topo + 100;
  print(topo);
}
";

/// Saída do Dart 3.6.2 e do 3.13.4: inicialização preguiçosa, no primeiro acesso.
const ORDEM_DART: &str =
    "main\ntopo\n1\nfixo\n2\n3\nBase.contador\n10\nDerivada.contador\n20\nbase\n42\n101\n";

/// Saída dos dois backends do DartForge: inicialização na carga, antes de `main`.
const ORDEM_DARTFORGE: &str =
    "Base.contador\nDerivada.contador\ntopo\nfixo\nmain\n1\n2\n3\n10\n20\nbase\n42\n101\n";

/// Construtores nomeados, listas de inicialização e `super` explícito nomeado.
const CONSTRUTORES: &str = r"
int marca(String nome, int valor) {
  print(nome);
  return valor;
}

class Base {
  int a;
  String etiqueta = 'padrao';
  Base(this.a) {
    print('corpo Base');
    print(a);
  }
  Base.nomeada(int n) : a = marca('init Base.nomeada', n * 2) {
    print('corpo Base.nomeada');
    print(a);
  }
  String descreve() => 'Base';
}

class Derivada extends Base {
  int b;
  int c = marca('campo c', 7);
  Derivada(int x) : b = marca('init b', x + 1), super(marca('super arg', x)) {
    print('corpo Derivada');
    print(b);
  }
  Derivada.viaNomeada(int x) : b = marca('init b nomeada', x), super.nomeada(marca('super nomeada arg', x)) {
    print('corpo Derivada.viaNomeada');
  }
  @override
  String descreve() => 'Derivada';
}

void main() {
  var d = Derivada(10);
  print(d.a);
  print(d.b);
  print(d.c);
  print(d.etiqueta);
  print(d.descreve());
  print('---');
  var e = Derivada.viaNomeada(3);
  print(e.a);
  print(e.b);
  print(e.descreve());
}
";

/// Saída do Dart 3.6.2 e do 3.13.4; os dois backends do DartForge reproduzem.
const CONSTRUTORES_ESPERADO: &str = "campo c\ninit b\nsuper arg\ncorpo Base\n10\ncorpo Derivada\n11\n10\n11\n7\npadrao\nDerivada\n---\ncampo c\ninit b nomeada\nsuper nomeada arg\ninit Base.nomeada\ncorpo Base.nomeada\n6\ncorpo Derivada.viaNomeada\n6\n3\nDerivada\n";

/// Operador ternário com efeito colateral nos dois ramos e resultado anulável.
const TERNARIO: &str = r"
int lado(String nome, int valor) {
  print(nome);
  return valor;
}

int escolhe(bool c) => c ? lado('then', 1) : lado('else', 2);

String texto(bool c) => c ? 'sim' : 'nao';

int? talvez(bool c) => c ? lado('presente', 7) : null;

void main() {
  print(escolhe(true));
  print(escolhe(false));
  print(texto(true));
  print(texto(false));
  print(talvez(true));
  print(talvez(false));
  int n = 0;
  bool crescer = true;
  n = crescer ? lado('cresce', n + 1) : lado('mantem', n);
  print(n);
}
";

/// Saída do Dart 3.6.2 e do 3.13.4; os dois backends do DartForge reproduzem.
const TERNARIO_ESPERADO: &str = "then\n1\nelse\n2\nsim\nnao\npresente\n7\nnull\ncresce\n1\n";

/// O pipeline completo aceita cada forma de produção coberta por esta frente.
#[test]
fn pipeline_aceita_as_formas_de_producao() {
    for source in [
        ORDEM,
        CONSTRUTORES,
        TERNARIO,
        // Variável de topo anulável sem valor escrito recebe null e aceita escrita.
        "int? n; void main(){print(n); n = 3; print(n);}",
        // Referência entre variáveis de topo na ordem escrita.
        "int a = 1; int b = a + 1; void main(){print(b);}",
        // Função de topo pode ler uma variável de topo; só o caminho de
        // inicialização é restrito.
        "int a = 1; int usa() => a; void main(){print(usa());}",
        // Estático de referência participa do rastreamento do GC.
        "String s = 'ola'; void main(){s = s + '!'; print(s);}",
        // Construtor nomeado sem construtor sem nome na mesma declaração.
        "class P { int x; P.origem() : x = 0; } void main(){print(P.origem().x);}",
        // `super()` escrito sem argumentos numa base sem parâmetros.
        "class B { int a = 1; } class D extends B { D() : super(); } void main(){print(D().a);}",
        // Método estático com dois parâmetros e chamada pelo nome da classe.
        "class C { static int soma(int a, int b) => a + b; } void main(){print(C.soma(1,2));}",
        // Apagamento de genérico com bound nominal: `T` vira o handle do bound.
        "class Base { int n = 1; } class Box<T extends Base> { T v; Box(this.v); T get valor => v; } void main(){print(Box(Base()).valor.n);}",
    ] {
        compile_llvm(source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        compile(source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
}

/// Estáticos ficam numa área única gravada antes da primeira instrução de `main`.
#[test]
fn a_area_de_estaticos_e_gravada_no_prologo_da_entrada() {
    let ir = compile_llvm(ORDEM).unwrap();
    let entry = ir
        .split("define void @dartforge_entry() {")
        .nth(1)
        .expect("entrada emitida");
    let entry = entry.split("\n }").next().expect("entrada fechada");
    // A área é um objeto do heap com classe sintética negativa: nenhum ID nominal
    // do frontend colide com ela e nenhum despacho virtual a seleciona.
    assert!(entry.contains("call i64 @dartforge_object_new(i64 -1, i64 "));
    assert!(ir.contains("@df_statics = internal global i64 0"));
    // O handle entra como raiz do frame da entrada, que vive até o fim do programa.
    assert!(entry.contains("call void @dartforge_gc_set_root(i64 %gcframe, i64 0, i64 %v0)"));
    assert!(entry.contains("store i64 %v0, ptr @df_statics"));
    // Os inicializadores precedem a primeira instrução de `main`, que imprime
    // a string 'main'; a ordem é a do backend JavaScript.
    let gravacoes = entry
        .split("call void @dartforge_print_string")
        .next()
        .expect("prólogo antes do primeiro print");
    assert_eq!(gravacoes.matches("@dartforge_object_set").count(), 6);
}

/// Cada estático ocupa um slot próprio, também quando a subclasse repete o nome.
#[test]
fn estatico_nao_e_herdado_e_resolve_na_declaracao_escrita() {
    let ir = compile_llvm(
        "class B { static int v = 1; } class D extends B { static int v = 2; } void main(){print(B.v);print(D.v);}",
    )
    .unwrap();
    // Slots distintos: `B.v` no zero e `D.v` no um, na ordem de herança.
    assert!(ir.contains("call void @dartforge_object_set(i64 %v1, i64 0, i64 1, i8 0)"));
    assert!(ir.contains("call void @dartforge_object_set(i64 %v2, i64 1, i64 2, i8 0)"));
    assert!(ir.contains("call i64 @dartforge_object_get(i64 %v3, i64 0)"));
    assert!(ir.contains("call i64 @dartforge_object_get(i64 %v5, i64 1)"));
    // Sem declaração própria, o nome da subclasse não alcança o estático da base:
    // a recusa vem da análise semântica e nomeia a declaração de origem.
    for source in [
        "class B { static int v = 1; } class D extends B {} void main(){print(D.v);}",
        "class B { static int f() => 1; } class D extends B {} void main(){print(D.f());}",
    ] {
        let error = compile_llvm(source).expect_err("estático não é herdado");
        assert!(error.message.contains("is not inherited"), "{error:?}");
        assert!(error.message.contains("'B'"), "{error:?}");
        assert_eq!(
            compile(source).expect_err("estático não é herdado").message,
            error.message,
            "os dois backends recusam com a mesma mensagem"
        );
    }
}

/// O ternário avalia exatamente um operando, exatamente uma vez.
#[test]
fn ternario_avalia_cada_operando_uma_unica_vez() {
    let ir = compile_llvm(
        "int um(){print(1);return 1;} int dois(){print(2);return 2;} int pega(bool c) => c ? um() : dois(); void main(){print(pega(true));}",
    )
    .unwrap();
    let corpo = ir
        .split("define i64 @df_fn_2(i1 %a0) {")
        .nth(1)
        .expect("função do ternário emitida")
        .split("\n }")
        .next()
        .expect("função fechada");
    // Uma chamada por ramo, cada uma no seu bloco, e um phi na junção.
    assert_eq!(corpo.matches("call i64 @df_fn_0()").count(), 1);
    assert_eq!(corpo.matches("call i64 @df_fn_1()").count(), 1);
    assert_eq!(corpo.matches(" = phi i64 ").count(), 1);
    assert_eq!(corpo.matches("br i1 ").count(), 1);
}

/// Construtores nomeados e estáticos usam símbolos determinísticos e sem nomes.
///
/// O índice escrito substitui o nome do usuário: `Classe.nome` vira
/// `df_new_{classe}_{índice}`. Nenhum identificador do programa entra na IR.
#[test]
fn construtores_nomeados_e_estaticos_usam_mangling_deterministico() {
    let ir = compile_llvm(
        "class Base { int a; Base(this.a); Base.dobro(int n) : a = n * 2; } class Filha extends Base { int b; Filha.tudo(int x) : b = x, super.dobro(x); } void main(){print(Filha.tudo(3).a);}",
    )
    .unwrap();
    for symbol in [
        "define i64 @df_new_0(i64 %a0)",
        "define void @df_init_0(i64 %this, i64 %a0)",
        "define i64 @df_new_0_0(i64 %a0)",
        "define void @df_init_0_0(i64 %this, i64 %a0)",
        "define i64 @df_new_1_0(i64 %a0)",
        "define void @df_init_1_0(i64 %this, i64 %a0)",
    ] {
        assert!(ir.contains(symbol), "{symbol} ausente");
    }
    // A cadeia derivada chama o construtor nomeado escolhido por `super.dobro`.
    assert!(ir.contains("call void @df_init_0_0(i64 %this, i64 %v2)"));
    // Nenhum identificador do programa aparece como símbolo da IR.
    for nome in ["Base", "Filha", "dobro", "tudo"] {
        assert!(!ir.contains(&format!("@{nome}")), "{nome} vazou para a IR");
    }
    let estaticos = compile_llvm(
        "class C { static int dobro(int n) => n * 2; } void main(){print(C.dobro(2));}",
    )
    .unwrap();
    assert!(estaticos.contains("define i64 @df_static_0_0(i64 %a0)"));
    assert!(estaticos.contains("call i64 @df_static_0_0(i64 2)"));
    // Um estático não recebe receptor: o símbolo não tem parâmetro `%this`.
    assert!(!estaticos.contains("@df_static_0_0(i64 %this"));
}

/// Cada limite recusado conserva a mensagem e o span exato do trecho responsável.
#[test]
fn limites_recusados_conservam_mensagem_e_span() {
    for (source, message, fragment) in [
        // Com inicialização na carga, ler um estático ainda não gravado não tem
        // resposta correta; o JavaScript emitido lança ReferenceError no mesmo caso.
        (
            "int a = b + 1; int b = 2; void main(){print(a);}",
            "LLVM AOT ainda não suporta a leitura de um estático ainda não inicializado (a ordem nativa é a da carga)",
            "b",
        ),
        (
            "int a = a + 1; void main(){print(a);}",
            "LLVM AOT ainda não suporta a leitura de um estático ainda não inicializado (a ordem nativa é a da carga)",
            "a",
        ),
        (
            "class A { static int x = B.y + 1; } class B { static int y = 2; } void main(){print(A.x);}",
            "LLVM AOT ainda não suporta a leitura de um estático ainda não inicializado (a ordem nativa é a da carga)",
            "B.y",
        ),
        // O fecho transitivo não sabe em que ponto da sequência a rotina executa.
        (
            "int f() => b; int a = f(); int b = 2; void main(){print(a);}",
            "LLVM AOT ainda não suporta acesso a estático dentro de rotina chamada por inicializador de estático",
            "b",
        ),
        // Uma escrita antecipada seria sobreposta pelo inicializador.
        (
            "void f(){ b = 9; } int a = 1; int b = 2; void main(){ a = 0; f(); print(b);}",
            "LLVM AOT ainda não suporta acesso a estático dentro de rotina chamada por inicializador de estático",
            "b = 9",
        ),
    ] {
        let error = compile_llvm(source).expect_err("o limite precisa ser explícito");
        assert_eq!(error.message, message, "{source}");
        assert_eq!(
            &source[error.span.start..error.span.end],
            fragment,
            "{source}"
        );
    }
}

/// O apagamento de genéricos só é aceito quando não é observável.
///
/// O frontend apaga `T` para o seu bound antes da HIR. Com bound nominal o alvo
/// nativo usa o handle da classe do bound, o que é seguro porque nenhuma
/// operação do subconjunto nativo observa o argumento de tipo. As formas que o
/// observariam — `is`, `as`, funções genéricas e argumentos reificados —
/// continuam recusadas com mensagem própria.
#[test]
fn apagamento_de_genericos_e_seguro_ou_recusado() {
    compile_llvm(
        "class Base { int n = 1; } class Box<T extends Base> { T v; Box(this.v); T get valor => v; } void main(){print(Box(Base()).valor.n);}",
    )
    .unwrap();
    for (source, message) in [
        (
            "void main(){int x=1; print(x is int);}",
            "LLVM AOT ainda não suporta testes e casts de tipos reificados",
        ),
        (
            "T id<T>(T x) => x; void main(){print(id<int>(1));}",
            "LLVM AOT ainda não suporta funções genéricas",
        ),
    ] {
        let error = compile_llvm(source).expect_err("o limite precisa ser explícito");
        assert_eq!(error.message, message, "{source}");
        assert!(error.span.end <= source.len());
    }
}

/// Diretório exclusivo do teste; nada fora dele é removido.
struct Saida(PathBuf);
impl Saida {
    /// Reserva um diretório único mesmo com execuções simultâneas na máquina.
    fn nova() -> Self {
        let marca = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-nativo-producao-{}-{marca}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Saida {
    /// Limpa apenas o diretório criado por esta instância.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Executa o módulo JavaScript emitido e devolve a saída padrão normalizada.
fn executa_javascript(source: &str) -> String {
    let js = compile(source).unwrap();
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// Liga e executa o programa nativo no nível de otimização indicado.
fn executa_nativo(source: &str, dir: &Saida, rotulo: &str, optimize: bool) -> String {
    let ir = compile_llvm(source).unwrap();
    let output = dir.0.join(format!(
        "{rotulo}-{optimize}{}",
        std::env::consts::EXE_SUFFIX
    ));
    let options = dartforge_native::NativeOptions {
        optimize,
        ..Default::default()
    };
    dartforge_native::build_executable(&ir, &output, &options).unwrap();
    let result = std::process::Command::new(&output)
        // Coleta forçada a cada alocação prova que a área de estáticos e os
        // handles construídos permanecem enraizados.
        .env("DARTFORGE_GC_STRESS", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// Teste diferencial: o nativo e o JavaScript concordam nos mesmos programas.
///
/// Onde o DartForge reproduz o SDK, a saída é comparada também com o Dart 3.6.2.
/// Onde não reproduz — a ordem de inicialização de estáticos —, os dois backends
/// continuam obrigados a concordar entre si, e a divergência com o SDK é
/// afirmada explicitamente em vez de tolerada em silêncio.
#[test]
#[ignore = "requer Node.js e clang LLVM 17+/rustc nativos no PATH ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn diferencial_nativo_e_javascript() {
    let dir = Saida::nova();
    for (rotulo, source, esperado) in [
        ("construtores", CONSTRUTORES, CONSTRUTORES_ESPERADO),
        ("ternario", TERNARIO, TERNARIO_ESPERADO),
        ("ordem", ORDEM, ORDEM_DARTFORGE),
    ] {
        let javascript = executa_javascript(source);
        assert_eq!(javascript, esperado, "{rotulo}: JavaScript");
        for optimize in [false, true] {
            let nativo = executa_nativo(source, &dir, rotulo, optimize);
            assert_eq!(nativo, javascript, "{rotulo}: nativo vs JavaScript");
            assert_eq!(nativo, esperado, "{rotulo}: nativo");
        }
    }
    // A divergência documentada é um fato conferido, não uma suposição.
    assert_ne!(ORDEM_DARTFORGE, ORDEM_DART);
}

/// A divergência de ordem pertence ao projeto: os dois backends concordam nela.
#[test]
#[ignore = "requer Node.js no PATH"]
fn a_ordem_de_carga_e_a_mesma_nos_dois_backends_e_difere_do_sdk() {
    assert_eq!(executa_javascript(ORDEM), ORDEM_DARTFORGE);
    assert_ne!(ORDEM_DARTFORGE, ORDEM_DART);
    // O conteúdo impresso é o mesmo; só a ordem muda, e o valor final de cada
    // leitura coincide com o do SDK.
    let mut nossas: Vec<&str> = ORDEM_DARTFORGE.lines().collect();
    let mut deles: Vec<&str> = ORDEM_DART.lines().collect();
    nossas.sort_unstable();
    deles.sort_unstable();
    assert_eq!(nossas, deles);
}

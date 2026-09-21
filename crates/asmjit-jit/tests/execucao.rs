//! Execução da fatia escalar pelo JIT por montador e acordo com o backend AOT.
//!
//! As saídas esperadas foram obtidas rodando os mesmos programas na VM do Dart
//! 3.6.2 (`dart run`), que é o oráculo do subconjunto nativo: inteiro de 64 bits
//! com estouro modular. O corpus é deliberadamente o mesmo de
//! `crates/cranelift-jit/tests/execucao.rs`, para que os três backends do perfil
//! de desenvolvimento sejam comparáveis linha a linha.
//!
//! O teste marcado `#[ignore]` compara a saída do JIT com a do executável AOT
//! produzido por Clang e rustc; sem essas ferramentas no PATH ele não roda, e o
//! acordo entre os backends fica coberto pelo teste que confere que ambos
//! aceitam exatamente o mesmo corpus e recusam o mesmo ponto do programa.
use dartforge_diagnostics::Diagnostic;
use dartforge_hir::Module;
use std::path::PathBuf;

/// Aritmética, comparações, curto-circuito, `if`, `while`, `for` e `do`/`while`.
const CONTROLE: &str = r"
int soma(int a, int b) {
  return a + b;
}

bool maior(int a, int b) {
  return a > b;
}

void main() {
  int x = 7;
  int y = 5;
  print(soma(x, y));
  print(soma(x, -y));
  print(x * y - 3);
  print(-x);
  print(maior(x, y));
  print(maior(y, x));
  print(x == 7 && y != 5);
  print(x == 7 || y != 5);
  print(!maior(x, y));
  if (x > y) {
    print(1);
  } else {
    print(2);
  }
  int i = 0;
  while (i < 3) {
    print(i);
    i = i + 1;
  }
  for (int j = 0; j < 5; j = j + 1) {
    if (j == 1) {
      continue;
    }
    if (j == 4) {
      break;
    }
    print(j * 10);
  }
  int k = 0;
  do {
    print(100 + k);
    k = k + 1;
  } while (k < 2);
}
";
const CONTROLE_ESPERADO: &str =
    "12\n2\n32\n-7\ntrue\nfalse\nfalse\ntrue\nfalse\n1\n0\n1\n2\n0\n20\n30\n100\n101\n";

/// Recursão simples, recursão de árvore e recursão mútua entre duas funções.
const RECURSAO: &str = r"
int fatorial(int n) {
  if (n <= 1) {
    return 1;
  }
  return n * fatorial(n - 1);
}

int fib(int n) {
  if (n < 2) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

bool par(int n) {
  if (n == 0) {
    return true;
  }
  return impar(n - 1);
}

bool impar(int n) {
  if (n == 0) {
    return false;
  }
  return par(n - 1);
}

void main() {
  print(fatorial(10));
  print(fib(20));
  print(par(10));
  print(impar(10));
}
";
const RECURSAO_ESPERADO: &str = "3628800\n6765\ntrue\nfalse\n";

/// Estouro modular de i64: 2^63 vira o mínimo e o produto satura em zero.
///
/// Os literais da AST são de 32 bits, então os valores grandes são construídos
/// por multiplicação sucessiva — que é justamente o que exercita o estouro.
const ESTOURO: &str = r"
void main() {
  int grande = 1;
  for (int i = 0; i < 63; i = i + 1) {
    grande = grande * 2;
  }
  print(grande);
  print(grande - 1);
  print(grande * grande);
  int m = 1;
  for (int i = 0; i < 62; i = i + 1) {
    m = m * 3;
  }
  print(m);
  print(-m * m);
}
";
const ESTOURO_ESPERADO: &str =
    "-9223372036854775808\n9223372036854775807\n0\n5069619362125685561\n6034816272323861839\n";

/// Laço com um milhão de iterações: prova o quadro de pilha sob repetição longa.
///
/// A variante de cem milhões de iterações, usada no eixo 2 de `docs/ASMJIT.md`,
/// está em `examples/experimento.rs`. Aqui o laço é mil vezes menor de propósito:
/// o código deste backend vai à pilha em toda operação e uma suíte de testes não
/// é lugar para segundos de laço.
const SOMATORIO: &str = r"
int somatorio(int n) {
  int total = 0;
  for (int i = 0; i < n; i = i + 1) {
    total = total + i;
  }
  return total;
}

void main() {
  print(somatorio(1000000));
}
";
const SOMATORIO_ESPERADO: &str = "499999500000\n";

/// Corpo arrow, parâmetros escalares e chamadas encadeadas.
const ARROW: &str = r"
int dobro(int a) => a * 2;

bool positivo(int a) => a > 0;

void main() {
  print(dobro(21));
  print(positivo(dobro(-1)));
}
";
const ARROW_ESPERADO: &str = "42\nfalse\n";

/// Sombreamento em bloco aninhado e o limite de quatro argumentos da ABI.
///
/// A chamada aninhada é o caso que um montador sem alocador de registradores
/// erra com mais facilidade: os argumentos externos já avaliados não podem morar
/// em registrador enquanto a chamada interna acontece. Este programa cobre isso.
const ESCOPO: &str = r"
int quatro(int a, int b, int c, int d) {
  return a + b * c - d;
}

void main() {
  int x = 1;
  {
    int x = 2;
    print(x);
  }
  print(x);
  print(quatro(1, 2, 3, 4));
  print(quatro(quatro(1, 1, 1, 1), 2, 3, quatro(0, 1, 1, 0)));
}
";
const ESCOPO_ESPERADO: &str = "2\n1\n3\n6\n";

/// `continue` em `while` e `do`/`while`, e `break` em laço aninhado.
///
/// O `CONTROLE` só exercita `break` e `continue` dentro de um `for`, onde o
/// destino de `continue` é a atualização. Nos outros dois laços o destino é a
/// condição, e no `do`/`while` isso é contraintuitivo o suficiente para merecer
/// um programa próprio: `continue` pula para o teste, não para o começo do corpo.
/// O `break` aninhado prova que o salto vai para a saída do laço **interno**.
const LACOS: &str = r"
void main() {
  int i = 0;
  while (i < 6) {
    i = i + 1;
    if (i == 2) {
      continue;
    }
    if (i == 5) {
      break;
    }
    print(i);
  }
  print(100);
  for (int a = 0; a < 3; a = a + 1) {
    for (int b = 0; b < 3; b = b + 1) {
      if (b == 1) {
        break;
      }
      print(a * 10 + b);
    }
  }
  int n = 0;
  do {
    n = n + 1;
    if (n == 2) {
      continue;
    }
    print(-n);
  } while (n < 4);
}
";
const LACOS_ESPERADO: &str = "1\n3\n4\n100\n0\n10\n20\n-1\n-3\n-4\n";

/// Programas da fatia coberta, com a saída conferida na VM do Dart 3.6.2.
fn corpus() -> [(&'static str, &'static str, &'static str); 7] {
    [
        ("controle", CONTROLE, CONTROLE_ESPERADO),
        ("recursao", RECURSAO, RECURSAO_ESPERADO),
        ("estouro", ESTOURO, ESTOURO_ESPERADO),
        ("somatorio", SOMATORIO, SOMATORIO_ESPERADO),
        ("arrow", ARROW, ARROW_ESPERADO),
        ("escopo", ESCOPO, ESCOPO_ESPERADO),
        ("lacos", LACOS, LACOS_ESPERADO),
    ]
}

/// Usa o frontend real para preservar tipos de expressão, spans e resolução.
fn hir(fonte: &str) -> Result<Module<'_>, Diagnostic> {
    let tokens = dartforge_lexer::lex(fonte)?;
    let programa = dartforge_parser::parse(&tokens, fonte.len())?;
    let resolucao = dartforge_semantic::analyze(&programa)?;
    Ok(dartforge_hir::lower_resolved(programa, resolucao))
}

/// Compila e executa capturando a saída, como faria o perfil de desenvolvimento.
fn executar(fonte: &str) -> Result<String, Diagnostic> {
    let modulo = hir(fonte)?;
    Ok(dartforge_asmjit_jit::compilar(&modulo)?.executar_capturando())
}

/// Mensagem e trecho exato apontado pelo span de uma recusa do JIT.
fn recusa(fonte: &str) -> (String, &str) {
    let modulo = hir(fonte).expect("o frontend aceita o programa; quem recusa é o backend");
    let erro = dartforge_asmjit_jit::compilar(&modulo)
        .expect_err("o programa está fora da fatia e deveria ser recusado");
    assert!(
        erro.span.start <= erro.span.end && erro.span.end <= fonte.len(),
        "span fora da fonte: {:?}",
        erro.span
    );
    (erro.message, &fonte[erro.span.start..erro.span.end])
}

/// Toda a fatia coberta reproduz, byte a byte, a saída da VM do Dart 3.6.2.
#[test]
fn a_fatia_coberta_reproduz_a_saida_do_dart_3_6_2() {
    for (nome, fonte, esperado) in corpus() {
        assert_eq!(executar(fonte).unwrap(), esperado, "programa {nome}");
    }
}

/// Recursão simples, de árvore e mútua funcionam porque tudo é declarado antes.
///
/// No montador isso depende de uma propriedade concreta: os rótulos de todas as
/// funções são criados antes de qualquer corpo, então um `call` para função
/// ainda não emitida vira relocação resolvida na confirmação do bloco.
#[test]
fn recursao_simples_de_arvore_e_mutua() {
    assert_eq!(executar(RECURSAO).unwrap(), RECURSAO_ESPERADO);
}

/// `int` é i64 com estouro modular, igual ao backend LLVM e à VM do Dart.
#[test]
fn inteiro_e_i64_com_estouro_modular() {
    assert_eq!(executar(ESTOURO).unwrap(), ESTOURO_ESPERADO);
}

/// Chamada aninhada em posição de argumento não corrompe os argumentos externos.
#[test]
fn chamadas_aninhadas_e_sombreamento_de_escopo() {
    assert_eq!(executar(ESCOPO).unwrap(), ESCOPO_ESPERADO);
}

/// `break` e `continue` saltam para o destino certo nos três laços e aninhados.
#[test]
fn break_e_continue_nos_tres_lacos() {
    assert_eq!(executar(LACOS).unwrap(), LACOS_ESPERADO);
}

/// Executar o mesmo programa duas vezes não acumula nem perde saída.
#[test]
fn a_execucao_e_repetivel_no_mesmo_programa_compilado() {
    let modulo = hir(CONTROLE).unwrap();
    let compilado = dartforge_asmjit_jit::compilar(&modulo).unwrap();
    assert_eq!(compilado.executar_capturando(), CONTROLE_ESPERADO);
    assert_eq!(compilado.executar_capturando(), CONTROLE_ESPERADO);
}

/// As medições existem, são coerentes entre si e contam trabalho de verdade.
#[test]
fn as_medicoes_separam_traducao_de_publicacao_do_bloco() {
    let modulo = hir(CONTROLE).unwrap();
    let compilado = dartforge_asmjit_jit::compilar(&modulo).unwrap();
    let medicoes = compilado.medicoes();
    assert!(medicoes.instrucoes_emitidas > 0, "{medicoes:?}");
    assert!(medicoes.bytes_codigo > 0, "{medicoes:?}");
    assert!(medicoes.total >= medicoes.traducao, "{medicoes:?}");
    assert!(medicoes.total >= medicoes.geracao, "{medicoes:?}");
    // Um montador emite pelo menos um byte por instrução: se o contador de
    // instruções passar do de bytes, um dos dois está sendo contado errado.
    assert!(
        medicoes.bytes_codigo >= medicoes.instrucoes_emitidas,
        "{medicoes:?}"
    );
    // O mapeamento é arredondado para páginas, então nunca é menor que o código.
    assert!(compilado.bytes() >= medicoes.bytes_codigo, "{medicoes:?}");
}

/// O contador de instruções cresce com o programa, não com a carga da máquina.
///
/// É o contador que `docs/DESEMPENHO.md` exige ao lado da mediana: duas
/// compilações do mesmo módulo emitem exatamente o mesmo número de instruções e
/// os mesmos bytes, e um programa maior emite mais.
#[test]
fn os_contadores_de_trabalho_sao_deterministicos() {
    let modulo = hir(CONTROLE).unwrap();
    let primeira = dartforge_asmjit_jit::compilar(&modulo).unwrap();
    let segunda = dartforge_asmjit_jit::compilar(&modulo).unwrap();
    assert_eq!(
        primeira.medicoes().instrucoes_emitidas,
        segunda.medicoes().instrucoes_emitidas
    );
    assert_eq!(
        primeira.medicoes().bytes_codigo,
        segunda.medicoes().bytes_codigo
    );
    let pequeno = hir(ARROW).unwrap();
    let pequeno = dartforge_asmjit_jit::compilar(&pequeno).unwrap();
    assert!(
        pequeno.medicoes().instrucoes_emitidas < primeira.medicoes().instrucoes_emitidas,
        "{:?} não é menor que {:?}",
        pequeno.medicoes(),
        primeira.medicoes()
    );
}

/// Contrato central: o JIT e o backend LLVM aceitam exatamente o mesmo corpus.
///
/// A comparação da saída executada exige Clang e rustc e fica no teste
/// `#[ignore]` abaixo; este confere o acordo que pode ser verificado sempre.
#[test]
fn o_jit_e_o_backend_llvm_aceitam_o_mesmo_corpus() {
    for (nome, fonte, _) in corpus() {
        let modulo = hir(fonte).unwrap();
        let ir = dartforge_llvm::emit(&modulo);
        assert!(ir.is_ok(), "LLVM recusou {nome}: {:?}", ir.err());
        let compilado = dartforge_asmjit_jit::compilar(&modulo);
        assert!(
            compilado.is_ok(),
            "asmjit recusou {nome}: {:?}",
            compilado.err()
        );
    }
}

/// Formas fora de ambos os backends são recusadas no mesmo ponto do programa.
///
/// A mensagem difere — cada backend diz que foi ele quem recusou, e cada um
/// descreve o recurso com as suas palavras — mas o span tem de ser o mesmo,
/// senão um dos dois está apontando errado.
///
/// A lista é curta de propósito: ela só pode conter formas que os **dois**
/// backends recusam, e a fatia do backend AOT cresce. Formas que já eram
/// comuns aos dois e deixaram de ser — o operador condicional, strings e tipos
/// anuláveis, aceitos pelo LLVM desde então — saíram daqui e continuam cobertas,
/// do lado do montador, por `cada_forma_fora_da_fatia_tem_mensagem_e_span_exatos`.
/// `for-in` fica fora por outro motivo: os dois recusam, mas o LLVM aponta para
/// o literal de coleção e o montador para a instrução inteira.
#[test]
fn jit_e_llvm_recusam_o_mesmo_ponto_do_programa() {
    const CASOS: [&str; 5] = [
        "void main() {\n  int x = 7 ~/ 2;\n  print(x);\n}\n",
        "void main() {\n  int x = 7 % 2;\n  print(x);\n}\n",
        "void main() {\n  int x = 1 << 2;\n  print(x);\n}\n",
        "void main() {\n  try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }\n}\n",
        "void main() {\n  assert(1 > 0);\n  print(1);\n}\n",
    ];
    for fonte in CASOS {
        let modulo = hir(fonte).unwrap();
        let llvm = dartforge_llvm::emit(&modulo)
            .err()
            .unwrap_or_else(|| panic!("LLVM aceitou o que deveria recusar: {fonte}"));
        let asmjit = dartforge_asmjit_jit::compilar(&modulo)
            .err()
            .unwrap_or_else(|| panic!("asmjit aceitou o que deveria recusar: {fonte}"));
        assert_eq!(llvm.span, asmjit.span, "spans divergentes em {fonte}");
        assert!(
            llvm.message.starts_with("LLVM AOT ainda não suporta "),
            "{}",
            llvm.message
        );
        assert!(
            asmjit.message.starts_with("asmjit JIT ainda não suporta "),
            "{}",
            asmjit.message
        );
    }
}

/// Cada forma fora da fatia tem mensagem e span exatos, nunca aceite silencioso.
///
/// O segundo elemento de cada par é o texto que o span recorta da fonte: é
/// assim que o teste prova que o diagnóstico aponta para o lugar certo.
#[test]
fn cada_forma_fora_da_fatia_tem_mensagem_e_span_exatos() {
    let casos: [(&str, &str, &str); 17] = [
        (
            "void main() {\n  double x = 1.5;\n  print(x);\n}\n",
            "asmjit JIT ainda não suporta double e num",
            "double x = 1.5;",
        ),
        (
            "void main() {\n  String s = 'a';\n  print(s);\n}\n",
            "asmjit JIT ainda não suporta strings",
            "String s = 'a';",
        ),
        (
            "void main() {\n  int? x = 1;\n  print(x);\n}\n",
            "asmjit JIT ainda não suporta tipos anuláveis",
            "int? x = 1;",
        ),
        (
            "void main() {\n  int x = 7 ~/ 2;\n  print(x);\n}\n",
            "asmjit JIT ainda não suporta divisão double e truncada (`/`, `~/`)",
            "7 ~/ 2",
        ),
        (
            "void main() {\n  int x = 7 % 2;\n  print(x);\n}\n",
            "asmjit JIT ainda não suporta módulo euclidiano",
            "7 % 2",
        ),
        (
            "void main() {\n  int x = 1 << 2;\n  print(x);\n}\n",
            "asmjit JIT ainda não suporta operadores bit a bit e deslocamentos",
            "1 << 2",
        ),
        (
            "void main() {\n  print(1 > 0 ? 2 : 3);\n}\n",
            "asmjit JIT ainda não suporta o operador condicional",
            "1 > 0 ? 2 : 3",
        ),
        (
            "void main() {\n  var l = [1, 2];\n  print(l[0]);\n}\n",
            "asmjit JIT ainda não suporta coleções",
            "[1, 2]",
        ),
        (
            "void main() {\n  switch (1) {\n    case 1:\n      print(1);\n  }\n}\n",
            "asmjit JIT ainda não suporta switch",
            "switch (1) {\n    case 1:\n      print(1);\n  }",
        ),
        (
            "void main() {\n  try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }\n}\n",
            "asmjit JIT ainda não suporta try, catch, finally e rethrow",
            "try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }",
        ),
        (
            "void main() {\n  assert(1 > 0);\n  print(1);\n}\n",
            "asmjit JIT ainda não suporta assert",
            "assert(1 > 0);",
        ),
        (
            "void main() {\n  for (final x in [1, 2]) {\n    print(x);\n  }\n}\n",
            "asmjit JIT ainda não suporta for-in",
            "for (final x in [1, 2]) {\n    print(x);\n  }",
        ),
        (
            "class C {\n  int f = 1;\n}\n\nvoid main() {\n  print(1);\n}\n",
            "asmjit JIT ainda não suporta classes, enums e membros estáticos",
            "class C {\n  int f = 1;\n}",
        ),
        (
            "int g = 1;\n\nvoid main() {\n  print(g);\n}\n",
            "asmjit JIT ainda não suporta variáveis de topo",
            "int g = 1;",
        ),
        (
            "int f([int a = 1]) {\n  return a;\n}\n\nvoid main() {\n  print(f(2));\n}\n",
            "asmjit JIT ainda não suporta parâmetros opcionais ou nomeados",
            "int a = 1",
        ),
        (
            "void main() {\n  int x = 1;\n  return;\n  print(x ~/ 2);\n}\n",
            "asmjit JIT ainda não suporta divisão double e truncada (`/`, `~/`)",
            "x ~/ 2",
        ),
        (
            "void main() {\n  print('a' + 'b');\n}\n",
            "asmjit JIT ainda não suporta strings",
            "'a'",
        ),
    ];
    for (fonte, mensagem, trecho) in casos {
        let (obtida, recorte) = recusa(fonte);
        assert_eq!(obtida, mensagem, "programa:\n{fonte}");
        assert_eq!(recorte, trecho, "programa:\n{fonte}");
    }
}

/// O limite de quatro parâmetros da ABI é recusado com span do quinto parâmetro.
///
/// É um limite deste backend e não do subconjunto: a System V aceitaria seis
/// argumentos em registrador e a Win64 só quatro, e a fatia para no menor dos
/// dois em vez de divergir entre alvos. Consulte `src/abi.rs`.
#[test]
fn o_quinto_parametro_e_recusado_por_causa_da_abi() {
    let (mensagem, recorte) = recusa(
        "int f(int a, int b, int c, int d, int e) {\n  return a;\n}\n\nvoid main() {\n  print(f(1, 2, 3, 4, 5));\n}\n",
    );
    assert_eq!(
        mensagem,
        "asmjit JIT ainda não suporta mais de quatro parâmetros por função"
    );
    assert_eq!(recorte, "int e");
}

/// Código morto também é validado: nada fora da fatia passa por não executar.
#[test]
fn codigo_morto_depois_do_return_ainda_e_recusado() {
    let (mensagem, _) = recusa("void main() {\n  return;\n  print(1.5);\n}\n");
    assert_eq!(mensagem, "asmjit JIT ainda não suporta literais double");
}

/// Diretório temporário exclusivo para os executáveis do teste diferencial.
struct Fixture(PathBuf);
impl Fixture {
    /// Reserva um diretório próprio, sem depender de caminho estático compartilhado.
    fn new() -> Self {
        let marca = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let caminho =
            std::env::temp_dir().join(format!("dartforge-asmjit-{}-{marca}", std::process::id()));
        std::fs::create_dir(&caminho).unwrap();
        Self(caminho)
    }
}
impl Drop for Fixture {
    /// Remove apenas os artefatos criados por este teste.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Teste diferencial de execução: JIT e executável AOT imprimem os mesmos bytes.
///
/// É o contrato central do experimento — um backend que discorda do AOT no mesmo
/// programa está errado, por mais rápido que seja. O montador é o backend em que
/// esse teste pega mais coisa: convenção de chamada, alinhamento de pilha,
/// *shadow space* da Win64 e largura dos registradores não são verificados por
/// nada além dele.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn a_saida_do_jit_bate_com_a_do_executavel_aot() {
    let fixture = Fixture::new();
    let opcoes = dartforge_native::NativeOptions::default();
    for (nome, fonte, esperado) in corpus() {
        let modulo = hir(fonte).unwrap();
        let ir = dartforge_llvm::emit(&modulo).unwrap();
        let executavel = fixture
            .0
            .join(format!("{nome}{}", if cfg!(windows) { ".exe" } else { "" }));
        dartforge_native::build_executable(&ir, &executavel, &opcoes).unwrap();
        let aot = std::process::Command::new(&executavel).output().unwrap();
        assert!(aot.status.success(), "AOT falhou em {nome}");
        let aot = String::from_utf8(aot.stdout).unwrap().replace("\r\n", "\n");
        let jit = dartforge_asmjit_jit::compilar(&modulo)
            .unwrap()
            .executar_capturando();
        assert_eq!(jit, aot, "JIT e AOT divergiram em {nome}");
        assert_eq!(jit, esperado, "ambos divergiram do Dart 3.6.2 em {nome}");
    }
}

/// Recarga: cada compilação é um bloco próprio e a versão antiga continua válida.
///
/// É o eixo 4 de `docs/ASMJIT.md` reduzido a um teste. O `dynasmrt` não oferece
/// redefinição in-place de uma função já publicada: o `ExecutableBuffer` é
/// imutável depois do `finalize`. A recarga possível hoje é montar a versão nova
/// num bloco novo; as duas coexistem e a memória de cada uma é devolvida ao
/// sistema quando o seu `ProgramaCompilado` é destruído.
#[test]
fn recompilar_cria_bloco_novo_e_mantem_o_antigo_valido() {
    const V1: &str = "int valor() {\n  return 1;\n}\n\nvoid main() {\n  print(valor());\n}\n";
    const V2: &str = "int valor() {\n  return 2;\n}\n\nvoid main() {\n  print(valor());\n}\n";
    let modulo_v1 = hir(V1).unwrap();
    let modulo_v2 = hir(V2).unwrap();
    let v1 = dartforge_asmjit_jit::compilar(&modulo_v1).unwrap();
    let v2 = dartforge_asmjit_jit::compilar(&modulo_v2).unwrap();
    assert_eq!(v2.executar_capturando(), "2\n");
    // A versão antiga não foi invalidada nem redirecionada pela nova.
    assert_eq!(v1.executar_capturando(), "1\n");
    drop(v2);
    assert_eq!(v1.executar_capturando(), "1\n");
}

/// Liberar o bloco antigo e montar outro em seguida não reaproveita código vivo.
///
/// Muitas recargas seguidas é o cenário do perfil de desenvolvimento. O teste
/// prova que a sequência montar/executar/descartar é estável: se o alocador do
/// `dynasmrt` reciclasse páginas ainda referenciadas, alguma iteração
/// imprimiria o valor de outra.
#[test]
fn recargas_sucessivas_nao_reaproveitam_codigo_vivo() {
    let mut vivos = Vec::new();
    for esperado in 1..=8_i64 {
        let fonte = format!(
            "int valor() {{\n  return {esperado};\n}}\n\nvoid main() {{\n  print(valor());\n}}\n"
        );
        let modulo = hir(&fonte).unwrap();
        let compilado = dartforge_asmjit_jit::compilar(&modulo).unwrap();
        assert_eq!(compilado.executar_capturando(), format!("{esperado}\n"));
        vivos.push((esperado, compilado));
        // Todas as versões anteriores continuam corretas depois de cada montagem.
        for (valor, antigo) in &vivos {
            assert_eq!(antigo.executar_capturando(), format!("{valor}\n"));
        }
    }
}

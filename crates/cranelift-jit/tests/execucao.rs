//! Execução da fatia escalar pelo JIT Cranelift e acordo com o backend LLVM/AOT.
//!
//! As saídas esperadas foram obtidas rodando os mesmos programas na VM do Dart
//! 3.6.2 (`dart run`), que é o oráculo do subconjunto nativo: inteiro de 64 bits
//! com estouro modular. Os testes marcados `#[ignore]` comparam a saída do JIT
//! com a do executável AOT produzido por Clang e rustc; sem essas ferramentas no
//! PATH eles não rodam, e o acordo entre os backends é verificado pelo teste que
//! confere que ambos aceitam exatamente o mesmo corpus.
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
const CONTROLE_ESPERADO: &str = "12\n2\n32\n-7\ntrue\nfalse\nfalse\ntrue\nfalse\n1\n0\n1\n2\n0\n20\n30\n100\n101\n";

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

/// Laço pesado usado no experimento de tempo de execução do código gerado.
const SOMATORIO: &str = r"
int somatorio(int n) {
  int total = 0;
  for (int i = 0; i < n; i = i + 1) {
    total = total + i;
  }
  return total;
}

void main() {
  print(somatorio(100000000));
}
";
const SOMATORIO_ESPERADO: &str = "4999999950000000\n";

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

/// Programas da fatia coberta, com a saída conferida na VM do Dart 3.6.2.
fn corpus() -> [(&'static str, &'static str, &'static str); 5] {
    [
        ("controle", CONTROLE, CONTROLE_ESPERADO),
        ("recursao", RECURSAO, RECURSAO_ESPERADO),
        ("estouro", ESTOURO, ESTOURO_ESPERADO),
        ("somatorio", SOMATORIO, SOMATORIO_ESPERADO),
        ("arrow", ARROW, ARROW_ESPERADO),
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
    Ok(dartforge_cranelift_jit::compilar(&modulo)?.executar_capturando())
}

/// Mensagem e trecho exato apontado pelo span de uma recusa do JIT.
fn recusa(fonte: &str) -> (String, &str) {
    let modulo = hir(fonte).expect("o frontend aceita o programa; quem recusa é o backend");
    let erro = dartforge_cranelift_jit::compilar(&modulo)
        .err()
        .expect("o programa está fora da fatia e deveria ser recusado");
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
#[test]
fn recursao_simples_de_arvore_e_mutua() {
    assert_eq!(executar(RECURSAO).unwrap(), RECURSAO_ESPERADO);
}

/// `int` é i64 com estouro modular, igual ao backend LLVM e à VM do Dart.
#[test]
fn inteiro_e_i64_com_estouro_modular() {
    assert_eq!(executar(ESTOURO).unwrap(), ESTOURO_ESPERADO);
}

/// Executar o mesmo programa duas vezes não acumula nem perde saída.
#[test]
fn a_execucao_e_repetivel_no_mesmo_programa_compilado() {
    let modulo = hir(CONTROLE).unwrap();
    let compilado = dartforge_cranelift_jit::compilar(&modulo).unwrap();
    assert_eq!(compilado.executar_capturando(), CONTROLE_ESPERADO);
    assert_eq!(compilado.executar_capturando(), CONTROLE_ESPERADO);
}

/// As medições existem, são coerentes entre si e contam trabalho de verdade.
#[test]
fn as_medicoes_separam_traducao_de_geracao_de_codigo() {
    let modulo = hir(CONTROLE).unwrap();
    let compilado = dartforge_cranelift_jit::compilar(&modulo).unwrap();
    let medicoes = compilado.medicoes();
    assert!(medicoes.instrucoes_clif > 0, "{medicoes:?}");
    assert!(medicoes.bytes_codigo > 0, "{medicoes:?}");
    assert!(medicoes.total >= medicoes.traducao, "{medicoes:?}");
    assert!(medicoes.total >= medicoes.geracao, "{medicoes:?}");
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
        let compilado = dartforge_cranelift_jit::compilar(&modulo);
        assert!(
            compilado.is_ok(),
            "Cranelift recusou {nome}: {:?}",
            compilado.err()
        );
    }
}

/// Formas fora de ambos os backends são recusadas no mesmo ponto do programa.
///
/// A mensagem difere pelo prefixo — cada backend diz que foi ele quem recusou —
/// mas o span tem de ser o mesmo, senão um dos dois está apontando errado.
#[test]
fn jit_e_llvm_recusam_o_mesmo_ponto_do_programa() {
    const CASOS: [&str; 5] = [
        "void main() {\n  int x = 7 ~/ 2;\n  print(x);\n}\n",
        "void main() {\n  int x = 7 % 2;\n  print(x);\n}\n",
        "void main() {\n  print(1 > 0 ? 2 : 3);\n}\n",
        "void main() {\n  try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }\n}\n",
        "void main() {\n  assert(1 > 0);\n  print(1);\n}\n",
    ];
    for fonte in CASOS {
        let modulo = hir(fonte).unwrap();
        let llvm = dartforge_llvm::emit(&modulo)
            .err()
            .unwrap_or_else(|| panic!("LLVM aceitou o que deveria recusar: {fonte}"));
        let cranelift = dartforge_cranelift_jit::compilar(&modulo)
            .err()
            .unwrap_or_else(|| panic!("Cranelift aceitou o que deveria recusar: {fonte}"));
        assert_eq!(llvm.span, cranelift.span, "spans divergentes em {fonte}");
        assert!(
            llvm.message.starts_with("LLVM AOT ainda não suporta "),
            "{}",
            llvm.message
        );
        assert!(
            cranelift
                .message
                .starts_with("Cranelift JIT ainda não suporta "),
            "{}",
            cranelift.message
        );
    }
}

/// Cada forma fora da fatia tem mensagem e span exatos, nunca aceite silencioso.
///
/// O segundo elemento de cada par é o texto que o span recorta da fonte: é
/// assim que o teste prova que o diagnóstico aponta para o lugar certo.
#[test]
fn cada_forma_fora_da_fatia_tem_mensagem_e_span_exatos() {
    let casos: [(&str, &str, &str); 16] = [
        (
            "void main() {\n  double x = 1.5;\n  print(x);\n}\n",
            "Cranelift JIT ainda não suporta double e num",
            "double x = 1.5;",
        ),
        (
            "void main() {\n  String s = 'a';\n  print(s);\n}\n",
            "Cranelift JIT ainda não suporta strings",
            "String s = 'a';",
        ),
        (
            "void main() {\n  int? x = 1;\n  print(x);\n}\n",
            "Cranelift JIT ainda não suporta tipos anuláveis",
            "int? x = 1;",
        ),
        (
            "void main() {\n  int x = 7 ~/ 2;\n  print(x);\n}\n",
            "Cranelift JIT ainda não suporta divisão double e truncada (`/`, `~/`)",
            "7 ~/ 2",
        ),
        (
            "void main() {\n  int x = 7 % 2;\n  print(x);\n}\n",
            "Cranelift JIT ainda não suporta módulo euclidiano",
            "7 % 2",
        ),
        (
            "void main() {\n  print(1 > 0 ? 2 : 3);\n}\n",
            "Cranelift JIT ainda não suporta o operador condicional",
            "1 > 0 ? 2 : 3",
        ),
        (
            "void main() {\n  var l = [1, 2];\n  print(l[0]);\n}\n",
            "Cranelift JIT ainda não suporta coleções",
            "[1, 2]",
        ),
        (
            "void main() {\n  switch (1) {\n    case 1:\n      print(1);\n  }\n}\n",
            "Cranelift JIT ainda não suporta switch",
            "switch (1) {\n    case 1:\n      print(1);\n  }",
        ),
        (
            "void main() {\n  try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }\n}\n",
            "Cranelift JIT ainda não suporta try, catch, finally e rethrow",
            "try {\n    print(1);\n  } catch (e) {\n    print(2);\n  }",
        ),
        (
            "void main() {\n  assert(1 > 0);\n  print(1);\n}\n",
            "Cranelift JIT ainda não suporta assert",
            "assert(1 > 0);",
        ),
        (
            "void main() {\n  for (final x in [1, 2]) {\n    print(x);\n  }\n}\n",
            "Cranelift JIT ainda não suporta for-in",
            "for (final x in [1, 2]) {\n    print(x);\n  }",
        ),
        (
            "class C {\n  int f = 1;\n}\n\nvoid main() {\n  print(1);\n}\n",
            "Cranelift JIT ainda não suporta classes, enums e membros estáticos",
            "class C {\n  int f = 1;\n}",
        ),
        (
            "int g = 1;\n\nvoid main() {\n  print(g);\n}\n",
            "Cranelift JIT ainda não suporta variáveis de topo",
            "int g = 1;",
        ),
        (
            "int f([int a = 1]) {\n  return a;\n}\n\nvoid main() {\n  print(f(2));\n}\n",
            "Cranelift JIT ainda não suporta parâmetros opcionais ou nomeados",
            "int a = 1",
        ),
        (
            "void main() {\n  int x = 1;\n  return;\n  print(x ~/ 2);\n}\n",
            "Cranelift JIT ainda não suporta divisão double e truncada (`/`, `~/`)",
            "x ~/ 2",
        ),
        (
            "void main() {\n  print('a' + 'b');\n}\n",
            "Cranelift JIT ainda não suporta strings",
            "'a'",
        ),
    ];
    for (fonte, mensagem, trecho) in casos {
        let (obtida, recorte) = recusa(fonte);
        assert_eq!(obtida, mensagem, "programa:\n{fonte}");
        assert_eq!(recorte, trecho, "programa:\n{fonte}");
    }
}

/// Código morto também é validado: nada fora da fatia passa por não executar.
#[test]
fn codigo_morto_depois_do_return_ainda_e_recusado() {
    let (mensagem, _) = recusa("void main() {\n  return;\n  print(1.5);\n}\n");
    assert_eq!(
        mensagem,
        "Cranelift JIT ainda não suporta literais double"
    );
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
        let caminho = std::env::temp_dir().join(format!(
            "dartforge-cranelift-{}-{marca}",
            std::process::id()
        ));
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
/// É o contrato central do experimento — escolher o backend do perfil de
/// desenvolvimento só faz sentido se os dois caminhos concordarem no resultado.
#[test]
#[ignore = "requer Clang e rustc/linker nativos ou DARTFORGE_CLANG/DARTFORGE_RUSTC"]
fn a_saida_do_jit_bate_com_a_do_executavel_aot() {
    let fixture = Fixture::new();
    let opcoes = dartforge_native::NativeOptions::default();
    for (nome, fonte, esperado) in corpus() {
        let modulo = hir(fonte).unwrap();
        let ir = dartforge_llvm::emit(&modulo).unwrap();
        let executavel = fixture.0.join(format!(
            "{nome}{}",
            if cfg!(windows) { ".exe" } else { "" }
        ));
        dartforge_native::build_executable(&ir, &executavel, &opcoes).unwrap();
        let aot = std::process::Command::new(&executavel).output().unwrap();
        assert!(aot.status.success(), "AOT falhou em {nome}");
        let aot = String::from_utf8(aot.stdout).unwrap().replace("\r\n", "\n");
        let jit = dartforge_cranelift_jit::compilar(&modulo)
            .unwrap()
            .executar_capturando();
        assert_eq!(jit, aot, "JIT e AOT divergiram em {nome}");
        assert_eq!(jit, esperado, "ambos divergiram do Dart 3.6.2 em {nome}");
    }
}

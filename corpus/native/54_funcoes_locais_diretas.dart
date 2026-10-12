// Funções locais que não escapam viram chamadas diretas
// (`crates/emit_native/src/lower/funcoes_diretas.rs`): sem closure,
// ambiente nem célula. A semântica tem de ser a da VM em todos os jeitos de
// capturar — valor, variável escalar atribuída (pelo endereço), `Ref`
// atribuído (célula), captura também por uma closure que escapa — e nas
// que não podem ser diretas (tear-off, passada adiante, guardada,
// devolvida, chamada de uma closure que escapa, genérica, `async`).
import 'dart:async';

class Analisador {
  final String texto;
  int chamadas = 0;
  Analisador(this.texto);

  // O molde do `_HeaderValue._parse`: várias funções locais que leem e
  // gravam o mesmo índice.
  List<String> partes(String separador) {
    int index = 0;
    final saida = <String>[];
    bool fim() => index == texto.length;
    void pularEspacos() {
      while (!fim()) {
        if (texto[index] != ' ') return;
        index++;
      }
    }

    String valor() {
      chamadas++;
      int inicio = index;
      while (!fim()) {
        var c = texto[index];
        if (c == ' ' || c == separador) break;
        index++;
      }
      return texto.substring(inicio, index);
    }

    void esperar(String s) {
      if (fim() || texto[index] != s) {
        throw FormatException('esperava $s em $index');
      }
      index++;
    }

    while (!fim()) {
      pularEspacos();
      if (fim()) break;
      saida.add(valor());
      pularEspacos();
      if (!fim()) esperar(separador);
    }
    return saida;
  }
}

int fatorial(int n) {
  int passos = 0;
  int f(int k) {
    passos++;
    return k <= 1 ? 1 : k * f(k - 1);
  }

  final r = f(n);
  print('fatorial passos $passos');
  return r;
}

List<int Function()> escapam() {
  var contador = 0;
  int inc() => ++contador;
  // `inc` é lida como valor: escapa e é closure; o contador é célula.
  final fs = <int Function()>[inc, inc];
  inc();
  return fs;
}

void capturaModificadaDepois() {
  var x = 1;
  int ler() => x;
  print('antes ${ler()}');
  x = 2;
  print('depois ${ler()}');
  void gravar(int v) {
    x = v;
  }

  gravar(10);
  print('gravada $x ${ler()}');
}

void refAtribuido() {
  String s = 'a';
  void junta(String t) {
    s = s + t;
  }

  junta('b');
  junta('c');
  print('ref $s');
  List<int>? l;
  void cria() {
    l = [1, 2];
  }

  cria();
  print('lista $l');
}

void sombreamento() {
  var i = 1;
  int ler() => i;
  {
    var i = 100;
    print('sombra $i ${ler()}');
  }
  void outra() {
    var i = 50;
    print('outra $i ${ler()}');
  }

  outra();
}

void escapaPorClosure() {
  var n = 0;
  void soma() {
    n++;
  }

  // Chamada de dentro de uma closure que escapa: `soma` fica closure.
  final f = () => soma();
  f();
  f();
  soma();
  print('por closure $n');
}

void tambemCapturadaPorQueEscapa() {
  var total = 0;
  void somar(int v) {
    total += v;
  }

  // `total` também é capturado por uma closure que escapa: célula.
  final ver = () => total;
  somar(3);
  somar(4);
  print('compartilhada ${ver()} $total');
}

void laco() {
  final fs = <int Function()>[];
  for (var i = 0; i < 3; i++) {
    int dobro() => i * 2;
    fs.add(() => i);
    print('laco ${dobro()}');
  }
  print('closures ${fs.map((f) => f()).toList()}');
  var soma = 0;
  for (final x in [1, 2, 3]) {
    void acumula() {
      soma += x;
    }

    acumula();
  }
  print('soma $soma');
}

void aninhadas() {
  var nivel = 0;
  int fora(int a) {
    int dentro(int b) {
      nivel++;
      return a + b + nivel;
    }

    return dentro(1) + dentro(2);
  }

  print('aninhadas ${fora(10)} $nivel');
}

T primeiro<T>(List<T> xs) {
  var i = 0;
  T pega() => xs[i];
  bool vazio() => xs.isEmpty;
  if (vazio()) throw StateError('vazia');
  i = xs.length - 1;
  final ultimo = pega();
  i = 0;
  print('generica $ultimo ${ultimo is T}');
  return pega();
}

void genericaLocal() {
  // Função local genérica: continua closure (a tupla vem da entrada).
  List<T> repete<T>(T x, int n) => List<T>.filled(n, x);
  print('repete ${repete<String>('a', 2)} ${repete(1, 3).runtimeType}');
}

void excecoes() {
  var k = 0;
  void falha() {
    k++;
    throw ArgumentError('k=$k');
  }

  try {
    falha();
  } on ArgumentError catch (e) {
    print('pegou $e');
  }
  try {
    try {
      falha();
    } finally {
      print('finally $k');
    }
  } on ArgumentError catch (e) {
    print('fora $e');
  }
}

void tearOffEPassada() {
  var n = 0;
  void um() => n++;
  void dois() => n += 2;
  void aplica(void Function() f) => f();
  aplica(um);
  final guardada = dois;
  guardada();
  print('tear-off $n');
}

int Function() devolvida() {
  var n = 5;
  int f() => n++;
  f();
  return f;
}

void comLate() {
  late int tarde;
  int ler() => tarde;
  try {
    ler();
  } catch (e) {
    print('late ${e.runtimeType}');
  }
  tarde = 7;
  print('late ${ler()}');
}

Future<int> assincrona() async {
  var n = 0;
  void soma() {
    n++;
  }

  soma();
  await Future<void>.delayed(Duration.zero);
  soma();
  int dobro() => n * 2;
  return dobro();
}

Iterable<int> gerador() sync* {
  var n = 0;
  int prox() => ++n;
  yield prox();
  yield prox();
}

class Contador {
  int total = 0;
  void somaTudo(List<int> xs) {
    void um(int x) {
      total += x;
    }

    for (final x in xs) {
      um(x);
    }
  }
}

void main() async {
  final a = Analisador('um, dois ,tres');
  print(a.partes(','));
  print('chamadas ${a.chamadas}');
  try {
    Analisador('a b').partes(',');
  } on FormatException catch (e) {
    print('formato ${e.message}');
  }
  print(fatorial(6));
  final fs = escapam();
  print('escapam ${fs[0]()} ${fs[1]()}');
  capturaModificadaDepois();
  refAtribuido();
  sombreamento();
  escapaPorClosure();
  tambemCapturadaPorQueEscapa();
  laco();
  aninhadas();
  print(primeiro<num>([1, 2.5, 3]));
  try {
    primeiro<int>([]);
  } on StateError catch (e) {
    print('vazia ${e.message}');
  }
  genericaLocal();
  excecoes();
  tearOffEPassada();
  final d = devolvida();
  print('devolvida ${d()} ${d()}');
  comLate();
  print('async ${await assincrona()}');
  print('gerador ${gerador().toList()}');
  final c = Contador()..somaTudo([1, 2, 3]);
  print('this ${c.total}');
  var muitas = 0;
  for (var i = 0; i < 100000; i++) {
    int f(int x) => x + i;
    muitas = f(muitas) % 1000003;
  }
  print('muitas $muitas');
}

// late: campo inicializado depois, late final (segunda atribuição lança), inicializador preguiçoso, local, top-level.
late String topLevel;
late final int topLevelFinal;
late int topLevelPreguicoso = calcula('topLevelPreguicoso', 7);
late int globalReentrante = lerGlobalReentrante();

int lerGlobalReentrante() => globalReentrante;

int calcula(String nome, int v) {
  print('calculando $nome');
  return v;
}

class Servico {
  late String conexao;
  late final int porta;
  late int cache = calcula('cache', 99);
  late final List<int> dados = calcula('dados', 1) == 1 ? [1, 2, 3] : [];
  int leituras = 0;

  late int comThis = calcula('comThis', leituras + 10);

  void conecta(String host) {
    conexao = 'conectado a $host';
  }
}

class Preguicosa {
  final String nome;
  Preguicosa(this.nome);
  late final String pesado = _monta();
  String _monta() {
    print('montando para $nome');
    return 'valor de $nome';
  }
}

void main() {
  final s = Servico();
  try {
    print(s.conexao);
  } catch (e) {
    print('leitura antes de init: ${e is Error}');
  }
  s.conecta('host1');
  print(s.conexao);
  s.conecta('host2');
  print(s.conexao);

  s.porta = 8080;
  print(s.porta);
  try {
    s.porta = 9090;
    print('segunda atribuicao passou');
  } catch (e) {
    print('segunda atribuicao lancou: ${e is Error}');
  }
  print(s.porta);

  print('antes de ler cache');
  print(s.cache);
  print(s.cache);
  s.cache = 5;
  print(s.cache);
  final s2 = Servico();
  s2.cache = 1;
  print(s2.cache);

  print('antes de ler dados');
  print(s.dados);
  print(s.dados.length);
  s.leituras = 3;
  print(s.comThis);
  print(s.comThis);

  final p = Preguicosa('p');
  print('criada');
  print(p.pesado);
  print(p.pesado);
  final p2 = Preguicosa('q');
  print(p2.pesado);

  final condicao = calcula('condicao', 1) == 2;
  late int local;
  late final String localFinal;
  late String localPreguicoso = calcula('localPreguicoso', 3).toString();
  if (condicao) local = 0;
  try {
    print(local);
  } catch (e) {
    print('local antes de init: ${e is Error}');
  }
  local = 10;
  local += 5;
  print(local);
  if (!condicao) localFinal = 'a';
  print(localFinal);
  try {
    localFinal = 'b';
  } catch (e) {
    print('localFinal de novo: ${e is Error}');
  }
  print('antes de localPreguicoso');
  print(localPreguicoso);
  print(localPreguicoso);

  try {
    print(topLevel);
  } catch (e) {
    print('topLevel antes: ${e is Error}');
  }
  topLevel = 'top';
  print(topLevel);
  topLevelFinal = 1;
  print(topLevelFinal);
  try {
    topLevelFinal = 2;
  } catch (e) {
    print('topLevelFinal de novo: ${e is Error}');
  }
  print('antes de topLevelPreguicoso');
  print(topLevelPreguicoso);
  print(topLevelPreguicoso);

  var chamadas = 0;
  late final int contado = ++chamadas;
  print(chamadas);
  print(contado);
  print(contado);
  print(chamadas);

  late final int capturado;
  int lerCapturado() => capturado;
  void gravarCapturado(int valor) { capturado = valor; }
  try {
    print(lerCapturado());
  } catch (e) {
    print(e);
  }
  gravarCapturado(0);
  print(lerCapturado());
  try {
    gravarCapturado(2);
  } catch (e) {
    print(e);
  }
  print(lerCapturado());

  for (var i = 0; i < 2; i++) {
    try {
      print(globalReentrante);
    } catch (e) {
      print(e);
    }
  }
}

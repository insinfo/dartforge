// funções locais dentro de main, recursão (fatorial, fibonacci com memo), recursão mútua, captura de parâmetro.
int fatorial(int n) => n <= 1 ? 1 : n * fatorial(n - 1);

final memo = <int, int>{};
int chamadasFib = 0;

int fib(int n) {
  chamadasFib++;
  if (n < 2) return n;
  if (memo.containsKey(n)) return memo[n]!;
  final r = fib(n - 1) + fib(n - 2);
  memo[n] = r;
  return r;
}

bool ehPar(int n) => n == 0 ? true : ehImpar(n - 1);
bool ehImpar(int n) => n == 0 ? false : ehPar(n - 1);

int mdc(int a, int b) => b == 0 ? a : mdc(b, a % b);

int somaLista(List<int> xs) => xs.isEmpty ? 0 : xs.first + somaLista(xs.sublist(1));

String inverte(String s) => s.length <= 1 ? s : inverte(s.substring(1)) + s[0];

int potencia(int base, int exp) {
  if (exp == 0) return 1;
  final metade = potencia(base, exp ~/ 2);
  return exp.isEven ? metade * metade : metade * metade * base;
}

void main() {
  // funções locais
  int quadrado(int x) => x * x;
  String saudacao(String nome) {
    return 'olá, $nome';
  }

  print(quadrado(6));
  print(saudacao('mundo'));

  // função local capturando variável local
  var fator = 3;
  int multiplica(int x) => x * fator;
  print(multiplica(5));
  fator = 4;
  print(multiplica(5));

  // função local capturando parâmetro de outra função local
  List<int> Function(int) multiplicadorDe(int k) {
    List<int> aplicaEm(int n) => [for (var i = 1; i <= n; i++) i * k];
    return aplicaEm;
  }

  print(multiplicadorDe(2)(4));
  print(multiplicadorDe(7)(3));

  // função local recursiva
  int contagem(int n) => n == 0 ? 0 : 1 + contagem(n - 1);
  print(contagem(10));

  // função local recursiva com acumulador
  int somaAte(int n, [int acc = 0]) => n == 0 ? acc : somaAte(n - 1, acc + n);
  print(somaAte(100));

  // ordem de declaração: função local pode ser usada só depois de declarada
  var antes = 'declarada antes';
  String usaAntes() => antes;
  antes = 'modificada';
  print(usaAntes());

  // recursão de topo
  print(fatorial(10));
  print(fatorial(0));
  print(fib(30));
  print('chamadas fib: $chamadasFib');
  print('memo tem ${memo.length} entradas');
  print(fib(30));
  print('chamadas fib: $chamadasFib');
  print(ehPar(10));
  print(ehImpar(7));
  print(ehPar(7));
  print(mdc(48, 18));
  print(somaLista([1, 2, 3, 4]));
  print(inverte('recursão'));
  print(potencia(2, 20));
  print(potencia(3, 5));

  // recursão mútua com funções locais (declaração via variável)
  late bool Function(int) parLocal;
  late bool Function(int) imparLocal;
  parLocal = (n) => n == 0 ? true : imparLocal(n - 1);
  imparLocal = (n) => n == 0 ? false : parLocal(n - 1);
  print(parLocal(8));
  print(imparLocal(8));

  // função local chamando função local declarada antes
  int dobro(int x) => x * 2;
  int quadruplo(int x) => dobro(dobro(x));
  print(quadruplo(5));

  // função local com estado interno via closure
  var chamadas = 0;
  int conta() => ++chamadas;
  conta();
  conta();
  print('chamadas=${conta()}');

  // profundidade de recursão moderada
  int profundo(int n) => n == 0 ? 0 : 1 + profundo(n - 1);
  print(profundo(2000));

  // função local dentro de função local
  int externa(int a) {
    int interna(int b) => a + b;
    return interna(10) + interna(20);
  }

  print(externa(1));

  // hanoi contando movimentos
  var movimentos = 0;
  void hanoi(int n, String de, String para, String via) {
    if (n == 0) return;
    hanoi(n - 1, de, via, para);
    movimentos++;
    hanoi(n - 1, via, para, de);
  }

  hanoi(6, 'A', 'C', 'B');
  print('movimentos $movimentos');
}

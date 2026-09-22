// switch statement com padrões: constantes, guardas, records, listas, mapas, objetos, ||, tipos, null, relacionais.
class Ponto {
  final int x;
  final int y;
  Ponto(this.x, this.y);
}

class Circulo {
  final Ponto centro;
  final int raio;
  Circulo(this.centro, this.raio);
}

void classifica(Object? v) {
  switch (v) {
    case null:
      print('nulo');
    case 0:
      print('zero');
    case 1 || 2 || 3:
      print('pequeno constante');
    case int n when n < 0:
      print('negativo $n');
    case int n when n > 100:
      print('grande $n');
    case int n when n > 50:
      print('acima de 50');
    case int n:
      print('int qualquer $n');
    case 'oi':
      print('saudacao');
    case String s when s.isEmpty:
      print('string vazia');
    case String s:
      print('string ${s.length}');
    case (int a, int b):
      print('par de ints ${a + b}');
    case (int a, String b):
      print('int e string $a/$b');
    case (x: 0, y: var y):
      print('no eixo y em $y');
    case (x: var x, y: var y):
      print('nomeado $x,$y');
    case []:
      print('lista vazia');
    case [var unico]:
      print('lista de um: $unico');
    case [int a, int b]:
      print('dois ints $a $b');
    case [var primeiro, ...var resto]:
      print('lista: primeiro=$primeiro resto=$resto');
    case {'tipo': 'a'}:
      print('mapa tipo a');
    case {'tipo': var t, 'valor': int val}:
      print('mapa tipo $t valor $val');
    case {'tipo': var t}:
      print('mapa tipo $t');
    case Ponto(x: 0, y: 0):
      print('origem');
    case Ponto(x: 0):
      print('sobre y');
    case Ponto(:var x, :var y) when x == y:
      print('diagonal $x');
    case Ponto(x: var x, y: var y):
      print('ponto $x,$y');
    case Circulo(centro: Ponto(x: 0, y: 0), raio: var r):
      print('circulo na origem raio $r');
    case Circulo(raio: > 10):
      print('circulo grande');
    case Circulo():
      print('circulo qualquer');
    case bool b:
      print('bool $b');
    case _:
      print('outro');
  }
}

void main() {
  classifica(null);
  classifica(0);
  classifica(2);
  classifica(-5);
  classifica(150);
  classifica(75);
  classifica(42);
  classifica('oi');
  classifica('');
  classifica('abc');
  classifica((1, 2));
  classifica((1, 'b'));
  classifica((x: 0, y: 5));
  classifica((x: 3, y: 4));
  classifica(<int>[]);
  classifica([7]);
  classifica([1, 2]);
  classifica([1, 2, 3, 4]);
  classifica(['a', 'b']);
  classifica({'tipo': 'a'});
  classifica({'tipo': 'b', 'valor': 9});
  classifica({'tipo': 'c', 'valor': 'nao int'});
  classifica({'outro': 1});
  classifica(Ponto(0, 0));
  classifica(Ponto(0, 3));
  classifica(Ponto(4, 4));
  classifica(Ponto(1, 2));
  classifica(Circulo(Ponto(0, 0), 3));
  classifica(Circulo(Ponto(1, 1), 30));
  classifica(Circulo(Ponto(1, 1), 3));
  classifica(true);
  classifica(2.5);

  final valores = [1, 'dois', 3.5, null, [4]];
  for (final v in valores) {
    switch (v) {
      case int() || double():
        print('numero');
      case String():
        print('texto');
      case List():
        print('lista');
      default:
        print('desconhecido');
    }
  }

  const limite = 10;
  for (final n in [5, 10, 15]) {
    switch (n) {
      case < limite:
        print('$n abaixo');
      case == limite:
        print('$n igual');
      case > limite && < 20:
        print('$n entre');
    }
  }

  switch ((1, 2)) {
    case (var a, var b) when a > b:
      print('maior primeiro');
    case (var a, var b) when a < b:
      print('menor primeiro');
    case (_, _):
      print('iguais');
  }
}

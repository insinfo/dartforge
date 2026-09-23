// requer-dart: 3.13
// Atalhos de ponto (3.10): `.id` é `D.id`, com `D` a declaração denotada pelo
// tipo de contexto da cadeia inteira (anulável e FutureOr incluídos).
import 'dart:async';

enum Cor {
  vermelho,
  azul;

  static Cor get padrao => azul;
}

class P {
  final int x, y;
  const P(this.x, this.y);
  const P.origem()
      : x = 0,
        y = 0;
  static const P um = P(1, 1);
  static P de(int v) => P(v, v);
  factory P.fab(int v) => P(v, -v);
  @override
  String toString() => 'P($x,$y)';
}

String nome(Cor c) => switch (c) {
      .vermelho => 'V',
      .azul => 'A',
    };

void mostra(P p) => print(p);
void opcional([Cor c = .azul, P? p]) => print('$c ${p ?? 'sem p'}');
void nomeado({required Cor cor, P p = const .origem()}) => print('$cor $p');

Future<Cor> assincrona() async => .vermelho;
FutureOr<P> talvezFuturo() => .um;

void main() async {
  Cor c = .vermelho;
  print(nome(c));
  print(nome(.azul));
  Cor? cn = .padrao;
  print(cn);
  mostra(.new(2, 3));
  mostra(.origem());
  mostra(.um);
  mostra(.de(4));
  mostra(.fab(5));
  const P k = .origem();
  print(k);
  print(identical(k, const P.origem()));
  print(c == .vermelho);
  print(c != .azul);
  int i = .parse('42');
  print(i + 1);
  int j = .parse('-7').abs();
  print(j);
  List<P> ps = [.um, .origem()];
  print(ps);
  Duration d = .zero;
  print(d);
  List<int> l = .filled(3, 0);
  print(l);
  opcional();
  opcional(.vermelho, .um);
  nomeado(cor: .azul);
  nomeado(cor: .vermelho, p: .de(8));
  print(await assincrona());
  print(await talvezFuturo());
  if (c case .vermelho) print('if case');
  switch (cn) {
    case .vermelho:
      print('switch vermelho');
    case .azul:
      print('switch azul');
    case null:
      print('nulo');
  }
  var t = (Cor x) => x == .azul ? 'é azul' : 'não é azul';
  print(t(.azul));
  String s = .fromCharCode(65);
  print(s);
}

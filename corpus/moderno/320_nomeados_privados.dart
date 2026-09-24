// requer-dart: 3.13
// Parâmetros nomeados privados (3.12): `{this._x}` é chamado como `x:`; o
// nome externo (assinatura, tear-off) é o público; na lista de
// inicializadores `_x` é o parâmetro, no corpo é o campo; `super.x` encaminha
// com o nome público.
class Pt {
  final int _x, _y;
  final String origem;
  Pt({required this._x, this._y = 9})
      : origem = 'x=$_x',
        assert(_x >= 0);
  String get s => '$_x,$_y ($origem)';
}

class Sub extends Pt {
  Sub({required super.x}) {
    print('Sub corpo: $_x');
  }
}

class Casa {
  int? _janelas;
  int? _quartos;
  Casa({this._janelas, this._quartos});
  @override
  String toString() => 'Casa($_janelas, $_quartos)';
}

void main() {
  print(Pt(x: 1).s);
  print(Pt(x: 1, y: 2).s);
  print(Sub(x: 3).s);
  print(Casa(quartos: 2));
  print(Casa(janelas: 4, quartos: 1));
  var f = Pt.new;
  print(f(x: 7, y: 8).s);
  Pt Function({required int x, int y}) g = Pt.new;
  print(g(x: 5).s);
}

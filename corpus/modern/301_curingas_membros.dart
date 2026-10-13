// requer-dart: 3.13
// Curingas (3.7) contra membros: campo `_` continua nome; um local curinga
// não o esconde ("wildcards do not shadow"); `this._`/`super._` inicializam
// e encaminham sem ligar nome.
class C {
  var _ = 'campo';
  String teste() {
    var _ = 'local';
    _ = 'atribuído';
    return _;
  }
}

class A {
  final int x, y;
  A(this.x, this.y);
}

class B extends A {
  final int _;
  B(this._, super._, super._) {
    print('B: ${_ + x + y}');
  }
}

class D extends A {
  // `super.x` liga `x` no inicializador; `super._` não liga nada.
  D(super.x, super._) : assert(x > 0) {
    print('D: $x $y');
  }
}

class Campo {
  final int _;
  Campo(this._);
  int get valor => _ * 2;
}

void main() {
  print(C().teste());
  B(1, 20, 300);
  D(4, 5);
  print(Campo(21).valor);
}

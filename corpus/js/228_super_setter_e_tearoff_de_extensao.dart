// `super.x = v` para o setter da superclasse (dentro de outro setter) e o
// tear-off de um método de extensão (`21.dobro`), casos do
// package:string_scanner e do package:async.
class Base {
  int _p = 0;
  int get position => _p;
  set position(int p) {
    print('base set $p');
    _p = p;
  }
}

class Filha extends Base {
  int linha = 0;
  @override
  set position(int p) {
    linha++;
    super.position = p;
  }
  set estado(int e) {
    super.position = e * 2;
  }
}

extension Dobro on int {
  int dobro() => this * 2;
}

void main() {
  final f = Filha();
  f.position = 3;
  f.estado = 5;
  print('${f.position} ${f.linha}');
  final t = 21.dobro;
  print(t());
  print([1, 2].map(3.dobro == null ? (x) => x : (x) => x + 1).toList());
}

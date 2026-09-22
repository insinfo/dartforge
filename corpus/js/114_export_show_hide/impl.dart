// Implementação: só X e y são reexportados por api.dart; Interno, z e escondida ficam ocultos.
class X {
  final int v;
  X(this.v);
  @override
  String toString() => 'X($v)';
  Interno interno() => Interno(v);
}

class Interno {
  final int v;
  Interno(this.v);
  @override
  String toString() => 'Interno($v)';
}

int y(int a) => a * 10;

int z(int a) => a * 100;

const escondida = 'não exportada';

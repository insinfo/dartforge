// Biblioteca a: importa b (que importa a). Funções mutuamente recursivas e top-level preguiçoso que imprime.
import 'b.dart';

bool ehPar(int n) => n == 0 ? true : ehImpar(n - 1);

String pingA(int n) => n <= 0 ? 'a' : 'a>${pongB(n - 1)}';

int calculaA() {
  print('  inicializando xA');
  return 10;
}

final xA = calculaA();

final yA = () {
  print('  inicializando yA (lê xB)');
  return xB + 1;
}();

class NoA {
  final NoB? filho;
  NoA(this.filho);
  int profundidade() => 1 + (filho?.profundidade() ?? 0);
}

// Biblioteca b: importa a (ciclo). Funções mutuamente recursivas com a e top-level preguiçoso que lê xA.
import 'a.dart';

bool ehImpar(int n) => n == 0 ? false : ehPar(n - 1);

String pongB(int n) => n <= 0 ? 'b' : 'b>${pingA(n - 1)}';

int calculaB() {
  print('  inicializando xB (lê xA)');
  return xA * 2;
}

final xB = calculaB();

int contadorB = 0;

int proximoB() => ++contadorB;

class NoB {
  final NoA? filho;
  NoB(this.filho);
  int profundidade() => 1 + (filho?.profundidade() ?? 0);
}

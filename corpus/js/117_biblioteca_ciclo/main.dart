// Ciclo de imports (a <-> b): funções mutuamente dependentes entre bibliotecas e ordem de inicialização preguiçosa de top-levels encadeados.
import 'a.dart';
import 'b.dart' as b;

void main() {
  print('main início (nada inicializado ainda)');
  print(ehPar(10));
  print(ehPar(7));
  print(b.ehImpar(7));
  print(pingA(3));
  print(b.pongB(2));
  print('--');
  print('lendo yA:');
  print(yA);
  print('lendo xB de novo (já inicializado):');
  print(b.xB);
  print('lendo xA:');
  print(xA);
  print('--');
  final arvore = NoA(b.NoB(NoA(b.NoB(null))));
  print(arvore.profundidade());
  print(b.NoB(null).profundidade());
  print(b.proximoB());
  print(b.proximoB());
  print(b.contadorB);
  print('fim');
}

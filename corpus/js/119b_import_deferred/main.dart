// import deferred as: loadLibrary() antes de usar, chamada dupla de loadLibrary, top-level da lib deferida inicializado após carregar.
import 'util.dart' deferred as tarde;

Future<void> main() async {
  print('antes de carregar');
  await tarde.loadLibrary();
  print('carregado');
  print(tarde.proximo());
  print(tarde.proximo());
  print(tarde.Ponto(3, 4).distancia().toStringAsFixed(1));
  print(tarde.origem);
  print(tarde.maximo([1, 5, 2]));
  await tarde.loadLibrary();
  print('carregar de novo não reinicializa: ${tarde.contador}');
  print('fim');
}

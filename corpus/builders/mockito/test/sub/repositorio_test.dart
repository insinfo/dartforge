// Mock "nice" num subdiretório: a captura {{}} leva o caminho junto
// (test/sub/x_test.dart -> test/mocks/sub/x_test.mocks.dart).
import 'package:corpus_mockito/servicos.dart';
import 'package:mockito/annotations.dart';
import 'package:mockito/mockito.dart';

import '../apoio.dart';
import '../mocks/sub/repositorio_test.mocks.dart';

@GenerateNiceMocks([MockSpec<Repositorio<int>>()])
Future<void> main() async {
  final repositorio = MockRepositorio();
  imprimir(await repositorio.buscar(1));
  imprimir(repositorio.tamanho);
  when(repositorio.tamanho).thenReturn(3);
  imprimir(repositorio.tamanho);
  await repositorio.salvar(1, 10);
  imprimir(verify(repositorio.salvar(1, 10)).callCount);
  imprimir('repositorio ok');
}

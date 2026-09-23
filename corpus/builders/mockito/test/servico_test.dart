// Roda sem package:test: `main` imprime; `verify(...).callCount` em vez de
// `.called(n)`, que usa `expect` e exige estar dentro de um teste.
import 'package:corpus_mockito/servicos.dart';
import 'package:mockito/annotations.dart';
import 'package:mockito/mockito.dart';

import 'apoio.dart';
import 'mocks/servico_test.mocks.dart';

@GenerateMocks([Relogio], customMocks: [MockSpec<Repositorio<String>>(as: #MockRepositorioTexto)])
Future<void> main() async {
  final repositorio = MockRepositorioTexto();
  final relogio = MockRelogio();
  when(repositorio.buscar(7)).thenAnswer((_) async => 'sete');
  when(relogio.agora()).thenReturn(42);
  final servico = Servico(repositorio, relogio);
  imprimir(await servico.descrever(7));
  imprimir(await servico.descrever(7, prefixo: 'n', limite: 6));
  imprimir(verify(repositorio.buscar(7)).callCount);
  verifyNever(repositorio.salvar(any, any));
  imprimir('servico ok');
}

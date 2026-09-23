// Os mocks vivem em test/; o programa roda os dois "testes" em sequência.
import '../test/servico_test.dart' as servico;
import '../test/sub/repositorio_test.dart' as repositorio;

Future<void> main() async {
  await servico.main();
  await repositorio.main();
}

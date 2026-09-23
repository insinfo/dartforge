// Saída determinística: lê e altera os provedores gerados num container.
import 'package:corpus_riverpod_generator/provedores.dart';
import 'package:riverpod/riverpod.dart';

Future<void> main() async {
  final container = ProviderContainer();
  print(container.read(saudacaoProvider));
  print(container.read(saudarProvider('Ana', vezes: 2)));
  print(await container.read(dobroProvider(21).future));
  final contador = contadorProvider(5);
  container.read(contador.notifier).incrementar();
  container.read(contador.notifier).incrementar();
  print(container.read(contador));
  print('${saudarProvider('Ana') == saudarProvider('Ana')} ${contador.name} ${saudarProvider.name}');
  container.dispose();
}

// riverpod_generator em Dart puro: provedor funcional, com família, assíncrono
// e de classe (Notifier), com keepAlive e dependência entre provedores.
// riverpod_annotation 2.6 não exporta `Ref`; em Dart puro ele vem do riverpod.
import 'package:riverpod/riverpod.dart' show Ref;
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'provedores.g.dart';

@riverpod
String saudacao(Ref ref) => 'olá';

@riverpod
String saudar(Ref ref, String nome, {int vezes = 1}) =>
    List.filled(vezes, '${ref.watch(saudacaoProvider)}, $nome').join(' / ');

@Riverpod(keepAlive: true)
Future<int> dobro(Ref ref, int valor) async => valor * 2;

@riverpod
class Contador extends _$Contador {
  @override
  int build(int inicio) => inicio;

  void incrementar() => state++;
}

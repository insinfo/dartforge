// Escreve, para cada biblioteca Dart do dependente, o nome do builder, as
// opções recebidas (JSON com chaves ordenadas) e a primeira linha da entrada.
import 'dart:convert';

import 'package:build/build.dart';

Builder carimbo(BuilderOptions opcoes) => _Carimbo(opcoes);

class _Carimbo implements Builder {
  final BuilderOptions opcoes;
  _Carimbo(this.opcoes);

  @override
  Map<String, List<String>> get buildExtensions => const {
        '.dart': ['.carimbo.txt'],
      };

  @override
  Future<void> build(BuildStep passo) async {
    final texto = await passo.readAsString(passo.inputId);
    final chaves = opcoes.config.keys.toList()..sort();
    final json = jsonEncode({for (final k in chaves) k: opcoes.config[k]});
    await passo.writeAsString(
        passo.inputId.changeExtension('.carimbo.txt'),
        'builder: carimbo\n'
        'entrada: ${passo.inputId}\n'
        'isRoot: ${opcoes.isRoot}\n'
        'opcoes: $json\n'
        '---\n'
        '${const LineSplitter().convert(texto).first}\n');
  }
}

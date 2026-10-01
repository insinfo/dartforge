// Mapas e conjuntos com chaves de hash de identidade atravessando isolados:
// o índice da origem não vale no destino e é refeito, como a VM faz
// (`object_graph_copy.cc`, `CopyLinkedHashBase`). O `pdf` monta o documento
// e chama `save()` dentro de `Isolate.run` (docs/NATIVO-PROJETOS-REAIS.md,
// C20).

import 'dart:isolate';

class Chave {
  final String nome;
  Chave(this.nome);
  @override
  String toString() => 'Chave($nome)';
}

class Doc {
  final List<Chave> chaves = [Chave('a'), Chave('b'), Chave('c')];
  final Map<Chave, int> pesos = {};
  final Set<Chave> marcadas = {};
  final Map<String, int> porNome = {'x': 1, 'y': 2};
  Doc() {
    for (var i = 0; i < chaves.length; i++) {
      pesos[chaves[i]] = i * 10;
    }
    marcadas.add(chaves[1]);
    final extra = Chave('apagada');
    pesos[extra] = 99;
    pesos.remove(extra);
  }

  String resumo() {
    final p = chaves.map((c) => '${c.nome}=${pesos[c]}').join(',');
    final m = chaves.map((c) => marcadas.contains(c)).join(',');
    return '$p | $m | ${pesos.length} | ${porNome['y']}';
  }
}

Future<void> main() async {
  final d = Doc();
  print(d.resumo());
  print(await Isolate.run(() => d.resumo()));
  final volta = await Isolate.run(() {
    final n = Doc();
    n.pesos[n.chaves[2]] = 7;
    return n;
  });
  print(volta.resumo());
  volta.pesos[Chave('nova')] = 5;
  print(volta.pesos.length);
  print(volta.pesos.containsKey(volta.chaves[0]));

  final porta = ReceivePort();
  await Isolate.spawn((SendPort s) {
    final k = Chave('k');
    s.send([k, {k: 'achou'}, {k}]);
  }, porta.sendPort);
  final msg = await porta.first as List;
  final k = msg[0] as Chave;
  print((msg[1] as Map)[k]);
  print((msg[2] as Set).contains(k));
}

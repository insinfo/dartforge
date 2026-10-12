// Pressão de coleta no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.7,
// §2.8, §5.3): milhões de strings, listas e caixas (`double`, `int` fora do Smi)
// curtas e longas; estruturas que sobrevivem e que morrem a cada rodada;
// gravação velho→jovem em listas, mapas e closures (barreira e cartões); tudo
// conferido no fim. Dimensionado para rodar com `--gc-stress` dentro de
// `--limite-exec 60`: ~1 s na VM.

const grande = 4611686018427387904; // 2^62, fora do Smi

class No {
  final int id;
  Object? carga;
  No? prox;
  No(this.id, this.carga);
}

/// Valor esperado da carga de índice i gravada na rodada r.
Object esperado(int r, int i) {
  switch (i % 5) {
    case 0:
      return 'r$r-i$i';
    case 1:
      return r * 1000.0 + i + 0.5;
    case 2:
      return grande + r * 100000 + i;
    case 3:
      return [r, i, 's$i'];
    default:
      return No(r * 100000 + i, 'n$i');
  }
}

bool confere(Object? v, int r, int i) {
  final e = esperado(r, i);
  if (v is List) {
    return e is List && v.length == 3 && v[0] == e[0] && v[1] == e[1] && v[2] == e[2];
  }
  if (v is No) return e is No && v.id == e.id && v.carga == e.carga;
  return v == e;
}

/// Lixo de vida curta: devolve um resumo para não ser eliminado.
int rodadaDeLixo(int r) {
  var h = r;
  for (var i = 0; i < 15000; i++) {
    final s = 'lixo $i ${r * i}';
    final d = i * 0.25 + r;
    final g = grande + i;
    final l = <Object>[s, d, g, i];
    final longa = i % 500 == 0 ? List<int>.filled(3000, i) : const <int>[];
    h = (h * 31 + s.length + (l[1] as double).toInt() + (g - grande) + longa.length) & 0x3FFFFFFF;
  }
  final longo = StringBuffer();
  for (var i = 0; i < 400; i++) {
    longo.write('bloco $i;');
  }
  h = (h + longo.toString().length) & 0x3FFFFFFF;
  return h;
}

void main() {
  const n = 20000;
  const rodadas = 30;

  // Estruturas velhas, criadas antes de tudo.
  final velhaLista = List<Object?>.filled(n, null);
  final velhoMapa = <int, Object>{};
  final velhaCadeia = List<No>.generate(n, (i) => No(i, null));
  for (var i = 1; i < n; i++) {
    velhaCadeia[i - 1].prox = velhaCadeia[i];
  }
  final ultimaRodada = List<int>.filled(n, -1);
  final ultimaRodadaMapa = <int, int>{};
  var capturado = <Object>[];
  var somaCaptura = 0.0;
  final closures = <Object Function()>[];

  var resumoLixo = 0;
  var sobreviventes = <String>[];
  for (var r = 0; r < rodadas; r++) {
    resumoLixo = (resumoLixo + rodadaDeLixo(r)) & 0x3FFFFFFF;

    // Gravações velho→jovem em pontos espalhados.
    for (var k = 0; k < 1500; k++) {
      final i = (r * 4099 + k * 13) % n;
      velhaLista[i] = esperado(r, i);
      ultimaRodada[i] = r;
      final j = (r * 7 + k * 31) % (n * 2);
      velhoMapa[j] = esperado(r, j);
      ultimaRodadaMapa[j] = r;
      velhaCadeia[(i * 3) % n].carga = 'c$r-${(i * 3) % n}';
    }

    // Uma estrutura que sobrevive uma rodada e morre na seguinte.
    final proximos = List<String>.generate(2000, (i) => 'sobrevive $r/$i ${'Ā' * (i % 4)}');
    if (sobreviventes.isNotEmpty) {
      final r0 = r - 1;
      for (var i = 0; i < sobreviventes.length; i += 97) {
        if (sobreviventes[i] != 'sobrevive $r0/$i ${'Ā' * (i % 4)}') {
          throw StateError('sobrevivente errado na rodada $r: $i');
        }
      }
    }
    sobreviventes = proximos;

    // Closures velhas que capturam valores novos.
    final valor = r * 1.5;
    final lista = [valor, grande + r, 'cap$r'];
    capturado = lista;
    closures.add(() => capturado);
    closures.add(() => lista);
    somaCaptura += valor;

    // Uma lista longa por rodada, parte mantida.
    if (r % 5 == 0) {
      velhoMapa[-r - 1] = List<double>.generate(5000, (i) => i + r / 10);
      ultimaRodadaMapa[-r - 1] = r;
    }
  }

  // Conferência.
  var erros = 0, preenchidos = 0;
  for (var i = 0; i < n; i++) {
    final r = ultimaRodada[i];
    if (r < 0) {
      if (velhaLista[i] != null) erros++;
      continue;
    }
    preenchidos++;
    if (!confere(velhaLista[i], r, i)) erros++;
  }
  var chavesMapa = 0;
  ultimaRodadaMapa.forEach((j, r) {
    chavesMapa++;
    final v = velhoMapa[j];
    if (j < 0) {
      if (v is! List<double> || v.length != 5000 || v[4999] != 4999 + r / 10) erros++;
    } else if (!confere(v, r, j)) {
      erros++;
    }
  });
  if (velhoMapa.length != chavesMapa) erros++;
  var noCarga = 0, passos = 0;
  for (No? no = velhaCadeia[0]; no != null; no = no.prox) {
    passos++;
    final c = no.carga;
    if (c is String) {
      noCarga++;
      if (!c.endsWith('-${no.id}')) erros++;
    } else if (c != null) {
      erros++;
    }
  }
  var somaClosures = 0.0;
  for (var k = 0; k < closures.length; k += 2) {
    final atual = closures[k]() as List;
    final propria = closures[k + 1]() as List;
    if (!identical(atual, capturado)) erros++;
    somaClosures += propria[0] as double;
    if (propria[1] != grande + k ~/ 2 || propria[2] != 'cap${k ~/ 2}') erros++;
  }
  if (somaClosures != somaCaptura) erros++;
  print('lista velha: preenchidos=$preenchidos');
  print('mapa velho: ${velhoMapa.length} chaves');
  print('cadeia: $passos nós, $noCarga com carga');
  print('closures: ${closures.length} soma=$somaClosures');
  print('sobreviventes da última rodada: ${sobreviventes.length} ${sobreviventes.last}');
  print('resumo do lixo: $resumoLixo');
  print('erros: $erros');
}

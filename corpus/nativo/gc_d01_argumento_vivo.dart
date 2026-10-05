// D1 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): o argumento vivo na
// chamada que coleta (contrato C1). `a + b` de dois textos recém-criados e
// `List.add` de objeto novo, em laço, com `--gc-stress`: o runtime conta com
// quem chama para manter os operandos vivos enquanto a chamada aloca.
// Sabotagens que o derrubam: `sem_uso_ficticio` (o mapa perde os operandos)
// e `folha:dartforge_string_concat` (D7: a concatenação vira folha e as
// chamadas a ela saem do mapa).

class Caixa {
  final int v;
  Caixa(this.v);
}

void main() {
  var total = 0;
  final lista = <Caixa>[];
  for (var i = 0; i < 2000; i++) {
    final a = 'a$i';
    final b = 'b${i * 2}';
    final c = a + b;
    total += c.length;
    lista.add(Caixa(i));
  }
  var soma = 0;
  for (final x in lista) {
    soma += x.v;
  }
  print('$total $soma ${lista.length}');
}

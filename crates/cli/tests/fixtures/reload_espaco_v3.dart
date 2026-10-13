// Reload with the state of the unified space (crates/cli/tests/reload_estado.rs,
// `cli_preserves_unified_space_state_across_three_reloads`): generation 3.
// The globals survive the reload (`--preservar-estado`) and `main` runs again
// with the generation's code.

int geracao = 0;
String texto = '';
final inteiros = <int>[];
final reais = <double>[];
final objetos = <Object?>[];
double caixa = 0.5;
Object? emCaixa;
int Function() contador = () => 0;
// Larger than 2014 words: a general list with cards, old after the first
// collection, receiving new objects between generations.
final grande = List<Object?>.filled(3000, null);

String resumo() {
  var n = 0;
  for (final e in grande) {
    if (e != null) n++;
  }
  return '$geracao $texto ${inteiros.join(',')} ${reais.join(',')} '
      '${objetos.join(',')} $emCaixa ${contador()} $n';
}

void main() {
  geracao += 1;
  texto += 'a$geracao';
  inteiros.add(geracao);
  reais.add(geracao + 0.5);
  objetos.add('s$geracao');
  caixa += 1.0;
  emCaixa = caixa;
  final antes = contador;
  final g = geracao;
  contador = () => antes() + g;
  grande[g * 700 % 3000] = 'v$g';
  print('v3 ${resumo()}');
}

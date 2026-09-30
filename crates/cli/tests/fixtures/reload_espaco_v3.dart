// Recarga com o estado do espaço unificado (crates/cli/tests/reload_estado.rs,
// `cli_preserva_o_estado_do_espaco_unificado_em_tres_recargas`): geração 3.
// Os globais sobrevivem à recarga (`--preservar-estado`) e o `main` roda de
// novo com o código da geração.

int geracao = 0;
String texto = '';
final inteiros = <int>[];
final reais = <double>[];
final objetos = <Object?>[];
double caixa = 0.5;
Object? emCaixa;
int Function() contador = () => 0;
// Maior que 2014 palavras: uma lista geral com cartões, velha depois da
// primeira coleta, recebendo objetos novos entre as gerações.
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

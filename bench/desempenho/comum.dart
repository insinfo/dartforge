// Medição comum dos programas de `bench/desempenho`: roda o núcleo `n`
// vezes (a primeira aquece o JIT) e imprime o tempo de cada rodada e o
// resultado, que tem de ser igual em todos os executores.
void medir(String nome, Object? Function() nucleo, {int rodadas = 6}) {
  Object? r;
  final tempos = <int>[];
  for (var i = 0; i < rodadas; i++) {
    final sw = Stopwatch()..start();
    r = nucleo();
    tempos.add(sw.elapsedMicroseconds);
  }
  print('$nome: ${tempos.join(' ')} us | $r');
}

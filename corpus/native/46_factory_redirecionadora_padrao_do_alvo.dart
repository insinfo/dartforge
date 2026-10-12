// Factory redirecionadora sem padrão nos parâmetros: o argumento omitido
// vale o padrão do parâmetro correspondente do alvo (o
// `FeatureSet.latestLanguageVersion({flags}) = ExperimentStatus.latestLanguageVersion`
// do analyzer, chamado sem `flags`), também pelo tear-off.
abstract class Conjunto {
  factory Conjunto.ultimo({List<String> flags, int nivel}) = Status.ultimo;
  factory Conjunto.posicional([String nome]) = Status.posicional;
  List<String> get flags;
}

class Status implements Conjunto {
  final List<String> flags;
  final int nivel;
  final String nome;
  Status._(this.flags, this.nivel, this.nome);
  factory Status.ultimo({List<String> flags = const ['padrao'], int nivel = 7}) =>
      Status._(flags, nivel, '-');
  factory Status.posicional([String nome = 'anonimo']) => Status._(const [], 0, nome);
  String toString() => '$flags $nivel $nome';
}

void main() {
  print(Conjunto.ultimo());
  print(Conjunto.ultimo(flags: ['x']));
  print(Conjunto.ultimo(nivel: 1));
  print(Conjunto.posicional());
  print(Conjunto.posicional('z'));
  final f = Conjunto.ultimo;
  print(f());
}

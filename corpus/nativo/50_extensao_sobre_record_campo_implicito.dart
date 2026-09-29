// Campo de record lido pelo `this` implícito no corpo de uma extensão cujo
// `on` é um tipo record (o `applyPrefix` do `query_writer.dart` do drift):
// nomeado e posicional, também dentro de um `switch` e de uma closure.
typedef _Ctx = ({String? prefixo, bool anulavel});

extension on _Ctx {
  String aplicar(String nome) {
    return switch (prefixo) {
      null => nome,
      var s => '$s.$nome',
    };
  }

  bool get ehAnulavel => anulavel;
  String Function() get descrever => () => '$prefixo/$anulavel';
}

extension Par on (int, String) {
  String get junto => '${$1}:${$2}';
  int get dobro => $1 * 2;
}

void main() {
  final _Ctx a = (prefixo: null, anulavel: false);
  final _Ctx b = (prefixo: 't', anulavel: true);
  print(a.aplicar('x'));
  print(b.aplicar('y'));
  print(b.ehAnulavel);
  print(a.descrever());
  print((3, 'z').junto);
  print((4, 'w').dobro);
}

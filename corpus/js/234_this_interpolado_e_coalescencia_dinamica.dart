// `$this` numa interpolação simples (o identificador `this`) e `d ?? x`
// com `d` dinâmico num contexto não anulável: o `??` vem antes do teste de
// tipo do contexto (achados validando o limitless_ui).
abstract class Campo {
  final String p;
  Campo(this.p);

  @override
  String toString() => p;

  String descreve(Object pilha) => 'lendo $this de $pilha; ${this}!';
}

class C extends Campo {
  C() : super('hh');
}

extension Ext on int {
  String get dito => 'int $this';
}

void main() {
  print(C().descreve('xyz'));
  print(7.dito);
  final Map<String, dynamic> m = {'id': 1};
  final String nome = m['nome'] ?? 'padrao';
  print(nome);
  dynamic d;
  final String s = d ?? 'x';
  print(s);
  d = 'y';
  final String t = d ?? 'x';
  print(t);
  int? n;
  final int k = n ?? 3;
  print(k);
  dynamic e = 5;
  try {
    final String u = e ?? 'z';
    print(u);
  } catch (err) {
    print('erro de tipo: ${err is TypeError}');
  }
}

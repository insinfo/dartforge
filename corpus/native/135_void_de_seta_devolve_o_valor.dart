// `void f() => e;` devolve o valor de `e` em tempo de execução: o `void` é só
// estático, e quem chama por `Function`/`dynamic` recebe o valor (o
// `handleSpace` do `NumberParser` do `intl`; docs/NATIVO-PROJETOS-REAIS.md,
// C23).

class Leitor {
  int lidos = 0;
  void espaco() => lidos > 100 ? erro() : '';
  void ponto() => '.';
  void conta() => lidos++;
  void nada() {}
  Never erro() => throw StateError('não');

  Map<String, Function> get regras => {' ': espaco, ',': ponto, '+': conta, '-': nada};
}

void topo() => 42;

class Filho extends Leitor {
  @override
  void ponto() => ',';
}

void main() {
  final l = Leitor();
  final s = StringBuffer();
  for (final c in ['1', ' ', '2', ',', '3']) {
    final r = l.regras[c];
    s.write(r == null ? c : r());
  }
  print(s);
  print((l.regras['+']!)());
  print(l.lidos);
  print((l.regras['-']!)());
  final Function f = topo;
  print(f());
  dynamic d = Filho();
  print(d.ponto());
  print((Filho().regras[',']!)());
  // Chamada direta continua sem valor (estático).
  l.ponto();
  topo();
  print('fim');
}

// A poda das tabelas de métodos na ligação do AOT de produção
// (docs/NATIVO-PODA-DE-TABELAS.md). A classe `Poda` é instanciada, e cada
// forma de entrada é usada por um membro: chamada dinâmica (`c:`), getter e
// setter dinâmicos (`g:`, `s:`), tear-off de método (`g:` de um método),
// chamada direta, `toString`, sobrescrita numa subclasse, classe chamável
// por `Function.apply` (`c:call`, que o runtime procura pelo texto) e
// `noSuchMethod` (o encaminhador e o `Invocation`). Os membros `naoUsadoPoda…`
// nunca são usados de forma nenhuma: no executável de produção não pode
// ficar nem o corpo nem as entradas `$c`/`$tc`/`$g`/`$s`/`$tearm` deles — o
// teste `poda_tira_membros_nao_usados` (`sdk_modulo.rs`) confere pelo mapa da
// ligação. Aqui, a saída é a mesma da VM.

class Poda {
  int campo;
  int _escrito = 0;
  Poda(this.campo);

  int usadoDinamicoPoda(int x) => x + campo;
  int usadoDiretoPoda(int x) => x * 2 + campo;
  int get lidoPorGetterPoda => campo + 10;
  set gravadoPorSetterPoda(int v) => _escrito = v;
  int tearoffUsadoPoda(int x) => x - campo;
  int tearoffDinamicoPoda(int x) => x + 100;
  T genericoUsadoPoda<T>(T x) => x;

  @override
  String toString() => 'Poda($campo, $_escrito)';

  // Nunca usados.
  int naoUsadoPodaMetodo(int x) => x * 1000 + campo;
  int get naoUsadoPodaGetter => campo * 7;
  set naoUsadoPodaSetter(int v) => campo = v * 3;
  String naoUsadoPodaGenerico<T>(T x) => 'g$x$campo';
  int naoUsadoPodaCampo = 42;
}

class PodaFilha extends Poda {
  PodaFilha(super.campo);

  @override
  int usadoDinamicoPoda(int x) => x * 100 + campo;

  int naoUsadoPodaNaFilha() => campo + 5;
}

class ChamavelPoda {
  int call(int x, {int y = 1}) => x * y + 3;
}

class ComNsmPoda {
  @override
  dynamic noSuchMethod(Invocation i) =>
      'nsm ${i.memberName} ${i.positionalArguments} ${i.namedArguments.length}';
}

void main() {
  final p = Poda(3);
  dynamic d = p;
  print(d.usadoDinamicoPoda(4));
  print(p.usadoDiretoPoda(5));
  print(d.lidoPorGetterPoda);
  d.gravadoPorSetterPoda = 9;
  final f = p.tearoffUsadoPoda;
  print(f(10));
  final g = d.tearoffDinamicoPoda;
  print(g(1));
  print(d.genericoUsadoPoda<String>('t'));
  print(p);
  final lista = <Object>[p, PodaFilha(2)];
  for (final o in lista) {
    print((o as dynamic).usadoDinamicoPoda(1));
  }
  print(Function.apply(ChamavelPoda(), [5], {#y: 2}));
  dynamic c = ChamavelPoda();
  print(c(4));
  dynamic n = ComNsmPoda();
  print(n.qualquerCoisaPoda(1, 2, nome: 3));
  try {
    d.naoExisteEmLugarNenhum();
  } on NoSuchMethodError {
    print('NoSuchMethodError');
  }
}

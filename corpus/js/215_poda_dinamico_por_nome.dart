// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): chamadas
// dinâmicas por nome. O mundo fechado registra o seletor pelo nome e o
// mantém em TODA classe instanciada — inclusive a criada só por fábrica e a
// que tem um campo com o nome do método. Se a poda remover um membro vivo, o
// caso imprime "caso <nome>: ERRO" em vez do valor.

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

class A {
  String fala() => 'A.fala';
  String get rotulo => 'A.rotulo';
  set valor(int v) => print('A.valor=$v');
  String so() => 'A.so';
}

class B {
  String fala() => 'B.fala';
  String get rotulo => 'B.rotulo';
  int valor = 0;
}

// Campo com o nome do método dos outros.
class C {
  String fala = 'C.fala-campo';
}

// Instanciada só por fábrica.
class SoFabrica {
  SoFabrica._();
  factory SoFabrica() => SoFabrica._();
  String fala() => 'SoFabrica.fala';
}

// Nunca criada: só aparece em `is`.
class NuncaCriada {
  String fala() => 'nunca';
}

Object escolhe(int i) => [A(), B(), C(), SoFabrica()][i];

void main() {
  for (var i = 0; i < 4; i++) {
    dynamic d = escolhe(i);
    caso('fala $i', () => d is C ? d.fala : d.fala());
  }
  caso('getter', () {
    dynamic d = escolhe(1);
    return d.rotulo;
  });
  caso('setter', () {
    dynamic a = escolhe(0);
    a.valor = 3;
    dynamic b = escolhe(1);
    b.valor = 4;
    return b.valor;
  });
  caso('tearoff dinâmico', () {
    dynamic d = escolhe(1);
    var f = d.fala;
    return f();
  });
  caso('método por nome em string', () {
    dynamic d = escolhe(3);
    return Function.apply(d.fala, const []);
  });
  caso('nunca criada é só tipo', () => escolhe(0) is NuncaCriada);
  caso('nome ausente', () {
    dynamic d = escolhe(0);
    try {
      return d.inexistente();
    } on NoSuchMethodError {
      return 'NoSuchMethodError';
    }
  });
}

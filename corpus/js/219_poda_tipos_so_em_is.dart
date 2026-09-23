// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): classes que nunca
// são construídas mas aparecem em `is`, `as`, `catch`, argumento de tipo e
// literal de tipo. Elas ficam no nível "só tipo" — casca e regras rti — e as
// respostas de subtipagem não podem mudar. Se a poda apagar a regra, um `is`
// muda de resposta em silêncio; aqui ele é impresso.

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

abstract class Animal {}

abstract class Voador {}

class Pato implements Animal, Voador {}

// Nunca instanciadas.
class Peixe implements Animal {}

class Erro1 implements Exception {}

class Erro2 implements Exception {}

class Base<T> {}

class Filha<T> extends Base<T> {}

void main() {
  final Object o = Pato();
  caso('is interface implementada', () => o is Voador);
  caso('is classe nunca criada', () => o is Peixe);
  caso('lista de tipo nunca criado', () {
    final l = <Peixe>[];
    return '${l is List<Animal>} ${l is List<Voador>}';
  });
  caso('genérico por superclasse', () {
    final b = <Filha<Pato>>[];
    return b is List<Base<Animal>>;
  });
  caso('as falha', () {
    try {
      o as Peixe;
      return 'passou';
    } on TypeError {
      return 'TypeError';
    }
  });
  caso('catch por tipo', () {
    try {
      throw Erro2();
    } on Erro1 {
      return 'Erro1';
    } on Erro2 {
      return 'Erro2';
    }
  });
  caso('literal de tipo', () => '$Peixe ${Peixe == Peixe}');
  caso('tipo em mapa', () => <Type, int>{Peixe: 1, Pato: 2}[Peixe]);
}

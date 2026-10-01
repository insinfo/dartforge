// Biblioteca auxiliar do 121.

class Caixa<T> {
  final String nome;
  const Caixa(this.nome);
  static const Caixa<Object> texto = Caixa('texto');
  static const Caixa<Object> inteiro = Caixa('int4');
  static Caixa<Object> fabricar(String n) => Caixa(n);
  static int contador = 0;
  static int get dobroDoContador => contador * 2;
  @override
  String toString() => 'Caixa($nome)';
}

typedef Apelido = Caixa<Object>;
typedef ApelidoGenerico<T> = Caixa<T>;

enum Cor { vermelho, verde }

typedef ApelidoDeEnum = Cor;

extension Dobro on int {
  int get dobro => this * 2;
  int vezes(int n) => this * n;
}

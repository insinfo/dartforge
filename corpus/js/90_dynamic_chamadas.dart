// dynamic: chamadas de métodos/getters/setters/operadores, length, closures, [], nomeados, runtimeType.
class Pessoa {
  String nome;
  int idade;
  Pessoa(this.nome, this.idade);

  String saudacao([String prefixo = 'Oi']) => '$prefixo, $nome';
  String apresenta({required String cidade, int anos = 0}) =>
      '$nome de $cidade ha $anos anos';
  int get dobroIdade => idade * 2;
  set apelido(String a) {
    nome = '$nome ($a)';
  }

  Pessoa operator +(int anos) => Pessoa(nome, idade + anos);
  int operator [](int i) => nome.codeUnitAt(i);
  bool operator ==(Object o) => o is Pessoa && o.nome == nome;
  int get hashCode => nome.hashCode;
  T generico<T>(T v) => v;
  @override
  String toString() => 'Pessoa($nome, $idade)';
}

void main() {
  dynamic d = Pessoa('Ana', 30);
  print(d.nome);
  print(d.idade);
  print(d.saudacao());
  print(d.saudacao('Ola'));
  print(d.apresenta(cidade: 'Rio'));
  print(d.apresenta(cidade: 'Rio', anos: 5));
  print(d.dobroIdade);
  d.apelido = 'aninha';
  print(d.nome);
  d.idade = 31;
  print(d.idade);
  print(d + 4);
  print(d[0]);
  print(d == Pessoa('Ana (aninha)', 0));
  print(d == 'x');
  print(d.toString());
  print(d.runtimeType);
  print(d.generico<int>(3));
  print(d.generico('s'));
  print(d.hashCode == d.hashCode);

  dynamic s = 'texto';
  print(s.length);
  print(s.toUpperCase());
  print(s + '!');
  print(s * 2);
  print(s[1]);
  print(s.contains('ext'));
  print(s.substring(1, 3));
  print(s == 'texto');

  dynamic l = [3, 1, 2];
  print(l.length);
  print(l[0]);
  l[0] = 9;
  print(l);
  l.add(4);
  print(l.length);
  l.sort();
  print(l);
  print(l.map((x) => x * 2).toList());
  print(l.first + l.last);
  print(l.contains(4));

  dynamic n = 10;
  print(n + 1);
  print(n - 1);
  print(n * 3);
  print(n ~/ 3);
  print(n % 3);
  print(n > 5);
  print(n.isEven);
  print(n.toString() + '!');
  print(-n);
  n += 5;
  print(n);
  n++;
  print(n);

  dynamic f = (int a, int b) => a * b;
  print(f(3, 4));
  dynamic g = (String s, {int vezes = 1}) => s * vezes;
  print(g('ab', vezes: 3));
  print(g('c'));
  dynamic h = () => 'sem args';
  print(h());
  dynamic metodo = d.saudacao;
  print(metodo('Ei'));

  dynamic m = {'a': 1, 'b': 2};
  print(m['a']);
  m['c'] = 3;
  print(m.length);
  print(m.keys.toList());
  print(m.containsKey('b'));

  dynamic b = true;
  print(!b);
  print(b && false);
  print(b ? 'sim' : 'nao');
  dynamic nulo = null;
  print(nulo == null);
  print(nulo ?? 'padrao');
  print(nulo?.qualquer);
}

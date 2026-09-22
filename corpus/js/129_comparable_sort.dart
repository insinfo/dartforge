// Comparable próprio, sort com compareTo e comparator, Comparable.compare, vários campos, estabilidade e toList()..sort.
class Versao implements Comparable<Versao> {
  final int maior, menor, patch;
  Versao(this.maior, this.menor, this.patch);
  @override
  int compareTo(Versao o) {
    if (maior != o.maior) return maior.compareTo(o.maior);
    if (menor != o.menor) return menor.compareTo(o.menor);
    return patch.compareTo(o.patch);
  }

  @override
  String toString() => '$maior.$menor.$patch';
}

class Pessoa {
  final String nome;
  final int idade;
  final String cidade;
  Pessoa(this.nome, this.idade, this.cidade);
  @override
  String toString() => '$nome($idade,$cidade)';
}

int porIdadeDepoisNome(Pessoa a, Pessoa b) {
  var c = a.idade.compareTo(b.idade);
  return c != 0 ? c : a.nome.compareTo(b.nome);
}

void main() {
  var vs = [Versao(1, 2, 3), Versao(1, 0, 9), Versao(0, 9, 9), Versao(1, 2, 0), Versao(2, 0, 0)];
  vs.sort();
  print(vs);
  print(Versao(1, 2, 3).compareTo(Versao(1, 2, 3)));
  print(Versao(1, 2, 3).compareTo(Versao(1, 3, 0)));
  print(Versao(2, 0, 0).compareTo(Versao(1, 9, 9)));
  print(Comparable.compare(Versao(1, 0, 0), Versao(1, 0, 1)));
  print(Comparable.compare(3, 3));
  print(Comparable.compare('b', 'a'));
  print(Comparable.compare(2.5, 1.5));
  vs.sort((a, b) => b.compareTo(a));
  print(vs);
  print(vs.reduce((a, b) => a.compareTo(b) <= 0 ? a : b));
  print(vs.reduce((a, b) => a.compareTo(b) >= 0 ? a : b));

  var ps = [
    Pessoa('Ana', 30, 'SP'),
    Pessoa('Bruno', 25, 'RJ'),
    Pessoa('Carla', 30, 'RJ'),
    Pessoa('Davi', 25, 'SP'),
    Pessoa('Eva', 35, 'SP'),
  ];
  ps.sort(porIdadeDepoisNome);
  print(ps);
  ps.sort((a, b) => a.nome.compareTo(b.nome));
  print(ps);
  ps.sort((a, b) {
    var c = a.cidade.compareTo(b.cidade);
    if (c != 0) return c;
    c = b.idade.compareTo(a.idade);
    if (c != 0) return c;
    return a.nome.compareTo(b.nome);
  });
  print(ps);
  ps.sort((a, b) => a.idade - b.idade);
  print(ps.map((p) => p.idade).toList());
  print(ps.map((p) => p.nome).toList());

  var ints = [5, 3, 8, 1, 9, 2];
  print(ints.toList()..sort());
  print(ints);
  var ordenado = ints.toList()..sort((a, b) => b - a);
  print(ordenado);
  print(ints.toList()..sort((a, b) => (a % 3).compareTo(b % 3) != 0 ? (a % 3).compareTo(b % 3) : a.compareTo(b)));
  var strs = ['banana', 'Abacaxi', 'cereja', 'abacate'];
  print(strs.toList()..sort());
  print(strs.toList()..sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase())));
  print(strs.toList()..sort((a, b) => a.length.compareTo(b.length) != 0 ? a.length.compareTo(b.length) : a.compareTo(b)));
  var dbl = [2.5, -1.5, 0.5, 10.25];
  print(dbl.toList()..sort());
  var mistos = <num>[3, 1.5, 2, 0.5];
  print(mistos.toList()..sort());
  print(<Comparable>[3, 1, 2].toList()..sort((a, b) => a.compareTo(b)));
  print([Duration(seconds: 3), Duration(seconds: 1)]..sort());
  print([DateTime.utc(2024), DateTime.utc(2020)]..sort());
  print(['b', 'a'].toList()..sort(Comparable.compare));
  print([3, 1, 2]..sort(Comparable.compare));
  print(<int>[]..sort());
  print([1]..sort());
  print([2, 2, 2]..sort());
  var pares = [(2, 'b'), (1, 'z'), (2, 'a'), (1, 'y')];
  pares.sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
  print(pares);
  var mapa = {'c': 3, 'a': 1, 'b': 2};
  var chaves = mapa.keys.toList()..sort();
  print(chaves);
  var entradas = mapa.entries.toList()..sort((a, b) => b.value.compareTo(a.value));
  print(entradas.map((e) => e.key).toList());
  var invertido = [1, 2, 3, 4, 5]..sort((a, b) => b.compareTo(a));
  print(invertido);
  print(invertido.reversed.toList());
  var comNull = <int?>[3, null, 1, null, 2];
  comNull.sort((a, b) {
    if (a == null) return b == null ? 0 : 1;
    if (b == null) return -1;
    return a.compareTo(b);
  });
  print(comNull);
  var indices = List.generate(5, (i) => i);
  var pesos = [30, 10, 50, 20, 40];
  indices.sort((a, b) => pesos[a].compareTo(pesos[b]));
  print(indices);
  print(indices.map((i) => pesos[i]).toList());
  var copia = [...ints]..sort();
  print(copia.first);
  print(copia.last);
  print(copia.indexOf(8));
  print(copia.sublist(1, 3));
  print((ints.toList()..sort()).join(' < '));
  print(vs.map((v) => v.toString()).toList()..sort());
  print([Versao(1, 1, 1), Versao(1, 1, 1)].toSet().length);
  print(Versao(1, 1, 1) == Versao(1, 1, 1));
  print(Versao(1, 2, 3).compareTo(Versao(1, 2, 3)) == 0);
}

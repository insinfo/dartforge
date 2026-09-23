// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): o runtime
// pré-compilado (dart_sdk.js) chama membros do usuário sem que o programa os
// cite — `compareTo` no sort, `iterator`/`moveNext`/`current` no for-in e no
// spread, `length`/`[]` numa ListBase, `toJson` no jsonEncode, `==`/`hashCode`
// em Set e Map, `toString` na interpolação, `listen` num Stream próprio. As
// regras (i), (ii) e (iii) do contrato mantêm esses membros. Se a poda
// errar, o caso imprime "caso <nome>: ERRO".
import 'dart:async';
import 'dart:collection';
import 'dart:convert';

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

class Versao implements Comparable<Versao> {
  final int n;
  Versao(this.n);
  @override
  int compareTo(Versao o) => n - o.n;
  @override
  String toString() => 'v$n';
}

class Contagem extends Iterable<int> {
  final int ate;
  Contagem(this.ate);
  @override
  Iterator<int> get iterator => _Passo(ate);
}

class _Passo implements Iterator<int> {
  final int ate;
  int _i = -1;
  _Passo(this.ate);
  @override
  bool moveNext() => ++_i < ate;
  @override
  int get current => _i;
}

class Trio extends ListBase<String> {
  final _itens = ['a', 'b', 'c'];
  @override
  int get length => _itens.length;
  @override
  set length(int n) => throw UnsupportedError('fixo');
  @override
  String operator [](int i) => _itens[i].toUpperCase();
  @override
  void operator []=(int i, String v) => _itens[i] = v;
}

class Pessoa {
  final String nome;
  Pessoa(this.nome);
  Map<String, Object> toJson() => {'nome': nome};
}

class Chave {
  final int id;
  Chave(this.id);
  @override
  bool operator ==(Object o) => o is Chave && o.id == id;
  @override
  int get hashCode => id.hashCode;
}

class Numeros extends Stream<int> {
  @override
  StreamSubscription<int> listen(void Function(int)? onData, {Function? onError, void Function()? onDone, bool? cancelOnError}) =>
      Stream.fromIterable([1, 2, 3]).listen(onData, onError: onError, onDone: onDone, cancelOnError: cancelOnError);
}

Future<void> main() async {
  caso('sort usa compareTo', () => ([Versao(3), Versao(1), Versao(2)]..sort()).join(' '));
  caso('for-in e spread', () {
    final l = <int>[];
    for (final i in Contagem(3)) {
      l.add(i);
    }
    return [...l, ...Contagem(2)];
  });
  caso('ListBase', () => Trio().join('-'));
  caso('jsonEncode chama toJson', () => jsonEncode([Pessoa('Ana')]));
  caso('Set e Map usam == e hashCode', () => {Chave(1), Chave(1), Chave(2)}.length);
  caso('interpolação usa toString', () => 'versão ${Versao(9)}');
  final soma = await Numeros().fold<int>(0, (a, b) => a + b);
  caso('Stream próprio', () => soma);
}

// if-case: tipos com guarda, mapas (JSON), else, records, listas, aninhados, objetos.
class Usuario {
  final String nome;
  final int? idade;
  final List<String> papeis;
  Usuario(this.nome, this.idade, this.papeis);
}

String descreveJson(Object? json) {
  if (json case {'tipo': 'usuario', 'nome': String nome, 'idade': int idade}) {
    return 'usuario $nome ($idade)';
  } else if (json case {'tipo': 'usuario', 'nome': String nome}) {
    return 'usuario $nome sem idade';
  } else if (json case {'tipo': String t}) {
    return 'tipo $t desconhecido';
  } else if (json case [var primeiro, ...]) {
    return 'lista comecando com $primeiro';
  } else if (json case null) {
    return 'json nulo';
  }
  return 'formato invalido';
}

void main() {
  final Object? v1 = 42;
  if (v1 case int n when n > 0) {
    print('positivo $n');
  }
  if (v1 case int n when n > 100) {
    print('nunca');
  } else {
    print('nao passou da guarda');
  }
  final Object? v2 = 'texto';
  if (v2 case int n) {
    print('int $n');
  } else if (v2 case String s) {
    print('string $s');
  }
  if (v2 case String(length: 5)) {
    print('cinco letras');
  }
  if (v2 case String(length: var l, isEmpty: false)) {
    print('nao vazia com $l');
  }

  print(descreveJson({'tipo': 'usuario', 'nome': 'ana', 'idade': 30}));
  print(descreveJson({'tipo': 'usuario', 'nome': 'bia'}));
  print(descreveJson({'tipo': 'usuario', 'nome': 7}));
  print(descreveJson({'tipo': 'pedido'}));
  print(descreveJson([1, 2, 3]));
  print(descreveJson(<int>[]));
  print(descreveJson(null));
  print(descreveJson('x'));
  print(descreveJson({'tipo': 'usuario', 'nome': 'c', 'idade': 'nao'}));

  final Object rec = (3, 'tres');
  if (rec case (int n, String s)) {
    print('record $n $s');
  }
  if (rec case (int n, String s) when s.length == n) {
    print('nunca');
  } else {
    print('tamanho diferente');
  }
  if (rec case (var a, var b)) {
    print('$b$a');
  }
  final Object nomeado = (x: 1, y: 2);
  if (nomeado case (x: int x, y: int y)) {
    print(x + y);
  }
  if (nomeado case (x: 0, y: _)) {
    print('nunca');
  } else {
    print('x nao e zero');
  }

  final lista = [1, 2, 3, 4, 5];
  if (lista case [var a, var b, ...var resto]) {
    print('$a $b $resto');
  }
  if (lista case [..., var ultimo]) {
    print('ultimo $ultimo');
  }
  if (lista case [1, 2, ...]) {
    print('comeca com 1 2');
  }
  if (lista case [_, _, 3, _, _]) {
    print('meio 3');
  }
  if (lista case [var so]) {
    print('nunca $so');
  } else {
    print('nao tem um so');
  }

  final Object aninhado = [
    (1, {'k': 'v'}),
    (2, {'k': 'w'}),
  ];
  if (aninhado case [(int a, {'k': String s}), (int b, {'k': var t})]) {
    print('$a$s $b$t');
  }
  if (aninhado case [(_, {'z': _}), ...]) {
    print('nunca');
  } else {
    print('chave z ausente');
  }

  final u = Usuario('dora', 25, ['admin', 'dev']);
  if (u case Usuario(nome: var n, idade: int i) when i >= 18) {
    print('$n adulta');
  }
  if (u case Usuario(papeis: [var primeiro, ...])) {
    print('primeiro papel $primeiro');
  }
  if (u case Usuario(papeis: ['admin', ...])) {
    print('e admin');
  }
  if (u case Usuario(idade: null)) {
    print('nunca');
  } else {
    print('idade conhecida');
  }
  final u2 = Usuario('eva', null, []);
  if (u2 case Usuario(idade: null, papeis: [])) {
    print('sem idade e sem papeis');
  }
  if (u2 case Usuario(:var nome, :var idade)) {
    print('$nome/$idade');
  }
  final Object? talvez = null;
  if (talvez case var x?) {
    print('nunca $x');
  } else {
    print('nulo nao casa com x?');
  }
  final Object? cheio = 9;
  if (cheio case var x?) {
    print('nao nulo $x');
  }
}

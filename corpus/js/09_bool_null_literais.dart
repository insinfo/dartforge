// Literais bool e null: operadores lógicos com efeitos, !, curto-circuito, bool.parse/tryParse, toString.
int chamadas = 0;
bool t(String nome) {
  chamadas++;
  print('avaliou $nome');
  return true;
}

bool f(String nome) {
  chamadas++;
  print('avaliou $nome');
  return false;
}

void main() {
  print(true);
  print(false);
  print(null);
  print(!true);
  print(!false);
  print(!!true);
  print(true && false);
  print(true || false);
  print(false || false);
  print(true ^ false);
  print(true ^ true);
  print(true & false);
  print(false | true);
  print(true == true);
  print(true != false);
  print(true.toString());
  print(false.toString() + '!');
  print('${true}');
  print(null.toString());
  print('${null}');
  print(null == null);
  print(true.hashCode == true.hashCode);

  print(f('a') && t('b'));
  print(t('c') || f('d'));
  print(t('e') && f('f'));
  print(f('g') || t('h'));
  print(f('i') & t('j'));
  print(t('k') | f('l'));
  print(chamadas);

  print(bool.parse('true'));
  print(bool.parse('false'));
  print(bool.parse('TRUE', caseSensitive: false));
  print(bool.tryParse('yes'));
  print(bool.tryParse('True'));
  print(bool.tryParse('True', caseSensitive: false));
  try {
    bool.parse('nope');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }

  Object? n;
  print(n ?? 'padrão');
  print(n == null);
  print(n?.toString());
  n ??= 'agora';
  print(n);
  n ??= 'ignorado';
  print(n);
  bool? b;
  print(b ?? false);
  print(b == true);
  print(b != true);
  print(b == false);
  b = true;
  print(b);
  print(b ? 'sim' : 'não');
  var lista = <bool>[true, false, true];
  print(lista);
  print(lista.where((e) => e).length);
  print(lista.every((e) => e));
  print(lista.any((e) => !e));
  print(true.runtimeType == bool);
  print(null is Null);
  print(null is Object);
  print(null is Object?);
  var contagem = 0;
  if (t('m') || t('n')) contagem++;
  if (f('o') && t('p')) contagem++;
  print(contagem);
  print(chamadas);
  print(true && !false || false);
  print(!(true && false));
  print(true ? null : false);
  print(false ? null : true);
  print([null, null].length);
  print(<int?>[1, null, 3]);
  print({'a': null});
  print((null, true));
}

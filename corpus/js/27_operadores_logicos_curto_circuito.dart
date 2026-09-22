// curto-circuito de && e ||, ?? e ??=, negação, combinação com efeitos colaterais.
bool t(String nome) {
  print('t($nome)');
  return true;
}

bool f(String nome) {
  print('f($nome)');
  return false;
}

int? talvez(String nome, int? v) {
  print('talvez($nome)');
  return v;
}

void main() {
  print('--- && ---');
  print(f('a') && t('b'));
  print(t('c') && f('d'));
  print(t('e') && t('g'));

  print('--- || ---');
  print(t('a') || f('b'));
  print(f('c') || t('d'));
  print(f('e') || f('g'));

  print('--- misto ---');
  print(f('a') && t('b') || t('c'));
  print(t('a') || f('b') && t('c'));
  print((t('a') || f('b')) && f('c'));
  print(!f('a') && !t('b'));

  print('--- ?? ---');
  print(talvez('x', 1) ?? talvez('y', 2));
  print(talvez('p', null) ?? talvez('q', 2));
  print(talvez('m', null) ?? talvez('n', null) ?? 3);

  print('--- ??= ---');
  int? a;
  a ??= talvez('primeiro', 10);
  a ??= talvez('segundo', 20);
  print(a);
  int? b = 5;
  b ??= talvez('nunca', 99);
  print(b);

  print('--- if ---');
  if (f('cond1') && t('cond2')) {
    print('não');
  } else {
    print('else');
  }
  if (t('cond3') || f('cond4')) print('sim');

  print('--- em laço ---');
  var i = 0;
  while (i < 5 && t('laço$i')) {
    i++;
  }
  print('i=$i');

  print('--- negação ---');
  final v = 3;
  print(!(v > 2));
  print(!(v > 2) || v == 3);
  print(!!(v == 3));

  print('--- resultado como valor ---');
  final r = f('r1') || t('r2');
  print('r=$r');
  final s = t('s1') && f('s2') && t('s3');
  print('s=$s');

  print('--- ?. com curto-circuito ---');
  String? nulo;
  print(nulo?.length ?? -1);
  print(nulo?.toUpperCase() ?? 'era nulo');
  String? cheio = 'abc';
  print(cheio?.length ?? -1);

  print('--- ordem com efeitos ---');
  var contador = 0;
  bool inc() {
    contador++;
    return true;
  }

  final _ = inc() || inc();
  print('contador $contador');
  final __ = inc() && inc();
  print('contador $contador');
  print('fim');
}

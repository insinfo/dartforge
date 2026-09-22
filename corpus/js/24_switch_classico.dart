// switch clássico: int, String, enum, casos agrupados, default, corpo vazio, continue para outro case, return.
enum Cor { vermelho, verde, azul, amarelo }

String nomeDia(int d) {
  switch (d) {
    case 1:
      return 'segunda';
    case 2:
      return 'terça';
    case 6:
    case 7:
      return 'fim de semana';
    default:
      return 'outro';
  }
}

String tipoCor(Cor c) {
  switch (c) {
    case Cor.vermelho:
    case Cor.amarelo:
      return 'quente';
    case Cor.verde:
    case Cor.azul:
      return 'fria';
  }
}

int comando(String s) {
  var r = 0;
  switch (s) {
    case 'add':
      r = 1;
      break;
    case 'sub':
      r = 2;
      break;
    case 'nop':
      break;
    default:
      r = -1;
      break;
  }
  return r;
}

void main() {
  for (final d in [1, 2, 6, 7, 3]) {
    print('$d ${nomeDia(d)}');
  }
  for (final c in Cor.values) {
    print('${c.name} ${tipoCor(c)}');
  }
  for (final s in ['add', 'sub', 'nop', 'xyz']) {
    print('$s ${comando(s)}');
  }

  // corpo vazio no último case
  switch (3) {
    case 3:
  }
  print('corpo vazio ok');

  // switch em String com casos agrupados e default
  for (final vogal in ['a', 'e', 'x', 'u']) {
    switch (vogal) {
      case 'a':
      case 'e':
      case 'i':
      case 'o':
      case 'u':
        print('$vogal é vogal');
        break;
      default:
        print('$vogal não é vogal');
    }
  }

  // continue para outro case rotulado
  for (final n in [1, 2, 3]) {
    switch (n) {
      case 1:
        print('um');
        continue dois;
      dois:
      case 2:
        print('dois');
        break;
      case 3:
        print('três');
    }
  }

  // switch dentro de laço com break só do switch
  var soma = 0;
  for (var i = 0; i < 5; i++) {
    switch (i % 3) {
      case 0:
        soma += 100;
        break;
      case 1:
        soma += 10;
        break;
      default:
        soma += 1;
    }
  }
  print('soma $soma');

  // switch em constante calculada
  const base = 2;
  switch (base * 2) {
    case base:
      print('igual a base');
      break;
    case const (base * 2):
      print('dobro da base');
      break;
  }

  // switch com bool
  switch (soma > 100) {
    case true:
      print('grande');
    case false:
      print('pequeno');
  }

  // switch sem case que bate e sem default: nada acontece
  switch (99) {
    case 1:
      print('nunca');
  }
  print('fim');
}

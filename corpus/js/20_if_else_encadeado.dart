// if/else if/else encadeado, aninhado, condições compostas, if sem chaves.
String classifica(int n) {
  if (n < 0) {
    return 'negativo';
  } else if (n == 0) {
    return 'zero';
  } else if (n < 10) {
    return 'pequeno';
  } else if (n < 100) {
    return 'médio';
  } else {
    return 'grande';
  }
}

String faixaEtaria(int idade, bool estudante) {
  if (idade < 18) {
    if (estudante) {
      return 'menor estudante';
    } else {
      return 'menor';
    }
  } else {
    if (estudante) {
      return 'adulto estudante';
    }
    return 'adulto';
  }
}

void main() {
  for (final n in [-5, 0, 7, 42, 500]) {
    print('$n -> ${classifica(n)}');
  }

  print(faixaEtaria(10, true));
  print(faixaEtaria(10, false));
  print(faixaEtaria(30, true));
  print(faixaEtaria(30, false));

  // condições com &&, ||, !
  final a = 5, b = 10, c = 0;
  if (a < b && b > c) print('a<b e b>c');
  if (a > b || b > c) print('a>b ou b>c');
  if (!(a > b)) print('não a>b');
  if (a < b && !(c > a) || false) print('composta');
  if (a == 5 && b == 10 && c == 0) print('todas iguais');

  // if sem chaves com else
  if (a.isEven)
    print('a par');
  else
    print('a ímpar');

  // atribuição antes do if
  var resultado = 'vazio';
  final valor = a * b;
  if (valor > 40) resultado = 'alto';
  print(resultado);

  // if com condição de método e string
  final nome = 'dart';
  if (nome.startsWith('d') && nome.length == 4) {
    print('nome bate');
  } else {
    print('nome não bate');
  }

  // if aninhado profundamente
  var contador = 0;
  for (var i = 0; i < 5; i++) {
    if (i > 0) {
      if (i % 2 == 0) {
        if (i == 4) {
          contador += 100;
        } else {
          contador += 10;
        }
      } else {
        contador += 1;
      }
    }
  }
  print('contador $contador');

  // if com null check
  int? talvez;
  if (talvez == null) {
    print('nulo');
  } else {
    print('valor $talvez');
  }
  talvez = 3;
  if (talvez != null && talvez > 2) print('maior que 2: $talvez');

  // else if sem bloco final
  final x = 15;
  if (x < 10) {
    print('menos de 10');
  } else if (x < 20) {
    print('entre 10 e 20');
  }
  print('fim');
}

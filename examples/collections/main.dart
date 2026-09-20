import 'dart:core';

// A célula capturada continua viva e mutável após o retorno de contador.
int Function() contador(int inicial) {
  var atual = inicial;
  return () {
    atual += 1;
    return atual;
  };
}

int aplicar(int Function(int) f, int valor) => f(valor);

void main() {
  List<int> numeros = [1, 2, 3, 4, 5];
  var pares = numeros.where((n) => n % 2 == 0);
  var dobrados = pares.map((n) => n * 2).toList();
  dobrados.add(12);
  print(dobrados);
  print(dobrados.any((n) => n > 10));
  dobrados.forEach((n) { print(n); });
  var proximo = contador(40);
  print(proximo());
  print(proximo());
  print(aplicar((n) => n + 1, 9));
}

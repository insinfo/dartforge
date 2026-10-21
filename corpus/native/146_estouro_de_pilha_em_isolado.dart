// Recursão sem fim num isolado (outra thread) lança `StackOverflowError`,
// capturável, como na thread principal (134_estouro_de_pilha).

import 'dart:isolate';

int profundidade = 0;

void desce() {
  profundidade++;
  desce();
}

void main() async {
  final r = await Isolate.run(() {
    try {
      desce();
      return 'nao';
    } on StackOverflowError {
      return 'isolado: pegou';
    }
  });
  print(r);
}

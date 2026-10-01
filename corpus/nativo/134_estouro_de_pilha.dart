// Recursão sem fim lança `StackOverflowError`, capturável, em vez de
// derrubar o processo (o `Trace.from` do `stack_trace` testa assim;
// docs/NATIVO-PROJETOS-REAIS.md, C22).

import 'dart:isolate';

int profundidade = 0;

void desce() {
  profundidade++;
  desce();
}

abstract class No {
  int visitar(int n);
}

class A implements No {
  No? outro;
  @override
  int visitar(int n) => outro!.visitar(n + 1) + 1;
}

int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);

void main() async {
  try {
    desce();
  } on StackOverflowError catch (e) {
    print('pegou: ${e.runtimeType}');
    print(profundidade > 1000);
  }
  // A pilha voltou: de novo, e o programa segue.
  profundidade = 0;
  try {
    desce();
  } catch (e) {
    print('de novo: ${e is StackOverflowError}');
  }
  // Recursão por despacho dinâmico.
  final a = A()..outro = null;
  a.outro = a;
  try {
    a.visitar(0);
  } on StackOverflowError {
    print('virtual: pegou');
  }
  print(fib(20));
  // Num isolado (outra thread).
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

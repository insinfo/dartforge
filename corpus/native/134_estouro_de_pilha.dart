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

// Recursão sem nenhuma outra operação que lance (o `Trace.from` do
// `stack_trace`): a função num ciclo de chamadas lança pela pilha.
void pura() => pura();
int par(int n) => impar(n + 1);
int impar(int n) => par(n + 1);

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
  try {
    pura();
  } on StackOverflowError {
    print('pura: pegou');
  }
  try {
    par(0);
  } on StackOverflowError {
    print('mútua: pegou');
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

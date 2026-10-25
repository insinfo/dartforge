// Um literal inteiro em contexto `double` é o valor exato como double
// (`9223372036854775808` é 2^63, não o `int` que daria a volta; um hexadecimal
// de qualquer tamanho é arredondado ao mais próximo). Estáticos e campos
// `late` com inicializador como na VM: uma leitura durante a avaliação do
// inicializador o avalia de novo; uma atribuição feita nesse meio-tempo
// conta, mesmo se o inicializador lançar; um `final` atribuído durante a
// própria inicialização lança `LateInitializationError`.
double p2_63 = 9223372036854775808;
double p2_64 = 18446744073709551616;
double big = 0xfffffffffffff800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000;

var trace = [];
var depth = 0;

var foo = bar;

get bar {
  if (depth > 3) throw 'recursed';
  depth++;
  trace.add(depth);
  try {
    return foo ?? 1;
  } catch (e) {
    trace.add(e);
  }
  return 42;
}

var x2 = assignsThenThrows();
int assignsThenThrows() {
  x2 = 499;
  throw 'interrupted';
}

var x3Depth = 0;
int x3 = (++x3Depth > 10) ? x3Depth : x3 + 1;

final x4 = finalInit();
var x4Depth = 0;
int finalInit() => (++x4Depth > 3) ? x4Depth : x4 + 1;

class Late {
  late int field = init();
  int init() {
    field = 456;
    return 123;
  }

  int count = 0;
  late final int finalField = initFinal();
  int initFinal() {
    if (count == 5) return count;
    return ++count + finalField;
  }

  late int afterWrite = initAfterWrite();
  int initAfterWrite() {
    afterWrite = 456;
    throw StateError('after write');
  }
}

void main() {
  print([p2_63, -p2_63, p2_64, big == double.maxFinite]);
  print(p2_63 == 9223372036854775807.0);

  print([foo, trace]);
  trace = [];
  depth = 0;
  print([foo, trace]);

  try {
    print(x2);
  } catch (e) {
    print('x2: $e');
  }
  print(x2);
  print(x3);
  try {
    print(x4);
  } on Error catch (e) {
    print(e);
  }

  final l = Late();
  print(l.field);
  try {
    print(l.finalField);
  } on Error catch (e) {
    print(e);
  }
  print(l.finalField);
  try {
    print(l.afterWrite);
  } on StateError catch (e) {
    print(e);
  }
  print(l.afterWrite);
}

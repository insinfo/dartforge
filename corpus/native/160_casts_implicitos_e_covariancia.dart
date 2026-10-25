// O downcast implícito de um valor `dynamic` em todo contexto tipado
// (atribuição a local, campo, estático, global e setter; `return` síncrono,
// de closure e `async`; inicializadores de campo e de construtor; `??=`);
// as verificações de covariância: escrita em campo genérico por um
// supertipo, leitura de membro cujo tipo usa `T` em posição contravariante
// (campo, resultado de método, tear-off, chamada do valor de um campo) e os
// limites `E extends T` de um método genérico; o tipo reificado de um tear-off
// com parâmetros covariantes (`Object?`); subtipagem e texto de tipos de função
// genéricos com limites; e o tear-off de `super.m`, ligado ao método da
// superclasse.
typedef F<T> = void Function(T t);
typedef G<T> = void Function<S extends T>(S s);

class O {
  String name = '';
  static String st = '';
  set s(String v) {}
}

class Init {
  String f;
  Init(dynamic v) : f = v;
}

class Generic<T> {
  T field = 42 as dynamic;
}

class Fields<T> {
  late T x;
  T? plain;
}

class FnChecks<T> {
  late F<T> f;
  late G<T> g;
  F<T> setterForT() => (T t) {};
}

class Bounds<T> {
  Type get t => T;
  Bounds<E> foo<E extends T>() => Bounds<E>();
}

class TearOff<T> {
  method1(T t) => null;
  method3(T Function() returnsT) => null;
}

class Base {
  int g() => 42;
}

class Derived extends Base {
  int g() => -1;
  int viaSuper() => invoke(super.g);
}

int invoke(int Function() f) => f();

String top = '';
String ret(dynamic d) => d;
String retBlock(dynamic d) {
  return d;
}

Future<String> retAsync(dynamic d) async => d;

void check(String name, Object? Function() f) {
  try {
    f();
    print('$name: no error');
  } on TypeError {
    print('$name: TypeError');
  }
}

void main() async {
  dynamic d = 42;
  String local = '';
  final o = O();
  check('local', () => local = d);
  check('field', () => o.name = d);
  check('static', () => O.st = d);
  check('top-level', () => top = d);
  check('setter', () => o.s = d);
  check('return =>', () => ret(d));
  check('return {}', () => retBlock(d));
  check('closure return', () {
    String Function() c = () => d;
    return c();
  });
  check('constructor initializer', () => Init(d));
  check('field initializer', () => Generic<String>());
  check('??=', () {
    String? x;
    x ??= d;
    return x;
  });
  try {
    await retAsync(d);
    print('async: no error');
  } on TypeError {
    print('async: TypeError');
  }
  check('dynamic to String ok', () => local = 'ok' as dynamic);
  print(local);

  Fields<Object> fields = Fields<int>();
  check('late generic field', () => fields.x = 'hello');
  check('generic field', () => fields.plain = 'hello');
  check('generic field ok', () => fields.plain = 1);

  FnChecks<Object> c = FnChecks<String>()..f = (String b) {};
  check('contravariant field', () => c.f);
  check('call of the field', () => c.f(42));
  FnChecks<Object> cObj = FnChecks<int>();
  check('contravariant result', () => cObj.setterForT());
  check('contravariant tear-off', () => cObj.setterForT);
  FnChecks<Object> cg = FnChecks<num>()..g = <S extends num>(S s) {};
  check('generic function field', () => cg.g);

  Bounds<Object?> bounds = Bounds<num>();
  check('bound by the receiver', () => bounds.foo<String>());
  print(bounds.foo<int>().t);

  TearOff<num> tearOff = TearOff<int>();
  print(tearOff.method1.runtimeType);
  print(tearOff.method3.runtimeType);

  Object generic = <S extends num>(S s) => s.isNegative;
  print(generic is void Function<S extends Object>(S));
  print(generic is void Function<S extends num>(S));
  print(generic is void Function<S>(S));
  print(generic.runtimeType);

  print(Derived().viaSuper());
}

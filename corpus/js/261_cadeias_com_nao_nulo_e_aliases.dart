// `!` is part of a null-shorting chain: `a?.b!.c` does nothing when `a` is
// null. A void arrow function still returns its value at run time. A ??
// in a default value needs no temporary. Statics and constructors are reached
// through type aliases, also with type arguments and through an alias to a
// type variable. A local generic function's bound may be a function type
// with named parameters.
class C1 {
  C1? get cn => null;
  void method(Object? v) => print('method');
  C1 operator [](Object? _) => this;
  operator []=(Object? _, Object? __) {}
}

class A<X> {
  static const answer = 42;
  final String how;
  A.named() : how = 'named $X';
  A(this.how);
}

typedef T<X> = A<X>;
typedef Bounded<Y extends Object> = Y;
typedef ViaVariable = Bounded<A<int>>;

void voidFun() => 499;

class Holder<V> {
  V value;
  Holder(this.value);
  Object? get asObject => value;
}

const Object? nothing = null;

String describe([Object? value = nothing ?? 'default']) => '$value';

void main() {
  C1? none;
  C1 c1 = C1();
  none?.cn!;
  none?.cn!.method(0);
  none?.cn![0] = 1;
  c1..cn?.cn!.method(0);
  print('chains ok');

  var h = Holder<void>(null);
  h.value = voidFun();
  print(h.asObject);

  print(describe());
  print(T.answer);
  print(T<int>.named().how);
  print(ViaVariable('via').how);

  void local<Z extends void Function({Z p})>(Z z) => print('local');
  local<void Function({Object? p})>(({Object? p}) {});
}

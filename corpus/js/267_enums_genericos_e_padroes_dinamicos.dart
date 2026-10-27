// A generic enum value without type arguments has the inferred (or
// instantiated-to-bounds) type. An enum value with a call method can be
// called as E.v(x). A static setter is reachable through a type alias. A
// dynamic call of an unbounded generic method defaults the type argument to
// dynamic.
enum Kind<T extends num> {
  a,
  b;

  T call<T>(T value) => value;
}

class Holder {
  static int last = 0;
  static set value(int v) => last = v;
}

typedef H = Holder;

class C {
  List memberFn<T>([T? x]) => [T, x];
}

void main() {
  print(Kind.a is Kind<num>);
  print(Kind.a is Kind<int>);
  print(Kind.b(42));
  print(Kind.b<String>('s'));
  H.value = 7;
  print(Holder.last);
  dynamic o = C();
  print(o.memberFn(1));
  print(o.memberFn<int>(2));
}

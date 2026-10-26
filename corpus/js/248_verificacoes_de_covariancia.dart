// As verificações de covariância do DDC em classes do programa: um parâmetro
// genérico-covariante, um escrito `covariant`, um que sobrescreve um
// parâmetro covariante de um supertipo, e o setter de um campo genérico
// (também `late`), quando a chamada passa por um supertipo.
class C<T> {
  T? last;
  late T lateField;
  void f(T x) {
    last = x;
  }

  void g(covariant num x) {}
}

class D extends C<int> {
  void g(covariant int x) {}
}

class NumBounds<T extends num> {
  bool m(T t) => t.isNegative;
}

class TakesInt extends NumBounds<int> {
  bool m(int obj) => obj.isNegative;
}

class TakesNum extends NumBounds<int> {
  bool m(num obj) => obj.isNegative;
}

void check(String name, void Function() f) {
  try {
    f();
    print('$name: no error');
  } on TypeError {
    print('$name: TypeError');
  }
}

void main() {
  C<Object> c = C<int>();
  check('generic parameter', () => c.f('s'));
  check('generic parameter ok', () => c.f(1));
  check('generic field', () => c.last = 's');
  check('late generic field', () => c.lateField = 's');
  C<int> d = D();
  check('covariant keyword', () => d.g(1.5));
  NumBounds<num> b = TakesInt();
  check('override of a covariant parameter', () => b.m(-1.1));
  b = TakesNum();
  check('widened override', () => b.m(-1.1));
  dynamic dy = c;
  check('dynamic', () => dy.f('s'));
}

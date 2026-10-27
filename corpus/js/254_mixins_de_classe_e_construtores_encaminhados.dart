// @dart=2.19
// Before 3.0 a plain class can be mixed in: its field initializers run when
// the application is constructed, including through a named superclass
// constructor and through a named mixin application. The constructors a named
// mixin application forwards check their arguments against the superclass
// constructor's parameter types.
class Base {
  int i, j;
  Base.ctor(int i, this.j) : this.i = i + 7;
}

abstract class M {
  int get i;
  int get j;
  int k = 42;
  String? label;
  String name = 'm';
  foo() => i + j + k;
}

class C extends Base with M {
  int l = 131;
  C() : super.ctor(1, 13);
}

class D = Base with M;

class B<T> {
  T value;
  B(this.value);
}

class N {
  List<int> log = [1];
}

class E<T> = B<T> with N;

void main() {
  var c = C();
  print([c.i, c.j, c.k, c.l, c.label, c.name, c.foo()]);
  var d = D.ctor(2, 3);
  print([d.i, d.j, d.k, d.label, d.name]);
  var e = E<String>('x');
  print([e.value, e.log]);
  dynamic value = 0;
  try {
    E<String>(value);
    print('accepted');
  } catch (error) {
    print('rejected');
  }
}

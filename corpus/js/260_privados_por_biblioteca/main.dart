// A private name means the member of the library that uses it: c._field in
// this library is C1's, not the same-named field another library declares in
// a subclass. A class implementing another library's class gets throwers for
// its private members, whose tear-off has the member's type. A private field
// overridden by a getter in a subclass, or that overrides an inherited member,
// keeps its own storage, and an abstract field declares no storage. `f() is
// Object?` still calls f.
import 'other.dart';

class C1 {
  int _field = 499;
  int ownField() => _field;
}

class C3 extends C2 {}

class Foo implements Bar {}

mixin PrivateFieldMixin {
  int _foo = 40;
}

class PrivateFieldClass with PrivateFieldMixin {
  int get _foo => super._foo + 2;
}

abstract class HasAbstract {
  abstract int value;
}

class Concrete {
  int value = 7;
}

class Mixed extends Concrete implements HasAbstract {}

class Overrider extends Concrete {
  abstract int value;
}

int calls = 0;
int count() => ++calls;

void main() {
  var c = C3();
  print([c._field, c.ownField(), c.otherField(), readDynamically(c)]);
  print(describe(Foo()));
  print(PrivateFieldClass()._foo);
  print([Mixed().value, Overrider().value]);
  print(count() is Object?);
  print(calls);
}

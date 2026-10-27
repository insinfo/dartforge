// A generic method whose type parameter bound mentions the class's type
// parameter checks the bound against the receiver's instantiation, also in an
// override with a plain bound. A forwarding stub's runtime type has every
// covariant parameter as Object?, including those the implementation checks.
class Bounds<T> {
  String foo<E extends T>() => 'foo $E';
}

class Derived extends Bounds<num> {
  @override
  String foo<E extends num>() => 'derived $E';
}

class Interface1 {
  void method(covariant int a, int b) {}
}

mixin Mixin1 {
  void method(int a, covariant int b) {}
}

class Mixed extends Object with Mixin1 implements Interface1 {}

void tryCall(String label, String Function() f) {
  try {
    print('$label: ${f()}');
  } on TypeError {
    print('$label: TypeError');
  }
}

void main() {
  Bounds<Object?> g = Bounds<int>();
  tryCall('int bound, String', () => g.foo<String>());
  tryCall('int bound, int', () => g.foo<int>());
  Bounds<Object?> d = Derived();
  tryCall('derived, String', () => d.foo<String>());
  tryCall('derived, double', () => d.foo<double>());
  var method = Mixed().method;
  print(method is void Function(int, int));
  print(method is void Function(Object?, Object?));
}

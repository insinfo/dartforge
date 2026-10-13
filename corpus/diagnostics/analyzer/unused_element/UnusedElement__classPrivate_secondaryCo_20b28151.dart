class A {
  A({required this.a, required this.b});
  final String a;
  final String b;
}

class _B extends A {
  _B({required super.a, super.b = 'b'});
}

var foo = _B(a: 'a');

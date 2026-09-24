class A({int? a});
class _B._named({super.a}) extends A;
var b = _B._named(a: 1);

class A({int? a});
class _B.named({super.a}) extends A;
var b = _B.named(a: 1);

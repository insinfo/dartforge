class A({int? a});
class _B({super.a}) extends A;
var b = _B(a: 1);

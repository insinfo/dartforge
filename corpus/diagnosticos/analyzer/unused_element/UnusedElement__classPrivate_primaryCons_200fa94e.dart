class A({int? a});
class _B.named({super.a}) extends A;
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B.named();

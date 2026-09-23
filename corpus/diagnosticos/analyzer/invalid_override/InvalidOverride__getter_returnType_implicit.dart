class A {
  String? f;
//        ^
// [context 1] The member being overridden.
// [context 2] The setter being overridden.
}
class B extends A {
  int? f;
//     ^
// [diag.invalidOverride][context 1] 'B.f' ('int? Function()') isn't a valid override of 'A.f' ('String? Function()').
// [diag.invalidOverrideSetter][context 2] The setter 'B.f' ('void Function(int?)') isn't a valid override of 'A.f' ('void Function(String?)').
}

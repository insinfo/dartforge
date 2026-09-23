class A {
  int get g { return 0; }
//        ^
// [context 1] The member being overridden.
}
class B extends A {
  String get g { return 'a'; }
//           ^
// [diag.invalidOverride][context 1] 'B.g' ('String Function()') isn't a valid override of 'A.g' ('int Function()').
}

abstract class A {
  int get foo;
//        ^^^
// [context 1] The member being overridden.
}
enum E(final num foo) implements A {
//               ^^^
// [diag.invalidOverride][context 1] 'E.foo' ('num Function()') isn't a valid override of 'A.foo' ('int Function()').
  v(0)
}

mixin M {
  int get foo;
//        ^^^
// [context 1] The member being overridden.
}
class A(final num foo) with M;
//                ^^^
// [diag.invalidOverride][context 1] 'A.foo' ('num Function()') isn't a valid override of 'M.foo' ('int Function()').

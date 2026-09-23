abstract class A {
  int get foo;
//        ^^^
// [context 1] The member being overridden.
}
class B(var num foo) implements A;
//              ^^^
// [diag.invalidOverride][context 1] 'B.foo' ('num Function()') isn't a valid override of 'A.foo' ('int Function()').

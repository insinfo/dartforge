abstract class A {
  set foo(num value);
//    ^^^
// [context 1] The setter being overridden.
}
class B(var int foo) implements A;
//              ^^^
// [diag.invalidOverrideSetter][context 1] The setter 'B.foo' ('void Function(int)') isn't a valid override of 'A.foo' ('void Function(num)').

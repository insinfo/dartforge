abstract class A {
  abstract String foo;
//                ^^^
// [context 1] The member being overridden.
// [context 2] The setter being overridden.
}
class B(var int foo) implements A;
//              ^^^
// [diag.invalidOverride][context 1] 'B.foo' ('int Function()') isn't a valid override of 'A.foo' ('String Function()').
// [diag.invalidOverrideSetter][context 2] The setter 'B.foo' ('void Function(int)') isn't a valid override of 'A.foo' ('void Function(String)').

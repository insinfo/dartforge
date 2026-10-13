abstract class A {
  abstract num x;
//             ^
// [context 1] The setter being overridden.
}
abstract class B implements A {
  int get x;
  void set x(int value);
//         ^
// [diag.invalidOverrideSetter][context 1] The setter 'B.x' ('void Function(int)') isn't a valid override of 'A.x' ('void Function(num)').
}

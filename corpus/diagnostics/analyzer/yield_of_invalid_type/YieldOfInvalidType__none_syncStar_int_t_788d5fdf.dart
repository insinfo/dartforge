void f() {
  // ignore:unused_local_variable
  Iterable<String> Function() v = () sync* {
    yield 1;
//        ^
// [diag.yieldOfInvalidType] A yielded value of type 'int' must be assignable to 'String'.
  };
}

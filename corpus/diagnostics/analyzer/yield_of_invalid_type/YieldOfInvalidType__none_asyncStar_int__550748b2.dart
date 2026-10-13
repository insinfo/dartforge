void f() {
  // ignore:unused_local_variable
  Stream<String> Function() v = () async* {
    yield 1;
//        ^
// [diag.yieldOfInvalidType] A yielded value of type 'int' must be assignable to 'String'.
  };
}

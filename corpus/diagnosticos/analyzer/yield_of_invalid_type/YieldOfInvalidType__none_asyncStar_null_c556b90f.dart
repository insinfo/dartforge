Stream<int> f() async* {
  yield null;
//      ^^^^
// [diag.yieldOfInvalidType] A yielded value of type 'Null' must be assignable to 'int'.
}

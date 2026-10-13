void foo() {
  try {
    var x = () async => return Future.value(null);
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
//                      ^^^^^^
// [diag.unexpectedToken] Unexpected text 'return'.
  } catch (_) {}
}

import 'dart:foo';

void f() {
  foo ??= 0;
//^^^
// [diag.assignmentToFinal] 'foo' can't be used as a setter because it's final.
// [diag.sdkVersionSince] This API is available since SDK 3.5.0, but constraints '>=3.4.0' don't guarantee it.
}

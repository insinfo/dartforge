import 'dart:foo';

void f() {
  foo = 0;
//^^^
// [diag.sdkVersionSince] This API is available since SDK 3.6.0, but constraints '>=3.4.0' don't guarantee it.
}

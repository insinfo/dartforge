import 'dart:foo';

void f() {
  foo += 0;
//^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
// [diag.sdkVersionSince] This API is available since SDK 3.5.0, but constraints '>=3.4.0' don't guarantee it.
}

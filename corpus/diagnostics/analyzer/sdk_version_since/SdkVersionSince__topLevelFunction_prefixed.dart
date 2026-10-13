import 'dart:foo' as foo;

void f() {
  foo.bar();
//    ^^^
// [diag.sdkVersionSince] This API is available since SDK 2.15.0, but constraints '>=2.14.0' don't guarantee it.
}

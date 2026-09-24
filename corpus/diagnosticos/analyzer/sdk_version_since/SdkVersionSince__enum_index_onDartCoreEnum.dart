void f(Enum e) {
//     ^^^^
// [diag.sdkVersionSince] This API is available since SDK 2.14.0, but constraints '>=2.12.0' don't guarantee it.
  e.index;
//  ^^^^^
// [diag.sdkVersionSince] This API is available since SDK 2.14.0, but constraints '>=2.12.0' don't guarantee it.
}

void f(List<void> values) {
  for (Object? _ in values) {}
//                  ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  for (dynamic _ in values) {}
//                  ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}

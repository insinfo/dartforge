void f(Stream<void> values) async {
  await for (Object? _ in values) {}
//                        ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
  await for (dynamic _ in values) {}
//                        ^^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}

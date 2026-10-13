void f(Stream<int> s) {
  await for (var x in s) {}
}
void g(Stream<int> s) sync* {
  await for (var x in s) {}
}
void h(Stream<int> s) async {
  () { await for (var x in s) {} };
}

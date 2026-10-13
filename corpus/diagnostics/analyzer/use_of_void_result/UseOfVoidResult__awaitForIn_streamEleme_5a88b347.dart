void f(Stream<void> values) async {
  await for (void _ in values) {}
  await for (var _ in values) {}
}

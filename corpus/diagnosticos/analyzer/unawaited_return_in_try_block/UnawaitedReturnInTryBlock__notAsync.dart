Future<int> foo() {
  try {
    return Future.value(42);
  } catch (_) {}
  return Future.value(42);
}

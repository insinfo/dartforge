Future<int> foo() async {
  try {} catch (_) {
    return Future.value(42);
  }
  return -1;
}

Future<int> foo() async {
  try {
  } finally {
    return Future.value(42);
  }
}

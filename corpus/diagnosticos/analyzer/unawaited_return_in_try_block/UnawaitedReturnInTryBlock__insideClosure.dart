void foo() {
  try {
    () async {
      return Future<Null>.value(null);
    }();
  } catch (_) {}
}

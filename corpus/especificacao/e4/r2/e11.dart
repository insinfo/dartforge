void foo() {
  try {
    var x = () async => return Future.value(null);
  } catch (_) {}
}

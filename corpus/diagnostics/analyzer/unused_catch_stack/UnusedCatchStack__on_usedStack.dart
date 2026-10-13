void f() {
  try {} on String catch (exception, stackTrace) {
    print(stackTrace);
  }
}

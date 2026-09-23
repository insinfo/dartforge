f(dynamic a) async {
  await for (int i in a) {
    i;
  }
}

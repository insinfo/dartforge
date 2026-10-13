f(dynamic e) async {
  await for (var id in e) {
    id;
  }
}

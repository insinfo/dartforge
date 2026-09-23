void f(Stream<void> stream) {
  stream.listen((_) {}, onError: (Object a, StackTrace? b) {});
}

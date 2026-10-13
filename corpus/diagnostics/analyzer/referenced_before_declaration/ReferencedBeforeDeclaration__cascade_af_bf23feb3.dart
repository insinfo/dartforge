testRequestHandler() {}

main() {
  var s1 = null;
  testRequestHandler()
    ..stream(s1);
  var stream = 123;
  print(stream);
}

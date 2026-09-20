void main() {
  var i = 100;
  for (var i = 0; i < 3; i++) {
    print(i);
    { var i = 50; print(i); }
  }
  print(i);
  for (var i = 0; i < 2; i++) {
    var i = 42;
    print(i);
  }
  var count = 0;
  for (;;) {
    count++;
    if (count == 3) { break; }
  }
  print(count);
}

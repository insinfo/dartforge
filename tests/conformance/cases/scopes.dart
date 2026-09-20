void main() {
  var value = 10;
  {
    var value = 20;
    print(value);
    { value = value + 1; }
    print(value);
  }
  print(value);
  var console = 42;
  var main = 8;
  print(console + main);
}

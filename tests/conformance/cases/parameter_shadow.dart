int shadow(int value) {
  var value = 7;
  return value;
}
int nested(int value) {
  { var value = 11; print(value); }
  return value;
}
void main() {
  print(shadow(3));
  print(nested(5));
}

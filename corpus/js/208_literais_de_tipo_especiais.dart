// Literais de tipo `dynamic`, `Never`, `Null` como valores (`Type`).
void main() {
  print(dynamic);
  print(int == dynamic);
  print(dynamic == dynamic);
  var t = dynamic;
  print(t is Type);
  print([Never, Null]);
}

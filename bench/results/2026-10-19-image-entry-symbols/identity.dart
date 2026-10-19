class Box<T> {}
void main() {
  Type first = Box<int>;
  print(first == Box<int>);
  print(first == Box<String>);
  print(identical(first, Box<int>));
  print(first.toString());
  Object value = <int>[1].runtimeType;
  print(value is Type);
  print(identityHashCode(first) == identityHashCode(Box<int>));
}

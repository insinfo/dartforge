class Point<T extends num> {
  Point(T x, T y);
}

Point<T> f<T extends num>(T x, T y) {
  return new Point<T>(x, y);
}

main() {
  print(f<String>('hello', 'world'));
//        ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}

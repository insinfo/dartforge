class Point<T extends num> {
  Point(T x, T y);
}

class PointFactory {
  Point<T> point<T extends num>(T x, T y) {
    return new Point<T>(x, y);
  }
}

f(PointFactory factory) {
  print(factory.point<String>('hello', 'world'));
//                    ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}

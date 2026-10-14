abstract class Shape {
  factory Shape.square(int side) = Square;
  factory Shape.circle(int radius) = Circle;
  int get area;
  String describe();
}

class Square implements Shape {
  int side;
  Square(this.side);
  @override
  int get area => side * side;
  @override
  String describe() => 'square:$area';
}

class Circle implements Shape {
  final int radius;
  Circle(this.radius);
  @override
  int get area => 3 * radius * radius;
  @override
  String describe() => 'circle:$area';
}

class OffsetSquare extends Square {
  OffsetSquare(int side) : super(side);
  @override
  int get area => super.area + 1;
  @override
  String describe() => 'offset:$area';
}

Shape fresh(bool square) => square ? Shape.square(4) : Shape.circle(3);
final Shape cached = OffsetSquare(5);
Shape cachedFactory() => cached;
Shape identity(Shape value) => value;
String describeOpaque(Shape value) => value.describe();

void main() {
  final square = Shape.square(4);
  final circle = Shape.circle(3);
  print(square.describe());
  print(circle.describe());
  (square as Square).side = 6;
  print(square.describe());
  print(fresh(true).describe());
  print(fresh(false).describe());
  print(cachedFactory().describe());
  print(describeOpaque(identity(OffsetSquare(7))));
  final capture = () => square;
  print(capture().describe());
  final values = <Shape>[square, circle, cachedFactory(), OffsetSquare(2)];
  for (var i = 0; i < 200; i++) {
    fresh(i.isEven);
  }
  print(values.map((value) => describeOpaque(value)).join(','));
}

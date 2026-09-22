abstract class Shape {
  String get name;
  double area();
  String describe() => '$name: ${area().toStringAsFixed(1)}';
}

class Circle extends Shape {
  final double r;
  Circle(this.r);
  @override
  String get name => 'circle';
  @override
  double area() => 3 * r * r;
}

class Rect extends Shape implements Comparable<Rect> {
  double w, h;
  static int count = 0;
  static const int max = 10;
  Rect(this.w, this.h) : super() { count++; }
  Rect.square(double s) : this(s, s);
  Rect.unit() : w = 1, h = 1;
  factory Rect.wide() => Rect(2, 1);
  String get name => 'rect';
  double area() => w * h;
  set width(double v) { w = v; }
  Rect operator +(Rect o) => Rect(w + o.w, h + o.h);
  @override
  bool operator ==(Object other) => other is Rect && other.w == w && other.h == h;
  @override
  int get hashCode => w.hashCode ^ h.hashCode;
  @override
  String toString() => 'Rect(${w.toStringAsFixed(1)} x ${h.toStringAsFixed(1)})';
  int compareTo(Rect o) => area().compareTo(o.area());
  static Rect twice(Rect r) => r + r;
}

mixin Loud {
  String shout() => toString().toUpperCase();
}

class Dog with Loud {
  late String nick;
  String toString() => 'dog';
}

class Box<T> {
  T value;
  Box(this.value);
  T get() => value;
  void set(T v) => value = v;
  bool isA<S>() => value is S;
}

enum Color { red, green, blue }

enum Planet {
  mercury(1), venus(2);
  final int order;
  const Planet(this.order);
  String get label => 'planet $name #$order';
}

class Animal {
  String speak() => '...';
  String twice() => '${speak()} ${speak()}';
}
class Cat extends Animal {
  @override
  String speak() => 'meow ' + super.speak();
}
class Dyn {
  noSuchMethod(Invocation i) => 'nsm:${i.memberName}';
}

void main() {
  extra();
  final shapes = <Shape>[Circle(1), Rect(2, 3), Rect.square(2), Rect.wide(), Rect.unit()];
  for (var s in shapes) print(s.describe());
  print(Rect.count);
  print(Rect.max);
  var r = Rect(1, 2);
  r.width = 5;
  print(r);
  print(r + r);
  print(r == Rect(5, 2));
  print(r.hashCode == Rect(5, 2).hashCode);
  print(Rect.twice(r));
  print(r.compareTo(Rect(1, 1)));
  var d = Dog();
  d.nick = 'rex';
  print(d.nick);
  print(d.shout());
  var b = Box<int>(3);
  b.set(b.get() + 1);
  print(b.value);
  print(b.isA<int>());
  print(b.isA<String>());
  print(b is Box<int>);
  print(b is Box<String>);
  Object o = b;
  print((o as Box<int>).value);
  print(Color.values);
  print(Color.green.index);
  print(Color.blue);
  print(Planet.venus.label);
  print(Cat().twice());
  dynamic dy = Dyn();
  print(dy.foo());
  print(shapes[0] is Circle);
  print(shapes[1] is! Circle);
}
class Pt {
  final int x, y;
  const Pt(this.x, this.y);
  static const origin = Pt(0, 0);
}
void extra() {
  const p = Pt(1, 2);
  print(identical(p, const Pt(1, 2)));
  print(Pt.origin.x);
}

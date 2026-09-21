class Named {
  int x;
  Named(this.x);
  Named.named(this.x);
  Named.zero() : x = 0;
}
class Base {
  int v;
  Base(this.v);
  Base.named(this.v);
}
class Derived extends Base {
  int w;
  Derived(int v) : w = v * 10, super(v + 1);
}
class Renamed extends Base {
  Renamed(int v) : super.named(v * 2);
}
class ConstBox {
  final int x;
  const ConstBox(this.x);
}
class Config {
  static int count = 1;
  static int next() => Config.count + 1;
}
class Service {
  int x;
  Service(this.x);
  factory Service.make(int v) = Target;
  factory Service.makeNamed(int v) = Target.named;
}
class Target extends Service {
  Target(int v) : super(v);
  Target.named(int v) : super(v);
}
class Getter {
  int _x = 3;
  int get value => _x;
}
int topCount = 7;
final int topLimit = 9;
int topMutable = 11;
void main() {
  print(Named(1).x);
  print(Named.named(2).x);
  print(Named.zero().x);
  var d = Derived(5);
  print(d.w);
  print(d.v);
  print(Renamed(6).v);
  print(identical(const ConstBox(1), const ConstBox(1)));
  print(const ConstBox(2).x);
  print(Config.count);
  print(Config.next());
  print(topCount);
  print(topLimit);
  print(topMutable);
  topCount = 70;
  topMutable = 110;
  print(topCount);
  print(topMutable);
  print(Getter().value);
  print(Service.make(5).x);
  print(Service.makeNamed(6).x);
}

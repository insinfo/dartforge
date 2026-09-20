class Child extends Base {
  int extra = 3;
  int value() { return this.count + this.extra; }
}
class Base {
  int count = 2;
  int value() { return this.count; }
  void setCount(int value) { this.count = value; }
}
int read(Base value) { return value.value(); }
Base? missing() {}
void main() {
  var child = Child();
  print(child.value());
  child.setCount(7);
  print(read(child));
  child.extra = 4;
  Base parent = child;
  print(parent.value());
  Base? maybe = missing();
  print(maybe == null);
  maybe = child;
  if (maybe != null) { print(maybe.value()); }
  print((missing() ?? Base()).value());
}

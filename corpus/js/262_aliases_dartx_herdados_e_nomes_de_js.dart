// A class implementing a native interface (Comparable) with the member
// inherited from a plain superclass is reachable through the extension name.
// Members named constructor or prototype work, also dynamically.
class M {
  int compareTo(int x) => x.isEven ? 1 : -1;
}

class C extends M implements Comparable<int> {}

class A {
  var constructor = 1;
  var prototype = 2;
}

class B {
  get constructor => 3;
  get prototype => 4;
}

int compare(Comparable<int> c, int x) => c.compareTo(x);

void main() {
  var c = C();
  print([compare(c, 2), compare(c, 3)]);
  var list = <dynamic>[A(), B()];
  for (var o in list) {
    print([o.constructor, o.prototype]);
  }
  var a = A();
  a.constructor = 10;
  print(a.constructor);
}

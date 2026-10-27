// Final fields that override a getter: the setter of a superclass or a mixin
// stays reachable through the subclass; without one, a dynamic write fails.
class Base {
  final log = <String>[];
  int get value => 0;
  set value(int v) => log.add('Base.value=$v');
}

mixin Writable {
  final log2 = <String>[];
  set label(String v) => log2.add('Writable.label=$v');
}

class Derived extends Base with Writable {
  @override
  final int value;
  final String label;
  final int plain;
  Derived(this.value, this.label, this.plain);
}

class Other extends Derived {
  @override
  int get value => super.value + 1;
  Other() : super(10, 'o', 3);
}

void main() {
  final d = Derived(1, 'a', 2);
  d.value = 5;
  d.label = 'b';
  print('${d.value} ${d.label} ${d.plain} ${d.log} ${d.log2}');
  final o = Other();
  o.value = 7;
  print('${o.value} ${o.log}');
  dynamic x = d;
  try {
    x.plain = 9;
    print('no error');
  } on NoSuchMethodError {
    print('NoSuchMethodError');
  }
  x.value = 6;
  print('${x.value} ${d.plain} ${d.log}');
}

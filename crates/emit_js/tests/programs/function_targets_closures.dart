// Closures whose types are only ever the subtype test's left side: tested
// against interface types, printed, compared and called. The production
// profile drops `_isFunctionSubtype` here; every answer stays the VM's.

int twice(int x) => x * 2;

class Box {
  final Object value;
  Box(this.value);
}

void main() {
  final add = (int a, int b) => a + b;
  final greet = (String name) => 'hi $name';
  final Object o = add;
  print(o is Function);
  print(o is Object);
  print(o is String);
  print(o is Comparable);
  print(add(2, 3));
  print(greet('ana'));
  print(add.runtimeType);
  print(twice.runtimeType);
  print([1, 2, 3].map(twice).toList());
  print(identical(twice, twice));
  print(add == add);
  final boxes = [Box(add), Box(greet), Box(1)];
  print(boxes.map((b) => b.value is Function).toList());
  final Object? maybe = boxes.length > 2 ? greet : null;
  print(maybe is Function?);
  try {
    print((o as String).length);
  } catch (e) {
    print(e is TypeError);
  }
}

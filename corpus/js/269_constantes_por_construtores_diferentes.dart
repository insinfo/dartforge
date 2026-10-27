// Constants made by different const constructors with the same field values
// are the same object, whatever order each constructor initializes the
// fields in.
class A {
  final x;
  final y;
  final z;
  final t;

  const A([this.z = 99, tt = 100])
      : y = 499,
        t = tt,
        x = 3;
  const A.named({z, this.t})
      : y = 400 + z,
        this.z = z,
        x = 3;

  toString() => 'A $x $y $z $t';
}

const a1 = A(99, 100);
const a2 = A.named(z: 99, t: 100);
const a3 = A();

void main() {
  print([identical(a1, a2), identical(a1, a3), a1]);
}

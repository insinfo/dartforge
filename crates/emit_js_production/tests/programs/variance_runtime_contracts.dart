class Upper {}
class Middle extends Upper {}
class Lower extends Middle {}

class Contra<in T> {}
class Invariant<inout T> {}
class Covariant<out T> {}
class Pair<in A, out B> {}
class Child<in T> extends Contra<T> {}

void check<T>(Object value, bool expected, String label) {
  if ((value is T) != expected) throw StateError(label);
}

void main() {
  check<Contra<Lower>>(Contra<Middle>(), true, 'contravariant lower');
  check<Contra<Middle>>(Contra<Middle>(), true, 'contravariant equal');
  check<Contra<Upper>>(Contra<Middle>(), false, 'contravariant upper');
  check<Invariant<Lower>>(Invariant<Middle>(), false, 'invariant lower');
  check<Invariant<Middle>>(Invariant<Middle>(), true, 'invariant equal');
  check<Invariant<Upper>>(Invariant<Middle>(), false, 'invariant upper');
  check<Covariant<Lower>>(Covariant<Middle>(), false, 'covariant lower');
  check<Covariant<Middle>>(Covariant<Middle>(), true, 'covariant equal');
  check<Covariant<Upper>>(Covariant<Middle>(), true, 'covariant upper');
  check<Pair<Lower, Upper>>(Pair<Middle, Middle>(), true, 'mixed variance');
  check<Pair<Upper, Upper>>(Pair<Middle, Middle>(), false, 'mixed input');
  check<Pair<Lower, Lower>>(Pair<Middle, Middle>(), false, 'mixed output');
  check<Child<Lower>>(Child<Middle>(), true, 'subclass declaration');
  check<Contra<Middle>>(Child<Middle>(), true, 'supertype substitution');
  print('variance contracts ok');
}

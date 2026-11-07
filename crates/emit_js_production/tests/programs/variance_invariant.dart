// Invariance needs metadata even when the class has no supertype rule.
class Invariant<inout T> {}

bool check<T>(dynamic value) => value is T;

void main() {
  if (!check<Invariant<int>>(Invariant<int>())) {
    throw StateError('lost reflexivity');
  }
  if (check<Invariant<num>>(Invariant<int>())) {
    throw StateError('lost invariance');
  }
  print('invariance ok');
}

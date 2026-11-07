// A variance-only RTI entry must survive without user supertype rules.
class Contra<in T> {}

bool check<T>(dynamic value) => value is T;

void main() {
  if (!check<Contra<int>>(Contra<Object>())) {
    throw StateError('lost contravariance');
  }
  print('contravariance ok');
}

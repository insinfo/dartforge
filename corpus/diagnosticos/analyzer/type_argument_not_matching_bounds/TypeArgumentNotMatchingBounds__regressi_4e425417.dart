typedef G<X> = Function(X);
class A<X extends G<A<X, Y>>, Y extends Never> {}

test<X>() { print("OK"); }

main() {
  test<A<G<A<Never, Never>>, Object?>>();
}

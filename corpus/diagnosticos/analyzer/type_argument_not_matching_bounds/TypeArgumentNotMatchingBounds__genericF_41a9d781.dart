typedef F1 = T Function<T>(T);
typedef F2 = S Function<S>(S);
class CB<T extends F1> {}
void f(CB<F2> a) {}

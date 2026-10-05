typedef F<X> = void Function(X);
typedef L = List<int>;
typedef A<X> = int;
typedef R = (int, String);

void f(F<int> a, L? b, List<F<String>> c, A<String> d, R e, F<int>? g) {
  String s1 = a;
  String s2 = b;
  String s3 = c;
  String s4 = d;
  String s5 = e;
  String s6 = g;
  print([s1, s2, s3, s4, s5, s6]);
}

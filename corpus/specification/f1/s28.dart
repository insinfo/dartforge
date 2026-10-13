int topv = 0;
void topf<T>() {}
typedef TF<T> = void Function(T);
typedef TD<T> = T;
class A<T> {
  A.named();
}
f(x) {
  UnresolvedClass<int>.named();
  unresolved.Class<int>.named();
  x.a<int>.b();
  topv<int>.b();
  topf<int>.b();
  TF<int>.b();
  TD<int>.named();
  A<int>.named();
  A<int>.zzz();
}

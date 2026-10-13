class I {
  int call(int x) => 0;
}
class C implements I {
  noSuchMethod(_) => null;
}
typedef int IntToInt(int x);
IntToInt f = new I();

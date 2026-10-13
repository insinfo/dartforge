int v = 0;
String v = "";
void g() {}
int g() => 0;
int get h => 0;
String get h => "";
void f() {
  v.isEven;
  v.length;
  int x = g();
  h.isEven;
  h.length;
  print(x);
}

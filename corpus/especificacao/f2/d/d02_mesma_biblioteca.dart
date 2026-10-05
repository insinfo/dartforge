@deprecated
void _velha() {}
@deprecated
int x = 0;
void f({@deprecated int? p}) { print(p); }
void main() {
  _velha();
  x = 1;
  x++;
  f(p: 1);
}

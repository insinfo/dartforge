print(x) {}
void f(int p) {
  p.();
//  ^
// [diag.missingIdentifier] Expected an identifier.
// [diag.undefinedGetter] The getter '(' isn't defined for the type 'int'.
}

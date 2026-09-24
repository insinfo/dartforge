class byte {
  int _value;
//    ^^^^^^
// [diag.unusedField] The value of the field '_value' isn't used.
  byte(this._value);
  byte operator +(int val) { return this; }
}

void main() {
  byte b = new byte(52);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
  b += 3;
}

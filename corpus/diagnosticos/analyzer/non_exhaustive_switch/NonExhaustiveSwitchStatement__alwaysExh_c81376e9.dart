void f(Null x) {
  switch (x) {}
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'Null' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'null'.
}

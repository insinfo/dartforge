typedef Getter(self);
Getter getter = (x) => x;
main() {
  getter();
//       ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'getter', but 0 found.
}

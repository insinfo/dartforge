// Identidade de strings iguais vindas do programa e do SDK. No perfil de
// desenvolvimento o SDK mora numa DLL com os seus literais estáticos; o
// literal do programa tem de ser o mesmo objeto (canonicalizado), como na VM.
void main() {
  print(identical('a'.substring(1, 1), ''));
  print(identical('abc', 'abc'));
  const c = 'x';
  print(identical(c, 'x'));
  print(identical('a' + 'b', 'ab'));
  print(identical(''.trim(), ''));
  print(identical('ab'.substring(0, 0), ''));
  print(identical([].join(), ''));
  print(identical(''.toUpperCase(), ''));
}

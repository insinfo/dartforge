// Biblioteca auxiliar do 92: literais iguais aos de main.dart, para conferir
// que a canonicalização de constantes vale entre bibliotecas.

const literalConst = 'olá, mundo';
const emojiConst = 'ok 😀';
final literalFinal = 'olá, mundo';

String literalDeFuncao() => 'texto compartilhado';
String emojiDeFuncao() => 'ok 😀';
String vazioDeFuncao() => '';

const listaConst = ['a', 'b', 'olá, mundo'];

String montadoNaParte(int n) => 'texto ${n > 0 ? 'compartilhado' : 'outro'}';

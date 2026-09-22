// Convertido de tests/conformance/modules/strings25 (módulo antigo do corpus de conformidade).
import 'etiquetas.dart';

// Literais de string do subconjunto: interpolação, aspas triplas e adjacência.
// A saída esperada em main.stdout foi produzida por `dart run` com o Dart SDK
// 3.6.2 instalado, usado como oráculo real para cada construção deste arquivo.

class Pessoa {
  String nome = 'Ana';
  int idade = 34;
}

class Contador {
  int valor = 0;
  int proximo() {
    valor = valor + 1;
    return valor;
  }
}

void main() {
  var nome = 'Ana';
  var idade = 34;

  // Interpolação simples e com expressão entre chaves.
  print('Ola, $nome! Voce tem $idade anos.');
  print('Ano que vem: ${idade + 1}');

  // $nome.campo interpola apenas nome; ${obj.campo} interpola tudo.
  var pessoa = Pessoa();
  print('$nome.nome');
  print('${pessoa.nome} tem ${pessoa.idade}');

  // O escape mantém o cifrão literal, assim como a string raw.
  print('\$nome nao interpola');
  print(r'$nome tambem nao');

  // Literais adjacentes concatenam em tempo de compilação, com ou sem interpolação.
  print('a' 'b' ' $nome ' r'$cru');
  print('vazio:' '');

  // Aspas triplas: a primeira quebra de linha logo após a abertura some.
  print('''
linha 1
linha 2 de $nome''');
  print('''sem quebra inicial''');
  print("""
dupla com ${idade - 4}""");
  print(r'''cru triplo: \n $nome''');

  // null vira "null"; bool e List seguem o toString do Dart.
  int? ausente = null;
  print('ausente=$ausente');
  print('${true} ${false} ${[1, 2, 3]}');

  // Cada expressão é avaliada exatamente uma vez, na ordem escrita.
  var contador = Contador();
  print('${contador.proximo()} ${contador.proximo()} ${contador.proximo()}');
  print('total=${contador.valor}');

  // Nomes de outra biblioteca dentro da interpolação.
  print('${etiqueta()}: ${dobro(idade)}');

  // Interpolação aninhada dentro de outra interpolação.
  print('fora ${'dentro $nome'} fim');

  // Aspas simples dentro de aspas triplas e escapes Unicode preservados.
  print('''aspas ' e " sem escape''');
  print('unicode é \u{1F600} fim $idade');
}

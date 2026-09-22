// Interpolação de strings: $x, ${expr}, aninhada, métodos, toString implícito e $ escapado.
class Ponto {
  final int x, y;
  Ponto(this.x, this.y);
  @override
  String toString() => 'Ponto($x, $y)';
}

class SemToString {}

void main() {
  var nome = 'Ana';
  var idade = 30;
  var altura = 1.75;
  var ativo = true;
  Object? nada;
  var lista = [1, 2, 3];
  var mapa = {'k': 'v', 'n': 2};
  var p = Ponto(3, 4);

  print('nome: $nome');
  print('idade: $idade');
  print('altura: $altura');
  print('ativo: $ativo');
  print('nada: $nada');
  print('lista: $lista');
  print('mapa: $mapa');
  print('ponto: $p');
  print('${nome}x');
  print('$nome$idade');
  print('${idade + 1}');
  print('${idade * 2 + 1} anos');
  print('${nome.toUpperCase()} tem ${nome.length} letras');
  print('${lista.length} itens, primeiro ${lista[0]}, último ${lista.last}');
  print('${mapa['k']} e ${mapa['n']}');
  print('${ativo ? 'sim' : 'não'}');
  print('${'interna ${'mais ${idade}'}'}');
  print('${[1, 2].map((e) => e * 10).join(',')}');
  print('${lista.map((e) => '<$e>').toList()}');
  print('preço: \$$idade');
  print('\$');
  print('\${nao_interpola}');
  print(r'$nome nao interpola');
  print('${p.x + p.y}');
  print('${SemToString().toString() == "Instance of 'SemToString'"}');
  print('${null}');
  print('${1 == 1}');
  print('${'a' * 3}');
  print('${(1, 'b')}');
  print('a' 'b' '$nome' 'c');
  print("aspas \"duplas\" e $nome");
  print('${idade}${idade}');
  print('${-idade}');
  print('$nome.length');
  print('${{'x': [1, 2]}}');
  print('${nada ?? 'vazio'}');
  print('${lista.isEmpty}');
  print('${'${'${'fundo'}'}'}');
}

import 'package:ngdart/angular.dart';

class Item {
  final String nome;
  Item(this.nome);
}

/// `isImmutable` do oficial em expressões compostas: `a.b` e `!x` são
/// mutáveis mesmo com tudo `final`; o campo `final` lido direto é imutável e,
/// como pode ser nulo (`canBeNull`), a ligação constante ganha um
/// `if (x != null)`; `a ?? b` de imutáveis é imutável.
@Component(
  selector: 'a21-imutabilidade-composta',
  templateUrl: 'a21_imutabilidade_composta.html',
)
class A21ImutabilidadeComposta {
  final Item item = Item('x');
  final bool fixo = true;
  final String rotulo = 'r';
  final int? numero = null;
  String? talvez;
}

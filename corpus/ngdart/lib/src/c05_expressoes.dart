import 'package:ngdart/angular.dart';

class Item {
  String nome = 'x';
  bool ativo = true;
}

/// Formas de expressão em ligação: cadeia, chamada de método, literal e
/// negação.
@Component(
  selector: 'c05-expressoes',
  templateUrl: 'c05_expressoes.html',
)
class C05Expressoes {
  Item item = Item();

  String titulo() => 't';
}

import 'package:ngdart/angular.dart';

class Pessoa {
  String nome = 'n';
  int idade = 1;
}

/// Sonda: `?.` em interpolação e ligação.
@Component(
  selector: 'i13-seguro-nulo',
  templateUrl: 'i13_seguro_nulo.html',
  directives: [coreDirectives],
)
class I13SeguroNulo {
  Pessoa? pessoa;
}

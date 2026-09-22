import 'package:ngdart/angular.dart';

class Item {
  String nome = 'x';
  int quantidade = 1;
}

/// `{{ a.b }}` com tipos diferentes no fim da cadeia.
@Component(
  selector: 'c06-interpolacao-em-cadeia',
  templateUrl: 'c06_interpolacao_em_cadeia.html',
)
class C06InterpolacaoEmCadeia {
  Item item = Item();
}

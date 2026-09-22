import 'package:ngdart/angular.dart';

class Pessoa {
  String nome = 'x';
}

/// `{{ obj.prop }}` — acesso encadeado.
@Component(
  selector: 'a06-acesso-a-propriedade',
  templateUrl: 'a06_acesso_a_propriedade.html',
)
class A06AcessoAPropriedade {
  Pessoa pessoa = Pessoa();
}

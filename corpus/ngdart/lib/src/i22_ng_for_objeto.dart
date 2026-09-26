import 'package:ngdart/angular.dart';

class Pessoa {
  String nome = 'n';
  int idade = 1;
}

/// Sonda: `*ngFor` sobre lista de objetos.
@Component(
  selector: 'i22-ng-for-objeto',
  templateUrl: 'i22_ng_for_objeto.html',
  directives: [coreDirectives],
)
class I22NgForObjeto {
  List<Pessoa> pessoas = [Pessoa()];
  void escolher(Pessoa p) {}
}

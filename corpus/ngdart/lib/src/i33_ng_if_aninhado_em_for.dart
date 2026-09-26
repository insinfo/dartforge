import 'package:ngdart/angular.dart';

class Pessoa {
  String nome = 'n';
  int idade = 1;
}

/// Sonda: `*ngIf` dentro de `*ngFor` lendo o local.
@Component(
  selector: 'i33-ng-if-aninhado-em-for',
  templateUrl: 'i33_ng_if_aninhado_em_for.html',
  directives: [coreDirectives],
)
class I33NgIfAninhadoEmFor {
  List<Pessoa> pessoas = [Pessoa()];
}

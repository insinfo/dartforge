import 'package:ngdart/angular.dart';

/// Sonda: `ngSwitch` em `*ngFor`, caso dinâmico, `ngSwitchWhen` e `*` dentro do caso.
@Component(
  selector: 'i42-ng-switch-formas',
  templateUrl: 'i42_ng_switch_formas.html',
  directives: [coreDirectives],
)
class I42NgSwitchFormas {
  List<String> itens = ['a'];
  String modo = 'a';
  bool mostrar = true;
}

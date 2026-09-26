import 'package:ngdart/angular.dart';

/// Sonda: `[ngClass]` com `class` e `[class.x]` em `*ngFor`, `[ngStyle]` com `style`.
@Component(
  selector: 'i41-ng-class-formas',
  templateUrl: 'i41_ng_class_formas.html',
  directives: [coreDirectives],
)
class I41NgClassFormas {
  List<String> itens = ['a'];
  bool ativo = true;
  String x = 'y';
  Map<String, String> estilos = {'top': '1px'};
}

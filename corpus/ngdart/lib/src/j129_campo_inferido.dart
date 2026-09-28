import 'package:ngdart/angular.dart';

/// Campo sem tipo escrito numa classe de fora (`final itemsRole =
/// 'menuitem';` do `MenuItemGroup`): o tipo é o que o analyzer infere do
/// inicializador, e escolhe o `interpolateString0`/`interpolate0`.
class J129Grupo {
  final papel = 'menuitem';
  final quantos = 3;
  var ativo = true;
}

@Component(
  selector: 'j129-item',
  template: '<ng-content></ng-content>',
)
class J129Item {}

@Component(
  selector: 'j129-usa',
  template: '''<p>{{g.papel}} {{g.quantos}} {{g.ativo}}</p>
<j129-item role="{{g.papel}}" title="{{g.quantos}}">x</j129-item>''',
  directives: [J129Item],
)
class J129Usa {
  J129Grupo g = J129Grupo();
}

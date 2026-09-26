import 'package:ngdart/angular.dart';

/// Sonda: `<ng-container>` sem `*`, com âncora e várias raízes na visão embutida.
@Component(
  selector: 'i39-ng-container-formas',
  templateUrl: 'i39_ng_container_formas.html',
  directives: [coreDirectives],
)
class I39NgContainerFormas {
  int n = 1;
  bool mostrar = true;
  List<String> itens = ['a'];
  void usar(String x) {}
}

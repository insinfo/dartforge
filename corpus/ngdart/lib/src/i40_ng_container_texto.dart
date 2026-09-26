import 'package:ngdart/angular.dart';

/// Sonda: texto solto na raiz da visão embutida de um `<ng-container>`.
@Component(
  selector: 'i40-ng-container-texto',
  templateUrl: 'i40_ng_container_texto.html',
  directives: [coreDirectives],
)
class I40NgContainerTexto {
  bool mostrar = true;
}

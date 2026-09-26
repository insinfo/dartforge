import 'package:ngdart/angular.dart';

/// Sonda: `<ng-container>` com `*ngFor`.
@Component(
  selector: 'i21-ng-container-for',
  templateUrl: 'i21_ng_container_for.html',
  directives: [coreDirectives],
)
class I21NgContainerFor {
  List<String> itens = ['a'];
}

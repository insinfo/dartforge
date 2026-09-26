import 'package:ngdart/angular.dart';

/// Sonda: `<ng-container>` com `*ngIf`.
@Component(
  selector: 'i09-ng-container',
  templateUrl: 'i09_ng_container.html',
  directives: [coreDirectives],
)
class I09NgContainer {
  bool mostrar = true;
}

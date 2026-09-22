import 'package:ngdart/angular.dart';

/// `*ngIf` — visão embutida e `ViewContainer`.
@Component(
  selector: 'a09-ng-if',
  templateUrl: 'a09_ng_if.html',
  directives: [coreDirectives],
)
class A09NgIf {
  bool mostrar = true;
}

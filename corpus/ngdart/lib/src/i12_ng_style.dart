import 'package:ngdart/angular.dart';

/// Sonda: `[ngStyle]`.
@Component(
  selector: 'i12-ng-style',
  templateUrl: 'i12_ng_style.html',
  directives: [coreDirectives],
)
class I12NgStyle {
  Map<String, String> estilos = {'color': 'red'};
}

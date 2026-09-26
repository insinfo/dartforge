import 'package:ngdart/angular.dart';

/// Sonda: `[ngClass]`.
@Component(
  selector: 'i11-ng-class',
  templateUrl: 'i11_ng_class.html',
  directives: [coreDirectives],
)
class I11NgClass {
  Map<String, bool> classes = {'x': true};
  List<String> lista = ['a'];
}

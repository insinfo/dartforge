import 'package:ngdart/angular.dart';

/// Sonda: `[attr.x]`, `[class.x]`, `[class]`.
@Component(
  selector: 'i28-attr-class',
  templateUrl: 'i28_attr_class.html',
  directives: [coreDirectives],
)
class I28AttrClass {
  bool ativo = true;
  String pressionado = 'true';
  String classe = 'c';
  String id = '1';
}

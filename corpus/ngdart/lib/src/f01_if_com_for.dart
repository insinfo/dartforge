import 'package:ngdart/angular.dart';

/// `*ngIf` contendo `*ngFor`: a numeração das visões embutidas aninhadas.
@Component(
  selector: 'f01-if-com-for',
  templateUrl: 'f01_if_com_for.html',
  directives: [coreDirectives],
)
class F01IfComFor {
  bool mostrar = true;
  List<String> itens = ['a'];
}

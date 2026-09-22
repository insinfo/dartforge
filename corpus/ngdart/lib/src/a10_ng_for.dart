import 'package:ngdart/angular.dart';

/// `*ngFor` — visão embutida com contexto de laço.
@Component(
  selector: 'a10-ng-for',
  templateUrl: 'a10_ng_for.html',
  directives: [coreDirectives],
)
class A10NgFor {
  List<String> itens = ['a', 'b'];
}

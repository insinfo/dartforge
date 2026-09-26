import 'package:ngdart/angular.dart';

/// Sonda: `*ngFor` com `first`/`last`/`even`/`odd`.
@Component(
  selector: 'i03-ng-for-first-last',
  templateUrl: 'i03_ng_for_first_last.html',
  directives: [coreDirectives],
)
class I03NgForFirstLast {
  List<String> itens = ['a', 'b'];
}

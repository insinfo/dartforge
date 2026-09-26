import 'package:ngdart/angular.dart';

/// Sonda: `*ngFor` com `index`.
@Component(
  selector: 'i01-ng-for-index',
  templateUrl: 'i01_ng_for_index.html',
  directives: [coreDirectives],
)
class I01NgForIndex {
  List<String> itens = ['a', 'b'];
}

import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// `<select [(ngModel)]>` casa o `SelectControlValueAccessor`, e o
/// `<option>` o `NgSelectOption`: fora do catálogo de diretivas, recusado.
@Component(
  selector: 'g02-select-ng-model',
  templateUrl: 'g02_select_ng_model.html',
  directives: [formDirectives],
)
class G02SelectNgModel {
  String escolha = 'a';
}

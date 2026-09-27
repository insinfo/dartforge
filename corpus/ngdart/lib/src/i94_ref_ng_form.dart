import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// Sonda: `#f="ngForm"`.
@Component(
  selector: 'i94-ref-ng-form',
  templateUrl: 'i94_ref_ng_form.html',
  directives: [formDirectives],
)
class I94RefNgForm {
  String nome = '';
}

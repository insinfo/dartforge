import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// Um `<form>` simples com `formDirectives` ganha `NgForm` pelo seletor
/// (`form:not([ngNoForm]):not([ngFormModel])`). Emitir o elemento puro
/// compilaria e faria outra coisa: o gerador recusa até saber instanciar a
/// diretiva.
@Component(
  selector: 'a22-form-com-forms-directives',
  templateUrl: 'a22_form_com_forms_directives.html',
  directives: [formDirectives],
)
class A22FormComFormsDirectives {}

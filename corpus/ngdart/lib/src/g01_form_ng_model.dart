import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// Formulário com `ngforms`: `NgForm` no `<form>` com `(ngSubmit)`,
/// `[(ngModel)]` em `input` e `textarea` (`NgModel` e
/// `DefaultValueAccessor`), `required` estático e ligado
/// (`RequiredValidator`), um `[disabled]` do próprio nó e um campo dentro de
/// `*ngIf`. Os provedores saem na ordem das dependências, com o
/// `injectorGetInternal` de cada visão.
@Component(
  selector: 'g01-form-ng-model',
  templateUrl: 'g01_form_ng_model.html',
  directives: [coreDirectives, formDirectives],
)
class G01FormNgModel {
  String nome = '';
  String texto = '';
  String senha = '';
  bool bloqueado = false;
  bool mostra = true;

  void salvar() {}
}

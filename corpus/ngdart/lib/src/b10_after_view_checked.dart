import 'package:ngdart/angular.dart';

/// `AfterViewChecked`: o que o oficial emite na visão-hospedeira por causa dele.
@Component(
  selector: 'b10-after-view-checked',
  templateUrl: 'b10_after_view_checked.html',
)
class B10AfterViewChecked implements AfterViewChecked {
  @override
  void ngAfterViewChecked() {}
}

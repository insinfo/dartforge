import 'package:ngdart/angular.dart';

/// `AfterChanges`: o que o oficial emite na visão-hospedeira por causa dele.
@Component(
  selector: 'b08-after-changes',
  templateUrl: 'b08_after_changes.html',
)
class B08AfterChanges implements AfterChanges {
  @override
  void ngAfterChanges() {}
}

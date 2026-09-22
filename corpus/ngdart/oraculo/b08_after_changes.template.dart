// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b08_after_changes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b08_after_changes.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B08AfterChanges = const [];

class ViewB08AfterChanges0 extends import0.ComponentView<import1.B08AfterChanges> {
  static import2.ComponentStyles? _componentStyles;
  ViewB08AfterChanges0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b08-after-changes'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b08_after_changes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B08AfterChanges, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B08AfterChangesNgFactory = ComponentFactory<import1.B08AfterChanges>('b08-after-changes', viewFactory_B08AfterChangesHost0);
ComponentFactory<import1.B08AfterChanges> get B08AfterChangesNgFactory {
  return _B08AfterChangesNgFactory;
}

ComponentFactory<import1.B08AfterChanges> createB08AfterChangesFactory() {
  return ComponentFactory('b08-after-changes', viewFactory_B08AfterChangesHost0);
}

final List<Object> styles$B08AfterChangesHost = const [];

class _ViewB08AfterChangesHost0 extends import9.HostView<import1.B08AfterChanges> {
  @override
  void build() {
    this.componentView = ViewB08AfterChanges0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B08AfterChanges();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B08AfterChanges> viewFactory_B08AfterChangesHost0() {
  return _ViewB08AfterChangesHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b12_do_check.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b12_do_check.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B12DoCheck = const [];

class ViewB12DoCheck0 extends import0.ComponentView<import1.B12DoCheck> {
  static import2.ComponentStyles? _componentStyles;
  ViewB12DoCheck0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b12-do-check'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b12_do_check.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B12DoCheck, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B12DoCheckNgFactory = ComponentFactory<import1.B12DoCheck>('b12-do-check', viewFactory_B12DoCheckHost0);
ComponentFactory<import1.B12DoCheck> get B12DoCheckNgFactory {
  return _B12DoCheckNgFactory;
}

ComponentFactory<import1.B12DoCheck> createB12DoCheckFactory() {
  return ComponentFactory('b12-do-check', viewFactory_B12DoCheckHost0);
}

final List<Object> styles$B12DoCheckHost = const [];

class _ViewB12DoCheckHost0 extends import9.HostView<import1.B12DoCheck> {
  @override
  void build() {
    this.componentView = ViewB12DoCheck0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B12DoCheck();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    if ((!import10.debugThrowIfChanged)) {
      this.component.ngDoCheck();
    }
    this.componentView.detectChanges();
  }
}

import9.HostView<import1.B12DoCheck> viewFactory_B12DoCheckHost0() {
  return _ViewB12DoCheckHost0();
}

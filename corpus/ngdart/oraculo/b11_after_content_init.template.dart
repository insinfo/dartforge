// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b11_after_content_init.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b11_after_content_init.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B11AfterContentInit = const [];

class ViewB11AfterContentInit0 extends import0.ComponentView<import1.B11AfterContentInit> {
  static import2.ComponentStyles? _componentStyles;
  ViewB11AfterContentInit0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b11-after-content-init'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b11_after_content_init.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B11AfterContentInit, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B11AfterContentInitNgFactory = ComponentFactory<import1.B11AfterContentInit>('b11-after-content-init', viewFactory_B11AfterContentInitHost0);
ComponentFactory<import1.B11AfterContentInit> get B11AfterContentInitNgFactory {
  return _B11AfterContentInitNgFactory;
}

ComponentFactory<import1.B11AfterContentInit> createB11AfterContentInitFactory() {
  return ComponentFactory('b11-after-content-init', viewFactory_B11AfterContentInitHost0);
}

final List<Object> styles$B11AfterContentInitHost = const [];

class _ViewB11AfterContentInitHost0 extends import9.HostView<import1.B11AfterContentInit> {
  @override
  void build() {
    this.componentView = ViewB11AfterContentInit0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B11AfterContentInit();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
    }
    this.componentView.detectChanges();
  }
}

import9.HostView<import1.B11AfterContentInit> viewFactory_B11AfterContentInitHost0() {
  return _ViewB11AfterContentInitHost0();
}

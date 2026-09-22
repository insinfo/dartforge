// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b09_after_view_init.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b09_after_view_init.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B09AfterViewInit = const [];

class ViewB09AfterViewInit0 extends import0.ComponentView<import1.B09AfterViewInit> {
  static import2.ComponentStyles? _componentStyles;
  ViewB09AfterViewInit0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b09-after-view-init'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b09_after_view_init.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B09AfterViewInit, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B09AfterViewInitNgFactory = ComponentFactory<import1.B09AfterViewInit>('b09-after-view-init', viewFactory_B09AfterViewInitHost0);
ComponentFactory<import1.B09AfterViewInit> get B09AfterViewInitNgFactory {
  return _B09AfterViewInitNgFactory;
}

ComponentFactory<import1.B09AfterViewInit> createB09AfterViewInitFactory() {
  return ComponentFactory('b09-after-view-init', viewFactory_B09AfterViewInitHost0);
}

final List<Object> styles$B09AfterViewInitHost = const [];

class _ViewB09AfterViewInitHost0 extends import9.HostView<import1.B09AfterViewInit> {
  @override
  void build() {
    this.componentView = ViewB09AfterViewInit0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B09AfterViewInit();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectChanges();
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
    }
  }
}

import9.HostView<import1.B09AfterViewInit> viewFactory_B09AfterViewInitHost0() {
  return _ViewB09AfterViewInitHost0();
}

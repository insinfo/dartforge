// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b23_so_on_destroy.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b23_so_on_destroy.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B23SoOnDestroy = const [];

class ViewB23SoOnDestroy0 extends import0.ComponentView<import1.B23SoOnDestroy> {
  static import2.ComponentStyles? _componentStyles;
  ViewB23SoOnDestroy0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b23-so-on-destroy'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b23_so_on_destroy.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B23SoOnDestroy, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B23SoOnDestroyNgFactory = ComponentFactory<import1.B23SoOnDestroy>('b23-so-on-destroy', viewFactory_B23SoOnDestroyHost0);
ComponentFactory<import1.B23SoOnDestroy> get B23SoOnDestroyNgFactory {
  return _B23SoOnDestroyNgFactory;
}

ComponentFactory<import1.B23SoOnDestroy> createB23SoOnDestroyFactory() {
  return ComponentFactory('b23-so-on-destroy', viewFactory_B23SoOnDestroyHost0);
}

final List<Object> styles$B23SoOnDestroyHost = const [];

class _ViewB23SoOnDestroyHost0 extends import9.HostView<import1.B23SoOnDestroy> {
  @override
  void build() {
    this.componentView = ViewB23SoOnDestroy0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B23SoOnDestroy();
    this.initRootNode(_el_0);
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import9.HostView<import1.B23SoOnDestroy> viewFactory_B23SoOnDestroyHost0() {
  return _ViewB23SoOnDestroyHost0();
}

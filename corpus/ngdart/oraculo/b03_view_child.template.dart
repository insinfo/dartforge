// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b03_view_child.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b03_view_child.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B03ViewChild = const [];

class ViewB03ViewChild0 extends import0.ComponentView<import1.B03ViewChild> {
  static import2.ComponentStyles? _componentStyles;
  ViewB03ViewChild0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b03-view-child'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b03_view_child.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
    _ctx.caixa = _el_0;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B03ViewChild, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B03ViewChildNgFactory = ComponentFactory<import1.B03ViewChild>('b03-view-child', viewFactory_B03ViewChildHost0);
ComponentFactory<import1.B03ViewChild> get B03ViewChildNgFactory {
  return _B03ViewChildNgFactory;
}

ComponentFactory<import1.B03ViewChild> createB03ViewChildFactory() {
  return ComponentFactory('b03-view-child', viewFactory_B03ViewChildHost0);
}

final List<Object> styles$B03ViewChildHost = const [];

class _ViewB03ViewChildHost0 extends import9.HostView<import1.B03ViewChild> {
  @override
  void build() {
    this.componentView = ViewB03ViewChild0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B03ViewChild();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B03ViewChild> viewFactory_B03ViewChildHost0() {
  return _ViewB03ViewChildHost0();
}

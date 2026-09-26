// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i15_view_children.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i15_view_children.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I15ViewChildren = const [];

class ViewI15ViewChildren0 extends import0.ComponentView<import1.I15ViewChildren> {
  static import2.ComponentStyles? _componentStyles;
  ViewI15ViewChildren0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i15-view-children'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i15_view_children.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, '1');
    final _el_2 = import7.appendDiv(doc, parentRenderNode);
    final _text_3 = import7.appendText(_el_2, '2');
    _ctx.caixas = [_el_0, _el_2];
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I15ViewChildren, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I15ViewChildrenNgFactory = ComponentFactory<import1.I15ViewChildren>('i15-view-children', viewFactory_I15ViewChildrenHost0);
ComponentFactory<import1.I15ViewChildren> get I15ViewChildrenNgFactory {
  return _I15ViewChildrenNgFactory;
}

ComponentFactory<import1.I15ViewChildren> createI15ViewChildrenFactory() {
  return ComponentFactory('i15-view-children', viewFactory_I15ViewChildrenHost0);
}

final List<Object> styles$I15ViewChildrenHost = const [];

class _ViewI15ViewChildrenHost0 extends import9.HostView<import1.I15ViewChildren> {
  @override
  void build() {
    this.componentView = ViewI15ViewChildren0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I15ViewChildren();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.I15ViewChildren> viewFactory_I15ViewChildrenHost0() {
  return _ViewI15ViewChildrenHost0();
}

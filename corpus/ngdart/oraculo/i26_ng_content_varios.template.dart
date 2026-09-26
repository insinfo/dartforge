// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i26_ng_content_varios.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i26_ng_content_varios.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I26NgContentVarios = const [];

class ViewI26NgContentVarios0 extends import0.ComponentView<import1.I26NgContentVarios> {
  static import2.ComponentStyles? _componentStyles;
  ViewI26NgContentVarios0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i26-ng-content-varios'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i26_ng_content_varios.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'header');
    this.project(_el_0, 0);
    final _el_1 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'main');
    this.project(_el_1, 1);
    this.project(parentRenderNode, 2);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I26NgContentVarios, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I26NgContentVariosNgFactory = ComponentFactory<import1.I26NgContentVarios>('i26-ng-content-varios', viewFactory_I26NgContentVariosHost0);
ComponentFactory<import1.I26NgContentVarios> get I26NgContentVariosNgFactory {
  return _I26NgContentVariosNgFactory;
}

ComponentFactory<import1.I26NgContentVarios> createI26NgContentVariosFactory() {
  return ComponentFactory('i26-ng-content-varios', viewFactory_I26NgContentVariosHost0);
}

final List<Object> styles$I26NgContentVariosHost = const [];

class _ViewI26NgContentVariosHost0 extends import9.HostView<import1.I26NgContentVarios> {
  @override
  void build() {
    this.componentView = ViewI26NgContentVarios0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I26NgContentVarios();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.I26NgContentVarios> viewFactory_I26NgContentVariosHost0() {
  return _ViewI26NgContentVariosHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i37_filho_callback.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i37_filho_callback.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I37FilhoCallback = const [];

class ViewI37FilhoCallback0 extends import0.ComponentView<import1.I37FilhoCallback> {
  static import2.ComponentStyles? _componentStyles;
  ViewI37FilhoCallback0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i37-filho-callback'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i37_filho_callback.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I37FilhoCallback, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I37FilhoCallbackNgFactory = ComponentFactory<import1.I37FilhoCallback>('i37-filho-callback', viewFactory_I37FilhoCallbackHost0);
ComponentFactory<import1.I37FilhoCallback> get I37FilhoCallbackNgFactory {
  return _I37FilhoCallbackNgFactory;
}

ComponentFactory<import1.I37FilhoCallback> createI37FilhoCallbackFactory() {
  return ComponentFactory('i37-filho-callback', viewFactory_I37FilhoCallbackHost0);
}

final List<Object> styles$I37FilhoCallbackHost = const [];

class _ViewI37FilhoCallbackHost0 extends import9.HostView<import1.I37FilhoCallback> {
  @override
  void build() {
    this.componentView = ViewI37FilhoCallback0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I37FilhoCallback();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.I37FilhoCallback> viewFactory_I37FilhoCallbackHost0() {
  return _ViewI37FilhoCallbackHost0();
}

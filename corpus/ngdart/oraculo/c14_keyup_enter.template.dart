// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c14_keyup_enter.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c14_keyup_enter.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/core/linker/app_view_utils.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$C14KeyupEnter = const [];

class ViewC14KeyupEnter0 extends import0.ComponentView<import1.C14KeyupEnter> {
  static import2.ComponentStyles? _componentStyles;
  ViewC14KeyupEnter0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('c14-keyup-enter'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/c14_keyup_enter.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    final _el_1 = import7.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    import8.appViewUtils.eventManager.addEventListener(_el_0, 'keyup.enter', this.eventHandler0(_ctx.enviar));
    import8.appViewUtils.eventManager.addEventListener(_el_1, 'keydown.esc', this.eventHandler1(_ctx.tecla));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$C14KeyupEnter, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C14KeyupEnterNgFactory = ComponentFactory<import1.C14KeyupEnter>('c14-keyup-enter', viewFactory_C14KeyupEnterHost0);
ComponentFactory<import1.C14KeyupEnter> get C14KeyupEnterNgFactory {
  return _C14KeyupEnterNgFactory;
}

ComponentFactory<import1.C14KeyupEnter> createC14KeyupEnterFactory() {
  return ComponentFactory('c14-keyup-enter', viewFactory_C14KeyupEnterHost0);
}

final List<Object> styles$C14KeyupEnterHost = const [];

class _ViewC14KeyupEnterHost0 extends import10.HostView<import1.C14KeyupEnter> {
  @override
  void build() {
    this.componentView = ViewC14KeyupEnter0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C14KeyupEnter();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.C14KeyupEnter> viewFactory_C14KeyupEnterHost0() {
  return _ViewC14KeyupEnterHost0();
}

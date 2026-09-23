// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b20_view_child_dois.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b20_view_child_dois.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B20ViewChildDois = const [];

class ViewB20ViewChildDois0 extends import0.ComponentView<import1.B20ViewChildDois> {
  Object? _expr_0;
  late final import2.DivElement _el_2;
  static import3.ComponentStyles? _componentStyles;
  ViewB20ViewChildDois0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('b20-view-child-dois'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/b20_view_child_dois.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    final _el_0 = import7.appendSpan(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'a');
    this._el_2 = import7.appendDiv(doc, parentRenderNode);
    final _text_3 = import7.appendText(this._el_2, 'b');
    final _el_4 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_5 = import7.appendText(_el_4, 'c');
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
    _ctx.segundo = this._el_2;
    _ctx.primeiro = _el_0;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.titulo;
    if (import8.checkBinding(this._expr_0, currVal_0, 'titulo', 'package:corpus_ngdart/src/b20_view_child_dois.html')) {
      import7.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/b20_view_child_dois.html:57:73 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$B20ViewChildDois, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B20ViewChildDoisNgFactory = ComponentFactory<import1.B20ViewChildDois>('b20-view-child-dois', viewFactory_B20ViewChildDoisHost0);
ComponentFactory<import1.B20ViewChildDois> get B20ViewChildDoisNgFactory {
  return _B20ViewChildDoisNgFactory;
}

ComponentFactory<import1.B20ViewChildDois> createB20ViewChildDoisFactory() {
  return ComponentFactory('b20-view-child-dois', viewFactory_B20ViewChildDoisHost0);
}

final List<Object> styles$B20ViewChildDoisHost = const [];

class _ViewB20ViewChildDoisHost0 extends import10.HostView<import1.B20ViewChildDois> {
  @override
  void build() {
    this.componentView = ViewB20ViewChildDois0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B20ViewChildDois();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.B20ViewChildDois> viewFactory_B20ViewChildDoisHost0() {
  return _ViewB20ViewChildDoisHost0();
}

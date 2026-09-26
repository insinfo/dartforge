// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i36_prefixos_longos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i36_prefixos_longos.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I36PrefixosLongos = const [];

class ViewI36PrefixosLongos0 extends import0.ComponentView<import1.I36PrefixosLongos> {
  Object? _expr_0;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI36PrefixosLongos0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i36-prefixos-longos'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i36_prefixos_longos.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_0.addEventListener('click', this.eventHandler0(_ctx.f));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.t;
    if (import8.checkBinding(this._expr_0, currVal_0, 't', 'package:corpus_ngdart/src/i36_prefixos_longos.html')) {
      import7.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i36_prefixos_longos.html:5:19 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I36PrefixosLongos, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I36PrefixosLongosNgFactory = ComponentFactory<import1.I36PrefixosLongos>('i36-prefixos-longos', viewFactory_I36PrefixosLongosHost0);
ComponentFactory<import1.I36PrefixosLongos> get I36PrefixosLongosNgFactory {
  return _I36PrefixosLongosNgFactory;
}

ComponentFactory<import1.I36PrefixosLongos> createI36PrefixosLongosFactory() {
  return ComponentFactory('i36-prefixos-longos', viewFactory_I36PrefixosLongosHost0);
}

final List<Object> styles$I36PrefixosLongosHost = const [];

class _ViewI36PrefixosLongosHost0 extends import10.HostView<import1.I36PrefixosLongos> {
  @override
  void build() {
    this.componentView = ViewI36PrefixosLongos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I36PrefixosLongos();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I36PrefixosLongos> viewFactory_I36PrefixosLongosHost0() {
  return _ViewI36PrefixosLongosHost0();
}

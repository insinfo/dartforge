// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i28_attr_class.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i28_attr_class.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I28AttrClass = const [];

class ViewI28AttrClass0 extends import0.ComponentView<import1.I28AttrClass> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  late final import2.ButtonElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI28AttrClass0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i28-attr-class'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i28_attr_class.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_1 = import7.appendText(this._el_0, 'x');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.pressionado;
    if (import8.checkBinding(this._expr_0, currVal_0, 'pressionado', 'package:corpus_ngdart/src/i28_attr_class.html')) {
      import7.updateAttribute(this._el_0, 'aria-pressed', currVal_0) /* REF:package:corpus_ngdart/src/i28_attr_class.html:8:41 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.ativo;
    if (import8.checkBinding(this._expr_1, currVal_1, 'ativo', 'package:corpus_ngdart/src/i28_attr_class.html')) {
      import7.updateClassBinding(this._el_0, 'ativo', currVal_1) /* REF:package:corpus_ngdart/src/i28_attr_class.html:42:63 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.classe;
    if (import8.checkBinding(this._expr_2, currVal_2, 'classe', 'package:corpus_ngdart/src/i28_attr_class.html')) {
      this.updateChildClass(this._el_0, currVal_2) /* REF:package:corpus_ngdart/src/i28_attr_class.html:64:80 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.id;
    if (import8.checkBinding(this._expr_3, currVal_3, 'id', 'package:corpus_ngdart/src/i28_attr_class.html')) {
      import7.updateAttribute(this._el_0, 'data-id', currVal_3) /* REF:package:corpus_ngdart/src/i28_attr_class.html:81:100 */;
      this._expr_3 = currVal_3;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I28AttrClass, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I28AttrClassNgFactory = ComponentFactory<import1.I28AttrClass>('i28-attr-class', viewFactory_I28AttrClassHost0);
ComponentFactory<import1.I28AttrClass> get I28AttrClassNgFactory {
  return _I28AttrClassNgFactory;
}

ComponentFactory<import1.I28AttrClass> createI28AttrClassFactory() {
  return ComponentFactory('i28-attr-class', viewFactory_I28AttrClassHost0);
}

final List<Object> styles$I28AttrClassHost = const [];

class _ViewI28AttrClassHost0 extends import10.HostView<import1.I28AttrClass> {
  @override
  void build() {
    this.componentView = ViewI28AttrClass0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I28AttrClass();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I28AttrClass> viewFactory_I28AttrClassHost0() {
  return _ViewI28AttrClassHost0();
}

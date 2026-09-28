// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j124_atributo_condicional.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j124_atributo_condicional.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J124Usa = const [];

class ViewJ124Usa0 extends import0.ComponentView<import1.J124Usa> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import2.DivElement _el_0;
  late final import2.ButtonElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewJ124Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j124-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendElement<import2.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_2 = import7.appendText(this._el_1, 'b');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.aberto;
    if (import8.checkBinding(this._expr_0, currVal_0, 'aberto', 'asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart')) {
      import7.updateAttribute(this._el_0, 'aberto', (currVal_0 ? '' : null)) /* REF:asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart:300:325 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.ref;
    if (import8.checkBinding(this._expr_1, currVal_1, 'ref', 'asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart')) {
      import7.updateAttributeNS(this._el_0, 'http://www.w3.org/1999/xlink', 'href', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart:326:349 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.ref;
    if (import8.checkBinding(this._expr_2, currVal_2, 'ref', 'asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart')) {
      import7.updateAttribute(this._el_0, 'bar', currVal_2) /* REF:asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart:350:370 */;
      this._expr_2 = currVal_2;
    }
    if (firstCheck) {
      import7.updateAttribute(this._el_1, 'disabled', (true ? '' : null)) /* REF:asset:corpus_ngdart/lib/src/j124_atributo_condicional.dart:386:411 */;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J124Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J124UsaNgFactory = ComponentFactory<import1.J124Usa>('j124-usa', viewFactory_J124UsaHost0);
ComponentFactory<import1.J124Usa> get J124UsaNgFactory {
  return _J124UsaNgFactory;
}

ComponentFactory<import1.J124Usa> createJ124UsaFactory() {
  return ComponentFactory('j124-usa', viewFactory_J124UsaHost0);
}

final List<Object> styles$J124UsaHost = const [];

class _ViewJ124UsaHost0 extends import10.HostView<import1.J124Usa> {
  @override
  void build() {
    this.componentView = ViewJ124Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J124Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J124Usa> viewFactory_J124UsaHost0() {
  return _ViewJ124UsaHost0();
}

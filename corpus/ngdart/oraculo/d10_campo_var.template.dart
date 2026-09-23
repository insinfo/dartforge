// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd10_campo_var.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd10_campo_var.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'd10_filho_var.template.dart' as import3;
import 'd10_filho_var.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/runtime/interpolate.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$D10CampoVar = const [];

class ViewD10CampoVar0 extends import0.ComponentView<import1.D10CampoVar> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final import3.ViewD10FilhoVar0 _compView_2;
  late final import4.D10FilhoVar _D10FilhoVar_2_5;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewD10CampoVar0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('d10-campo-var'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/d10_campo_var.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendElement<import9.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    this._compView_2 = import3.ViewD10FilhoVar0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._D10FilhoVar_2_5 = import4.D10FilhoVar();
    this._compView_2.create(this._D10FilhoVar_2_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if ((_ctx.perfil != null)) {
        if (import11.isDevToolsEnabled) {
          import11.Inspector.instance.recordInput(this._D10FilhoVar_2_5, 'perfil', _ctx.perfil);
        }
        this._D10FilhoVar_2_5.perfil = _ctx.perfil /* REF:package:corpus_ngdart/src/d10_campo_var.html:49:66 */;
      }
    }
    final currVal_0 = _ctx.filtro;
    if (import12.checkBinding(this._expr_0, currVal_0, 'filtro', 'package:corpus_ngdart/src/d10_campo_var.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._D10FilhoVar_2_5, 'filtro', currVal_0);
      }
      this._D10FilhoVar_2_5.filtro = currVal_0 /* REF:package:corpus_ngdart/src/d10_campo_var.html:31:48 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import13.interpolateString0(_ctx.nome)) /* REF:package:corpus_ngdart/src/d10_campo_var.html:3:11 */;
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$D10CampoVar, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D10CampoVarNgFactory = ComponentFactory<import1.D10CampoVar>('d10-campo-var', viewFactory_D10CampoVarHost0);
ComponentFactory<import1.D10CampoVar> get D10CampoVarNgFactory {
  return _D10CampoVarNgFactory;
}

ComponentFactory<import1.D10CampoVar> createD10CampoVarFactory() {
  return ComponentFactory('d10-campo-var', viewFactory_D10CampoVarHost0);
}

final List<Object> styles$D10CampoVarHost = const [];

class _ViewD10CampoVarHost0 extends import15.HostView<import1.D10CampoVar> {
  @override
  void build() {
    this.componentView = ViewD10CampoVar0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D10CampoVar();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.D10CampoVar> viewFactory_D10CampoVarHost0() {
  return _ViewD10CampoVarHost0();
}

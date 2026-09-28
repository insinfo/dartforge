// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j28_usa_caixa.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j28_usa_caixa.dart' as import1;
import 'j27_caixa.template.dart' as import2;
import 'j27_caixa.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'j26_gatilho.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J28UsaCaixa = const [];

class ViewJ28UsaCaixa0 extends import0.ComponentView<import1.J28UsaCaixa> {
  late final import2.ViewJ27Caixa0 _compView_0;
  late final import3.J27Caixa _J27Caixa_0_5;
  late final ViewContainer _appEl_1;
  late final import5.J26Gatilho _J26Gatilho_1_8;
  late final import2.ViewJ27Caixa0 _compView_2;
  late final import3.J27Caixa _J27Caixa_2_5;
  late final ViewContainer _appEl_3;
  late final import5.J26Gatilho _J26Gatilho_3_8;
  static import6.ComponentStyles? _componentStyles;
  ViewJ28UsaCaixa0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('j28-usa-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j28_usa_caixa.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ27Caixa0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J27Caixa_0_5 = import3.J27Caixa();
    final _anchor_1 = import11.createAnchor();
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_7 = TemplateRef(this._appEl_1, viewFactory_J28UsaCaixa1);
    this._J26Gatilho_1_8 = import5.J26Gatilho(_TemplateRef_1_7);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._J26Gatilho_1_8);
    }
    this._J27Caixa_0_5.gatilho = this._J26Gatilho_1_8;
    this._compView_0.createAndProject(this._J27Caixa_0_5, [
      <Object>[_anchor_1]
    ]);
    this._compView_2 = import2.ViewJ27Caixa0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._J27Caixa_2_5 = import3.J27Caixa();
    final _anchor_3 = import11.createAnchor();
    this._appEl_3 = ViewContainer(3, 2, this, _anchor_3);
    var _TemplateRef_3_7 = TemplateRef(this._appEl_3, viewFactory_J28UsaCaixa2);
    this._J26Gatilho_3_8 = import5.J26Gatilho(_TemplateRef_3_7);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._J26Gatilho_3_8);
    }
    this._J27Caixa_2_5.gatilho = this._J26Gatilho_3_8;
    this._compView_2.createAndProject(this._J27Caixa_2_5, [
      <Object>[_anchor_3]
    ]);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J28UsaCaixa, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J28UsaCaixaNgFactory = ComponentFactory<import1.J28UsaCaixa>('j28-usa-caixa', viewFactory_J28UsaCaixaHost0);
ComponentFactory<import1.J28UsaCaixa> get J28UsaCaixaNgFactory {
  return _J28UsaCaixaNgFactory;
}

ComponentFactory<import1.J28UsaCaixa> createJ28UsaCaixaFactory() {
  return ComponentFactory('j28-usa-caixa', viewFactory_J28UsaCaixaHost0);
}

class _ViewJ28UsaCaixa1 extends import15.EmbeddedView<import1.J28UsaCaixa> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  final import16.TextBinding _textBinding_3 = import16.TextBinding();
  _ViewJ28UsaCaixa1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('span'));
    this.updateChildClass(_el_0, 'rotulo');
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import11.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_ctx = this.locals['\$implicit'];
    this._textBinding_1.updateText(import18.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/j28_usa_caixa.html:69:79 */;
    this._textBinding_3.updateText(import18.interpolate0(local_ctx)) /* REF:package:corpus_ngdart/src/j28_usa_caixa.html:80:87 */;
  }
}

import15.EmbeddedView<void> viewFactory_J28UsaCaixa1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ28UsaCaixa1(parentView, parentIndex);
}

class _ViewJ28UsaCaixa2 extends import15.EmbeddedView<import1.J28UsaCaixa> {
  _ViewJ28UsaCaixa2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('b'));
    final _text_1 = import11.appendText(_el_0, 'fixo');
    this.initRootNode(_el_0);
  }
}

import15.EmbeddedView<void> viewFactory_J28UsaCaixa2(import17.RenderView parentView, int parentIndex) {
  return _ViewJ28UsaCaixa2(parentView, parentIndex);
}

final List<Object> styles$J28UsaCaixaHost = const [];

class _ViewJ28UsaCaixaHost0 extends import19.HostView<import1.J28UsaCaixa> {
  @override
  void build() {
    this.componentView = ViewJ28UsaCaixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J28UsaCaixa();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J28UsaCaixa> viewFactory_J28UsaCaixaHost0() {
  return _ViewJ28UsaCaixaHost0();
}

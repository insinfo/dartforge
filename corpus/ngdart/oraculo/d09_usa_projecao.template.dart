// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd09_usa_projecao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd09_usa_projecao.dart' as import1;
import 'd09_projecao_select.template.dart' as import2;
import 'd09_projecao_select.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
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
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$D09UsaProjecao = const [];

class ViewD09UsaProjecao0 extends import0.ComponentView<import1.D09UsaProjecao> {
  late final import2.ViewD09ProjecaoSelect0 _compView_0;
  late final import3.D09ProjecaoSelect _D09ProjecaoSelect_0_5;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  late final import2.ViewD09ProjecaoSelect0 _compView_8;
  late final import3.D09ProjecaoSelect _D09ProjecaoSelect_8_5;
  static import6.ComponentStyles? _componentStyles;
  ViewD09UsaProjecao0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('d09-usa-projecao'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/d09_usa_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewD09ProjecaoSelect0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._D09ProjecaoSelect_0_5 = import3.D09ProjecaoSelect();
    final doc = import10.document;
    final _el_1 = import9.unsafeCast(doc.createElement('h1'));
    import11.setAttribute(_el_1, 'cabecalho', '');
    final _text_2 = import11.appendText(_el_1, 't');
    final _el_3 = import9.unsafeCast(doc.createElement('p'));
    final _text_4 = import11.appendText(_el_3, 'meio');
    final _anchor_5 = import11.createAnchor();
    this._appEl_5 = ViewContainer(5, 0, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_D09UsaProjecao1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
    final _el_6 = import9.unsafeCast(doc.createElement('span'));
    this.updateChildClass(_el_6, 'rodape');
    final _text_7 = import11.appendText(_el_6, 'r');
    this._D09ProjecaoSelect_0_5.marcas = [];
    this._compView_0.createAndProject(this._D09ProjecaoSelect_0_5, [
      <Object>[_el_1],
      <Object>[_el_3, this._appEl_5],
      <Object>[_el_6]
    ]);
    this._compView_8 = import2.ViewD09ProjecaoSelect0(this, 8);
    final _el_8 = this._compView_8.rootElement;
    parentRenderNode.append(_el_8);
    this._D09ProjecaoSelect_8_5 = import3.D09ProjecaoSelect();
    this._D09ProjecaoSelect_8_5.marcas = [];
    this._compView_8.createAndProject(this._D09ProjecaoSelect_8_5, [const <Object>[], const <Object>[], const <Object>[]]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_5_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d09_usa_projecao.html:66:80 */;
    this._appEl_5.detectChangesInNestedViews();
    this._compView_0.detectChanges();
    this._compView_8.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_8.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$D09UsaProjecao, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D09UsaProjecaoNgFactory = ComponentFactory<import1.D09UsaProjecao>('d09-usa-projecao', viewFactory_D09UsaProjecaoHost0);
ComponentFactory<import1.D09UsaProjecao> get D09UsaProjecaoNgFactory {
  return _D09UsaProjecaoNgFactory;
}

ComponentFactory<import1.D09UsaProjecao> createD09UsaProjecaoFactory() {
  return ComponentFactory('d09-usa-projecao', viewFactory_D09UsaProjecaoHost0);
}

class _ViewD09UsaProjecao1 extends import15.EmbeddedView<import1.D09UsaProjecao> {
  _ViewD09UsaProjecao1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    final _text_1 = import11.appendText(_el_0, 'cond');
    this.initRootNode(_el_0);
  }
}

import15.EmbeddedView<void> viewFactory_D09UsaProjecao1(import16.RenderView parentView, int parentIndex) {
  return _ViewD09UsaProjecao1(parentView, parentIndex);
}

final List<Object> styles$D09UsaProjecaoHost = const [];

class _ViewD09UsaProjecaoHost0 extends import17.HostView<import1.D09UsaProjecao> {
  @override
  void build() {
    this.componentView = ViewD09UsaProjecao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D09UsaProjecao();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.D09UsaProjecao> viewFactory_D09UsaProjecaoHost0() {
  return _ViewD09UsaProjecaoHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd08_usa_injetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd08_usa_injetado.dart' as import1;
import 'd08_filho_injetado.template.dart' as import2;
import 'd08_filho_injetado.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/di/errors.dart' as import11;
import 'd08_servico.dart' as import12;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import13;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$D08UsaInjetado = const [];

class ViewD08UsaInjetado0 extends import0.ComponentView<import1.D08UsaInjetado> {
  late final import2.ViewD08FilhoInjetado0 _compView_0;
  late final import3.D08FilhoInjetado _D08FilhoInjetado_0_5;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  static import6.ComponentStyles? _componentStyles;
  ViewD08UsaInjetado0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('d08-usa-injetado'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/d08_usa_injetado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewD08FilhoInjetado0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._D08FilhoInjetado_0_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import3.D08FilhoInjetado, () {
            return import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_0, this._compView_0);
          })
        : import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_0, this._compView_0));
    this._compView_0.create(this._D08FilhoInjetado_0_5);
    final _anchor_1 = import13.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_D08UsaInjetado1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final _anchor_2 = import13.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_D08UsaInjetado2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final doc = import10.document;
    final _el_3 = import13.appendElement<import10.HtmlElement>(doc, parentRenderNode, 'section');
    final _anchor_4 = import13.appendAnchor(_el_3);
    this._appEl_4 = ViewContainer(4, 3, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_D08UsaInjetado3);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final _anchor_5 = import13.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_D08UsaInjetado4);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_1_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d08_usa_injetado.html:47:61 */;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_2_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d08_usa_injetado.html:130:144 */;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_4_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d08_usa_injetado.html:196:210 */;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_5_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d08_usa_injetado.html:248:262 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_4.detectChangesInNestedViews();
    this._appEl_5.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._appEl_4.destroyNestedViews();
    this._appEl_5.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$D08UsaInjetado, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D08UsaInjetadoNgFactory = ComponentFactory<import1.D08UsaInjetado>('d08-usa-injetado', viewFactory_D08UsaInjetadoHost0);
ComponentFactory<import1.D08UsaInjetado> get D08UsaInjetadoNgFactory {
  return _D08UsaInjetadoNgFactory;
}

ComponentFactory<import1.D08UsaInjetado> createD08UsaInjetadoFactory() {
  return ComponentFactory('d08-usa-injetado', viewFactory_D08UsaInjetadoHost0);
}

class _ViewD08UsaInjetado1 extends import17.EmbeddedView<import1.D08UsaInjetado> {
  late final import2.ViewD08FilhoInjetado0 _compView_1;
  late final import3.D08FilhoInjetado _D08FilhoInjetado_1_5;
  _ViewD08UsaInjetado1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    this._compView_1 = import2.ViewD08FilhoInjetado0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._D08FilhoInjetado_1_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import3.D08FilhoInjetado, () {
            return import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_1, this._compView_1);
          })
        : import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_1, this._compView_1));
    this._compView_1.create(this._D08FilhoInjetado_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_D08UsaInjetado1(import18.RenderView parentView, int parentIndex) {
  return _ViewD08UsaInjetado1(parentView, parentIndex);
}

class _ViewD08UsaInjetado2 extends import17.EmbeddedView<import1.D08UsaInjetado> {
  late final import2.ViewD08FilhoInjetado0 _compView_0;
  late final import3.D08FilhoInjetado _D08FilhoInjetado_0_5;
  _ViewD08UsaInjetado2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = import2.ViewD08FilhoInjetado0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._D08FilhoInjetado_0_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import3.D08FilhoInjetado, () {
            return import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_0, this._compView_0);
          })
        : import3.D08FilhoInjetado((this.parentView!).injectorGet(import12.D08Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import12.D08Opcional, this.parentIndex), _el_0, this._compView_0));
    this._compView_0.create(this._D08FilhoInjetado_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_D08UsaInjetado2(import18.RenderView parentView, int parentIndex) {
  return _ViewD08UsaInjetado2(parentView, parentIndex);
}

class _ViewD08UsaInjetado3 extends import17.EmbeddedView<import1.D08UsaInjetado> {
  late final import2.ViewD08FilhoInjetado0 _compView_0;
  late final import3.D08FilhoInjetado _D08FilhoInjetado_0_5;
  _ViewD08UsaInjetado3(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = import2.ViewD08FilhoInjetado0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._D08FilhoInjetado_0_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import3.D08FilhoInjetado, () {
            return import3.D08FilhoInjetado(((this.parentView!).parentView!).injectorGet(import12.D08Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import12.D08Opcional, (this.parentView!).parentIndex), _el_0, this._compView_0);
          })
        : import3.D08FilhoInjetado(((this.parentView!).parentView!).injectorGet(import12.D08Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import12.D08Opcional, (this.parentView!).parentIndex), _el_0, this._compView_0));
    this._compView_0.create(this._D08FilhoInjetado_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_D08UsaInjetado3(import18.RenderView parentView, int parentIndex) {
  return _ViewD08UsaInjetado3(parentView, parentIndex);
}

class _ViewD08UsaInjetado4 extends import17.EmbeddedView<import1.D08UsaInjetado> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewD08UsaInjetado4(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import13.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_D08UsaInjetado5);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_1_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/d08_usa_injetado.html:266:280 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import17.EmbeddedView<void> viewFactory_D08UsaInjetado4(import18.RenderView parentView, int parentIndex) {
  return _ViewD08UsaInjetado4(parentView, parentIndex);
}

class _ViewD08UsaInjetado5 extends import17.EmbeddedView<import1.D08UsaInjetado> {
  late final import2.ViewD08FilhoInjetado0 _compView_1;
  late final import3.D08FilhoInjetado _D08FilhoInjetado_1_5;
  _ViewD08UsaInjetado5(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    this._compView_1 = import2.ViewD08FilhoInjetado0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._D08FilhoInjetado_1_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import3.D08FilhoInjetado, () {
            return import3.D08FilhoInjetado(((this.parentView!).parentView!).injectorGet(import12.D08Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import12.D08Opcional, (this.parentView!).parentIndex), _el_1, this._compView_1);
          })
        : import3.D08FilhoInjetado(((this.parentView!).parentView!).injectorGet(import12.D08Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import12.D08Opcional, (this.parentView!).parentIndex), _el_1, this._compView_1));
    this._compView_1.create(this._D08FilhoInjetado_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_D08UsaInjetado5(import18.RenderView parentView, int parentIndex) {
  return _ViewD08UsaInjetado5(parentView, parentIndex);
}

final List<Object> styles$D08UsaInjetadoHost = const [];

class _ViewD08UsaInjetadoHost0 extends import19.HostView<import1.D08UsaInjetado> {
  @override
  void build() {
    this.componentView = ViewD08UsaInjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D08UsaInjetado();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.D08UsaInjetado> viewFactory_D08UsaInjetadoHost0() {
  return _ViewD08UsaInjetadoHost0();
}

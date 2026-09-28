// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j40_dependencia_de_fora.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j40_dependencia_de_fora.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'j40_com_host.template.dart' as import4;
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/di/errors.dart' as import11;
import 'package:ngdart/src/meta/di_tokens.dart' as import12;
import 'dart:core';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'j40_com_host.dart' as import16;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J40DependenciaDeFora = const [];

class ViewJ40DependenciaDeFora0 extends import0.ComponentView<import1.J40DependenciaDeFora> {
  late final import1.J40Usa _J40Usa_0_5;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final import4.J40ComHostNgCd _J40ComHost_3_5;
  late final ViewContainer _appEl_6;
  late final NgIf _NgIf_6_9;
  late final ViewContainer _appEl_7;
  late final NgIf _NgIf_7_9;
  late final import1.J40ComInjetor _J40ComInjetor_8_5;
  late final import5.HtmlElement _el_3;
  static import6.ComponentStyles? _componentStyles;
  ViewJ40DependenciaDeFora0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('j40-dependencia-de-fora'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j40_dependencia_de_fora.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    import10.setAttribute(_el_0, 'j40-usa', '');
    this._J40Usa_0_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import1.J40Usa, () {
            return import1.J40Usa((this.parentView!).injectorGet(import1.J40Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import1.J40Outro, this.parentIndex), (this.parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), this.parentIndex));
          })
        : import1.J40Usa((this.parentView!).injectorGet(import1.J40Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import1.J40Outro, this.parentIndex), (this.parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), this.parentIndex)));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_0, this._J40Usa_0_5);
    }
    final _text_1 = import10.appendText(_el_0, 'a');
    final _anchor_2 = import10.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J40DependenciaDeFora1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this._el_3 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'b');
    import10.setAttribute(this._el_3, 'j40-com-host', '');
    this._J40ComHost_3_5 = import4.J40ComHostNgCd((import9.isDevMode
        ? import11.debugInjectorWrap(import16.J40ComHost, () {
            return import16.J40ComHost((this.parentView!).injectorGet(import16.J40Rastro, this.parentIndex));
          })
        : import16.J40ComHost((this.parentView!).injectorGet(import16.J40Rastro, this.parentIndex))));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(this._el_3, this._J40ComHost_3_5.instance);
    }
    final _text_4 = import10.appendText(this._el_3, 'c');
    final _el_5 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'section');
    final _anchor_6 = import10.appendAnchor(_el_5);
    this._appEl_6 = ViewContainer(6, 5, this, _anchor_6);
    var _TemplateRef_6_8 = TemplateRef(this._appEl_6, viewFactory_J40DependenciaDeFora2);
    this._NgIf_6_9 = NgIf(this._appEl_6, _TemplateRef_6_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_6, this._NgIf_6_9);
    }
    final _anchor_7 = import10.appendAnchor(parentRenderNode);
    this._appEl_7 = ViewContainer(7, null, this, _anchor_7);
    var _TemplateRef_7_8 = TemplateRef(this._appEl_7, viewFactory_J40DependenciaDeFora3);
    this._NgIf_7_9 = NgIf(this._appEl_7, _TemplateRef_7_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_7, this._NgIf_7_9);
    }
    final _el_8 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'u');
    import10.setAttribute(_el_8, 'j40-injetor', '');
    this._J40ComInjetor_8_5 = import1.J40ComInjetor(this.injector(8));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_8, this._J40ComInjetor_8_5);
    }
    final _text_9 = import10.appendText(_el_8, 'f');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j40_dependencia_de_fora.html:24:39 */;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_6_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_6_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j40_dependencia_de_fora.html:101:116 */;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_7_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_7_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j40_dependencia_de_fora.html:151:166 */;
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_6.detectChangesInNestedViews();
    this._appEl_7.detectChangesInNestedViews();
    this._J40ComHost_3_5.detectHostChanges(this, this._el_3);
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._appEl_6.destroyNestedViews();
    this._appEl_7.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J40DependenciaDeFora, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J40DependenciaDeForaNgFactory = ComponentFactory<import1.J40DependenciaDeFora>('j40-dependencia-de-fora', viewFactory_J40DependenciaDeForaHost0);
ComponentFactory<import1.J40DependenciaDeFora> get J40DependenciaDeForaNgFactory {
  return _J40DependenciaDeForaNgFactory;
}

ComponentFactory<import1.J40DependenciaDeFora> createJ40DependenciaDeForaFactory() {
  return ComponentFactory('j40-dependencia-de-fora', viewFactory_J40DependenciaDeForaHost0);
}

class _ViewJ40DependenciaDeFora1 extends import18.EmbeddedView<import1.J40DependenciaDeFora> {
  late final import1.J40Usa _J40Usa_1_5;
  _ViewJ40DependenciaDeFora1(import19.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    final _el_1 = import10.appendSpan(doc, _el_0);
    import10.setAttribute(_el_1, 'j40-usa', '');
    this._J40Usa_1_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import1.J40Usa, () {
            return import1.J40Usa((this.parentView!).injectorGet(import1.J40Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import1.J40Outro, this.parentIndex), (this.parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), this.parentIndex));
          })
        : import1.J40Usa((this.parentView!).injectorGet(import1.J40Servico, this.parentIndex), (this.parentView!).injectorGetOptional(import1.J40Outro, this.parentIndex), (this.parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), this.parentIndex)));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_1, this._J40Usa_1_5);
    }
    final _text_2 = import10.appendText(_el_1, 'b');
    this.initRootNode(_el_0);
  }
}

import18.EmbeddedView<void> viewFactory_J40DependenciaDeFora1(import19.RenderView parentView, int parentIndex) {
  return _ViewJ40DependenciaDeFora1(parentView, parentIndex);
}

class _ViewJ40DependenciaDeFora2 extends import18.EmbeddedView<import1.J40DependenciaDeFora> {
  late final import1.J40Usa _J40Usa_1_5;
  _ViewJ40DependenciaDeFora2(import19.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    final _el_1 = import10.appendElement<import5.HtmlElement>(doc, _el_0, 'i');
    import10.setAttribute(_el_1, 'j40-usa', '');
    this._J40Usa_1_5 = (import9.isDevMode
        ? import11.debugInjectorWrap(import1.J40Usa, () {
            return import1.J40Usa(((this.parentView!).parentView!).injectorGet(import1.J40Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import1.J40Outro, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), (this.parentView!).parentIndex));
          })
        : import1.J40Usa(((this.parentView!).parentView!).injectorGet(import1.J40Servico, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(import1.J40Outro, (this.parentView!).parentIndex), ((this.parentView!).parentView!).injectorGetOptional(const import12.OpaqueToken<String>('j40.token'), (this.parentView!).parentIndex)));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_1, this._J40Usa_1_5);
    }
    final _text_2 = import10.appendText(_el_1, 'd');
    this.initRootNode(_el_0);
  }
}

import18.EmbeddedView<void> viewFactory_J40DependenciaDeFora2(import19.RenderView parentView, int parentIndex) {
  return _ViewJ40DependenciaDeFora2(parentView, parentIndex);
}

class _ViewJ40DependenciaDeFora3 extends import18.EmbeddedView<import1.J40DependenciaDeFora> {
  late final import1.J40ComInjetor _J40ComInjetor_1_5;
  _ViewJ40DependenciaDeFora3(import19.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    final _el_1 = import10.appendElement<import5.HtmlElement>(doc, _el_0, 'em');
    import10.setAttribute(_el_1, 'j40-injetor', '');
    this._J40ComInjetor_1_5 = import1.J40ComInjetor(this.injector(1));
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_1, this._J40ComInjetor_1_5);
    }
    final _text_2 = import10.appendText(_el_1, 'e');
    this.initRootNode(_el_0);
  }
}

import18.EmbeddedView<void> viewFactory_J40DependenciaDeFora3(import19.RenderView parentView, int parentIndex) {
  return _ViewJ40DependenciaDeFora3(parentView, parentIndex);
}

final List<Object> styles$J40DependenciaDeForaHost = const [];

class _ViewJ40DependenciaDeForaHost0 extends import20.HostView<import1.J40DependenciaDeFora> {
  @override
  void build() {
    this.componentView = ViewJ40DependenciaDeFora0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J40DependenciaDeFora();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J40DependenciaDeFora> viewFactory_J40DependenciaDeForaHost0() {
  return _ViewJ40DependenciaDeForaHost0();
}

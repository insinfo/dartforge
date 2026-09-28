// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j38_preguicoso_e_texto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j38_preguicoso_e_texto.dart' as import1;
import 'i60_provider_use_class.dart' as import2;
import 'package:ngdart/src/runtime/text_binding.dart' as import3;
import 'i60_provider_use_class.template.dart' as import4;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'dart:html' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/runtime/interpolate.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J38PreguicosoETexto = const [];

class ViewJ38PreguicosoETexto0 extends import0.ComponentView<import1.J38PreguicosoETexto> {
  late import2.I60Impl _I60Base_2_6 = import2.I60Impl();
  late import2.I60OutroImpl _I60Outro_2_7 = import2.I60OutroImpl();
  late import2.I60Terceiro _I60Terceiro_2_8 = import2.I60Terceiro();
  final import3.TextBinding _textBinding_1 = import3.TextBinding();
  final import3.TextBinding _textBinding_4 = import3.TextBinding();
  late final import4.ViewI60ProviderUseClass0 _compView_2;
  late final import2.I60ProviderUseClass _I60ProviderUseClass_2_5;
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  static import7.ComponentStyles? _componentStyles;
  ViewJ38PreguicosoETexto0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import11.document.createElement('j38-preguicoso-e-texto'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/j38_preguicoso_e_texto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import11.document;
    final _el_0 = import12.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    this._compView_2 = import4.ViewI60ProviderUseClass0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._I60ProviderUseClass_2_5 = import2.I60ProviderUseClass();
    this._compView_2.create(this._I60ProviderUseClass_2_5);
    final _el_3 = import12.appendSpan(doc, parentRenderNode);
    _el_3.append(this._textBinding_4.element);
    final _anchor_5 = import12.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_J38PreguicosoETexto1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((2 == nodeIndex)) {
      if (identical(token, import2.I60Base)) {
        return this._I60Base_2_6;
      }
      if (identical(token, import2.I60Outro)) {
        return this._I60Outro_2_7;
      }
      if (identical(token, import2.I60Terceiro)) {
        return this._I60Terceiro_2_8;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_5_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j38_preguicoso_e_texto.html:93:108 */;
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_1.updateText(import15.interpolateString0(_ctx.a)) /* REF:package:corpus_ngdart/src/j38_preguicoso_e_texto.html:6:11 */;
    this._textBinding_4.updateText(import15.interpolateString0(_ctx.b)) /* REF:package:corpus_ngdart/src/j38_preguicoso_e_texto.html:75:80 */;
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$J38PreguicosoETexto, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J38PreguicosoETextoNgFactory = ComponentFactory<import1.J38PreguicosoETexto>('j38-preguicoso-e-texto', viewFactory_J38PreguicosoETextoHost0);
ComponentFactory<import1.J38PreguicosoETexto> get J38PreguicosoETextoNgFactory {
  return _J38PreguicosoETextoNgFactory;
}

ComponentFactory<import1.J38PreguicosoETexto> createJ38PreguicosoETextoFactory() {
  return ComponentFactory('j38-preguicoso-e-texto', viewFactory_J38PreguicosoETextoHost0);
}

class _ViewJ38PreguicosoETexto1 extends import17.EmbeddedView<import1.J38PreguicosoETexto> {
  late import2.I60Impl _I60Base_2_6 = import2.I60Impl();
  late import2.I60OutroImpl _I60Outro_2_7 = import2.I60OutroImpl();
  late import2.I60Terceiro _I60Terceiro_2_8 = import2.I60Terceiro();
  final import3.TextBinding _textBinding_1 = import3.TextBinding();
  final import3.TextBinding _textBinding_3 = import3.TextBinding();
  late final import4.ViewI60ProviderUseClass0 _compView_2;
  late final import2.I60ProviderUseClass _I60ProviderUseClass_2_5;
  _ViewJ38PreguicosoETexto1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import11.document;
    final _el_0 = import10.unsafeCast(doc.createElement('div'));
    _el_0.append(this._textBinding_1.element);
    this._compView_2 = import4.ViewI60ProviderUseClass0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    _el_0.append(_el_2);
    this._I60ProviderUseClass_2_5 = import2.I60ProviderUseClass();
    this._compView_2.create(this._I60ProviderUseClass_2_5);
    _el_0.append(this._textBinding_3.element);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((2 == nodeIndex)) {
      if (identical(token, import2.I60Base)) {
        return this._I60Base_2_6;
      }
      if (identical(token, import2.I60Outro)) {
        return this._I60Outro_2_7;
      }
      if (identical(token, import2.I60Terceiro)) {
        return this._I60Terceiro_2_8;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import15.interpolateString0(_ctx.a)) /* REF:package:corpus_ngdart/src/j38_preguicoso_e_texto.html:109:114 */;
    this._textBinding_3.updateText(import15.interpolateString0(_ctx.b)) /* REF:package:corpus_ngdart/src/j38_preguicoso_e_texto.html:163:168 */;
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_2.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_J38PreguicosoETexto1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ38PreguicosoETexto1(parentView, parentIndex);
}

final List<Object> styles$J38PreguicosoETextoHost = const [];

class _ViewJ38PreguicosoETextoHost0 extends import19.HostView<import1.J38PreguicosoETexto> {
  @override
  void build() {
    this.componentView = ViewJ38PreguicosoETexto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J38PreguicosoETexto();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J38PreguicosoETexto> viewFactory_J38PreguicosoETextoHost0() {
  return _ViewJ38PreguicosoETextoHost0();
}

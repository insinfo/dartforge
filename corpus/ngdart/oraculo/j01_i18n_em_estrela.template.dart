// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j01_i18n_em_estrela.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j01_i18n_em_estrela.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:intl/intl.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J01I18nEmEstrela = const [];

class ViewJ01I18nEmEstrela0 extends import0.ComponentView<import1.J01I18nEmEstrela> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ01I18nEmEstrela0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j01-i18n-em-estrela'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j01_i18n_em_estrela.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J01I18nEmEstrela1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j01_i18n_em_estrela.html:3:18 */;
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J01I18nEmEstrela, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J01I18nEmEstrelaNgFactory = ComponentFactory<import1.J01I18nEmEstrela>('j01-i18n-em-estrela', viewFactory_J01I18nEmEstrelaHost0);
ComponentFactory<import1.J01I18nEmEstrela> get J01I18nEmEstrelaNgFactory {
  return _J01I18nEmEstrelaNgFactory;
}

ComponentFactory<import1.J01I18nEmEstrela> createJ01I18nEmEstrelaFactory() {
  return ComponentFactory('j01-i18n-em-estrela', viewFactory_J01I18nEmEstrelaHost0);
}

class _ViewJ01I18nEmEstrela1 extends import13.EmbeddedView<import1.J01I18nEmEstrela> {
  static final String _message_0 = import14.Intl.message('Olá', desc: 'aviso');
  _ViewJ01I18nEmEstrela1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, _message_0);
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_J01I18nEmEstrela1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ01I18nEmEstrela1(parentView, parentIndex);
}

final List<Object> styles$J01I18nEmEstrelaHost = const [];

class _ViewJ01I18nEmEstrelaHost0 extends import16.HostView<import1.J01I18nEmEstrela> {
  @override
  void build() {
    this.componentView = ViewJ01I18nEmEstrela0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J01I18nEmEstrela();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J01I18nEmEstrela> viewFactory_J01I18nEmEstrelaHost0() {
  return _ViewJ01I18nEmEstrelaHost0();
}

// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j69_export_as_projetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j69_export_as_projetado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;

final List<Object> styles$J69Cartao = const [];

class ViewJ69Cartao0 extends import0.ComponentView<import1.J69Cartao> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ69Cartao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j69-cartao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j69_export_as_projetado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J69Cartao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J69CartaoNgFactory = ComponentFactory<import1.J69Cartao>('j69-cartao', viewFactory_J69CartaoHost0);
ComponentFactory<import1.J69Cartao> get J69CartaoNgFactory {
  return _J69CartaoNgFactory;
}

ComponentFactory<import1.J69Cartao> createJ69CartaoFactory() {
  return ComponentFactory('j69-cartao', viewFactory_J69CartaoHost0);
}

final List<Object> styles$J69CartaoHost = const [];

class _ViewJ69CartaoHost0 extends import9.HostView<import1.J69Cartao> {
  @override
  void build() {
    this.componentView = ViewJ69Cartao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J69Cartao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J69Cartao> viewFactory_J69CartaoHost0() {
  return _ViewJ69CartaoHost0();
}

final List<Object> styles$J69ExportAsProjetado = const [];

class ViewJ69ExportAsProjetado0 extends import0.ComponentView<import1.J69ExportAsProjetado> {
  late final ViewJ69Cartao0 _compView_0;
  late final import1.J69Cartao _J69Cartao_0_5;
  late final J69FormNgCd _J69Form_1_5;
  late final import1.J69Dica _J69Dica_2_5;
  late final import6.DivElement _el_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ69ExportAsProjetado0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j69-export-as-projetado'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j69_export_as_projetado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ69Cartao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J69Cartao_0_5 = import1.J69Cartao();
    final doc = import6.document;
    this._el_1 = import5.unsafeCast(doc.createElement('div'));
    import7.setAttribute(this._el_1, 'j69-form', '');
    this._J69Form_1_5 = J69FormNgCd(import1.J69Form());
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(this._el_1, this._J69Form_1_5.instance);
    }
    final _el_2 = import7.appendElement<import6.ButtonElement>(doc, this._el_1, 'button');
    import7.setAttribute(_el_2, 'j69-dica', '');
    this._J69Dica_2_5 = import1.J69Dica();
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_2, this._J69Dica_2_5);
    }
    final _text_3 = import7.appendText(_el_2, 'ok');
    this._compView_0.createAndProject(this._J69Cartao_0_5, [
      <Object>[this._el_1]
    ]);
    _el_2.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    _ctx.forma = this._J69Form_1_5.instance;
    _ctx.dica = this._J69Dica_2_5;
  }

  @override
  void detectChangesInternal() {
    this._J69Form_1_5.detectHostChanges(this, this._el_1);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final local_forma = this._J69Form_1_5.instance;
    final _ctx = this.ctx;
    _ctx.enviar(local_forma);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J69ExportAsProjetado, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J69ExportAsProjetadoNgFactory = ComponentFactory<import1.J69ExportAsProjetado>('j69-export-as-projetado', viewFactory_J69ExportAsProjetadoHost0);
ComponentFactory<import1.J69ExportAsProjetado> get J69ExportAsProjetadoNgFactory {
  return _J69ExportAsProjetadoNgFactory;
}

ComponentFactory<import1.J69ExportAsProjetado> createJ69ExportAsProjetadoFactory() {
  return ComponentFactory('j69-export-as-projetado', viewFactory_J69ExportAsProjetadoHost0);
}

final List<Object> styles$J69ExportAsProjetadoHost = const [];

class _ViewJ69ExportAsProjetadoHost0 extends import9.HostView<import1.J69ExportAsProjetado> {
  @override
  void build() {
    this.componentView = ViewJ69ExportAsProjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J69ExportAsProjetado();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J69ExportAsProjetado> viewFactory_J69ExportAsProjetadoHost0() {
  return _ViewJ69ExportAsProjetadoHost0();
}

class J69FormNgCd extends import11.DirectiveChangeDetector {
  final import1.J69Form instance;
  Object? _expr_0;
  J69FormNgCd(this.instance);
  void detectHostChanges(import12.RenderView view, import6.Element el) {
    final currVal_0 = this.instance.ok;
    if (import13.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'ok', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}

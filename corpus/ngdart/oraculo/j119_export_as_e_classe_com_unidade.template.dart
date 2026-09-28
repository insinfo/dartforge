// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j119_export_as_e_classe_com_unidade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j119_export_as_e_classe_com_unidade.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/text_binding.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/runtime/interpolate.dart' as import13;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;

final List<Object> styles$J119Gaveta = const [];

class ViewJ119Gaveta0 extends import0.ComponentView<import1.J119Gaveta> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ119Gaveta0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j119-gaveta'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j119_export_as_e_classe_com_unidade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'g');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J119Gaveta, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J119GavetaNgFactory = ComponentFactory<import1.J119Gaveta>('j119-gaveta', viewFactory_J119GavetaHost0);
ComponentFactory<import1.J119Gaveta> get J119GavetaNgFactory {
  return _J119GavetaNgFactory;
}

ComponentFactory<import1.J119Gaveta> createJ119GavetaFactory() {
  return ComponentFactory('j119-gaveta', viewFactory_J119GavetaHost0);
}

final List<Object> styles$J119GavetaHost = const [];

class _ViewJ119GavetaHost0 extends import9.HostView<import1.J119Gaveta> {
  @override
  void build() {
    this.componentView = ViewJ119Gaveta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J119Gaveta();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J119Gaveta> viewFactory_J119GavetaHost0() {
  return _ViewJ119GavetaHost0();
}

final List<Object> styles$J119Usa = const [];

class ViewJ119Usa0 extends import0.ComponentView<import1.J119Usa> {
  final import10.TextBinding _textBinding_2 = import10.TextBinding();
  late final ViewJ119Gaveta0 _compView_0;
  late final import1.J119Gaveta _J119Gaveta_0_5;
  late final J119IconeNgCd _J119Icone_1_5;
  Object? _expr_0;
  late final import6.HtmlElement _el_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ119Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j119-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j119_export_as_e_classe_com_unidade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ119Gaveta0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J119Gaveta_0_5 = import1.J119Gaveta();
    this._compView_0.create(this._J119Gaveta_0_5);
    final doc = import6.document;
    this._el_1 = import7.appendSpan(doc, parentRenderNode);
    import7.setAttribute(this._el_1, 'j119Icone', '');
    this._J119Icone_1_5 = J119IconeNgCd(import1.J119Icone());
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(this._el_1, this._J119Icone_1_5.instance);
    }
    this._el_1.append(this._textBinding_2.element);
  }

  @override
  void detectChangesInternal() {
    final local_g = this._J119Gaveta_0_5;
    final currVal_0 = local_g.aberta;
    if (import12.checkBinding(this._expr_0, currVal_0, 'g.aberta', 'asset:corpus_ngdart/lib/src/j119_export_as_e_classe_com_unidade.dart')) {
      import7.updateClassBinding(this._el_1, 'aberta', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j119_export_as_e_classe_com_unidade.dart:596:624 */;
      this._expr_0 = currVal_0;
    }
    this._J119Icone_1_5.detectHostChanges(this, this._el_1);
    this._textBinding_2.updateText(import13.interpolate0(local_g.aberta)) /* REF:asset:corpus_ngdart/lib/src/j119_export_as_e_classe_com_unidade.dart:625:639 */;
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J119Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J119UsaNgFactory = ComponentFactory<import1.J119Usa>('j119-usa', viewFactory_J119UsaHost0);
ComponentFactory<import1.J119Usa> get J119UsaNgFactory {
  return _J119UsaNgFactory;
}

ComponentFactory<import1.J119Usa> createJ119UsaFactory() {
  return ComponentFactory('j119-usa', viewFactory_J119UsaHost0);
}

final List<Object> styles$J119UsaHost = const [];

class _ViewJ119UsaHost0 extends import9.HostView<import1.J119Usa> {
  @override
  void build() {
    this.componentView = ViewJ119Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J119Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J119Usa> viewFactory_J119UsaHost0() {
  return _ViewJ119UsaHost0();
}

class J119IconeNgCd extends import14.DirectiveChangeDetector {
  final import1.J119Icone instance;
  Object? _expr_0;
  J119IconeNgCd(this.instance);
  void detectHostChanges(import15.RenderView view, import6.Element el) {
    final currVal_0 = this.instance.basico;
    if (import12.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'basic-icon', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}

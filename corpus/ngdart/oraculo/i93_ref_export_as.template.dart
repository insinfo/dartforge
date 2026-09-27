// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i93_ref_export_as.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i93_ref_export_as.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'i93_dica.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/interpolate.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$I93RefExportAs = const [];

class ViewI93RefExportAs0 extends import0.ComponentView<import1.I93RefExportAs> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final import3.I93Dica _I93Dica_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewI93RefExportAs0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i93-ref-export-as'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i93_ref_export_as.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    import9.setAttribute(_el_0, 'dica', '');
    this._I93Dica_0_5 = import3.I93Dica();
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._I93Dica_0_5);
    }
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import9.appendElement<import8.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_3 = import9.appendText(_el_2, 'b');
    _el_2.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final local_d = this._I93Dica_0_5;
    this._textBinding_1.updateText(import11.interpolate0(local_d.texto)) /* REF:package:corpus_ngdart/src/i93_ref_export_as.html:20:31 */;
  }

  void _handleEvent_0($event) {
    final local_d = this._I93Dica_0_5;
    final _ctx = this.ctx;
    _ctx.usar(local_d);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I93RefExportAs, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I93RefExportAsNgFactory = ComponentFactory<import1.I93RefExportAs>('i93-ref-export-as', viewFactory_I93RefExportAsHost0);
ComponentFactory<import1.I93RefExportAs> get I93RefExportAsNgFactory {
  return _I93RefExportAsNgFactory;
}

ComponentFactory<import1.I93RefExportAs> createI93RefExportAsFactory() {
  return ComponentFactory('i93-ref-export-as', viewFactory_I93RefExportAsHost0);
}

final List<Object> styles$I93RefExportAsHost = const [];

class _ViewI93RefExportAsHost0 extends import13.HostView<import1.I93RefExportAs> {
  @override
  void build() {
    this.componentView = ViewI93RefExportAs0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I93RefExportAs();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.I93RefExportAs> viewFactory_I93RefExportAsHost0() {
  return _ViewI93RefExportAsHost0();
}

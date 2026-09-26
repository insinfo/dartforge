// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i20_i18n.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i20_i18n.dart' as import1;
import 'package:intl/intl.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I20I18n = const [];

class ViewI20I18n0 extends import0.ComponentView<import1.I20I18n> {
  static final String _message_0 = import2.Intl.message('Olá', desc: 'descrição');
  static final String _message_1 = import2.Intl.message('imagem', desc: 'alt');
  static import3.ComponentStyles? _componentStyles;
  ViewI20I18n0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i20-i18n'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i20_i18n.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import8.appendText(_el_0, _message_0);
    final _el_2 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'img');
    import8.setAttribute(_el_2, 'alt', _message_1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I20I18n, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I20I18nNgFactory = ComponentFactory<import1.I20I18n>('i20-i18n', viewFactory_I20I18nHost0);
ComponentFactory<import1.I20I18n> get I20I18nNgFactory {
  return _I20I18nNgFactory;
}

ComponentFactory<import1.I20I18n> createI20I18nFactory() {
  return ComponentFactory('i20-i18n', viewFactory_I20I18nHost0);
}

final List<Object> styles$I20I18nHost = const [];

class _ViewI20I18nHost0 extends import10.HostView<import1.I20I18n> {
  @override
  void build() {
    this.componentView = ViewI20I18n0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I20I18n();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I20I18n> viewFactory_I20I18nHost0() {
  return _ViewI20I18nHost0();
}

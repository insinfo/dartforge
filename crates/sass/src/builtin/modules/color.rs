//! O módulo `sass:color` (`module` de `lib/src/functions/color.dart`).
use crate::builtin::{color as c, modules::Module};

pub(crate) fn declare(f: &mut Module) {
    f.insert_builtin("red", c::red);
    f.insert_builtin("green", c::green);
    f.insert_builtin("blue", c::blue);
    f.insert_builtin("mix", c::mix);
    f.insert_builtin("invert", c::invert_module);
    f.insert_builtin("hue", c::hue);
    f.insert_builtin("saturation", c::saturation);
    f.insert_builtin("lightness", c::lightness);
    f.insert_builtin("adjust-hue", c::removida_adjust_hue);
    f.insert_builtin("lighten", c::removida_lighten);
    f.insert_builtin("darken", c::removida_darken);
    f.insert_builtin("saturate", c::removida_saturate);
    f.insert_builtin("desaturate", c::removida_desaturate);
    f.insert_builtin("grayscale", c::grayscale_module);
    f.insert_builtin("hwb", c::hwb_module);
    f.insert_builtin("whiteness", c::whiteness);
    f.insert_builtin("blackness", c::blackness);
    f.insert_builtin("opacify", c::removida_opacify);
    f.insert_builtin("fade-in", c::removida_fade_in);
    f.insert_builtin("transparentize", c::removida_transparentize);
    f.insert_builtin("fade-out", c::removida_fade_out);
    f.insert_builtin("alpha", c::alpha_module);
    f.insert_builtin("opacity", c::opacity_module);
    f.insert_builtin("space", c::space);
    f.insert_builtin("to-space", c::to_space);
    f.insert_builtin("is-legacy", c::is_legacy);
    f.insert_builtin("is-missing", c::is_missing);
    f.insert_builtin("is-in-gamut", c::is_in_gamut);
    f.insert_builtin("to-gamut", c::to_gamut);
    f.insert_builtin("channel", c::channel);
    f.insert_builtin("same", c::same);
    f.insert_builtin("is-powerless", c::is_powerless);
    f.insert_builtin("complement", c::complement);
    f.insert_builtin("adjust", c::adjust);
    f.insert_builtin("scale", c::scale);
    f.insert_builtin("change", c::change);
    f.insert_builtin("ie-hex-str", c::ie_hex_str);
}

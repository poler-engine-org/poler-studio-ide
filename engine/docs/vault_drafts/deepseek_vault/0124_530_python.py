if ident in mentioned_idents: mul *= 10
if is_snake_or_kebab_or_camel and len >= 8: mul *= 10
if ident.startswith("_"): mul *= 0.1
if len(defines[ident]) > 5: mul *= 0.1
if referencer in chat_rel_fnames: use_mul *= 50

# Remaining differences vs. `dt_lex.l`


## 4. `id_or_type()` disambiguation absent

Official:
```
<S0>{RGX_IDENT}   return (id_or_type(yytext));
```
`id_or_type()` returns `DT_TOK_TNAME` when the word is a known type name, and
`DT_TOK_IDENT` otherwise. It has lookahead: if the next token is `++`, `--`,
`[`, or `=`, it always returns `DT_TOK_IDENT` (the user is assigning to a
variable that happens to share a type name).

The Rust lexer always returns `Identifier`, so the parser cannot distinguish a
type name from a plain identifier without doing its own lookup.

## 5. Probe-specifier vs. type-name disambiguation in S2 absent

Official (in S2 / `ProgramOuterScope`):
```
if (!(yypcb->pcb_cflags & DTRACE_C_PSPEC) && strchr(yytext, ':') == NULL) {
    // If the fragment before '*' is a known type name, push '*' and suffix
    // back onto the input, call yybegin(YYS_EXPR), and return DT_TOK_TNAME.
}
```
When `C_PSPEC` is not set and a matched `RGX_PSPEC` contains no `:`, the
lexer checks whether the fragment before `*` is a known type. If so, it
pushes the `*` and anything after it back, transitions to S0 (expression
state), and returns `DT_TOK_TNAME`. This disambiguates `int*x;` (declaration)
from `int* { trace(timestamp); }` (glob probe specifier).

The Rust lexer always returns `ProbeSpecifier` from `ProgramOuterScope` with
no such check.


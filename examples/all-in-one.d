/* All-in-one example file: every grammar rule from `dt_grammar.y` (and
   the recursive-descent equivalents in `src/ast.rs::Parser`) appears at
   least once. Single-line comments name the production each line
   exercises. */

// ===== Control directives (lexer-recorded, passed through verbatim) =====

// `pragma_option` (no value).
#pragma  D  option  quiet
// `pragma_option` (`key=value`).
#pragma  D  option  bufsize=4m
// `pragma_depends_on` (module).
#pragma  D  depends_on  module  isa
// `pragma_depends_on` (library).
#pragma  D  depends_on  library  procfs.d

// Bare `#error`.
#error foo bar
// `#pragma D error`.
#pragma D error baz
// `#pragma error` (without the `D` prefix).
#pragma error something

// `pragma_line` (`#pragma D line`).
#pragma  D  line  1

// `cpp` `#include` (no preprocessor: passed through).
#include "stdio.h"
// `cpp` `#define` (no preprocessor: passed through).
#define  FOO 1
// `cpp` `#ifdef` / `#else` / `#endif`.
#ifdef FOO
int *foo;
#else
int *bar;
#endif
// `cpp` `#undef`.
#undef FOO

// `cpp` `#ifndef`, `#elif`, `#endif`.
#ifndef BAR
#define BAR 0
#elif BAR == 1
#define BAR 2
#endif

// `cpp` `#warning`.
#warning deprecated

// ===== `external_declaration` =====

// `declaration`: `declaration_specifiers init_declarator_list? ';'`.
// `type_specifier` `int` + `pointer`.
int *baz;
// `struct_or_union_specifier`: `struct` with body and tag.
struct Point {
  int x;
  int y;
};
// `struct` forward declaration (tag only, no body).
struct Node;
// `struct_or_union_specifier`: `union` with body and tag.
union Value {
  int i;
  char c;
};
// `enum_specifier` with explicit `enumerator` values.
enum Color {
  RED = 0,
  GREEN = 1,
  BLUE = 2
};
// `enum_specifier` with default-valued `enumerator`s.
enum Status {
  OK,
  ERR
};
// `enum_specifier` referenced by tag in a `declaration`.
enum Color cglobal;
// `typedef` (`storage_class_specifier`) wrapping an anonymous `enum`.
typedef enum {
  AccessKindRead = 1,
  AccessKindWrite = 2
} AccessKind;
// `typedef` wrapping an anonymous `struct` that references prior typedefs.
typedef struct {
  AccessKind kind;
  size_t tid;
  int ts;
} Access;
// D associative-array declaration: `array` keyed by `uintptr_t`.
Access accesses[uintptr_t /* data ptr */];
// `inline_definition`: `inline declaration_specifiers declarator '=' expression ';'`.
inline int MAX_SIZE = 1024;
// `pragma_binding` attaches to the inline above.
#pragma  D  binding  "1.0"  MAX_SIZE

inline char LEVEL = 1;

inline string LABEL = "ok";
// `pragma_attributes` attaches to the inline above.
#pragma  D  attributes  Stable/Stable/Common  LABEL

// `inline_definition` body is a `conditional_expression` (ternary).
inline int computed = x > 0 ? x : 0;
// `pointer` chain with multiple `type_qualifier`s: `const int * const cptr`.
const int * const cptr;
// Double `pointer`.
int **dptr;
// `typedef` wrapping a primitive `type_specifier`.
typedef int myint;
// `type_specifier`: `unsigned int`.
unsigned int uval;
// `type_specifier`: `signed long`.
signed long sval;
// `type_specifier`: `short`.
short shval;
// `type_specifier`: `void` (only valid in a pointer or function signature).
void *vptr;
// `type_qualifier`: `volatile`.
volatile int vval;
// `type_specifier`: D extension `string`.
string sstr;
// `storage_class_specifier`: `extern`.
extern int extvar;
// `storage_class_specifier`: `auto`.
auto int aval;
// `storage_class_specifier`: `static`.
static int sval2;
// `storage_class_specifier`: `register`.
register int rval;
// `type_qualifier`: `restrict` (rare but in the grammar).
restrict int *rptr;
// `d_storage_class_specifier`: `this` (clause-local variable).
this int tlsvar;
// `d_storage_class_specifier`: `self` (thread-local variable).
self int sclvar;
// `array` with no `array_parameters` — declaration of an unsized array.
int empty_arr[];
// `init_declarator_list` with two `init_declarator`s sharing one specifier.
int multi_a, multi_b;
// Function declarator (`direct_declarator` with `function_parameters`),
// fixed parameter list.
int foo(int a, int b);
// `(void)` — explicit zero-parameter list.
int func2(void);
// `parameter_type_list` with trailing `...` (`ParamEllipsis`).
int varargs0(int x, ...);
// Bare `...` parameter list (no fixed parameters).
extern int varargs1(...);
// `direct_declarator` = `'(' declarator ')'` — function-pointer form.
int (*fp)();
// Pointer-to-function returning pointer (`(void)` parameter list).
int *(*fp_ptr)(void);
// `struct_declarator` with `':' constant_expression` bit-field width.
struct Flags {
  unsigned int low : 3;
  unsigned int high : 5;
};
// `struct_declarator_list`: two declarators sharing one field type.
struct Pair {
  int a, b;
};
// `__attribute__` extension (lexer-skipped): standalone form.
__attribute__((nodtrace));
// `__attribute__` attached to the following `declaration`.
__attribute__((noreturn))
int noret_func(int x);
// `struct` containing a nested `union`.
struct WithUnion {
  union Tag {
    int i;
    char c;
  } tag;
  int x;
};
// `union` containing a nested `struct`.
union WithStruct {
  struct Sub {
    int x;
    int y;
  } sub;
  int raw;
};
// `struct` referencing a previously declared `struct` by tag.
struct HasPoint {
  struct Point p;
  int z;
};
// `translator_definition`:
// `translator from_type '<' to_type ident '>' '{' translator_member_list '}' ';'`.
// Each body line is a `translator_member`.
translator int < struct foo * P > {
  pr_pid = P->p_pid;
  pr_ppid = P->p_ppid;
};
// `provider_definition` containing a `provider_probe_list`. The first
// probe has a return-type list (`':' '(' parameter_list ')'`); the second
// has none.
provider myprov {
  probe start(int a, char *s) : (int);
  probe stop();
};
// ===== `probe_definition` =====

// `probe_specifiers`: comma-separated list, empty `statement_list_impl`.
BEGIN,
END
{
}
// Probe with `predicate` and a `statement_list` exercising every
// expression production. The empty `statement` rule (`';'` alone) is
// part of the grammar but the formatter drops it, so it is not shown
// here to keep this file idempotent under `fmt`.
syscall::open:entry
/ pid == 42 /
{
  // `multiplicative_expression`, `additive_expression`.
  x = a + b;
  x = a - b;
  x = a * b;
  x = a / b;
  x = a % b;

  // `and_expression`, `inclusive_or_expression`, `exclusive_or_expression`.
  x = a & b;
  x = a | b;
  x = a ^ b;

  // `shift_expression`.
  x = a << b;
  x = a >> b;

  // `equality_expression`.
  x = a == b;
  x = a != b;

  // `relational_expression`.
  x = a < b;
  x = a > b;
  x = a <= b;
  x = a >= b;

  // `logical_and_expression`, `logical_or_expression`.
  x = a && b;
  x = a || b;

  // `assignment_operator`: every variant.
  x = 1;
  x += 1;
  x -= 1;
  x *= 1;
  x /= 1;
  x %= 1;
  x &= 1;
  x |= 1;
  x ^= 1;
  x <<= 1;
  x >>= 1;

  // `unary_operator`.
  x = -y;
  x = !y;
  x = ~y;
  x = *y;
  x = &y;

  // Prefix increment / decrement (also `unary_expression`).
  ++x;
  --x;

  // Postfix increment / decrement (`postfix_expression`).
  x++;
  x--;

  // `postfix_expression`: function call.
  print();
  print(a);
  print(a, b, c);

  // `postfix_expression`: array subscript.
  x = a[i];
  x = a[i][j];

  // `postfix_expression`: `.` and `->` field access.
  x = a.b;
  x = a->b;
  x = a.b.c;

  // `conditional_expression` (ternary), possibly chained.
  x = a ? b : c;
  x = a == 1 ? 1 : a == 2 ? 2 : 3;

  // `expression`: `assignment_expression ( ',' assignment_expression )*`.
  x = a, b;

  // `unary_expression`: `sizeof` in every form.
  x = sizeof(int);
  x = sizeof(const int);
  x = sizeof(int *);
  x = sizeof(int * const);
  x = sizeof y;

  // `unary_expression`: `stringof` parenthesised and bare.
  x = stringof(y);
  x = stringof y;

  // `cast_expression`.
  x = (WithUnion)y;
  x = (int)-1;

  // `unary_expression`: `offsetof` keyword.
  n = offsetof(int, field);

  // `unary_expression`: `xlate <T>(expr)`.
  x = xlate <int>(ptr);

  // `primary_expression`: aggregation (D extension `@name`).
  @n++;

  // Anonymous aggregation `@` assigned from an aggregating function.
  @ = count();

  // Anonymous aggregation with a tuple key (`@[key]`).
  @[pid] = count();

  // Named aggregation with a tuple key (`@name[key]`).
  @x[pid] = sum(1);

  // `primary_expression`: thread-local (`self->name`) and clause-local
  // (`this->name`) variable references.
  self->y = 1;
  this->z = 2;

  // `primary_expression`: macro argument references (`$1`, `$name`).
  x = $1;
  x = $name;

  // `sizeof` with an `abstract_declarator` containing an `array`
  // (`DirectAbstractArray`).
  x = sizeof(int [10]);

  // `primary_expression`: identifier.
  x = NULL;

  // `primary_expression`: backtick-scoped kernel symbol reference.
  x = `global_sym;

  // `primary_expression`: character constant.
  x = 'a';

  // `primary_expression`: string literal (ASCII and multibyte UTF-8).
  x = "hello";
  x = "héllo";
  x = "日本語";
  x = "🎉";

  // `selection_statement`: `if` with braced block.
  if (x == 1) {
    y = 2;
  }

  // `if`/`else`.
  if (x == 1) {
    y = 2;
  } else {
    y = 3;
  }

  // `if`/`else if`/`else` chain.
  if (x == 1) {
    y = 2;
  } else if (x == 2) {
    y = 3;
  } else {
    y = 4;
  }

  // Braceless `statement_or_block` body: the grammar rule
  // `statement: expression ';'` fires here. The formatter wraps the
  // single statement in `{ … }` on output.
  if (x == 1) {
    y = 2;
  }
  if (x == 1) {
    y = 2;
  } else {
    y = 3;
  }
}

/* All-in-one  example  file .
   Every  grammar  construct  appears  here  at  least  once . */

#pragma  D  option  quiet
#pragma  D  option  bufsize=4m
#pragma  D  depends_on  module  isa
#pragma  D  depends_on  library  procfs.d

struct Point {
  int x;
  int y;
};

struct Node;

union Value {
  int i;
  char c;
};

enum Color {
  RED = 0,
  GREEN = 1,
  BLUE = 2
};

// Inline  constant  definition .
inline int MAX_SIZE = 1024;

#pragma  D  binding  "1.0"  MAX_SIZE

inline char LEVEL = 1;

inline string LABEL = "ok";

#pragma  D  attributes  Stable/Stable/Common  LABEL

inline int computed = x > 0 ? x : 0;

int *ptr;

const int * const cptr;

int **dptr;

typedef int myint;

unsigned int uval;

void *vptr;

volatile int vval;

// Whole line is skipped because `));` is followed by `;`.
__attribute__((nodtrace));
int foo(int a, int b);

// Only the attribute is skipped because there is no trailing `;`.
__attribute__((noreturn))
int baz(int x);

struct WithUnion {
  union Tag {
    int i;
    char c;
  } tag;
  int x;
};

union WithStruct {
  struct Sub {
    int x;
    int y;
  } sub;
  int raw;
};

struct HasPoint {
  struct Point p;
  int z;
};

translator int < struct foo * P > {
  pr_pid = P->p_pid;
  pr_ppid = P->p_ppid;
};

provider myprov {
  probe start(int a, char *s) : (int);
  probe stop();
};

BEGIN,
END
{
}

syscall::open:entry
/ pid == 42 /
{
  // Binary  operators .
  x = a + b;
  x = a - b;
  x = a * b;
  x = a / b;
  x = a % b;
  x = a & b;
  x = a | b;
  x = a ^ b;
  x = a << b;
  x = a >> b;
  x = a == b;
  x = a != b;
  x = a < b;
  x = a > b;
  x = a <= b;
  x = a >= b;
  x = a && b;
  x = a || b;
  x = 1;
  x += 1;
  x -= 1;
  x = -y;
  x = !y;
  x = ~y;
  x = *y;
  x = &y;
  ++x;
  --x;
  x++;
  x--;
  print();
  print(a);
  print(a, b, c);
  x = a[i];
  x = a[i][j];
  x = a.b;
  x = a->b;
  x = a.b.c;
  x = a ? b : c;
  x = a, b;
  x = sizeof(int);
  x = sizeof(const int);
  x = sizeof(int *);
  x = sizeof(int * const);
  x = sizeof y;
  x = stringof(y);
  x = stringof y;
  x = (mytype)y;
  n = offsetof(int, field);
  x = xlate <int>(ptr);
  @n++;
  x = NULL;
  x = `global_sym;
  x = (int)-1;
  x = a == 1 ? 1 : a == 2 ? 2 : 3;
  x = 'a';
  x = "hello";
  x = "héllo";
  x = "日本語";
  x = "🎉";
  /* Conditional  statements . */

  if (x == 1) {
    y = 2;
  }
  if (x == 1) {
    y = 2;
  } else {
    y = 3;
  }
  if (x == 1) {
    y = 2;
  } else if (x == 2) {
    y = 3;
  } else {
    y = 4;
  }
}

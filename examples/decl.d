struct Person {int age; int id;};

enum { Red, Green };

enum Color {Purple};

struct Color {int x;};

struct Outer {
  union  {
    int i;
  }  foo;
};

union  PersonOrColor { struct Person p; enum Color c;} ;

BEGIN {
  print(offsetof(struct Person, id));
  print(offsetof(union PersonOrColor, c));
  print(sizeof(struct Outer));
  print(offsetof(struct Outer, foo));
  print(sizeof(-2));
  print(sizeof -2);
  print(sizeof(int));
  print(sizeof int);
}

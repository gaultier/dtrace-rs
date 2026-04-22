struct Person {int age; int id;};

enum { Red, Green };

enum Color {Purple};

union  PersonOrColor { struct Person p; enum Color c;} ;

BEGIN {
  print(offsetof(struct Person, id));
  print(offsetof(union PersonOrColor, c));
}

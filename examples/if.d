BEGIN {
  if (1 == 2) {
    print("ok");
  } else {
    print("not ok");
  }

  if (1) print("inline")
}

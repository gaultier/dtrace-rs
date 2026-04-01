BEGIN {
  if (1 == 2) {
    print("ok");
  } else if (1==3) {
    print("maybe");
  }else {
    print("not ok");
  }

  if (1) print("inline");
  else print("else inline");
}

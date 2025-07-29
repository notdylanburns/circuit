# Anonymous Pins

Allows writing to pins without specifying them by name. Cannot be used with in a circ containing
both inputs/ouputs and transputs.

```
circ And(int N = 2) {
    assert N >= 2;

    -> anon(N);
    <- anon;

    with {
        And2 and[N-1];
    }

    for I in 0..N-1 {
        if I == 0 {
            and[I].a <- anon(I);
        } else {
            and[I].a <- and[I-1].q;
        }

        and[I].b <- anon(I);
    }
}

circ TestAnon {
    -> a, b, c;
    <- q;

    with And(3) and;

    and <- a; // writes to first anonymous input pin
    and <- b; // writes to second anonymous input pin
    and <- c; // writes to third anonymous input pin
    and -> q; // reads from the first anonymous output pin
}
```

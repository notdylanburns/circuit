# Pack Blocks

Adds a new block to (un)pack multiple locals from/into a bus

```
circ CollectIntoBus {
    -> a, b[5], c, d, e[2];
    <- bus[10];

    pack bus {
        0    <- a;
        1..2 <- b[3..4];
        3    <- c;
        4..6 <- b[0..2];
        7    <- d;
        8..9 <- e;
    }
}
```

```
circ UnpackFromBus {
    -> bus[10];
    <- a, b[5], c, d, e[2];

    pack bus {
        0    -> a;
        1..3 -> b[3..5];
        3    -> c;
        4..7 -> b[0..3];
        7    -> d;
        8..  -> e;
    }
}
```

```
circ FanOut {
    -> a;
    <- b, c, d, e, f;

    a -> b
      -> c
      -> d
      -> e
      -> f;

    pack a {
        .. -> b;
        .. -> c;
        .. -> d;
        .. -> e;
        .. -> f;
    }
}
```

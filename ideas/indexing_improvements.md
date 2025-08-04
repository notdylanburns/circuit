# Indexing Improvements

Support reverse indexing and multi indexing

## Reverse Indexing

```
circ Swap {
    -> a[2];
    <- q[2];

    a -> q[2..0];
}
```

## Multi Indexing

```
circ MultiIndex {
    -> a[5];
    <- q[5];

    a[0, 2, 4] -> q[..3];
    a[1, 3] -> q[3..];
}
```

## Bus Creation

```
circ IntoBus {
    -> a, b, c, d, e;
    <- q[5];

    {a, b, c, d, e} -> q;
}
```

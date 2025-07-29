# For Blocks

Allows repeating a block multiple times

```
circ Repeat(int N = 1) {
    -> input;
    <- output[N];

    for I in 0..N {
        output[I] <- input;
    }
}
```

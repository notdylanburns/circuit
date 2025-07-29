# Let Statement

Allows assigning a name to an expression

```
circ LetTest {
    -> bus[40];

    let data_bus = bus[..16];
    let addr_bus = bus[16..32];
    let rdy = bus[32];
    let flags = bus[33..39];
    let reset = bus[39];
    let nc = bus[40];
}
```

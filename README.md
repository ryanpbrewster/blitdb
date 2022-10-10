# Running an example
```
curl localhost:3000/v1/exec --data-binary @../data/add.wasm
```

should return `2 + 2 = 4`


# Resource Limits

We use wasmtime's fuel consumption to limit resources

```
curl localhost:3000/v1/exec --data-binary @../data/loop.wasm
```
should error out with
```
error: all fuel consumed by WebAssembly
wasm backtrace:
    0:   0x31 - <unknown>!<wasm function 0>
```

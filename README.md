# http_server
A simple HTTP/0.9 server implementation in Rust

# features

5ed451cb2edc5bf92a6d131db9384b48c186d830: return the content of the path file
ae6d3f7b2e4ec0f868a4a205323c1f3fb7ab72f4: anti directory traversal
93206ddf00c5c54eb6de3c126570bb8a685d6bd7: parse headers of HTTP/1.0


# run

``` console
$ cargo run

 # in another terminal
$ curl -0 http://localhost:8080/Cargo.toml
 # to see response headers,
$ curl -I0 http://localhost:8080/Cargo.toml
 # to see full conversation
$ curl -v0 http://localhost:8080/Cargo.toml
 # to check directory traversal
$ { echo "GET /../some_file HTTP/1.0\n" ; cat } | telnet localhost 8080
```


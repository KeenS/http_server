# http_server
A simple HTTP/0.9 server implementation in Rust

# features

5ed451cb2edc5bf92a6d131db9384b48c186d830: return the content of the path file


# run

``` console
$ cargo run

 # in another terminal
{ echo 'GET /Cargo.toml' ; cat } | telnet localhost 8080
 # to check directory traversal
$ { echo "GET ../some_file" ; cat } | telnet localhost 8080
```


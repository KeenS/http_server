mod parser;
mod data;
mod handler;
mod server;

use server::Server;

fn main() {
    let server = Server::new();
    match server.start("127.0.0.1:8080") {
        Ok(_) => (),
        Err(e) => println!("{:?}", e),
    }
}

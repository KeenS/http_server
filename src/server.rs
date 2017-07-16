use std::net::TcpListener;
use std::thread;
use std::io::{Read, Write};
use std::io;


use handler::FileHandler;
use parser;
use data::{Response, Status};


// star_serverだったものを一旦データ型とそのメソッドにしておく
// こうすることであとでデータを足してアレコレできる
pub struct Server;

impl Server {
    // 今は`Server`だけで初期化できるが、将来のため`new`を用意しておく。
    // もうちょっと本気を出すなら[`PhantomData`][^1]でプライベートなフィールドを用意して
    // `Server`では初期化できないようにしておくべき
    // [^1]: https://doc.rust-lang.org/std/marker/struct.PhantomData.html
    pub fn new() -> Self {
        Server
    }
    pub fn start(&self, address: &str) -> io::Result<()> {
        // 127.0.0.1:8080をlistenし、acceptしつづけるリスナーを作成
        let lis = TcpListener::bind(address)?;
        // コネクションがある度にstreamを取り出せる
        for stream in lis.incoming() {
            // streamを包むResultを一旦剥がす
            // エラーが出てもループを継続するために`?`は使わない
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(e) => {
                    // acceptでエラーが起きたらそれを通知して次のループへ
                    println!("An error occured while accepting a connection: {}", e);
                    continue;
                }
            };
            // IO処理はブロックするので別スレッドを立てる
            // そうすることでリクエストを処理しつつ新たなコネクションを受け付けられる
            // スレッドはspawnしたあと見捨てるので返り値のスレッドハンドルは無視
            let _ = thread::spawn(
                // spawnの引数にはクロージャを渡す
                // クロージャは `|引数| 本体` あるいは `|引数| -> 返り値の型 { 本体 }` で作る
                // 関数と違って`()`でなくとも型を推論できるなら引数の型や`-> 返り値の型`を省略できる
                // `move`はクロージャが捕捉した変数（今回は`stream`）の所有権をクロージャにムーブするためのキーワード
                move || -> io::Result<()> {
                    use parser::ParseResult::*;
                    // リクエスト全体を格納するバッファ
                    let mut buf = Vec::new();
                    loop {
                        // 1回のread分を格納する一時バッファ
                        let mut b = [0; 1024];
                        // 入力をバッファに読み込む
                        // nには読み込んだバイト数が入る
                        let n = stream.read(&mut b)?;
                        if n == 0 {
                            // 読み込んだバイト数が0ならストリームの終了。
                            // スレッドから抜ける。
                            return Ok(());
                        }
                        // リクエスト全体のバッファに今読み込んだ分を追記
                        buf.extend_from_slice(&b[0..n]);
                        // それ以外ではHTTP/1.0のリクエストの処理
                        // レスポンスのフォーマットは以下を参照
                        // https://tools.ietf.org/html/rfc1945#section-6
                        // 課題はヘッダのパースのみなのでレスポンスは雑に返す。
                        match parser::parse(buf.as_slice()) {
                            // 入力の途中なら新たな入力を待つため次のイテレーションへ
                            Partial => continue,
                            // エラーなら不正な入力なのでBad Requestを返す
                            // スレッドから抜けると`stream`のライフタイムが終わるため、コネクションが自動で閉じられる
                            Error => {
                                let res = Response::new(Status::BadRequest);
                                // continueとErrorでこの2行が被っている。
                                // [loop_break_values][^1]が入れば解決するが、
                                // それまであとちょっとだけ辛抱が必要
                                // [^1] https://github.com/rust-lang/rust/pull/42016
                                res.print_http(&mut stream)?;
                                return Ok(());

                            }
                            // リクエストが届けば処理をする
                            Complete(mut req) => {
                                // HTTPに依存しないファイルハンドリングは別に切り出した
                                let handler = FileHandler::new("./")?;
                                match handler.handle(&mut req) {
                                    Ok(res) => {
                                        res.print_http(&mut stream)?;
                                        return Ok(());
                                    }
                                    // 内部の処理でエラーが起きたらInternal Server Error
                                    Err(e) => {
                                        let mut res = Response::new(Status::InternalServerError);
                                        let mut body = Vec::new();
                                        write!(&mut body, "{}", e)?;
                                        res.body = Some(body);
                                        res.print_http(&mut stream)?;
                                        return Ok(());
                                    }
                                };
                            }
                        };
                    }
                },
            );
        }
        Ok(())
    }
}

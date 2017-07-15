mod parser;

use std::net::TcpListener;
use std::thread;
use std::io::Read;
use std::io;
use std::fs::File;
use std::path::PathBuf;

fn server_start() -> io::Result<()> {
    // 127.0.0.1:8080をlistenし、acceptしつづけるリスナーを作成
    let lis = TcpListener::bind("127.0.0.1:8080")?;
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
                    // それ以外ではHTTP/0.9のリクエストの処理
                    match parser::parse(buf.as_slice()) {
                        // 入力の途中なら新たな入力を待つため次のイテレーションへ
                        Partial => continue,
                        // エラーなら不正な入力なので何も返さずスレッドから抜ける
                        // スレッドから抜けると`stream`のライフタイムが終わるため、コネクションが自動で閉じられる
                        Error => {
                            return Ok(());
                        }
                        // リクエストが届けば処理をする
                        Complete(req) => {
                            // レスポンスを返す処理をここに書く

                            // 相対ディレクトリでアクセスするために
                            // リクエストのパスの先頭の`/`を取り除く
                            let mut path = req.0;
                            while path.starts_with("/") {
                                path = &path[1..];
                            }

                            // 相対パスにする
                            let path = PathBuf::new().join("./").join(path);
                            assert!(path.is_relative());
                            // ファイルを開く。HTTP/0.9はエラーがないので
                            // 純粋なIOエラーとFileが見付からないエラーを区別しない
                            let mut file = File::open(path)?;
                            // io::copyでinputからoutputへコピーできる
                            io::copy(&mut file, &mut stream)?;
                            // 処理が完了したらスレッドから抜ける
                            return Ok(());
                        }
                    };
                }
            },
        );
    }
    Ok(())
}

fn main() {
    match server_start() {
        Ok(_) => (),
        Err(e) => println!("{:?}", e),
    }
}

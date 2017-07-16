use std::io::Read;
use std::io;
use std::fs::File;
use std::path::PathBuf;

use data::{Request, Response, Status};


// HTTPに依存しないファイルハンドリングは別に切り出す。
// ついでにドキュメントルートも設定できるようにした。
pub struct FileHandler {
    root: PathBuf,
}


impl FileHandler {
    pub fn new<P: Into<PathBuf>>(path: P) -> io::Result<Self> {
        Ok(FileHandler { root: path.into().canonicalize()? })
    }

    // `Response` を導入したので直接ストリームにデータを書き出さずに `Response` を返すようにする
    // これでHTTPプロトコルの詳細と分離された
    pub fn handle(&self, req: &mut Request) -> io::Result<Response> {
        // 相対ディレクトリでアクセスするために
        // リクエストのパスの先頭の`/`を取り除く
        let mut path = req.path;
        while path.starts_with("/") {
            path = &path[1..];
        }

        let path = PathBuf::new()
        // リクエストをルートからの相対パスに変換して
            .join(self.root.as_path())
            .join(path)
        // 絶対パスにする
            .canonicalize()?;
        // 正準な絶対パス同士の比較でベースディレクトリから始まらないパスに
        // アクセスしようとしていればディレクトリトラバーサルなので
        // Bad Requestとする
        if !path.starts_with(&self.root) {
            return Ok(Response::new(Status::BadRequest));
        }
        // ファイルを開く。HTTP/1.0はエラーがあるので
        // エラーハンドリングをする。
        match File::open(path) {
            Ok(mut file) => {
                let mut res = Response::new(Status::Ok);
                let mut body = Vec::new();
                // `Response` にするために一旦メモリ上の`Vec`に読み出す。
                // ファイルデータをメモリに載せるのが嫌なら引数にストリームを受け取って自分で書く方法もあるが
                // ヘッダとかの一貫性のためかなり技巧的なコードになるので今回はこちらを採用
                file.read_to_end(&mut body)?;
                res.body = Some(body);
                return Ok(res);
            }
            Err(ioerror) => {
                use self::io::ErrorKind::*;
                match ioerror.kind() {
                    // ファイルがなければNot Found
                    NotFound => return Ok(Response::new(Status::NotFound)),
                    // それ以外はInternal Server Error
                    _ => return Ok(Response::new(Status::InternalServerError)),
                }
            }
        }
    }
}

use std::str::from_utf8;

// パーサの結果は成功/失敗の他に「入力が途中で終わってしまった」もあり、`Result`が使えないので自前定義
// モジュールを使うと可視性が必要になるが、`pub`がつくと上位のモジュールから見えるようになる
pub enum ParseResult<T> {
    // 列挙型の列挙子に`pub`は必要ない
    Complete(T),
    Partial,
    // 簡単のため今回はエラーデータを省略
    Error,
}

// ジェネリクス型の`impl`を書くにはこのようにする
impl<T> ParseResult<T> {
    // `pub`がついていないためこの関数は上位のモジュールから見えない
    fn is_complete(&self) -> bool {
        // 実はこのようにで列挙型の列挙子をインポートすることができる
        // この`self`は`parser`モジュールを指す
        use self::ParseResult::*;
        match *self {
            // インポートしてしまうとプリフィクスなしで参照できる
            Complete(_) => true,
            _ => false
        }
    }

    fn is_partial(&self) -> bool {
        use self::ParseResult::*;
        match *self {
            Partial => true,
            _ => false
        }
    }

}

// 便利のため、標準ライブラリの`From<T>`を実装する
// これで普通の`Result`を`ParseResult`に変換できるようになる
impl<T, E> From<Result<T, E>> for ParseResult<T> {
    fn from(r: Result<T, E>) -> Self {
        use self::ParseResult::*;
        match r {
            Ok(t) => Complete(t),
            Err(_) => Error,
        }
    }
}

// パース結果を表わす構造体
// `'`から始まるものはライフタイムパラメータ。ライフタイムも型のようにプログラム上で扱える
// 構造体のフィールドには1つ1つ`pub`が必要
pub struct Request<'a>(pub &'a str);

// HTTP/0.9のリクエストは `METHOD path\r\n`の形をしている
// 今回はGETだけパースする
pub fn parse(mut buf: &[u8]) -> ParseResult<Request> {
    use self::ParseResult::*;
    // `b".."` でバイト列リテラル
    let get = b"GET ";
    let end = b"\r\n";
    // GET がこなければエラー
    if !buf.starts_with(get) {
        return Error;
    }
    // 残りはパスネーム
    buf = &buf[get.len()..];
    if buf.ends_with(end) {
        buf = &buf[0..buf.len() - end.len()]
    } else {
        // 末尾が`\r\n`でなければ入力が完了していないとみなす
        return Partial;
    }

    // `from_utf8`で`&[u8]`から`&str`が作れる。データのコピーはしない。
    // ただし失敗するかもしれないので返り値は`Result`に包まれる
    from_utf8(buf)
        // タプル構造体は関数としても扱える
        // Result<&str, Utf8Error> -> Result<Request, Utf8Error>
        .map(Request)
        // `From`を実装したので自動で実装された`Into`の`into`メソッドを呼んでいる
        //  Result<Request, Utf8Error> -> ParseResult<Request>
        .into()
}


// `#[..]`でその次にくるアイテムに「アトリビュート」をつけられる
// `#[test]`アトリビュートでアイテムがテストであることをコンパイラに伝える
#[test]
fn http09_get_success_root() {
    let req = b"GET /\r\n";
    let res = parse(req);
    // テストの中身では主に`assert`マクロを使い、何もなければOパニックならXとなる
    assert!(res.is_complete());
}

#[test]
fn http09_get_success_foo_bar() {
    let req = b"GET /foo/bar\r\n";
    let res = parse(req);
    assert!(res.is_complete());
}

#[test]
fn http09_get_partial_root() {
    let req = b"GET /\r";
    let res = parse(req);
    assert!(res.is_partial());
}


// テストに`should_panic`アトリビュートをつけることでパニックしたらO、しなかったらXとなる
#[test]
#[should_panic]
fn http09_post_failure() {
    let req = b"POST /\r\n";
    let res = parse(req);
    assert!(res.is_complete());
}


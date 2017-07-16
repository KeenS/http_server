use std::str::from_utf8;

use data::{Request, Method, Version};

// パーサの結果は成功/失敗の他に「入力が途中で終わってしまった」もあり、`Result`が使えないので自前定義
// モジュールを使うと可視性が必要になるが、`pub`がつくと上位のモジュールから見えるようになる
pub enum ParseResult<T> {
    // 列挙型の列挙子に`pub`は必要ない
    Complete(T),
    Partial,
    // 簡単のため今回はエラーデータを省略
    Error,
}

// cfgを使うことでアイテムを条件コンパイルすることができる
// 今回はこれらのメソッドはテスト時にしか使っていないので
// テストの時のみコンパイルするようにする。
// 因みに`impl`ブロックは複数個書けるの
#[cfg(test)]
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
            _ => false,
        }
    }

    fn is_partial(&self) -> bool {
        use self::ParseResult::*;
        match *self {
            Partial => true,
            _ => false,
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


// rustにはマクロがある。 See [マクロ](https://rust-lang-ja.github.io/the-rust-programming-language-ja/1.6/book/macros.html)
// これはParseResurtに対して、「結果がCompleteならその結果を取り出すが、それ以外なら関数から抜ける」
// 処理をするマクロ。`Result`型の`?`演算子のような動きをする。
macro_rules! ptry {
    // マクロは引数でパターンマッチできるため、関数よりは`match`に近い構文で定義する
    // 今回は別にパターンマッチはしない。
    ($res: expr) => {
        match $res {
            ParseResult::Complete(t) => t,
            ParseResult::Partial => return ParseResult::Partial,
            ParseResult::Error => return ParseResult::Error,
        }
    }
}




// HTTP/1.0のヘッダの仕様は以下にある。
// https://tools.ietf.org/html/rfc1945#section-5
// おおまかには
// Request        = Simple-Request | Full-Request
//
// Simple-Request = "GET" SP Request-URI CRLF
//
// Full-Request   = Request-Line
//                  *( General-Header
//                   | Request-Header
//                   | Entity-Header )
//                  CRLF
//                  [ Entity-Body ]
// なので新たに
// ```
// METHOD /path/ HTTP/1.0\r\n
// Header: value\r\n
// ...
// \r\n
// body
// ```
// をパースできればよい。事前に定義されているヘッダーは以下にあるが
// https://tools.ietf.org/html/rfc1945#section-10
// それぞれのパースはただの作業なので今回は行なわない


// Request = Simple-Request | Full-Request
pub fn parse(buf: &[u8]) -> ParseResult<Request> {
    use self::ParseResult::*;
    // parse_09の作りが雑なので最初にFull-Requestを試してダメだったら
    // Simple-Requestにフォールバックする
    match parse_10(buf) {
        Complete(t) => Complete(t),
        Partial => Partial,
        Error => parse_09(buf),
    }
}

// HTTP/0.9のリクエストは `METHOD path\r\n`の形をしている
// 今回はGETだけパースする
fn parse_09(mut buf: &[u8]) -> ParseResult<Request> {
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
        .map(|path| {
            let mut req = Request::new(Method::Get, path);
            req.version = Version::HTTP09;
            req
        })
        // `From`を実装したので自動で実装された`Into`の`into`メソッドを呼んでいる
        //  Result<Request, Utf8Error> -> ParseResult<Request>
        .into()
}

// HTTP/1.0のFull-Requestは再掲すると
// Full-Request   = Request-Line
//                  *( General-Header
//                   | Request-Header
//                   | Entity-Header )
//                  CRLF
//                  [ Entity-Body ]
// なのでRequest-Line, Header, Bodyをパースできればよい。
// ただしbodyの長さはヘッダの`Content-Length`に入っているのでそこだけ特別扱いする
fn parse_10(mut buf: &[u8]) -> ParseResult<Request> {
    // C言語でよく出てくるポインタのポインタ
    // Rustであまり見ないが、今回サボるために使用
    let buf: &mut &[u8] = &mut buf;
    // Request-Lineのパース
    let (method, path) = ptry!(parse_request_line(buf));
    // 初期データが揃ったのでリクエストの準備
    let mut request = Request::new(method, path);

    // Headerのパース
    // 後で（ループの中などで）値が入る変数を宣言するには`Option`を使う
    // 必ず入ると保障できる(コンパイラがチェックできる)なら`let content_length;`のように`=`無しでも使える。
    // 今回は入ると保障できないので`Option`を使う（実際`let content_length;`にするとコンパイルエラー）
    let mut content_length: Option<usize> = None;
    // ヘッダフィールドが終わるまでヘッダをパースする
    while !buf.starts_with(b"\r\n") {
        // ヘッダをパース
        let (name, value) = ptry!(parse_header(buf));
        // Content-Lengthだけ特別に拾う
        // 上述の通りcase sensitiveに比較する。
        if name == "Content-Length" {
            match value {
                None => return ParseResult::Error,
                Some(v) => {
                    let v = ptry!(from_utf8(v).into());
                    content_length = Some(ptry!(v.parse().into()));
                }
            }
        }

        // 本来はフィールドがコンマで分割されるヘッダは複数回の出現が許されるが
        // 処理が結構ややこしくなるので2回以上出てきたら古いものを捨てることにする
        let prev = request.headers.insert(name, value);
        // `if let`構文。`if let パターン = 式`で式がパターンにマッチしたときだけifの中身を実行する。
        // 軽い`match`文のようなもの。
        if let Some(prev) = prev {
            println!(
                "[WARN] duplicated header: {}. discarding previous value: {:?}",
                name,
                prev
            )
        }
    }
    // \r\nを飛ばす
    let () = ptry!(parse_crlf(buf));

    // Content-LengthがあればBodyもパースする
    if let Some(size) = content_length {
        let body = ptry!(parse_body(buf, size));
        request.body = Some(body);
    }
    ParseResult::Complete(request)
}

// Request-Line = Method SP Request-URI SP HTTP-Version CRLF
fn parse_request_line<'a>(buf: &mut &'a [u8]) -> ParseResult<(Method<'a>, &'a str)> {
    let method = ptry!(parse_method(buf));
    let () = ptry!(parse_sp(buf));
    let path = ptry!(parse_uri(buf));
    let () = ptry!(parse_sp(buf));
    let () = ptry!(parse_http10_version(buf));
    let () = ptry!(parse_crlf(buf));
    ParseResult::Complete((method, path))
}


// HTTP-header    = field-name ":" [ field-value ] CRLF
//
// field-name     = token
// field-value    = *( field-content | LWS )
//
// field-content  = <the OCTETs making up the field-value
//                  and consisting of either *TEXT or combinations
//                  of token, tspecials, and quoted-string>
fn parse_header<'a>(buf: &mut &'a [u8]) -> ParseResult<(&'a str, Option<&'a [u8]>)> {
    let name = ptry!(parse_token(buf));
    let () = ptry!(parse_fixed(buf, b": "));
    let mut value = None;
    // フィールドの値がオプショナルなのでCRLFがこなかったときだけパースする
    if !buf.starts_with(b"\r\n") {
        value = Some(ptry!(parse_header_field_value(buf)));
    }
    let () = ptry!(parse_crlf(buf));
    ParseResult::Complete((name, value))
}

// BodyのパースはContent-Length分読むだけ
fn parse_body<'a>(buf: &mut &'a [u8], size: usize) -> ParseResult<&'a [u8]> {
    if size <= buf.len() {
        let ret = &buf[0..size];
        *buf = &buf[size..];
        ParseResult::Complete(ret)
    } else {
        ParseResult::Partial
    }
}


//  Method  = "GET"
//          | "HEAD"
//          | "POST"
//          | extension-method
fn parse_method<'a>(buf: &mut &'a [u8]) -> ParseResult<Method<'a>> {
    use self::Method::*;
    let pos = match buf.iter().position(|&b| b == ' ' as u8) {
        Some(p) => p,
        None => return ParseResult::Error,
    };
    let method = match &buf[0..pos] {
        b"GET" => Get,
        b"HEAD" => Head,
        b"POST" => Post,
        other => Ext(ptry!(from_utf8(other).into())),
    };
    *buf = &buf[pos..];
    ParseResult::Complete(method)
}

// URIは複雑なので正確にはパースしない
// 空白までをURLとする
fn parse_uri<'a>(buf: &mut &'a [u8]) -> ParseResult<&'a str> {
    let pos = match buf.iter().position(|b| b" \t\n\r".contains(b)) {
        Some(p) => p,
        None => return ParseResult::Error,
    };
    let s = &buf[0..pos];
    *buf = &buf[pos..];
    let s = ptry!(from_utf8(s).into());
    ParseResult::Complete(s)
}

fn parse_http10_version<'a>(buf: &mut &'a [u8]) -> ParseResult<()> {
    parse_fixed(buf, b"HTTP/1.0")
}

// ヘッダのvalueは先頭を空白にすることで複数行にまたがることができるが、今回は扱わない
// （扱おうとするとvalueが`Option<Vec<&'a str>>`になり面倒）
// 単純に最初のCRLFまでをvalueとする
fn parse_header_field_value<'a>(buf: &mut &'a [u8]) -> ParseResult<&'a [u8]> {
    let pos = match buf.iter().position(|&b| b == b'\r') {
        Some(p) => p,
        None => return ParseResult::Error,
    };

    if buf[pos + 1] == b'\n' {
        let v = &buf[..pos];
        *buf = &buf[pos..];
        ParseResult::Complete(v)
    } else {
        ParseResult::Error
    }
}



// token          = 1*<any CHAR except CTLs or tspecials>
//
// tspecials      = "(" | ")" | "<" | ">" | "@"
//                | "," | ";" | ":" | "\" | <">
//                | "/" | "[" | "]" | "?" | "="
//                | "{" | "}" | SP | HT
fn parse_token<'a>(buf: &mut &'a [u8]) -> ParseResult<&'a str> {
    let token_chars =
        br#"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890!#$%&'*+-.^_`|~"#;
    let mut pos = 0;
    while pos < buf.len() && token_chars.contains(&buf[pos]) {
        pos += 1;
    }
    let s = &buf[..pos];
    *buf = &buf[pos..];
    // ここでは`s`には`token_chars`しか含まれないことが分かっているので
    // `unwrap`を呼んでも安全
    ParseResult::Complete(from_utf8(s).unwrap())

}

fn parse_crlf<'a>(buf: &mut &'a [u8]) -> ParseResult<()> {
    parse_fixed(buf, b"\r\n")
}

fn parse_sp<'a>(buf: &mut &'a [u8]) -> ParseResult<()> {
    parse_fixed(buf, b" ")
}

fn parse_fixed<'a>(buf: &mut &'a [u8], fixed: &[u8]) -> ParseResult<()> {
    if buf.starts_with(fixed) {
        *buf = &buf[fixed.len()..];
        ParseResult::Complete(())
    } else {
        ParseResult::Error
    }
}


// `#[..]`でその次にくるアイテムに「アトリビュート」をつけられる
// `#[test]`アトリビュートでアイテムがテストであることをコンパイラに伝える
#[test]
fn http10_get_success_root() {
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
fn http10_get_partial_root() {
    let req = b"GET /\r";
    let res = parse(req);
    assert!(res.is_partial());
}


// テストに`should_panic`アトリビュートをつけることでパニックしたらO、しなかったらXとなる
#[test]
#[should_panic]
fn http10_post_failure() {
    let req = b"POST /\r\n";
    let res = parse(req);
    assert!(res.is_complete());
}


#[test]
fn http10_curl_request() {
    let req = b"GET /Calgo.toml HTTP/1.0\r\nHost: localhost:8080\r\nUser-Agent: curl/7.52.1\r\nAccept: */*\r\n\r\n";
    let res = parse(req);
    assert!(res.is_complete());
    let res = match res {
        ParseResult::Complete(r) => r,
        _ => unreachable!(),
    };
    assert_eq!(res.method, Method::Get);
    assert_eq!(res.path, "/Calgo.toml");
    assert_eq!(res.version, Version::HTTP10);
}

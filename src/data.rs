use std::io;
use std::collections::HashMap;


// 今までのコードで扱っていなかったが、バージョンを導入する
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Version {
    HTTP09,
    HTTP10,
}

// バージョンのデフォルトは1.0とする
impl<'a> Default for Version {
    fn default() -> Self {
        Version::HTTP10
    }
}

// データ型には`#[derive()]`アノテーションでいくつかのトレイトを自動で実装できる
// Copy以外は出来る限り実装した方がいいとされる。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
// `'`から始まるものはライフタイムパラメータ。ライフタイムも型のようにプログラム上で扱える
// RFC1945ではGET, HEAD, POST, extention-methodが定義されている。
pub enum Method<'a> {
    Get,
    Head,
    Post,
    Ext(&'a str),
}

// `Request`を`Default`にするためにここで`Default`を実装しておく
impl<'a> Default for Method<'a> {
    fn default() -> Self {
        Method::Get
    }
}

// パース結果を表わす構造体
// 構造体のフィールドには1つ1つ`pub`が必要
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Request<'a> {
    pub path: &'a str,
    pub method: Method<'a>,
    pub version: Version,
    // ヘッダは本来はcase insensitiveなのだが処理がややこしくなるので
    // 今回はcase sensitiveに扱うことにする。
    // 多くのクライアントはCamel-Kabab-Caseで送ってくるのであまり困らない。
    // case insensitiveに扱いたければ[unicase][^1]など使うとよい
    // [^1]: https://crates.io/crates/unicase
    pub headers: HashMap<&'a str, Option<&'a [u8]>>,
    pub body: Option<&'a [u8]>,
}

impl<'a> Request<'a> {
    pub fn new(method: Method<'a>, path: &'a str) -> Self {
        // Requestのimplの中では`Self`で`Request`を初期化できる
        Self {
            // フィールド名と初期化する値として渡す変数名が同じなら
            // `フィールド名: 値,`
            // を省略して
            // `フィールド名,`
            // だけで初期化できる
            path,
            method,
            // `..値`構文で指定したフィールド以外のフィールドを与えた値のフィールド値で埋める。
            // 今回は`Default`トレイトを使ってデフォルト値で埋める。
            ..Self::default()
        }
        // 上の初期化は以下のコードと同じ
        // ```rust
        // let mut tmp = Self::default();
        // Self {
        //     path: path,
        //     method: method,
        //     headers: tmp.headers,
        //     body: tmp.body,
        // }
        // ```

    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Status {
    // /// でドキュメントを付けることができる。
    /// 200 OK
    Ok, // 因みにこのようにコンマの後にコメントやドキュメントを書くと
    // `BadRequest`へのコメントやドキュメントになるので注意
    /// 400 Bad Request
    BadRequest,
    /// 404 Not Fonud
    NotFound,
    /// 500 Internal Server Error
    InternalServerError,
}

impl Default for Status {
    fn default() -> Self {
        Status::Ok
    }
}

// https://tools.ietf.org/html/rfc1945#section-6
#[derive(Debug, Clone, Eq, PartialEq, Default)]
// Responseは主に関数の中で作られるので参照を使わない
// `&str` -> `String`, `&[u8]` -> `Vec<u8>` の対応関係に注目
pub struct Response {
    pub status: Status,
    pub version: Version,
    // `Request`と同じくヘッダ内での改行は扱わないことにする
    // 改行が入るといけないのでユーザに直接は触らせず、getter, setterを通じてのみ許可する
    headers: HashMap<String, Option<Vec<u8>>>,
    pub body: Option<Vec<u8>>,
}
// 豆知識: このように一部プライベートなフィールドを持つ構造体を分配束縛するには
// ```rust
// let Response {status, version, body, ..} = res
// ```
// のように`..`で束縛できないフィールドを無視すると可能になる



impl Response {
    pub fn new(status: Status) -> Self {
        Self {
            status,
            // TODO: get version info from the request
            ..Self::default()
        }
    }

    // ちょっとライフタイムがややこしい。データの保有者の`self`とそこから取り出したデータのライフタイムは
    // 同じだが、検索に使う一時データの`name`のライフタイムは同じである必要がないのでそう書く。
    // Rustのライフタイムはデフォルトでは推論されるのではなく、典型的なユースケースでのみ書かなくていいという
    // 省略ルールなので典型から外れると手で書く必要がある。
    // （マニアックな話）何故推論せずに省略ルールかというとライフタイムはサブタイピングをするので恐らく推論は
    // 決定不能。
    pub fn get_header<'slf, 'key>(&'slf self, name: &'key str) -> Option<&'slf Vec<u8>> {
        // 以外にもRustに`Option<Option<T>>` -> `Option<T>`をやるメソッドがなかったので手書き
        match self.headers.get(name) {
            Some(&Some(ref t)) => Some(t),
            _ => None,
        }
    }
    // set_headerの実装は読者への課題とする


    // パーサと比べてプリンタは簡単なので別モジュールは立てずメソッドとして定義してしまう
    // ストリームに書き出すのではなくメモリ上に欲しい場合でも `Vec<u8>` が `Write` を実装しているので
    // `w` に `Vec<u8>` を渡してあげればよい。
    pub fn print_http<W: io::Write>(&self, mut w: &mut W) -> io::Result<()> {
        // 実は今までのコードはHTTP0.9のリクエストがきたときも1.0のレスポンスを返していたので規格違反だった
        match self.version {
            Version::HTTP09 => self.print_http09(w),
            Version::HTTP10 => self.print_http10(w),
        }

    }

    // https://tools.ietf.org/html/rfc1945#section-6
    fn print_http10<W: io::Write>(&self, mut w: &mut W) -> io::Result<()> {
        match self.status {
            Status::Ok => write!(w, "HTTP/1.0 200 Ok\r\n")?,
            Status::BadRequest => write!(w, "HTTP/1.0 400 Bad Request\r\n")?,
            Status::NotFound => write!(w, "HTTP/1.0 404 Not Fonud\r\n")?,
            Status::InternalServerError => write!(w, "HTTP/1.0 500 Internal Server Error\r\n")?,
        };
        for (name, value) in self.headers.iter() {
            write!(w, "{}: ", name)?;
            // ここの`value`は`iter()` 経由で`self.headers`から借りてきたものなのでムーブできない。
            // ref patternを使うことでマッチした変数が値をby moveでなくby refで束縛できる。
            if let &Some(ref v) = value {
                w.write(v.as_ref())?;
            }
            write!(w, "\r\n")?;
        }
        // こちらでも`Content-Length`は特別扱いする。
        if !self.headers.contains_key(&"Content-Length".to_string()) {
            if let Some(ref body) = self.body {
                write!(w, "Content-Length: {}\r\n", body.len().to_string())?;
            }

        }

        write!(w, "\r\n")?;
        if let Some(ref v) = self.body {
            w.write(v.as_ref())?;
        }
        Ok(())
    }

    fn print_http09<W: io::Write>(&self, mut w: &mut W) -> io::Result<()> {
        if let Some(ref body) = self.body {
            w.write(body.as_ref())?;
        }
        Ok(())

    }
}

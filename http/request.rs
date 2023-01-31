use crate::http::error::HttpError;
use crate::http::response::Response;
use crate::http::stream::HttpTcpStream;
use lazy_static_include::syn::__private::str;
use std::collections::HashMap;
use std::fmt::Debug;
use std::net::ToSocketAddrs;
use url::Url;

#[derive(PartialEq, Debug, Clone)]
pub enum ContentType {
    Length(usize),
    Chunked,
    None,
}
#[derive(Debug, Clone)]
pub struct Request {
    secondline: usize, //第二行起始位
    body_start: usize, //body起始位
    proto: String,
    method: String,
    client_method: String,
    pub path: String,
    query: String,
    pub uri: String,
    keep_alive: bool,
    pub content_length: ContentType,
    pub header: HashMap<String, String>,
    err: String,
    pub host: String,
    port: u16,
    pub(crate) code: u16,
    max_redirects: u8, //重定向次数，默认5，最大100
    redirects: u8,
    request_host: String,
    max_package_size: usize, //最大数据包
    post_body: String,
    is_https:bool,
}
impl Request {
    pub fn new(url :&str) -> Request {
       let mut req= Request {
            secondline: 0,
            body_start: 0,
            proto: "".to_string(),
            method: "".to_string(),
            client_method: "GET".to_string(),
            path: "/".to_string(),
            query: "".to_string(),
            uri: "".to_string(),
            keep_alive: false,
            content_length: ContentType::None,
            header: Default::default(),
            err: "".to_string(),
            host: "".to_string(),
            port: 0,
            code: 0,
            max_redirects: 5,
            redirects: 0,
            request_host: "".to_string(),
            max_package_size: 10 * 1024 * 1024,
            post_body: "".to_string(),
            is_https:false,
        };
        req.url_parse(url);
        req.header.insert("User-Agent".to_string(),"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.1 Safari/605.1.15".to_string());
        req
    }
    pub fn set_max_redirects(&mut self, n: u8) -> &Request {
        if n > 100 {
            self.max_redirects = 100
        } else {
            self.max_redirects = n
        }
        self
    }
    fn format_message(& self) -> String {
        let mut new_herder = "".to_string();
             for (k, v) in &self.header {
            new_herder =new_herder+ k+": " +v + "\r\n";
        }
        if self.post_body!=""{
            new_herder=new_herder+"content-length: "+self.post_body.len().to_string().as_str();
        }
        format!("{method} {path} HTTP/1.1\r\nAccept-Language: zh-CN,zh-Hans;q=0.9\r\nAccept-Encoding: gzip, deflate\r\nHost: {host}\r\nConnection: keep-alive\r\n{header}\r\n\r\n{body}",method=self.client_method,path=self.path,host=self.request_host,header=new_herder,body=self.post_body)
    }
    pub async fn doexec<'a>(&'a mut self, stream: Box<HttpTcpStream>) -> Result<Response, HttpError> {

        self.do_http1raw_request(self.format_message().as_bytes(), stream)
            .await
    }
    pub async fn exec<'a>(&mut self) -> Result<Response, HttpError> {
        let mut addrs_iter = format!("{}:{}", self.host, self.port).to_socket_addrs()?;
        match addrs_iter.next() {
            Some(addr) => {

                let  stream =Box::new(HttpTcpStream::new_with_addr(addr,self.is_https,&self.host).await?) ;
                let resp=self.doexec( stream).await?;
                return Ok(resp);
            }
            None => {
                return Err(HttpError::AddressError(format!(
                    "{}:{}",
                    self.host, self.port
                )))
            }
        }
    }
    pub fn get(url: &str) -> Request {
        let mut req = Request::new(url);
        req.client_method = "GET".to_string();

        req
    }
    pub fn post(url: &str, content_type: Option<&str>, body: &str) -> Request {
        let mut req = Request::new(url);
        req.client_method = "POST".to_string();

        if let Some(content_type) = content_type {
            req.header
                .insert("Content-Type".to_string(), content_type.to_string());
        }
        req.post_body = body.to_string();
        req
    }
    pub fn do_by_method(url: &str,method:String, content_type: Option<&str>, body: &str) -> Request {
        let mut req = Request::new(url);
        req.client_method = method;
        if let Some(content_type) = content_type {
            req.header
                .insert("Content-Type".to_string(), content_type.to_string());
        }
        req.post_body = body.to_string();
        req
    }
    pub async fn exec_by_method(&mut self,method:&str,content_type: Option<&str>, body: &str)->Result<Response, HttpError>{
        self.client_method = method.to_string();
        if let Some(content_type) = content_type {
            self.header
                .insert("Content-Type".to_string(), content_type.to_string());
        }
        self.post_body = body.to_string();

        self.exec().await
    }
    pub fn url_parse(&mut self, url: &str) {
        match Url::parse(url) {
            Ok(u) => {
                self.path = u.path().to_string();
                if u.has_host() {
                    self.host = u.host_str().unwrap().to_string();
                }
                self.request_host = match u.port() {
                    Some(p) => format!("{}:{}", self.host, p),
                    None => self.host.clone(),
                };
                match u.port_or_known_default() {
                    Some(port) => self.port = port,
                    None => self.err = "port 不存在".to_string(),
                }
                if u.scheme()=="https"{
                    self.is_https=true
                }
            }
            Err(e) => {
                self.err = e.to_string();
            }
        };
    }
    fn path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }
    pub async fn do_http1raw_request<'a>(
        &'a mut self,
        rawdata: &[u8],
        mut stream: Box<HttpTcpStream>,
    ) -> Result<Response, HttpError> {
        stream.as_mut().write_all(rawdata).await?;
        Request::parsereq_from_stream(self, stream.as_mut()).await?;
        Response::from_request(self.to_owned(),stream)
    }
    //读一个消息，可以是client，可以是server
    async fn parsereq_from_stream(
        req: &mut Request,
        stream: &mut HttpTcpStream ,
    ) -> Result<(), HttpError> {
        loop {
            req.next_from_stream(stream).await?;
            match req.code {
                301 | 302 | 307 => {
                    req.read_finish(stream).await?;
                    match req.max_redirects {
                        0 => return Ok(()),
                        _ => {
                            if req.redirects >= req.max_redirects {
                                return Err(HttpError::TooManyRedirects);
                            }
                        }
                    }

                    match req.header.get("Location") {
                        Some(a) => {
                            req.path = a.to_string();
                            let rawdata = req.format_message();
                            stream.write_all(rawdata.as_bytes()).await?;
                        }
                        None => {
                            return Err(HttpError::Custom("Location url not found".to_string()));
                        }
                    }
                }
                _ => {
                    stream.shift(req.body_start).await?;
                    return Ok(())
                },
            }
        }
    }

    async fn next_from_stream(&mut self, stream: &mut HttpTcpStream ) -> Result<(), HttpError> {
        self.content_length = ContentType::None;
        self.secondline = 0;
        self.header.clear();
        let mut first = true;
        while self.content_length == ContentType::None {
            if !first {
                stream.read_msg().await?;
            }
            first = false;
            let l = stream.buffer.len();
            unsafe {
                let sdata = String::from_utf8_unchecked(stream.buffer.clone());

                if self.secondline == 0 {
                    let method = match &sdata.find(' ') {
                        Some(i) => &sdata[..i.to_owned()],
                        None => continue,
                    };

                    let mut path = "";
                    let mut query = "";
                    let mut uri = "";
                    let mut proto = None;
                    let mut s = 0;
                    {
                        let mut q: i32 = -1;

                        let sdata = &sdata[method.len() + 1..];
                        for (i, char) in sdata.bytes().enumerate() {
                            if char == 63 && q == -1 {
                                q = i as i32;
                            } else if char == 32 {
                                if q != -1 {
                                    path = &sdata[s..(q as usize)];
                                    query = &sdata[(q as usize) + 1..i];
                                } else {
                                    path = &sdata[s..i];
                                }
                                uri = &sdata[s..i];

                                if let Some(f) = sdata.find(13 as char) {
                                    s = f;
                                    proto = Some(&sdata[i..f])
                                }

                                //判断http返回
                                if method == "HTTP/1.1" || method == "HTTP/1.0" {
                                    match path.parse::<u16>() {
                                        Ok(code) => self.code = code,
                                        Err(_) => continue,
                                    }
                                    /*code, err := strconv.Atoi(req.path)
                                    if err == nil {
                                        //req.Code = code
                                        //req.CodeMsg = req.Proto
                                    }*/

                                    proto = Some(method);
                                    path = "";
                                }
                                break;
                            }
                        }
                    }
                    (self.proto,self.keep_alive) = match proto {
                        Some("HTTP/1.0") => ("HTTP/1.0".to_string(),false),
                        Some("HTTP/1.1") => ("HTTP/1.0".to_string(),true),
                        None => {
                            if sdata.len() > self.max_package_size {
                                return Err(HttpError::LargePackage);
                            };
                            ("".to_string(),self.keep_alive)
                        }
                        _ => continue,
                    };

                    self.uri = uri.to_string();
                    self.query = query.to_string();
                    self.path = path.to_string();
                    self.secondline = s + method.len() + 1;
                }
                let mut s = self.secondline;
                let mut i = 0;
                while s < l {
                    s += i + 2;
                    match sdata[s..].find(13 as char) {
                        Some(f) => {
                            i = f;
                            if i > 0 {
                                let line = &sdata[s..s + i];
                                match line.find(58 as char) {
                                    Some(f) => {
                                        self.header.insert(
                                            line[..f].to_string(),
                                            line[f + 2..].to_string(),
                                        );
                                        self.header.insert(
                                            line[..f].to_lowercase(),
                                            line[f + 2..].to_string(),
                                        );
                                    }
                                    None => {
                                        self.content_length = ContentType::None;
                                        break;
                                    }
                                }
                            } else {
                                if let Some(keep_alive) = self.header.get("connection") {
                                    match keep_alive.as_str() {
                                        "close" => self.keep_alive = false,
                                        "keep-alive" => self.keep_alive = true,
                                        _ => (),
                                    }
                                }
                                if let Some(len) = self.header.get("content-length") {
                                    self.content_length =
                                        ContentType::Length(len.parse::<usize>()?);
                                }
                                if let Some(h) = self.header.get("transfer-encoding") {
                                    if h == "chunked" {
                                        self.content_length = ContentType::Chunked;
                                    }
                                }
                                self.body_start = s + 2;

                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        Ok(())
    }
    async fn read_finish(&mut self, stream: &mut HttpTcpStream ) -> Result<(), HttpError> {
        match self.content_length {
            ContentType::Length(size) => stream.shift(self.body_start + size).await?,
            ContentType::Chunked => {
                loop {
                    stream.shift(self.body_start).await?;
                    let buf = stream.readline().await?;
                    let len = u32::from_str_radix(String::from_utf8(buf)?.as_str(), 16)?;
                    if len == 0 {
                        stream.readline().await?;
                        //stream.buffer=Vec::new();
                        break;
                    } else {
                        stream.shift((len+2) as usize).await?
                    }
                }
            }
            _ => {
                return Err(HttpError::Custom("err content_length".to_string()));
            }
        }
        Ok(())
    }
}

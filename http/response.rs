use crate::http::error::HttpError;
use crate::http::request::Request;
use crate::http::request::ContentType;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use crate::http::stream::HttpTcpStream;
use flate2::read::GzDecoder;


#[derive(Debug)]
pub struct Response {
    pub code: u16,
    pub herder: HashMap<String, String>,
    stream : Box<HttpTcpStream>,
    body_length:ContentType,
}
impl Response {
    pub fn from_request(req: Request,stream :Box<HttpTcpStream>) -> Result<Response, HttpError> {

        let  resp = Response {
            code: req.code,
            herder: req.header,
            body_length:req.content_length,
            stream
        };
        Ok(resp)
    }
    pub async fn read_by_maxsize(&mut self,maxsize:usize)->Result<Vec<u8>,HttpError>{
        let res = match self.body_length {
            ContentType::Length(size) if size<=maxsize => {
                self.stream.next(size).await?
            },
            ContentType::Chunked => {
                let mut res=Vec::new();
                loop {
                    let buf =  self.stream.readline().await?;
                    let len = u32::from_str_radix(String::from_utf8(buf.clone())?.as_str(), 16)?;
                    if len == 0 {
                        self.stream.readline().await?;
                        break
                    } else {
                        res.append(&mut self.stream.next(len as usize).await?);
                        self.stream.shift(2).await?;
                    }
                }
                res
            }
            _ => {
                return Err(HttpError::Custom("err content_length".to_string()));
            }
        };

        match self.herder.get("content-encoding") {
            Some(code)=>{
                match code.as_str() {
                    "gzip"=>{
                        let d = GzDecoder::new(&res[..]);
                        let mut r = io::BufReader::new(d);
                        let mut buffer = Vec::new();

                        // read the whole file
                        r.read_to_end(&mut buffer)?;
                        return Ok(buffer)
                    },
                    s=>{
                        return Err(HttpError::Custom(format!("未支持的 content-encoding {}",s)))
                    }
                }
            },
            None=> return Ok(res)
        }
    }
}

//包裝一個含讀取buff的結構體

use std::fmt::{Debug, Formatter, Result as FormatResult};
use crate::http::error::HttpError;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::time::timeout;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_native_tls::{TlsStream};
use native_tls::TlsConnector;

pub struct HttpTcpStream {
    stream: Option<TcpStream>,
    tls_stream:Option<TlsStream<TcpStream>>,
    pub buffer: Vec<u8>,
    timeout: Duration,
    bufferstr: String,
    is_tls:bool,
}
impl Debug for HttpTcpStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> FormatResult {

            f.write_fmt(format_args!(
                "Context {{ f1: {:?}}}",
                self.stream
            ))

    }
}
impl HttpTcpStream {
    pub async fn new_with_addr(addr: SocketAddr,is_tls:bool,host:&str) -> Result<HttpTcpStream, HttpError> {

        let stream = TcpStream::connect(addr).await?;
        if is_tls{
            let socket = TcpStream::connect(&addr).await?;
            let cx = TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
            let cx = tokio_native_tls::TlsConnector::from(cx);

            let  socket = cx.connect(host, socket).await?;
            Ok(HttpTcpStream {
                tls_stream:Some(socket),
                stream: None,
                buffer: Vec::with_capacity(20),
                timeout: Duration::from_secs(10),
                bufferstr: "".to_string(),
                is_tls
            })
        }else{
            Ok(HttpTcpStream {
                tls_stream:None,
                stream: Some(stream),
                buffer: Vec::with_capacity(20),
                timeout: Duration::from_secs(10),
                bufferstr: "".to_string(),
                is_tls
            })
        }



    }
    pub async fn write_all(&mut self, rawdata: &[u8]) -> Result<(), HttpError> {
        if self.is_tls{
            self.tls_stream.as_mut().unwrap().write_all(rawdata).await?;
        }else{
            self.stream.as_mut().unwrap().write_all(rawdata).await?;
        }
        Ok(())
    }
    pub async fn read_msg(&mut self) -> Result<(), HttpError> {
        let oldlen = self.buffer.len();
        self.buffer.reserve(65535);
        unsafe {
            self.buffer.set_len(oldlen + 65535);
        }
        if self.is_tls{
            match timeout(self.timeout, self.tls_stream.as_mut().unwrap().read(&mut self.buffer[oldlen..])).await {
                Ok(len) => {
                    //再 match len 取读到数据的长度
                    match len {
                        Ok(n) => {
                            if n == 0 {
                                return Err(HttpError::TcpDisconnected);
                            }
                            unsafe {
                                self.buffer.set_len(oldlen + n);
                            }
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                }
                Err(_) => Err(HttpError::TcpReadTimeout),
            }
        }else{
            match timeout(self.timeout, self.stream.as_mut().unwrap().read(&mut self.buffer[oldlen..])).await {
                Ok(len) => {
                    //再 match len 取读到数据的长度
                    match len {
                        Ok(n) => {
                            if n == 0 {
                                return Err(HttpError::TcpDisconnected);
                            }
                            unsafe {
                                self.buffer.set_len(oldlen + n);
                            }
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                }
                Err(_) => Err(HttpError::TcpReadTimeout),
            }
        }

    }
    pub async fn next(&mut self, n: usize)->Result<Vec<u8>,HttpError>{
        while self.buffer.len() < n {
            self.read_msg().await?;
        }
        let res= self.buffer[..n].to_vec();
        self.buffer.copy_within(n.., 0);
        unsafe {
            self.buffer.set_len(self.buffer.len() - n);
        }
        Ok(res)
    }
    pub async fn shift(&mut self, n: usize) -> Result<(), HttpError> {
        while self.buffer.len() < n {
            self.read_msg().await?;
        }
        self.buffer.copy_within(n.., 0);
        unsafe {
            self.buffer.set_len(self.buffer.len() - n);
        }
        Ok(())
    }
    pub async fn peek_n(&mut self, n: usize) -> Result<&[u8], HttpError> {
        while self.buffer.len() < n {
            self.read_msg().await?;
        }
        Ok(&self.buffer[..n])
    }

    //针对http1读取到\r\n
    pub async fn readline(&mut self) -> Result<Vec<u8>, HttpError> {
        unsafe {
            loop {
                for (k, v) in self.buffer.iter().enumerate() {
                    if v == &10 && (k > 0 && self.buffer[k - 1] == 13) {
                        let res = self.buffer[..k - 1].to_vec();
                        self.buffer.copy_within(k + 1.., 0);
                        self.buffer.set_len(self.buffer.len() - k - 1);
                        return Ok(res);
                    }
                }

                self.read_msg().await?
            }
        }
    }
}

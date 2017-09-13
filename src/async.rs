// Copyright © 2017 by Michael Dilger (of New Zealand) and other buf-read-ext Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::mem;
use std::io::{BufRead, Error, ErrorKind};
use tokio_io::{AsyncRead, AsyncWrite};
use futures::{Poll, Future, Async};

pub fn async_stream_until_token<R, W>(stream: R, token: &[u8], out: W)
                                      -> AsyncStreamUntilToken<R, W>
    where R: AsyncRead + BufRead, W: AsyncWrite
{
    AsyncStreamUntilToken {
        stream: Some(stream),
        token: token.to_vec(),
        out: Some(out),
    }
}

pub struct AsyncStreamUntilToken<R, W> {
    stream: Option<R>,
    token: Vec<u8>,
    out: Option<W>,
}

pub struct AsyncStreamUntilTokenOutput<R, W> {
    pub stream: R,
    pub out: W,
    pub bytes_streamed: usize,
    pub token_found: bool
}

impl<R, W> Future for AsyncStreamUntilToken<R, W>
    where R: AsyncRead + BufRead, W: AsyncWrite
{
    type Item = AsyncStreamUntilTokenOutput<R, W>;
    type Error = Error;

    fn poll(&mut self) -> Poll<AsyncStreamUntilTokenOutput<R, W>, Error> {
        if self.stream.is_none() || self.out.is_none() {
            panic!("Polled AsyncStreamUntilToken after it was done.");
        }

        match ::stream_until_token(self.stream.as_mut().unwrap(),
                                   &*self.token,
                                   self.out.as_mut().unwrap())
        {
            Ok((size,found)) => Ok(
                Async::Ready(AsyncStreamUntilTokenOutput {
                    stream: mem::replace(&mut self.stream, None).unwrap(),
                    out: mem::replace(&mut self.out, None).unwrap(),
                    bytes_streamed: size,
                    token_found: found
                })),
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => Ok(Async::NotReady),
                ErrorKind::Interrupted => Ok(Async::NotReady),
                _ => Err(e)
            },
        }
    }
}

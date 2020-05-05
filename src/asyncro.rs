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

/// Returns a future that streams all bytes to `out` until the `token` delimiter or EOF is
/// reached.
///
/// This future will continue to read (and stream) bytes from the underlying stream until the
/// token or end-of-file is found. Once found, all bytes up to (but not including) the
/// token (if found) will have been streamed to `out` and the input stream will advance past
/// the token.
///
/// This function will return an `AsyncStreamUntilTokenOutput` which includes the number of
/// bytes that were streamed to `out` (this will exclude the count of token bytes, if the token
/// was found), and whether or not the token was found.
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

/// Future associated with async_stream_until_token().  Refer to that functions
/// documentation
pub struct AsyncStreamUntilToken<R, W> {
    stream: Option<R>,
    token: Vec<u8>,
    out: Option<W>,
}

/// A completed AsyncStreamUntilToken future yields this value, which returns the streams,
/// as well as the number of bytes streamed and whether or not the token was found.
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

        match crate::stream_until_token(self.stream.as_mut().unwrap(),
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

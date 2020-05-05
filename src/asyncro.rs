// Copyright © 2017 by Michael Dilger (of New Zealand) and other buf-read-ext Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use tokio::io::{AsyncRead, AsyncBufRead, AsyncWrite, Result};

/// Streams all bytes to `out` until the `token` delimiter or EOF is reached.
///
/// This function will continue to read (and stream) bytes from the underlying stream until the
/// token or end-of-file is found. Once found, all bytes up to (but not including) the
/// token (if found) will have been streamed to `out` and the input stream will advance past
/// the token.
///
/// This function will return the number of bytes that were streamed to `out` (this will
/// exclude the count of token bytes, if the token was found), and whether or not the token
/// was found.
pub async fn stream_until_token<R, W>(stream: &mut R, token: &[u8], out: &mut W)
                                      -> Result<(usize, bool)>
where R: AsyncRead + AsyncBufRead + ?Sized, W: AsyncWrite
{
    unimplemented!()
}


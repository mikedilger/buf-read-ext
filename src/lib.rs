// Copyright © 2016 by Michael Dilger (of New Zealand) and other buf-read-ext Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

extern crate memchr;

use std::io::{BufRead, ErrorKind, Result, Write};

/// Type of end-of-line sequence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    CrLf
}

/// Extends any type that implements BufRead with a stream_until_token() function.
pub trait BufReadExt: BufRead {
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
    ///
    /// # Errors
    ///
    /// This function will ignore all instances of `ErrorKind::Interrupted` and will otherwise
    /// return any errors returned by `fill_buf`.
    fn stream_until_token<W: Write>(&mut self, token: &[u8], out: &mut W) -> Result<(usize, bool)>
    {
        stream_until_token(self, token, out)
    }

    /// Streams all bytes to `out` until an LF or CRLF line terminating sequence is found.
    ///
    /// This function will continue to read (and stream) bytes from the underlying stream until
    /// the end-of-line sequences are found (either LF or CRLF).  Once found, all bytes up to
    /// (but not including) the end of line sequence will have been streamed to `out` and the
    /// input stream will advance past the end of line sequence.
    ///
    /// This function returns the nubmer of bytes that were streamed to `out` (this will
    /// exclude the end of line sequence characters themselves), and the type of end of line
    /// that was found, or None if the end-of-file was reached first.
    fn stream_line<W: Write>(&mut self, out: &mut W) -> Result<(usize, Option<Eol>)>
    {
        let mut read = 0;
        let mut heldback_cr = false;
        loop {

            let (done, used, streamed, eol) = {
                let available = match self.fill_buf() {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e)
                };

                if heldback_cr && available.len()>0 && available[0]==b'\n' {
                    (true, 1, 0, Some(Eol::CrLf))
                }
                else {
                    let mut streamed: usize = 0;

                    if heldback_cr {
                        try!(out.write_all(b"\r"));
                        streamed += 1;
                        heldback_cr = false;
                    }

                    match memchr::memchr(b'\n', available) {
                        Some(i) => {
                            if i>0 && available[i-1] == b'\r' {
                                if i>1 { try!(out.write_all(&available[..i-1])); }
                                streamed += i-1;
                                (true, i+1, streamed, Some(Eol::CrLf))
                            } else {
                                try!(out.write_all(&available[..i]));
                                streamed += i;
                                (true, i+1, streamed, Some(Eol::Lf))
                            }
                        },
                        None => {
                            if available.len() < 1 {
                                (true, available.len(), 0, None)
                            }
                            else if available[available.len()-1]==b'\r' {
                                try!(out.write_all(&available[..available.len()-1]));
                                heldback_cr = true;
                                streamed += available.len()-1;
                                (false, available.len(), streamed, None)
                            } else {
                                try!(out.write_all(&available[..]));
                                streamed += available.len();
                                (false, available.len(), streamed, None)
                            }
                        }
                    }
                }
            };

            self.consume(used);
            read += streamed;
            if done || used == 0 {
                return Ok((read, eol));
            }
        }
    }
}

// Implement BufReadExt for everything that implements BufRead.
impl<T: BufRead> BufReadExt for T { }

fn stream_until_token<R: BufRead + ?Sized, W: Write>(stream: &mut R, token: &[u8], mut out: &mut W)
                                                     -> Result<(usize, bool)>
{
    let mut read = 0;
    // Represents the sizes of possible token prefixes found at the end of the last buffer, usually
    // empty. If not empty, the beginning of this buffer is checked for the matching suffixes to
    // to find tokens that straddle two buffers. Entries should be in longest prefix to shortest
    // prefix order.
    let mut prefix_lengths: Vec<usize> = Vec::new();
    let mut found: bool;
    let mut used: usize;

    'stream:
    loop {
        found = false;
        used = 0;

        // This is not actually meant to repeat, we only need the break functionality of a loop.
        // The reader is encouraged to try their hand at coding this better, noting that buffer must
        // drop out of scope before stream can be used again.
        let mut do_once = true;
        'buffer:
        while do_once {
            do_once = false;

            // Fill the buffer (without consuming)
            let buffer = match stream.fill_buf() {
                Ok(n) => n,
                Err(ref err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => return Err(err)
            };
            if buffer.len() == 0 {
                break 'stream;
            }

            // If the buffer starts with a token suffix matching a token prefix from the end of the
            // previous buffer, then we have found a token.
            if !prefix_lengths.is_empty() {
                let largest_prefix_len = prefix_lengths[0];

                let drain: Vec<usize> = prefix_lengths.drain(..).collect();

                let mut partmatch = false;
                for &prefix_len in &drain {
                    // If the buffer is too small to fit an entire suffix
                    if buffer.len() < token.len() - prefix_len {
                        if buffer[..] == token[prefix_len..prefix_len + buffer.len()] {
                            // that prefix just got bigger and needs to be preserved
                            prefix_lengths.push(prefix_len + buffer.len());
                            partmatch = true;
                        }
                    } else {
                        if buffer[..token.len() - prefix_len] == token[prefix_len..] {
                            try!(out.write_all(&token[..largest_prefix_len - prefix_len]));
                            found = true;
                            used = token.len() - prefix_len;
                            break 'buffer;
                        }
                    }
                }

                if ! partmatch {
                    // No prefix matched, so we should write the largest prefix length, since we
                    // didn't write it when we first saw it
                    try!(out.write_all(&token[..largest_prefix_len]));
                }
            }

            // Get the index index of the first token in the middle of the buffer, if any
            let index = buffer
                .windows(token.len())
                .enumerate()
                .filter(|&(_, t)| t == token)
                .map(|(i, _)| i)
                .next();

            if let Some(index) = index {
                try!(out.write_all(&buffer[..index]));
                found = true;
                used = index + token.len();
                break 'buffer;
            }

            // Check for token prefixes at the end of the buffer.
            let mut window = token.len() - 1;
            if buffer.len() < window {
                window = buffer.len();
            }
            // Remember the largest prefix for writing later if it didn't match
            // (we don't write it now just in case it turns out to be the token)
            let mut reserve = if !prefix_lengths.is_empty() {
                buffer.len()
            } else {
                0
            };
            for prefix in (1..window+1).rev()
                .filter(|&w| token[..w] == buffer[buffer.len() - w..])
            {
                if reserve == 0 {
                    reserve = prefix;
                }
                prefix_lengths.push(prefix)
            }

            try!(out.write_all(&buffer[..buffer.len()-reserve]));
            used = buffer.len();
        }

        stream.consume(used);
        read += used;

        if found || used == 0 {
            break;
        }
    }

    return Ok((if found { read - token.len() } else { read }, found));
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{BufReader, Cursor};

    #[test]
    fn stream_until_token() {
        let mut buf = Cursor::new(&b"123456"[..]);
        let mut result: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"78", &mut result).unwrap(), (6, false));
        assert_eq!(result, b"123456");

        let mut buf = Cursor::new(&b"12345678"[..]);
        let mut result: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"34", &mut result).unwrap(), (2, true));
        assert_eq!(result, b"12");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"78", &mut result).unwrap(), (2, true));
        assert_eq!(result, b"56");

        let mut buf = Cursor::new(&b"bananas for nana"[..]);
        let mut result: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"nan", &mut result).unwrap(), (2, true));
        assert_eq!(result, b"ba");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"nan", &mut result).unwrap(), (7, true));
        assert_eq!(result, b"as for ");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"nan", &mut result).unwrap(), (1, false));
        assert_eq!(result, b"a");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"nan", &mut result).unwrap(), (0, false));
        assert_eq!(result, b"");
    }

    #[test]
    fn stream_until_token_straddle_test() {
        let cursor = Cursor::new(&b"12345TOKEN345678"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut result: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"TOKEN", &mut result).unwrap(), (5, true));
        assert_eq!(result, b"12345");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"TOKEN", &mut result).unwrap(), (6, false));
        assert_eq!(result, b"345678");

        result.truncate(0);
        assert_eq!(buf.stream_until_token(b"TOKEN", &mut result).unwrap(), (0, false));
        assert_eq!(result, b"");

        let cursor = Cursor::new(&b"12345TOKE23456781TOKEN78"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut result: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"TOKEN", &mut result).unwrap(), (17, true));
        assert_eq!(result, b"12345TOKE23456781");
    }

    // This tests against mikedilger/formdata github issue #1
    #[test]
    fn stream_until_token_large_token_test() {
        let cursor = Cursor::new(&b"IAMALARGETOKEN7812345678"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut v: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"IAMALARGETOKEN", &mut v).unwrap(), (0, true));
        assert_eq!(v, b"");
        assert_eq!(buf.stream_until_token(b"IAMALARGETOKEN", &mut v).unwrap(), (10, false));
        assert_eq!(v, b"7812345678");

        let cursor = Cursor::new(&b"0IAMALARGERTOKEN12345678"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut v: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"IAMALARGERTOKEN", &mut v).unwrap(), (1, true));
        assert_eq!(v, b"0");
        v.truncate(0);
        assert_eq!(buf.stream_until_token(b"IAMALARGERTOKEN", &mut v).unwrap(), (8, false));
        assert_eq!(v, b"12345678");
    }

    // This tests against mikedilger/formdata github issue #11
    #[test]
    fn stream_until_token_double_straddle_test() {
        let cursor = Cursor::new(&b"12345IAMALARGETOKEN4567"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut v: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"IAMALARGETOKEN", &mut v).unwrap(), (5, true));
        assert_eq!(v, b"12345");
        v.truncate(0);
        assert_eq!(buf.stream_until_token(b"IAMALARGETOKEN", &mut v).unwrap(), (4, false));
        assert_eq!(v, b"4567");
    }

    // This tests against mikedilger/formdata github issue #12
    #[test]
    fn stream_until_token_multiple_prefix_text() {
        let cursor = Cursor::new(&b"12barbarian4567"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut v: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"barbarian", &mut v).unwrap(), (2, true));
        assert_eq!(v, b"12");

        let cursor = Cursor::new(&b"12barbarbarian7812"[..]);
        let mut buf = BufReader::with_capacity(8, cursor);
        let mut v: Vec<u8> = Vec::new();
        assert_eq!(buf.stream_until_token(b"barbarian", &mut v).unwrap(), (5, true));
        assert_eq!(v, b"12bar");
    }

    #[test]
    fn stream_line_test() {
        for i in 1..20 {
            let cursor = Cursor::new(&b"line one\nline two\r\nline three\rstill\nline four"[..]);
            let mut buf = BufReader::with_capacity(i, cursor);
            let mut v: Vec<u8> = Vec::new();
            assert_eq!(buf.stream_line(&mut v).unwrap(), (8,Some(Eol::Lf)));
            assert_eq!(v, b"line one");
            v.truncate(0);
            assert_eq!(buf.stream_line(&mut v).unwrap(), (8,Some(Eol::CrLf)));
            assert_eq!(v, b"line two");
            v.truncate(0);
            assert_eq!(buf.stream_line(&mut v).unwrap(), (16,Some(Eol::Lf)));
            assert_eq!(v, b"line three\rstill");
            v.truncate(0);
            assert_eq!(buf.stream_line(&mut v).unwrap(), (9,None));
            assert_eq!(v, b"line four");
        }
    }

}

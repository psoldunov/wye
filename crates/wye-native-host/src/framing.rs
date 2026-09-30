//! Native-messaging framing: each message is a 32-bit length in native byte
//! order followed by that many bytes of UTF-8 JSON, on stdin (from the
//! browser) and stdout (to the browser).

use std::io::{self, Read, Write};

/// Largest message Wye accepts from the browser: a link and a few keys.
pub const MAX_INCOMING: usize = 1024 * 1024;

/// Largest message a browser accepts from a host (Chromium and Firefox).
pub const MAX_OUTGOING: usize = 1024 * 1024;

const LENGTH_BYTES: usize = 4;

/// Read one message, or `None` when the browser closed the stream between
/// messages.
///
/// # Errors
///
/// [`io::ErrorKind::InvalidData`] for a message over [`MAX_INCOMING`];
/// [`io::ErrorKind::UnexpectedEof`] when the stream ends inside a message.
pub fn read_message(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0_u8; LENGTH_BYTES];
    let first = read_some(reader, &mut length)?;
    if first == 0 {
        return Ok(None);
    }
    let rest = length
        .get_mut(first..)
        .ok_or_else(|| invalid("the reader reported more bytes than it was given"))?;
    reader.read_exact(rest)?;
    let length = usize::try_from(u32::from_ne_bytes(length))
        .map_err(|_| invalid("the message length does not fit in memory"))?;
    if length > MAX_INCOMING {
        return Err(invalid(&format!(
            "a message of {length} bytes is over the {MAX_INCOMING}-byte limit"
        )));
    }
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Write one message and flush it.
///
/// # Errors
///
/// [`io::ErrorKind::InvalidInput`] for a message over [`MAX_OUTGOING`]; the
/// writer's own errors.
pub fn write_message(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    if body.len() > MAX_OUTGOING {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the reply is over the browser's limit",
        ));
    }
    let length = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the reply is too long"))?;
    writer.write_all(&length.to_ne_bytes())?;
    writer.write_all(body)?;
    writer.flush()
}

/// Read into `buffer` until the first byte arrives: 0 at end of stream.
fn read_some(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::thread;

    use super::*;

    fn frame(body: &[u8]) -> Vec<u8> {
        let length = u32::try_from(body.len()).expect("small");
        length.to_ne_bytes().iter().chain(body).copied().collect()
    }

    #[test]
    fn messages_round_trip_over_a_pipe_bext_04() {
        let (mut reader, mut writer) = std::io::pipe().expect("pipe");
        let sender = thread::spawn(move || {
            write_message(&mut writer, br#"{"url":"https://a.example/"}"#).expect("first");
            write_message(&mut writer, b"{}").expect("second");
            // Dropping the writer closes the pipe.
        });
        let first = read_message(&mut reader).expect("read");
        let second = read_message(&mut reader).expect("read");
        sender.join().expect("sender");
        assert_eq!(
            first.as_deref(),
            Some(&br#"{"url":"https://a.example/"}"#[..])
        );
        assert_eq!(second.as_deref(), Some(&b"{}"[..]));
        assert_eq!(read_message(&mut reader).expect("end"), None);
    }

    #[test]
    fn the_length_is_native_endian() {
        let mut out = Vec::new();
        write_message(&mut out, b"abc").expect("written");
        assert_eq!(out, frame(b"abc"));
        assert_eq!(&out[..4], &3_u32.to_ne_bytes());
    }

    #[test]
    fn a_stream_cut_inside_a_message_is_an_error() {
        let mut short = Cursor::new(frame(b"hello")[..6].to_vec());
        let error = read_message(&mut short).expect_err("cut");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        let mut half_length = Cursor::new(vec![5_u8, 0]);
        assert!(read_message(&mut half_length).is_err());
    }

    #[test]
    fn an_oversized_message_is_refused_before_reading_it() {
        let length = u32::try_from(MAX_INCOMING + 1).expect("fits");
        let mut huge = Cursor::new(length.to_ne_bytes().to_vec());
        let error = read_message(&mut huge).expect_err("refused");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let too_long = vec![b' '; MAX_OUTGOING + 1];
        assert!(write_message(&mut Vec::new(), &too_long).is_err());
    }
}

use std::io::{self, BufRead, Write};

struct Message {}

impl Message {
    fn read(reader: &mut dyn BufRead) -> std::io::Result<Option<Message>> {
        let mut buf = String::with_capacity(8192);
        let mut size: Option<usize> = None;

        for _ in 0..100 {
            buf.clear();

            if reader.read_line(&mut buf)? == 0 {
                return Ok(None);
            }

            if !buf.ends_with("\r\n") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing CRLF after header",
                ));
            }
            let buf = &buf[..buf.len() - 2];

            if buf.is_empty() {
                // Start of real data.
                break;
            }

            let mut parts = buf.splitn(3, ": ");
            let header_name = parts.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed header",
            ))?;
            let header_value = parts.next().ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed header",
            ))?;
            if parts.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed header",
                ));
            }

            if header_name.eq_ignore_ascii_case("Content-Type") {
                size = header_value.parse().map_err(|err| {})?;
            }
        }
        todo!()
    }
}

pub fn run(reader: &mut dyn BufRead, writer: &mut impl Write) {
    loop {
        todo!()
    }
}

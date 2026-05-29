/// PeachDB command-line client.
///
/// Usage:
///   peachdb-cli [--addr <host:port>] <command> [args]
///
/// Commands:
///   ping
///   get  <key>
///   set  <key> <value>
///   del  <key>
///   keys
///
/// The default server address is 127.0.0.1:7878.
/// Values passed to `set` are auto-typed: integers, floats, booleans, or strings.
///
/// Examples:
///   cargo run --example peachdb-cli -- ping
///   cargo run --example peachdb-cli -- set name Alice
///   cargo run --example peachdb-cli -- get name
///   cargo run --example peachdb-cli -- keys
///   cargo run --example peachdb-cli -- del name
///   cargo run --example peachdb-cli -- --addr 10.0.0.1:7878 get name

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process;

use peachdb::dtypes::{Dtype, Field, Key, Primitive};
use peachdb::server::protocol::{ProtocolDecoder, ProtocolEncoder, Request, Response};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (addr, rest) = parse_addr(&args[1..]);

    if rest.is_empty() {
        eprintln!("{}", USAGE);
        process::exit(1);
    }

    let req = match build_request(rest) {
        Ok(r) => r,
        Err(msg) => {
            eprintln!("error: {}\n{}", msg, USAGE);
            process::exit(1);
        }
    };

    let mut stream = TcpStream::connect(&addr).unwrap_or_else(|e| {
        eprintln!("error: could not connect to {}: {}", addr, e);
        process::exit(1);
    });

    if let Err(e) = send_request(&mut stream, &req) {
        eprintln!("error: failed to send request: {}", e);
        process::exit(1);
    }

    let res = match read_response(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to read response: {}", e);
            process::exit(1);
        }
    };

    print_response(&res);
}

// ── argument parsing ──────────────────────────────────────────────────────────

const DEFAULT_ADDR: &str = "127.0.0.1:7878";

const USAGE: &str = "\
Usage: peachdb-cli [--addr <host:port>] <command> [args]

Commands:
  ping
  get  <key>
  set  <key> <value>
  del  <key>
  keys";

fn parse_addr<'a>(args: &'a [String]) -> (String, &'a [String]) {
    if args.len() >= 2 && args[0] == "--addr" {
        (args[1].clone(), &args[2..])
    } else {
        (DEFAULT_ADDR.to_string(), args)
    }
}

fn build_request(args: &[String]) -> Result<Request, String> {
    match args[0].as_str() {
        "ping" => Ok(Request::Ping),
        "keys" => Ok(Request::Keys),
        "get" => {
            let key = require_arg(args, 1, "get <key>")?;
            Ok(Request::Get { key: Key::from(key.as_str()) })
        }
        "del" => {
            let key = require_arg(args, 1, "del <key>")?;
            Ok(Request::Del { key: Key::from(key.as_str()) })
        }
        "set" => {
            let key = require_arg(args, 1, "set <key> <value>")?;
            let value = require_arg(args, 2, "set <key> <value>")?;
            Ok(Request::Set {
                key: Key::from(key.as_str()),
                field: parse_value(value),
            })
        }
        cmd => Err(format!("unknown command: '{}'", cmd)),
    }
}

fn require_arg<'a>(args: &'a [String], idx: usize, usage: &str) -> Result<&'a String, String> {
    args.get(idx).ok_or_else(|| format!("usage: {}", usage))
}

/// Auto-detects value type: bool → Boolean, integer → Integer, float → Float, else String.
fn parse_value(s: &str) -> Field {
    if s == "true" {
        return Field::Primitive(Primitive::Boolean(true));
    }
    if s == "false" {
        return Field::Primitive(Primitive::Boolean(false));
    }
    if let Ok(n) = s.parse::<i64>() {
        return Field::Primitive(Primitive::Integer(n));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Field::Primitive(Primitive::Float(f));
    }
    Field::Primitive(Primitive::String(s.to_string()))
}

// ── TCP framing ───────────────────────────────────────────────────────────────

fn send_request(stream: &mut TcpStream, req: &Request) -> std::io::Result<()> {
    let mut enc = ProtocolEncoder::new();
    enc.encode_request(req);
    let framed = enc.frame();
    stream.write_all(&framed)
}

fn read_response(stream: &mut TcpStream) -> std::io::Result<Response> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    ProtocolDecoder::new(&buf)
        .decode_response()
        .map_err(|e| std::io::Error::other(e.to_string()))
}

// ── output formatting ─────────────────────────────────────────────────────────

fn print_response(res: &Response) {
    match res {
        Response::Ok(None) => println!("OK"),
        Response::Ok(Some(field)) => println!("{}", display_field(field)),
        Response::NotFound => println!("NOT FOUND"),
        Response::TypeError => println!("TYPE ERROR"),
        Response::InvalidRequest => {
            eprintln!("error: invalid request");
            process::exit(1);
        }
        Response::ServerError(msg) => {
            eprintln!("server error: {}", msg);
            process::exit(1);
        }
        Response::Keys(keys) => {
            if keys.is_empty() {
                println!("(empty)");
            } else {
                for key in keys {
                    println!("{}", String::from_utf8_lossy(key.as_bytes()));
                }
            }
        }
    }
}

fn display_field(field: &Field) -> String {
    match field {
        Field::Primitive(p) => match p {
            Primitive::Integer(n) => format!("Integer({})", n),
            Primitive::Float(f) => format!("Float({})", f),
            Primitive::String(s) => format!("String(\"{}\")", s),
            Primitive::Boolean(b) => format!("Boolean({})", b),
        },
        Field::Raw(b) => format!("Raw(<{} bytes>)", b.len()),
        Field::Array(dtype, arr) => format!("Array({:?}, {} elements)", dtype, arr.len()),
    }
}

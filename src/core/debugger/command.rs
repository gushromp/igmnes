use nom::{IResult, space, le_i32, le_u16, eol};
use nom::IResult::*;

#[derive(Debug)]
pub enum Command {
    PrintState,
    PrintMemory,
    BreakpointSet(u16),
    BreakpointRemove(u16),
    Disassemble(i32),
    Goto(u16),
}

impl Command {
    pub fn parse(input: &str) -> Result<Command, String> {
        match parse_command(input.as_bytes()) {
            IResult::Done(_i, command) => Ok(command),
            IResult::Error(err) => Err(format!("Error while parsing command: {:?}", err)),
            _ => Err(format!("Incomplete parsing error"))
        }
    }
}

named!(
    parse_command<Command>,
    complete!(
    terminated!(
        alt_complete! (
            parse_print_state       |
            parse_print_memory      |
            parse_breakpoint_set    |
            parse_breakpoint_remove
        ),
        eol
    ))
);

named!(
    parse_print_state<Command>,
    map!(
        alt_complete! (
            tag_no_case!("printstate") |
            tag_no_case!("ps")
        )
        , |_| Command::PrintState
    )
);

named!(
    parse_print_memory<Command>,
    map!(
        alt_complete! (
            tag_no_case!("printmemory") |
            tag_no_case!("pm")
        )
        , |_| Command::PrintMemory
    )
);


named!(
    parse_breakpoint_set<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("breakpointset") |
            tag_no_case!("bs"))                 >>
        addr: preceded!(space, parse_literal)   >>
        ( Command::BreakpointSet(addr) )
    )
);

named!(
    parse_breakpoint_remove<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("breakpointremove") |
            tag_no_case!("br"))                 >>
        addr: preceded!(space, parse_literal)   >>
        ( Command::BreakpointRemove(addr) )
    )
);

named!(
    parse_disassemble<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("disassemble") |
            tag_no_case!("d"))              >>
        count: preceded!(space, le_i32)     >>
        ( Command::Disassemble(count) )
    )
);

//
// helpers
//

named!(
    parse_literal<u16>,
    alt_complete!(
            parse_hex_literal |
            le_u16
    )
);

named!(
    parse_hex_literal<u16>,
    preceded!(alt_complete!(tag!("0x") | tag!("$")), hex_u16)
);


// Modified version of nom's built-in hex_u32 parser
#[inline]
pub fn hex_u16(input: &[u8]) -> IResult<&[u8], u16> {
    match is_a!(input, &b"0123456789abcdef"[..]) {
        Error(e)    => Error(e),
        Incomplete(e) => Incomplete(e),
        Done(i,o) => {
            let mut res = 0u16;

            // Do not parse more than 4 characters for a u16
            let mut remaining = i;
            let mut parsed    = o;
            if o.len() > 4 {
                remaining = &input[4..];
                parsed    = &input[..4];
            }

            for &e in parsed {
                let digit = e as char;
                let value: u16 = digit.to_digit(16).unwrap_or(0) as u16;
                res = value + (res << 4);
            }
            Done(remaining, res)
        }
    }
}
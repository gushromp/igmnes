use std::str::{self, FromStr};
use std::ops::Range;
use nom::{IResult, line_ending, space, digit, alphanumeric, eol, ErrorKind};
use nom::IResult::*;

#[derive(Debug)]
pub enum Command {
    ShowUsage,
    PrintState,
    PrintMemory,
    PrintBreakpoints,
    PrintWatchpoints,
    PrintLabels,
    BreakpointSet(u16),
    BreakpointRemove(u16),
    WatchpointSet(u16),
    WatchpointRemove(u16),
    LabelSet(String, u16),
    LabelRemove(u16),
    Disassemble(Range<i16>),
    Goto(u16),
    Step,
    RepeatCommand(Box<Command>, u16),
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

macro_rules! opt_default (
  ($i:expr, $submac:ident!( $($args:tt)* ), $val:expr) => (
    {
      match $submac!($i, $($args)*) {
        IResult::Done(i,o)     => IResult::Done(i, o),
        IResult::Error(_)      => IResult::Done($i, $val),
        IResult::Incomplete(i) => IResult::Incomplete(i)
      }
    }
  );
  ($i:expr, $f:expr, $val:expr) => (
    opt_default!($i, call!($f), $val);
  );
);

named!(
    parse_command<Command>,
    complete!(
        alt!(
            parse_show_usage |
            alt_complete! (
                parse_print_state       |
                parse_print_memory      |
                parse_print_breakpoints |
                parse_print_watchpoints |
                parse_print_labels      |
                parse_breakpoint_set    |
                parse_breakpoint_remove |
                parse_watchpoint_set    |
                parse_watchpoint_remove |
                parse_label_set         |
                parse_label_remove      |
                parse_disassemble       |
                parse_goto              |
                parse_step              |
                parse_repeat_command
            )
        )
    )
);

named!(
    parse_show_usage<Command>,
    map!(
        line_ending
        , |_| Command::ShowUsage
    )
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
    parse_print_breakpoints<Command>,
    map!(
        alt_complete! (
            tag_no_case!("printbreakpoints") |
            tag_no_case!("pb")
        )
        , |_| Command::PrintBreakpoints
    )
);

named!(
    parse_print_watchpoints<Command>,
    map!(
        alt_complete! (
            tag_no_case!("printwatchpoints") |
            tag_no_case!("pw")
        )
        , |_| Command::PrintWatchpoints
    )
);

named!(
    parse_print_labels<Command>,
    map!(
        alt_complete! (
            tag_no_case!("printlabels") |
            tag_no_case!("pl")
        )
        , |_| Command::PrintLabels
    )
);

named!(
    parse_breakpoint_set<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("breakpointset") |
            tag_no_case!("bs"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::BreakpointSet(addr) )
    )
);

named!(
    parse_breakpoint_remove<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("breakpointremove") |
            tag_no_case!("br"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::BreakpointRemove(addr) )
    )
);

named!(
    parse_watchpoint_set<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("watchpointset") |
            tag_no_case!("ws"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::WatchpointSet(addr) )
    )
);

named!(
    parse_watchpoint_remove<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("watchpointremove") |
            tag_no_case!("wr"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::WatchpointRemove(addr) )
    )
);

named!(
    parse_label_set<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("labelset") |
            tag_no_case!("ls"))                         >>
        label: preceded!(opt!(tag!(".")), parse_string) >>
        addr: preceded!(space, parse_literal_u16)       >>
        ( Command::LabelSet(label, addr) )
    )
);

named!(
    parse_label_remove<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("labelremove") |
            tag_no_case!("lr"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::LabelRemove(addr) )
    )
);

named!(
    parse_disassemble<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("disassemble") |
            tag_no_case!("d"))                  >>
        count: opt_default!(
            preceded!(space,
                alt_complete!(
                    parse_range_i16 |
                    do_parse!(
                        end: parse_literal_i16 >>
                        ( 0..end )
                    )
                )
            )
            , 0..5)                             >>
        ( Command::Disassemble(count) )
    )
);

named!(
    parse_goto<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("goto") |
            tag_no_case!("g"))                     >>
        addr: preceded!(space, parse_literal_u16)  >>
        ( Command::Goto(addr) )
    )
);

named!(
    parse_step<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("step") |
            tag_no_case!("s"))      >>
        ( Command::Step )
    )
);

named!(
    parse_repeat_command<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("repeatcommand") |
            tag_no_case!("r"))                                      >>
        command: preceded!(space
            , delimited!(char!('('), parse_command, char!(')')))    >>
        count: preceded!(space, parse_literal_u16)                  >>
        ( Command::RepeatCommand(Box::new(command), count) )
    )
);

//
// helpers
//

named!(
    parse_range_i16<Range<i16>>,
    do_parse!(
        start: parse_literal_i16    >>
        tag!("..")                  >>
        end: parse_literal_i16      >>
        ( start..end )
    )
);

named!(
    parse_string<String>,
    map_res!(
        map_res!(
            preceded!(space, alphanumeric)
            , str::from_utf8
        )
        , FromStr::from_str
    )
);

named!(
    parse_literal_u16<u16>,
    alt_complete!(
            parse_hex_literal_u16 |
            parse_decimal_literal_u16
    )
);

named!(
    parse_hex_literal_u16<u16>,
    preceded!(alt_complete!(tag!("0x") | tag!("$")), hex_u16)
);

named!(
    parse_decimal_literal_u16<u16>,
    map_res!(
        map_res!(
            digit,
            str::from_utf8
        )
        , FromStr::from_str
    )
);

named!(
    parse_literal_i16<i16>,
    map_res!(
        map_res!(
            parse_integer,
            str::from_utf8
        )
        , FromStr::from_str
    )
);

named!(
    parse_integer<&[u8]>,
    recognize!(preceded!(opt!(tag!("-")), digit))
);

// Modified version of nom's built-in hex_u32 parser
#[inline]
fn hex_u16(input: &[u8]) -> IResult<&[u8], u16> {
    match is_a!(input, &b"0123456789abcdef"[..]) {
        Error(e) => Error(e),
        Incomplete(e) => Incomplete(e),
        Done(i, o) => {
            let mut res = 0u16;

            // Do not parse more than 4 characters for a u16
            let mut remaining = i;
            let mut parsed = o;
            if o.len() > 4 {
                remaining = &input[4..];
                parsed = &input[..4];
            }

            for &e in parsed {
                let digit = e as char;
                let value = digit.to_digit(16).unwrap_or(0) as u16;
                res = value + (res << 4);
            }
            Done(remaining, res)
        }
    }
}
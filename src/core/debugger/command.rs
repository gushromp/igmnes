use std::str::{self, FromStr};
use std::ops::Range;
use nom::{IResult, line_ending, space, digit, alphanumeric, eol};
use nom::IResult::*;

#[derive(Debug)]
pub enum Command {
    ShowUsage,
    PrintState,
    PrintMemory(Range<u16>),
    PrintBreakpoints,
    PrintWatchpoints,
    PrintLabels,
    SetBreakpoint(u16),
    SetBreakpointCycles(u64),
    SetWatchpoint(u16),
    SetLabel(String, u16),
    RemoveBreakpoint(u16),
    RemoveWatchpoint(u16),
    RemoveLabel(u16),
    ClearBreakpoints,
    ClearWatchpoints,
    ClearLabels,
    Disassemble(Range<u16>),
    Goto(u16),
    Step,
    Continue,
    Reset,
    Trace,
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
            terminated!(
                parse_command_non_terminated
                , eol
            )
        )
    )
);

named!(parse_command_non_terminated<Command>,
    alt_complete! (
        parse_print_state           |
        parse_print_memory          |
        parse_print_breakpoints     |
        parse_print_watchpoints     |
        parse_print_labels          |
        parse_set_breakpoint        |
        parse_remove_breakpoint     |
        parse_set_breakpoint_cycles |
        parse_set_watchpoint        |
        parse_remove_watchpoint     |
        parse_set_label             |
        parse_remove_label          |
        parse_clear_breakpoints     |
        parse_clear_watchpoints     |
        parse_clear_labels          |
        parse_disassemble           |
        parse_goto                  |
        parse_step                  |
        parse_continue              |
        parse_reset                 |
        parse_trace                 |
        parse_repeat_command
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
    do_parse! (
        alt_complete! (
            tag_no_case!("printmemory") |
            tag_no_case!("pm"))                 >>
        count: opt_default!(
            preceded!(space,
                alt_complete!(
                    parse_range_u16 |
                    do_parse!(
                        end: parse_literal_u16  >>
                        ( 0..end )
                    )
                )
            )
            , 0..5)                             >>
        ( Command::PrintMemory(count) )
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
    parse_set_breakpoint<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("setbreakpoint") |
            tag_no_case!("sb"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::SetBreakpoint(addr) )
    )
);

named!(
    parse_remove_breakpoint<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("removebreakpoint") |
            tag_no_case!("rb"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::RemoveBreakpoint(addr) )
    )
);

named!(
    parse_set_breakpoint_cycles<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("setbreakpointcycles") |
            tag_no_case!("sbc"))                    >>
        cycles: preceded!(space, parse_decimal_literal_u64)   >>
        ( Command::SetBreakpointCycles(cycles) )
    )
);

named!(
    parse_set_watchpoint<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("setwatchpoint") |
            tag_no_case!("sw"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::SetWatchpoint(addr) )
    )
);

named!(
    parse_remove_watchpoint<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("removewatchpoint") |
            tag_no_case!("rw"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::RemoveWatchpoint(addr) )
    )
);

named!(
    parse_set_label<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("setlabel") |
            tag_no_case!("sl"))                         >>
        label: preceded!(opt!(tag!(".")), parse_string) >>
        addr: preceded!(space, parse_literal_u16)       >>
        ( Command::SetLabel(label, addr) )
    )
);

named!(
    parse_remove_label<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("removelabel") |
            tag_no_case!("rl"))                     >>
        addr: preceded!(space, parse_literal_u16)   >>
        ( Command::RemoveLabel(addr) )
    )
);

named!(
    parse_clear_breakpoints<Command>,
    map!(
        alt_complete! (
            tag_no_case!("clearbreakpoints") |
            tag_no_case!("cb")
        )
        , |_| Command::ClearBreakpoints
    )
);

named!(
    parse_clear_watchpoints<Command>,
    map!(
        alt_complete! (
            tag_no_case!("clearwatchpoints") |
            tag_no_case!("cw")
        )
        , |_| Command::ClearWatchpoints
    )
);

named!(
    parse_clear_labels<Command>,
    map!(
        alt_complete! (
            tag_no_case!("clearlabels") |
            tag_no_case!("cl")
        )
        , |_| Command::ClearLabels
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
                    parse_range_u16 |
                    do_parse!(
                        end: parse_literal_u16 >>
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
    parse_continue<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("continue") |
            tag_no_case!("c"))      >>
        ( Command::Continue )
    )
);

named!(
    parse_reset<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("reset")) >>
        ( Command::Reset )
    )
);

named!(
    parse_trace<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("trace")) >>
        ( Command::Trace )
    )
);

named!(
    parse_repeat_command<Command>,
    do_parse! (
        alt_complete! (
            tag_no_case!("repeatcommand") |
            tag_no_case!("r"))                                      >>
        command: preceded!(space
            , terminated!(
                parse_command_non_terminated
                , char!(',')))                                      >>
        count: preceded!(space, parse_literal_u16)                  >>
        ( Command::RepeatCommand(Box::new(command), count) )
    )
);

//
// helpers
//

named!(
    parse_range_u16<Range<u16>>,
    do_parse!(
        start: parse_literal_u16    >>
        tag!("..")                  >>
        end: parse_literal_u16      >>
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
    parse_decimal_literal_u64<u64>,
    map_res!(
        map_res!(
            digit,
            str::from_utf8
        )
        , FromStr::from_str
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
    parse_integer<&'a [u8]>,
    recognize!(preceded!(opt!(tag!("-")), digit))
);

// Modified version of nom's built-in hex_u32 parser
#[inline]
fn hex_u16(input: &[u8]) -> IResult<&[u8], u16> {
    match is_a!(input, &b"0123456789abcdefABCDEF"[..]) {
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
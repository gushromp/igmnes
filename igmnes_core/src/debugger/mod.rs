use crate::debugger::frontends::terminal::TerminalDebugger;
use crate::{BusDebugger, BusOps};
use enum_dispatch::enum_dispatch;

mod command;
pub mod disassembler;
pub mod frontends;

#[enum_dispatch]
pub trait Debugger: BusOps {
    fn break_into(&mut self);

    fn start_listening(&mut self);
    fn stop_listening(&mut self);

    fn is_listening(&self) -> bool;
}

#[enum_dispatch(Debugger)]
#[enum_dispatch(BusOps)]
pub enum DebuggerFrontend {
    TerminalDebugger,
}

impl BusDebugger for DebuggerFrontend {
    fn debugger(&mut self) -> Option<&mut DebuggerFrontend> {
        Some(self)
    }
}

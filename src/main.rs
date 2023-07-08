use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
enum BasicOpcode {
    Add,
    Sub,
    Open,
    Close,
    Right,
    Left,
    Dot,
    Comma,
}

const DATA_LEN: usize = 1 << 16;

fn to_basic_opcode(c: u8) -> Option<BasicOpcode> {
    match c {
        b'+' => Some(BasicOpcode::Add),
        b'-' => Some(BasicOpcode::Sub),
        b'>' => Some(BasicOpcode::Right),
        b'<' => Some(BasicOpcode::Left),
        b'[' => Some(BasicOpcode::Open),
        b']' => Some(BasicOpcode::Close),
        b'.' => Some(BasicOpcode::Dot),
        b',' => Some(BasicOpcode::Comma),
        _ => None,
    }
}

trait BfEngine {
    const NAME: &'static str;
    type OPCODE: std::fmt::Debug;
    fn generate(iter: impl Iterator<Item = BasicOpcode>)
        -> Result<Vec<Self::OPCODE>, &'static str>;
    fn execute(
        opcodes: &[Self::OPCODE],
        data: &mut [u8; DATA_LEN],
        input: &[u8],
        stdout: &mut std::io::StdoutLock<'static>,
    );
}

#[derive(Debug)]
struct TimingReport {
    name: &'static str,
    compile_duration: Duration,
    execute_duration: Duration,
}

fn compile_execute<ENGINE: BfEngine>(
    iter: impl Iterator<Item = BasicOpcode>,
    input: &[u8],
) -> TimingReport {
    let start = Instant::now();
    println!("Generating...");
    let opcodes = ENGINE::generate(iter).unwrap();
    let compile_duration = start.elapsed();
    println!(
        "Generating opcodes for engine \"{}\" took {:?}",
        ENGINE::NAME,
        compile_duration
    );

    let start = Instant::now();
    println!("Allocating...");
    let mut data: Box<[u8; DATA_LEN]> = (0..DATA_LEN)
        .map(|_| 0_u8)
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .try_into()
        .unwrap();
    println!("Allocating {} bytes took {:?}", DATA_LEN, start.elapsed());

    let stdout = &mut std::io::stdout().lock();

    let start = Instant::now();
    println!("Executing...");
    ENGINE::execute(&opcodes, &mut data, input, stdout);
    let execute_duration = start.elapsed();
    println!(
        "Executing with engine \"{}\" took {:?}",
        ENGINE::NAME,
        execute_duration,
    );
    TimingReport {
        compile_duration,
        execute_duration,
        name: ENGINE::NAME,
    }
}

enum NewOpcode {
    //Add(u8),
    //Right(i16),
    AddRight(u8, i16),

    //Clear, = addto(0, canary)?
    //AddTo(i16),
    //Set(u8),
    SetAddTo(u8, i16),

    Seek(i16),

    BranchZero(u16),
    BranchNotZero(u16),
    Dot,
    Comma,
}

fn main() -> Result<(), &'static str> {
    let mut args = std::env::args().skip(1);

    let basic_opcodes_iter = std::fs::File::open(args.next().ok_or("input file missing")?)
        .map_err(|_| "could not open file")?
        .bytes()
        .filter_map(Result::ok)
        .filter_map(to_basic_opcode);

    let input = args.next().unwrap_or(String::new());
    let input = input.as_bytes();

    let opcodes: Vec<_> = basic_opcodes_iter.collect();

    let start = Instant::now();

    let mut reports = Vec::new();

    reports.push(dbg!(compile_execute::<ShiftAddEngine>(
        opcodes.iter().copied(),
        input
    )));
    reports.push(dbg!(compile_execute::<MergeTokenEngineExtra>(
        opcodes.iter().copied(),
        input
    )));
    reports.push(dbg!(compile_execute::<MergeTokenEngine>(
        opcodes.iter().copied(),
        input
    )));

    dbg!(reports);

    println!("elapsed total: {:?}", start.elapsed());

    Ok(())
}

use merge_token_engine_extra::*;
mod merge_token_engine_extra {
    use super::{BasicOpcode, BfEngine, Write, DATA_LEN};

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(super) enum Opcode {
        Add(u8),
        /// Open
        BranchZero(u16),
        /// Close
        BranchNotZero(u16),
        Right(i16),
        Dot,
        Comma,

        Clear,
        AddTo(i16),
        Seek(i16),
    }

    pub(super) struct MergeTokenEngineExtra {}
    impl BfEngine for MergeTokenEngineExtra {
        const NAME: &'static str = "Merge tokens extra";

        type OPCODE = Opcode;

        fn generate(
            iter: impl Iterator<Item = BasicOpcode>,
        ) -> Result<Vec<Self::OPCODE>, &'static str> {
            let mut iter = iter;
            let mut buffer = Vec::new();
            use Opcode::*;
            let mut open_stack: Vec<usize> = Vec::new();
            loop {
                let Some(opcode) = iter.next() else { break };

                match opcode {
                    BasicOpcode::Add => {
                        if let Some(Add(p)) = buffer.last_mut() {
                            *p = p.wrapping_add(1);
                        } else {
                            buffer.push(Add(1))
                        }
                    }
                    BasicOpcode::Sub => {
                        if let Some(Add(p)) = buffer.last_mut() {
                            *p = p.wrapping_sub(1)
                        } else {
                            buffer.push(Add(-1_i32 as _))
                        }
                    }
                    BasicOpcode::Right => {
                        if let Some(Right(p)) = buffer.last_mut() {
                            *p = p.wrapping_add(1)
                        } else {
                            buffer.push(Right(1))
                        }
                    }
                    BasicOpcode::Left => {
                        if let Some(Right(p)) = buffer.last_mut() {
                            *p = p.wrapping_sub(1)
                        } else {
                            buffer.push(Right(-1 as _))
                        }
                    }
                    BasicOpcode::Open => {
                        open_stack.push(buffer.len());
                        buffer.push(BranchZero(0));
                    }
                    BasicOpcode::Close => {
                        let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                        let this = buffer.len();
                        buffer[other] = BranchZero(this.try_into().unwrap());

                        match &buffer[..] {
                            &[.., BranchZero(_), Add(255)] => {
                                buffer.truncate(buffer.len() - 2);
                                buffer.push(Clear);
                            }
                            &[.., BranchZero(_), Add(255), Right(x), Add(1), Right(y)]
                                if -x == y =>
                            {
                                buffer.truncate(buffer.len() - 5);
                                buffer.push(AddTo(x));
                            }
                            &[.., BranchZero(_), Right(x)] => {
                                buffer.push(Seek(x));
                            }
                            _ => {
                                buffer.push(BranchNotZero(other.try_into().unwrap()));
                            }
                        }
                    }
                    BasicOpcode::Dot => buffer.push(Dot),
                    BasicOpcode::Comma => buffer.push(Comma),
                }
                match &buffer[..] {
                    &[.., Add(0)] | &[.., Right(0)] | &[.., AddTo(_), Clear] => {
                        println!("found redundant code {:?}", buffer);
                        let _ = buffer.pop();
                    }
                    _ => (),
                }
            }

            if !open_stack.is_empty() {
                return Err("unbalanced brackets: extra [");
            } else {
                Ok(buffer)
            }
        }

        fn execute(
            opcodes: &[Self::OPCODE],
            data: &mut [u8; DATA_LEN],
            input: &[u8],
            stdout: &mut std::io::StdoutLock<'static>,
        ) {
            let mut input = input.iter().copied();
            let mut pc: usize = 0;
            let mut dp: usize = 0;
            loop {
                let Some(opcode) = opcodes.get(pc) else { break };

                match opcode {
                    Opcode::Add(i) => {
                        data[dp] = data[dp].wrapping_add(*i);
                    }
                    Opcode::BranchZero(i) => {
                        if data[dp] == 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::BranchNotZero(i) => {
                        if data[dp] != 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::Right(i) => {
                        dp = dp.wrapping_add(*i as _);
                    }
                    Opcode::Dot => {
                        let _ = stdout.write(&[data[dp]]);
                    }
                    Opcode::Comma => {
                        data[dp as usize] = input.next().unwrap();
                    }
                    Opcode::Clear => data[dp as usize] = 0,
                    Opcode::AddTo(i) => {
                        let from = dp as usize;
                        let to = ((dp.wrapping_add(*i as _)) as usize) % DATA_LEN;

                        let tmp = data[from];

                        data[from] = tmp.wrapping_sub(tmp);
                        data[to] = data[to].wrapping_add(tmp);
                    }
                    Opcode::Seek(i) => {
                        while data[dp as usize] != 0 {
                            dp = dp.wrapping_add(*i as _);
                        }
                    }
                }
                pc += 1
            }
        }
    }
}

use merge_token_engine::*;
mod merge_token_engine {
    use super::{BasicOpcode, BfEngine, Write, DATA_LEN};

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(super) enum Opcode {
        Add(u8),
        /// Open
        BranchZero(u16),
        /// Close
        BranchNotZero(u16),
        Right(i16),
        Dot,
        Comma,
    }

    pub(super) struct MergeTokenEngine {}
    impl BfEngine for MergeTokenEngine {
        const NAME: &'static str = "Merge tokens basic";

        type OPCODE = Opcode;

        fn generate(
            iter: impl Iterator<Item = BasicOpcode>,
        ) -> Result<Vec<Self::OPCODE>, &'static str> {
            let mut iter = iter;
            let mut buffer = Vec::new();
            use Opcode::*;
            let mut open_stack: Vec<usize> = Vec::new();
            loop {
                let Some(opcode) = iter.next() else { break };

                match opcode {
                    BasicOpcode::Add => {
                        if let Some(Add(p)) = buffer.last_mut() {
                            *p = p.wrapping_add(1);
                        } else {
                            buffer.push(Add(1))
                        }
                    }
                    BasicOpcode::Sub => {
                        if let Some(Add(p)) = buffer.last_mut() {
                            *p = p.wrapping_sub(1)
                        } else {
                            buffer.push(Add(-1_i32 as _))
                        }
                    }
                    BasicOpcode::Right => {
                        if let Some(Right(p)) = buffer.last_mut() {
                            *p = p.wrapping_add(1)
                        } else {
                            buffer.push(Right(1))
                        }
                    }
                    BasicOpcode::Left => {
                        if let Some(Right(p)) = buffer.last_mut() {
                            *p = p.wrapping_sub(1)
                        } else {
                            buffer.push(Right(-1 as _))
                        }
                    }
                    BasicOpcode::Open => {
                        open_stack.push(buffer.len());
                        buffer.push(BranchZero(0));
                    }
                    BasicOpcode::Close => {
                        let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                        let this = buffer.len();
                        buffer[other] = BranchZero(this.try_into().unwrap());

                        match &buffer[..] {
                            _ => {
                                buffer.push(BranchNotZero(other.try_into().unwrap()));
                            }
                        }
                    }
                    BasicOpcode::Dot => buffer.push(Dot),
                    BasicOpcode::Comma => buffer.push(Comma),
                }
                match &buffer[..] {
                    &[.., Add(0)] | &[.., Right(0)] => {
                        println!("found redundant code {:?}", buffer);
                        let _ = buffer.pop();
                    }
                    _ => (),
                }
            }

            if !open_stack.is_empty() {
                return Err("unbalanced brackets: extra [");
            } else {
                Ok(buffer)
            }
        }

        fn execute(
            opcodes: &[Self::OPCODE],
            data: &mut [u8; DATA_LEN],
            input: &[u8],
            stdout: &mut std::io::StdoutLock<'static>,
        ) {
            let mut input = input.iter().copied();
            let mut pc: usize = 0;
            let mut dp: usize = 0;
            loop {
                let Some(opcode) = opcodes.get(pc) else { break };

                match opcode {
                    Opcode::Add(i) => {
                        data[dp] = data[dp].wrapping_add(*i);
                    }
                    Opcode::BranchZero(i) => {
                        if data[dp] == 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::BranchNotZero(i) => {
                        if data[dp] != 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::Right(i) => {
                        dp = dp.wrapping_add(*i as _);
                    }
                    Opcode::Dot => {
                        let _ = stdout.write(&[data[dp]]);
                    }
                    Opcode::Comma => {
                        data[dp as usize] = input.next().unwrap();
                    }
                }
                pc += 1
            }
        }
    }
}
use shift_add_engine::*;
mod shift_add_engine {
    use super::{BasicOpcode, BfEngine, Write, DATA_LEN};

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(super) enum Opcode {
        /// Open
        BranchZero(u16),
        /// Close
        BranchNotZero(u16),
        AddRight(u8, i16),
        //Add(u8),
        //Right(i16),
        Dot,
        Comma,
    }

    pub(super) struct ShiftAddEngine {}
    impl BfEngine for ShiftAddEngine {
        const NAME: &'static str = "Shift add";

        type OPCODE = Opcode;

        fn generate(
            iter: impl Iterator<Item = BasicOpcode>,
        ) -> Result<Vec<Self::OPCODE>, &'static str> {
            let mut iter = iter;
            let mut buffer = Vec::new();
            use Opcode::*;
            let mut open_stack: Vec<usize> = Vec::new();
            loop {
                let Some(opcode) = iter.next() else { break };

                match opcode {
                    BasicOpcode::Add => buffer.push(AddRight(1, 0)),
                    BasicOpcode::Sub => buffer.push(AddRight(-1_i32 as _, 0)),
                    BasicOpcode::Right => buffer.push(AddRight(0, 1)),
                    BasicOpcode::Left => buffer.push(AddRight(0, -1_i32 as _)),
                    //BasicOpcode::Add => {
                    //    if let Some(Add(p)) = buffer.last_mut() {
                    //        *p = p.wrapping_add(1);
                    //    } else {
                    //        buffer.push(Add(1))
                    //    }
                    //}
                    //BasicOpcode::Sub => {
                    //    if let Some(Add(p)) = buffer.last_mut() {
                    //        *p = p.wrapping_sub(1)
                    //    } else {
                    //        buffer.push(Add(-1_i32 as _))
                    //    }
                    //}
                    //BasicOpcode::Right => {
                    //    if let Some(Right(p)) = buffer.last_mut() {
                    //        *p = p.wrapping_add(1)
                    //    } else {
                    //        buffer.push(Right(1))
                    //    }
                    //}
                    //BasicOpcode::Left => {
                    //    if let Some(Right(p)) = buffer.last_mut() {
                    //        *p = p.wrapping_sub(1)
                    //    } else {
                    //        buffer.push(Right(-1 as _))
                    //    }
                    //}
                    BasicOpcode::Open => {
                        open_stack.push(buffer.len());
                        buffer.push(BranchZero(0));
                    }
                    BasicOpcode::Close => {
                        let other = open_stack.pop().ok_or("unbalanced brackets: extra ]")?;
                        let this = buffer.len();
                        buffer[other] = BranchZero(this.try_into().unwrap());

                        match &buffer[..] {
                            _ => {
                                buffer.push(BranchNotZero(other.try_into().unwrap()));
                            }
                        }
                    }
                    BasicOpcode::Dot => buffer.push(Dot),
                    BasicOpcode::Comma => buffer.push(Comma),
                }
                match &buffer[..] {
                    &[.., AddRight(0, 0)] => {
                        println!("found redundant code {:?}", buffer);
                        let _ = buffer.pop();
                    }
                    _ => (),
                }
            }

            if !open_stack.is_empty() {
                return Err("unbalanced brackets: extra [");
            } else {
                Ok(buffer)
            }
        }

        fn execute(
            opcodes: &[Self::OPCODE],
            data: &mut [u8; DATA_LEN],
            input: &[u8],
            stdout: &mut std::io::StdoutLock<'static>,
        ) {
            let mut input = input.iter().copied();
            let mut pc: usize = 0;
            let mut dp: usize = 0;
            loop {
                let Some(opcode) = opcodes.get(pc) else { break };

                match opcode {
                    Opcode::AddRight(a, i) => {
                        data[dp] = data[dp].wrapping_add(*a);
                        dp = dp.wrapping_add(*i as _);
                    }
                    //Opcode::Add(i) => {
                    //    data[dp] = data[dp].wrapping_add(*i);
                    //}
                    //Opcode::Right(i) => {
                    //    dp = dp.wrapping_add(*i as _);
                    //}
                    Opcode::BranchZero(i) => {
                        if data[dp] == 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::BranchNotZero(i) => {
                        if data[dp] != 0 {
                            pc = *i as _;
                        }
                    }
                    Opcode::Dot => {
                        let _ = stdout.write(&[data[dp]]);
                    }
                    Opcode::Comma => {
                        data[dp as usize] = input.next().unwrap();
                    }
                }
                pc += 1
            }
        }
    }
}
